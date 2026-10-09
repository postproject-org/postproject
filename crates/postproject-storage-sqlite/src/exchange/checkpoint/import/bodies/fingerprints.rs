use postproject_core::ObjectRef;
use postproject_protocol::{
    Document, FingerprintObservation, FingerprintRecomputation, FingerprintState,
};
use rusqlite::params;

use crate::{ExchangeResult, encode_metadata_target};

use super::{super::super::invalid, Bodies, media::insert};

#[cfg(test)]
mod tests;
mod validate;

impl Bodies<'_, '_> {
    pub(super) fn validate_fingerprints(&self) -> ExchangeResult<()> {
        validate::boundaries(self.transaction, self.manifest.head().sequence())
    }
}

impl Bodies<'_, '_> {
    pub(super) fn fingerprint(&mut self, document: &Document) -> ExchangeResult<()> {
        if document.kind()? == "fingerprint.recomputation" {
            let marker = FingerprintRecomputation::from_document(document)?;
            insert(self.transaction.execute("INSERT INTO representation_fingerprint_recomputations (representation_id, changed_resource_id, marked_revision_sequence) VALUES (?1, ?2, ?3)", params![marker.representation_id().as_bytes().as_slice(), marker.changed_resource_id().as_bytes().as_slice(), i64::try_from(marker.revision_sequence()).map_err(|_| invalid())?]))?;
            return Ok(());
        }
        let observation = FingerprintObservation::from_document(document)?;
        let target = observation.target();
        let (kind, id) = encode_metadata_target(&target)?;
        let (table, column) = match target {
            ObjectRef::Resource(_) => ("resource", "resource_id"),
            ObjectRef::Representation(_) => ("representation", "representation_id"),
            _ => return Err(invalid().into()),
        };
        let snapshot = observation.snapshot();
        let observed = snapshot
            .observed_revision_sequence()
            .map(i64::try_from)
            .transpose()
            .map_err(|_| invalid())?;
        match observation.state() {
            FingerprintState::Current => {
                insert(self.transaction.execute(&format!("INSERT INTO {table}_fingerprints ({column}, algorithm, algorithm_version, value, observed_revision_sequence) VALUES (?1, ?2, ?3, ?4, ?5)"), params![id.as_slice(), snapshot.algorithm(), snapshot.version(), snapshot.value(), observed]))?;
            }
            FingerprintState::Superseded {
                position,
                revision_sequence,
            } => {
                let key = (
                    kind,
                    id.to_vec(),
                    snapshot.algorithm().to_owned(),
                    snapshot.version(),
                );
                if self.fingerprint_history_key.as_ref() == Some(&key) {
                    self.fingerprint_history_position = self
                        .fingerprint_history_position
                        .checked_add(1)
                        .ok_or_else(invalid)?;
                } else {
                    if self
                        .fingerprint_history_key
                        .as_ref()
                        .is_some_and(|previous| previous >= &key)
                    {
                        return Err(invalid().into());
                    }
                    self.fingerprint_history_key = Some(key);
                    self.fingerprint_history_position = 0;
                }
                if position != self.fingerprint_history_position {
                    return Err(invalid().into());
                }
                insert(self.transaction.execute(&format!("INSERT INTO {table}_fingerprint_history ({column}, algorithm, algorithm_version, value, observed_revision_sequence, superseded_revision_sequence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"), params![id.as_slice(), snapshot.algorithm(), snapshot.version(), snapshot.value(), observed, i64::try_from(revision_sequence).map_err(|_| invalid())?]))?;
            }
        }
        Ok(())
    }
}
