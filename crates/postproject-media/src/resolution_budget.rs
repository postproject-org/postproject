//! Aggregate limits for resolver work and retained observations.

use postproject_core::{Error, ErrorKind, ResolutionEvidence, ResourceResolution, Result};

#[derive(Default)]
pub(crate) struct ResolutionBudget {
    items: usize,
    bytes: usize,
}

impl ResolutionBudget {
    pub(crate) fn record(&mut self, items: usize, bytes: usize) -> Result<()> {
        self.items = self.items.saturating_add(items);
        self.bytes = self.bytes.saturating_add(bytes);
        if self.items > 100_000 || self.bytes > 64 * 1024 * 1024 {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "resolution exceeds its aggregate budget; reduce the resource or search scope",
            ));
        }
        Ok(())
    }

    pub(crate) fn evidence(&mut self, evidence: &ResolutionEvidence) -> Result<()> {
        self.record(
            1,
            size_of::<ResolutionEvidence>().saturating_add(evidence.detail().map_or(0, str::len)),
        )
    }

    pub(crate) fn resolution(&mut self, resolution: &ResourceResolution) -> Result<()> {
        self.record(1, size_of::<ResourceResolution>())?;
        self.record(
            resolution.missing_frames().len(),
            size_of_val(resolution.missing_frames()),
        )?;
        for evidence in resolution.evidence() {
            self.evidence(evidence)?;
        }
        for candidate in resolution.candidates() {
            self.record(
                1,
                size_of_val(candidate)
                    .saturating_add(candidate.uri().len())
                    .saturating_add(candidate.media_root().map_or(0, str::len)),
            )?;
            if let Some(naming) = candidate.sequence_naming() {
                self.record(
                    0,
                    naming.prefix().len().saturating_add(naming.suffix().len()),
                )?;
            }
            for evidence in candidate.evidence() {
                self.evidence(evidence)?;
            }
        }
        Ok(())
    }
}
