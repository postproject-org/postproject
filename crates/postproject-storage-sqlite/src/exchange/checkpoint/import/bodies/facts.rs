use postproject_protocol::{
    ConflictVersion, Document, ProductionHeader, SnapshotAssertion, decode_conflict_floor,
    decode_root,
};
use rusqlite::params;

use crate::{
    CURRENT_SCHEMA_VERSION, ExchangeResult, metadata_codec, sqlite_error,
    transaction::encode_conflict_key,
};

use super::super::super::invalid;
use super::Bodies;

impl Bodies<'_, '_> {
    pub(super) fn production(&self, document: &Document) -> ExchangeResult<()> {
        let header = ProductionHeader::from_document(document)?;
        if header.id() != self.manifest.head().scope().production() {
            return Err(invalid().into());
        }
        self.transaction.execute("INSERT INTO productions (singleton, id, schema_version, created_at_micros, display_name) VALUES (1, ?1, ?2, ?3, ?4)", params![header.id().as_bytes().as_slice(), CURRENT_SCHEMA_VERSION, header.created_at().as_unix_micros(), header.display_name()])
            .map_err(sqlite_error("stage checkpoint production"))?;
        self.transaction.execute("UPDATE exchange_history SET role = 2, history_id = ?1, floor_revision_id = ?2, floor_sequence = ?3, anchor_digest = ?4 WHERE singleton = 1", params![self.manifest.head().scope().history().as_bytes().as_slice(), self.manifest.floor().revision().map(|id| id.as_bytes().to_vec()), i64::try_from(self.manifest.floor().sequence()).map_err(|_| invalid())?, self.manifest.floor().digest().as_bytes().as_slice()])
            .map_err(sqlite_error("stage passive checkpoint boundary"))?;
        Ok(())
    }

    pub(super) fn metadata(&mut self, document: &Document) -> ExchangeResult<()> {
        let assertion = SnapshotAssertion::from_document(document)?;
        self.target_scope(assertion.target())?;
        let target = assertion.target();
        let (kind, id) = crate::encode_metadata_target(&target)?;
        let property = assertion.property();
        let key = (
            kind,
            id.to_vec(),
            property.vocabulary().as_str().to_owned(),
            property.property().as_str().to_owned(),
        );
        if self.metadata_key.as_ref() == Some(&key) {
            self.metadata_position = self.metadata_position.checked_add(1).ok_or_else(invalid)?;
        } else {
            if self
                .metadata_key
                .as_ref()
                .is_some_and(|previous| previous >= &key)
            {
                return Err(invalid().into());
            }
            self.metadata_position = 0;
            self.metadata_key = Some(key);
        }
        if assertion.position() != self.metadata_position {
            return Err(invalid().into());
        }
        self.transaction.execute("INSERT INTO metadata_assertions (target_kind, target_id, vocabulary, property, position, encoded_value) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![kind, id, property.vocabulary().as_str(), property.property().as_str(), i64::try_from(assertion.position()).map_err(|_| invalid())?, metadata_codec::encode(assertion.value())?])
            .map_err(sqlite_error("stage checkpoint assertion"))?;
        Ok(())
    }

    pub(super) fn root(&self, document: &Document) -> ExchangeResult<()> {
        let root = decode_root(document)?;
        self.transaction.execute("INSERT INTO media_roots (id, name, label, legacy_uri, priority, enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![root.id().as_bytes().as_slice(), root.name(), root.label(), root.legacy_uri(), root.priority(), root.is_enabled()])
            .map_err(|error| {
                if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
                    crate::ExchangeError::Protocol(invalid())
                } else {
                    sqlite_error("stage checkpoint root")(error).into()
                }
            })?;
        Ok(())
    }

    pub(super) fn version(&mut self, document: &Document) -> ExchangeResult<()> {
        let version = ConflictVersion::from_document(document)?;
        self.semantic_target(version.key())?;
        self.validate_revision(version.revision(), version.sequence())?;
        let key = encode_conflict_key(version.key())?;
        let duplicate: bool = self
            .transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conflict_versions WHERE conflict_key = ?1)",
                [&key],
                |row| row.get(0),
            )
            .map_err(sqlite_error("check unique checkpoint semantic key"))?;
        if duplicate {
            return Err(invalid().into());
        }
        self.transaction.execute("INSERT INTO conflict_versions (conflict_key, last_changed_revision_id, last_changed_revision_sequence) VALUES (?1, ?2, ?3)", params![encode_conflict_key(version.key())?, version.revision().as_bytes().as_slice(), i64::try_from(version.sequence()).map_err(|_| invalid())?])
            .map_err(sqlite_error("stage checkpoint semantic version"))?;
        Ok(())
    }

    pub(super) fn conflict_floor(&self, document: &Document) -> ExchangeResult<()> {
        let base = decode_conflict_floor(document)?;
        if base.production_id() != self.manifest.head().scope().production() {
            return Err(invalid().into());
        }
        if let Some(revision) = base.revision_id() {
            self.validate_revision(revision, base.sequence())?;
            self.transaction.execute("INSERT INTO conflict_migration_baseline (singleton, revision_id, revision_sequence) VALUES (1, ?1, ?2)", params![revision.as_bytes().as_slice(), i64::try_from(base.sequence()).map_err(|_| invalid())?])
                .map_err(sqlite_error("stage original conflict baseline"))?;
        }
        Ok(())
    }
}
