//! Production-scoped bases for decisions made from coherent read views.

use std::{fmt, str::FromStr};

use crate::{Error, ErrorKind, ProductionId, ProductionRead, ProductionStore, Result, RevisionId};

/// Detached optimistic context; it does not retain a read view or a lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecisionBase {
    production_id: ProductionId,
    revision_id: Option<RevisionId>,
    sequence: u64,
}

impl fmt::Display for DecisionBase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ppdb1:{}:", self.production_id)?;
        if let Some(revision) = self.revision_id {
            write!(formatter, "{revision}")?;
        } else {
            formatter.write_str("empty")?;
        }
        write!(formatter, ":{}", self.sequence)
    }
}

impl FromStr for DecisionBase {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let invalid = || Error::new(ErrorKind::InvalidArgument, "invalid decision-base token");
        if value.len() > 128 {
            return Err(invalid());
        }
        let mut fields = value.split(':');
        if fields.next() != Some("ppdb1") {
            return Err(invalid());
        }
        let production = fields.next().ok_or_else(invalid)?.parse()?;
        let revision = match fields.next().ok_or_else(invalid)? {
            "empty" => None,
            id => Some(id.parse()?),
        };
        let sequence = fields
            .next()
            .ok_or_else(invalid)?
            .parse()
            .map_err(|_| invalid())?;
        if fields.next().is_some() {
            return Err(invalid());
        }
        let base = Self::new(production, revision, sequence)?;
        if base.to_string() != value {
            return Err(invalid());
        }
        Ok(base)
    }
}

impl DecisionBase {
    /// Validates a production-scoped revision observation.
    ///
    /// `None` with sequence zero represents the initial empty journal. The
    /// store validates membership and the revision/sequence pair when editing.
    ///
    /// # Errors
    ///
    /// Returns invalid argument for contradictory empty/revision values.
    pub fn new(
        production_id: ProductionId,
        revision_id: Option<RevisionId>,
        sequence: u64,
    ) -> Result<Self> {
        if revision_id.is_some() != (sequence != 0) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "a decision revision requires a positive sequence; an empty base requires zero",
            ));
        }
        Ok(Self {
            production_id,
            revision_id,
            sequence,
        })
    }

    /// Returns the production whose facts were read.
    #[must_use]
    pub const fn production_id(self) -> ProductionId {
        self.production_id
    }

    /// Returns the read view's revision, absent only for an empty journal.
    #[must_use]
    pub const fn revision_id(self) -> Option<RevisionId> {
        self.revision_id
    }

    /// Returns the read view's revision sequence, zero only for an empty journal.
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

/// An owned coherent production view with a base captured by the same read.
pub trait ProductionReadSession {
    /// Returns only domain read operations on the retained view.
    fn read(&self) -> &dyn ProductionRead;

    /// Detaches the view's production-scoped decision context.
    fn decision_base(&self) -> DecisionBase;

    /// Begins an edit carrying this view's decision base automatically.
    ///
    /// The store owns a fresh write transaction, not an upgraded read view.
    ///
    /// # Errors
    ///
    /// Rejects wrong production scope, invalid revisions or storage failures.
    fn edit<'production, Store: ProductionStore>(
        &self,
        store: &'production mut Store,
    ) -> Result<Store::Transaction<'production>> {
        store.begin_edit(self.decision_base())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_canonical_bounded_and_preserve_empty_state() {
        for revision in [None, Some(RevisionId::new())] {
            let base =
                DecisionBase::new(ProductionId::new(), revision, u64::from(revision.is_some()))
                    .unwrap();
            let token = base.to_string();
            assert_eq!(token.parse::<DecisionBase>().unwrap(), base);
            assert!(format!("{token}:extra").parse::<DecisionBase>().is_err());
            assert!(
                token
                    .replace("ppdb1", "ppdb2")
                    .parse::<DecisionBase>()
                    .is_err()
            );
            let (prefix, sequence) = token.rsplit_once(':').unwrap();
            assert!(
                format!("{prefix}:0{sequence}")
                    .parse::<DecisionBase>()
                    .is_err()
            );
        }
        assert!("x".repeat(129).parse::<DecisionBase>().is_err());
        assert!(DecisionBase::new(ProductionId::new(), None, 1).is_err());
    }
}
