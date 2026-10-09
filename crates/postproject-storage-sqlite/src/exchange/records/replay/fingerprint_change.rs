mod state;

use postproject_core::{ObjectRef, RepresentationId};
use postproject_protocol::{
    Document, FingerprintChangeStart, FingerprintRecomputation, RecordManifest,
};
use rusqlite::{Transaction, params};

use super::{effects::invalid, facts::structural};
use crate::{ExchangeResult, sqlite_error, transaction::mutation_error};

pub(super) struct FingerprintApply {
    start: FingerprintChangeStart,
    markers: u64,
    last_owner: Option<RepresentationId>,
}

impl FingerprintApply {
    pub(super) fn begin(
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        start: FingerprintChangeStart,
    ) -> ExchangeResult<Self> {
        state::apply(transaction, manifest, &start)?;
        if let ObjectRef::Resource(resource) = start.target() {
            let owners: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM representation_resources WHERE resource_id = ?1",
                    [resource.as_bytes().as_slice()],
                    |row| row.get(0),
                )
                .map_err(sqlite_error("validate replayed fingerprint owners"))?;
            if u64::try_from(owners).ok() != Some(start.marker_count()) {
                return Err(invalid().into());
            }
        }
        Ok(Self {
            start,
            markers: 0,
            last_owner: None,
        })
    }

    pub(super) fn push(
        &mut self,
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        document: &Document,
    ) -> ExchangeResult<bool> {
        let marker = FingerprintRecomputation::from_document(document)?;
        let ObjectRef::Resource(resource) = self.start.target() else {
            return Err(invalid().into());
        };
        if marker.changed_resource_id() != resource
            || marker.revision_sequence() != manifest.revision().sequence()
            || self.last_owner.is_some_and(|previous| {
                previous.as_bytes() >= marker.representation_id().as_bytes()
            })
            || self.markers >= self.start.marker_count()
        {
            return Err(invalid().into());
        }
        let owns: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM representation_resources WHERE representation_id = ?1 AND resource_id = ?2)",
            params![marker.representation_id().as_bytes().as_slice(), resource.as_bytes().as_slice()], |row|row.get(0))
            .map_err(sqlite_error("validate replayed recomputation membership"))?;
        if !owns {
            return Err(invalid().into());
        }
        structural(transaction.execute("INSERT INTO representation_fingerprint_recomputations (representation_id, changed_resource_id, marked_revision_sequence) VALUES (?1, ?2, ?3) ON CONFLICT(representation_id) DO UPDATE SET changed_resource_id = excluded.changed_resource_id, marked_revision_sequence = excluded.marked_revision_sequence",
            params![marker.representation_id().as_bytes().as_slice(), resource.as_bytes().as_slice(), i64::try_from(marker.revision_sequence()).map_err(|_|invalid())?])
            .map_err(mutation_error("stage original recomputation marker")))?;
        self.markers += 1;
        self.last_owner = Some(marker.representation_id());
        Ok(self.markers == self.start.marker_count())
    }
}
