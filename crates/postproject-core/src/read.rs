//! Production-scoped bases for decisions made from coherent read views.

use std::{fmt, str::FromStr};

use crate::{
    Asset, Error, ErrorKind, ProductionId, ProductionRead, ProductionStore, QueryPageRequest,
    Result, Revision, RevisionId,
};

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

    /// Scans all assets in this coherent view using bounded, lazy pages.
    ///
    /// The iterator borrows the view; copied assets may outlive it. A backend
    /// error is returned once, then the iterator ends. Dropping the iterator
    /// stops traversal without closing the caller-owned view. There is no
    /// total-result cap or hidden connection.
    ///
    /// # Errors
    ///
    /// Rejects page sizes outside `1..=1000` before returning an iterator.
    /// Individual items preserve errors from the underlying page operation.
    fn assets(&self, page_size: u32) -> Result<impl Iterator<Item = Result<Asset>> + '_>
    where
        Self: Sized,
    {
        let mut request = Some(QueryPageRequest::new(page_size, None)?);
        let mut buffered = Vec::new().into_iter();
        Ok(std::iter::from_fn(move || {
            if let Some(asset) = buffered.next() {
                return Some(Ok(asset));
            }
            let current = request.take()?;
            let page = match self.read().assets_page(&current) {
                Ok(page) => page,
                Err(error) => return Some(Err(error)),
            };
            if page.traversal_truncated()
                || (page.items().is_empty() && page.next_cursor().is_some())
                || page
                    .next_cursor()
                    .is_some_and(|next| Some(next) == current.cursor())
            {
                return Some(Err(Error::new(
                    ErrorKind::Internal,
                    "asset scan received an incomplete or nonadvancing page",
                )));
            }
            if let Some(cursor) = page.next_cursor().cloned() {
                match QueryPageRequest::new(page_size, Some(cursor)) {
                    Ok(next) => request = Some(next),
                    Err(error) => return Some(Err(error)),
                }
            }
            buffered = page.into_items().into_iter();
            buffered.next().map(Ok)
        }))
    }

    /// Scans revisions after `after`, in ascending order within this view.
    ///
    /// Page size bounds memory, not the number of revisions. The iterator
    /// borrows the view and ends after a backend error, returning that error
    /// unchanged. It observes the retained history, not future writes.
    ///
    /// # Errors
    ///
    /// Rejects page sizes outside `1..=1000` before returning an iterator.
    /// Individual items preserve errors from the underlying history operation.
    fn revisions(
        &self,
        after: u64,
        page_size: u32,
    ) -> Result<impl Iterator<Item = Result<Revision>> + '_>
    where
        Self: Sized,
    {
        QueryPageRequest::new(page_size, None)?;
        let mut sequence = Some(after);
        let mut buffered = Vec::new().into_iter();
        Ok(std::iter::from_fn(move || {
            if let Some(revision) = buffered.next() {
                return Some(Ok(revision));
            }
            let current = sequence.take()?;
            let page = match self.read().changes_since(current, page_size) {
                Ok(page) => page,
                Err(error) => return Some(Err(error)),
            };
            if page.len() == page_size as usize {
                sequence = page.last().map(Revision::sequence);
            }
            buffered = page.into_iter();
            buffered.next().map(Ok)
        }))
    }

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
