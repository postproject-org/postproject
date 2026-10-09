use postproject_core::{RevisionEventKind, SemanticConflictKey};
use postproject_protocol::{
    Document, FailureKind, MetadataEffectStart, MetadataOperation, ProtocolError, RecordManifest,
    decode_event,
};
use rusqlite::{Transaction, params};

use crate::{
    ExchangeResult, encode_metadata_target, metadata_codec, sqlite_error,
    transaction::{
        delete_metadata_property, encode_conflict_key, ensure_metadata_target_exists,
        insert_metadata_value, next_metadata_position, persist_revision_event,
    },
};

pub(super) struct ApplyEffects<'a, 'connection> {
    transaction: &'a Transaction<'connection>,
    manifest: &'a RecordManifest,
    effect: Option<(MetadataEffectStart, u64)>,
    effects: u64,
    events: u64,
}

impl<'a, 'connection> ApplyEffects<'a, 'connection> {
    pub(super) const fn new(
        transaction: &'a Transaction<'connection>,
        manifest: &'a RecordManifest,
    ) -> Self {
        Self {
            transaction,
            manifest,
            effect: None,
            effects: 0,
            events: 0,
        }
    }

    pub(super) fn document(&mut self, document: &Document) -> ExchangeResult<()> {
        if self.effect.is_some() {
            return self.value(document);
        }
        if self.effects < self.manifest.effect_count() {
            return self.start(MetadataEffectStart::from_document(document)?);
        }
        if self.events >= self.manifest.event_count() {
            return Err(invalid().into());
        }
        let event = decode_event(document)?;
        if event.revision_id() != self.manifest.revision().id()
            || u64::from(event.position()) != self.events
        {
            return Err(invalid().into());
        }
        let stored = self.transaction.query_row("SELECT position, kind, target_kind, primary_id, secondary_id, structural_position, vocabulary, property, identifier_scheme, identifier_value, identifier_qualifier, activity_kind, role, fingerprint_algorithm, fingerprint_version FROM revision_events WHERE revision_id = ?1 AND position = ?2", params![event.revision_id().as_bytes().as_slice(), i64::from(event.position())], crate::stored_revision_event_row)
            .map_err(sqlite_error("validate source observation"))?;
        if crate::decode_revision_event(event.revision_id(), stored)? != event {
            return Err(invalid().into());
        }
        self.events += 1;
        Ok(())
    }

    fn start(&mut self, effect: MetadataEffectStart) -> ExchangeResult<()> {
        let target = effect.target();
        let (kind, id) = encode_metadata_target(&target)?;
        ensure_metadata_target_exists(self.transaction, kind, id).map_err(|error| {
            if error.kind() == postproject_core::ErrorKind::NotFound {
                invalid().into()
            } else {
                crate::ExchangeError::Store(error)
            }
        })?;
        match effect.operation() {
            MetadataOperation::Appended(position) => {
                if u64::try_from(next_metadata_position(
                    self.transaction,
                    kind,
                    id,
                    effect.property(),
                )?)
                .ok()
                    != Some(position)
                {
                    return Err(invalid().into());
                }
            }
            MetadataOperation::Replaced | MetadataOperation::Removed => {
                let removed =
                    delete_metadata_property(self.transaction, kind, id, effect.property())?;
                if effect.value_count() == 0 && removed == 0 {
                    return Err(invalid().into());
                }
            }
        }
        let key = encode_conflict_key(&SemanticConflictKey::MetadataProperty {
            target: effect.target(),
            property: effect.property().clone(),
        })?;
        let revision = self.manifest.revision();
        self.transaction.execute("INSERT INTO conflict_versions (conflict_key, last_changed_revision_id, last_changed_revision_sequence) VALUES (?1, ?2, ?3) ON CONFLICT(conflict_key) DO UPDATE SET last_changed_revision_id = excluded.last_changed_revision_id, last_changed_revision_sequence = excluded.last_changed_revision_sequence", params![key, revision.id().as_bytes().as_slice(), i64::try_from(revision.sequence()).map_err(|_| invalid())?])
            .map_err(sqlite_error("persist replayed semantic version"))?;
        let event = if effect.value_count() == 0 {
            RevisionEventKind::MetadataRemoved {
                target: effect.target(),
                property: effect.property().clone(),
            }
        } else {
            RevisionEventKind::MetadataAddedOrReplaced {
                target: effect.target(),
                property: effect.property().clone(),
            }
        };
        let position = u32::try_from(self.effects).map_err(|_| invalid())?;
        persist_revision_event(self.transaction, revision.id(), i64::from(position), &event)?;
        self.effects += 1;
        if effect.value_count() > 0 {
            self.effect = Some((effect, 0));
        }
        Ok(())
    }

    fn value(&mut self, document: &Document) -> ExchangeResult<()> {
        let (effect, index) = self.effect.as_mut().ok_or_else(invalid)?;
        let value = MetadataEffectStart::decode_value(document)?;
        let encoded = metadata_codec::encode(&value)?;
        let target = effect.target();
        let (kind, id) = encode_metadata_target(&target)?;
        let position = match effect.operation() {
            MetadataOperation::Appended(position) => position,
            MetadataOperation::Replaced => *index,
            MetadataOperation::Removed => return Err(invalid().into()),
        };
        insert_metadata_value(
            self.transaction,
            kind,
            id,
            effect.property(),
            i64::try_from(position).map_err(|_| invalid())?,
            &encoded,
        )?;
        *index += 1;
        if *index == effect.value_count() {
            self.effect = None;
        }
        Ok(())
    }

    pub(super) fn finish(self) -> ExchangeResult<()> {
        if self.effect.is_some()
            || self.effects != self.manifest.effect_count()
            || self.events != self.manifest.event_count()
            || self.effects != self.events
        {
            return Err(invalid().into());
        }
        Ok(())
    }
}

fn invalid() -> ProtocolError {
    ProtocolError::new(
        FailureKind::Integrity,
        "record effects do not match structural state or observations",
    )
}
