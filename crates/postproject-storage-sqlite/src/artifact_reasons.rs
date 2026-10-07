//! Bounds owned explanations, including copies retained by traversal caches.

use postproject_core::{
    ArtifactDependencyPathSegment, ArtifactKnowledgeReason, Error, ErrorKind, Result,
};

use crate::read_budget::ReadBudget;

#[derive(Clone, Default)]
pub(crate) struct Reasons {
    items: Vec<ArtifactKnowledgeReason>,
    budget: ReadBudget,
}

impl Reasons {
    pub(crate) fn one(reason: ArtifactKnowledgeReason) -> Result<Self> {
        let mut result = Self::default();
        result.push(reason)?;
        Ok(result)
    }

    pub(crate) fn push(&mut self, reason: ArtifactKnowledgeReason) -> Result<()> {
        self.budget.record(payload_bytes(&reason)?)?;
        self.items.push(reason);
        Ok(())
    }

    pub(crate) fn extend(&mut self, reasons: Self) -> Result<()> {
        for reason in reasons.items {
            self.push(reason)?;
        }
        Ok(())
    }

    pub(crate) fn checked_clone(&self, budget: &mut ReadBudget) -> Result<Self> {
        for reason in &self.items {
            budget.record(payload_bytes(reason)?)?;
        }
        Ok(self.clone())
    }

    pub(crate) fn into_vec(self) -> Vec<ArtifactKnowledgeReason> {
        self.items
    }
}

fn payload_bytes(reason: &ArtifactKnowledgeReason) -> Result<usize> {
    use ArtifactKnowledgeReason as R;
    let extra = match reason {
        R::FingerprintChanged {
            algorithm,
            snapshot_value,
            current_value,
            ..
        } => algorithm
            .len()
            .saturating_add(snapshot_value.len())
            .saturating_add(current_value.len()),
        R::DependencyFingerprintChanged {
            path,
            algorithm,
            snapshot_value,
            current_value,
            ..
        } => path_bytes(path)
            .saturating_add(algorithm.len())
            .saturating_add(snapshot_value.len())
            .saturating_add(current_value.len()),
        R::FingerprintEvidenceMissing {
            algorithm,
            snapshot_value,
            current_value,
            ..
        } => optional_bytes(
            algorithm.as_deref(),
            snapshot_value.as_deref(),
            current_value.as_deref(),
        ),
        R::DependencyFingerprintEvidenceMissing {
            path,
            algorithm,
            snapshot_value,
            current_value,
            ..
        } => path_bytes(path).saturating_add(optional_bytes(
            algorithm.as_deref(),
            snapshot_value.as_deref(),
            current_value.as_deref(),
        )),
        R::DependencyKnowledgeIncomplete { path, .. }
        | R::DependencyPathChanged { path, .. }
        | R::DependencyFingerprintRecomputationPending { path, .. } => path_bytes(path),
        R::ProducingActivityMissing { .. }
        | R::ProducingActivityAmbiguous { .. }
        | R::SnapshotAbsent { .. }
        | R::DependencySnapshotAbsent { .. }
        | R::FingerprintRecomputationPending { .. }
        | R::UpstreamNotCurrent { .. }
        | R::TraversalTruncated { .. } => 0,
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "unknown required artifact explanation kind",
            ));
        }
    };
    Ok(std::mem::size_of_val(reason).saturating_add(extra))
}

fn optional_bytes(
    algorithm: Option<&str>,
    captured: Option<&[u8]>,
    current: Option<&[u8]>,
) -> usize {
    algorithm
        .map_or(0, str::len)
        .saturating_add(captured.map_or(0, <[u8]>::len))
        .saturating_add(current.map_or(0, <[u8]>::len))
}

fn path_bytes(path: &[ArtifactDependencyPathSegment]) -> usize {
    path.iter().fold(0_usize, |total, segment| {
        total
            .saturating_add(std::mem::size_of_val(segment))
            .saturating_add(segment.kind().as_str().len())
            .saturating_add(segment.authored_reference().len())
    })
}
