use postproject_core::{AssetId, RevisionEventKind, RevisionId, SemanticConflictKey};
mod creation;
mod media;

use postproject_protocol::{
    Document, MetadataEffectStart, RecordFeature, RecordManifest, decode_event,
};
use rusqlite::Connection;

use crate::ExchangeResult;

use super::super::{super::super::invalid, history::observation};

#[cfg(test)]
mod tests;

pub(super) struct RetainedEffects {
    revision: RevisionId,
    sequence: u64,
    total: u64,
    total_events: u64,
    media: bool,
    metadata: bool,
    effects: u64,
    expected_events: u64,
    events: u64,
    remaining_values: u64,
    value_index: u64,
    effect: Option<MetadataEffectStart>,
    creation: Option<creation::CreationAudit>,
    original: Option<AssetId>,
    fingerprint: Option<media::PendingFingerprint>,
    floor: u64,
    genesis: bool,
}

impl RetainedEffects {
    pub(super) fn new(manifest: &RecordManifest, floor: u64) -> Self {
        Self {
            revision: manifest.revision().id(),
            sequence: manifest.revision().sequence(),
            total: manifest.effect_count(),
            total_events: manifest.event_count(),
            media: manifest
                .required_features()
                .any(|feature| feature == RecordFeature::Media),
            metadata: manifest
                .required_features()
                .any(|feature| feature == RecordFeature::Metadata),
            effects: 0,
            expected_events: 0,
            events: 0,
            remaining_values: 0,
            value_index: 0,
            effect: None,
            creation: None,
            original: None,
            fingerprint: None,
            floor,
            genesis: floor == 0,
        }
    }

    pub(super) fn document(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<()> {
        if self.original.is_some() {
            return self.original_representation(connection, document);
        }
        if let Some(creation) = self.creation.as_mut() {
            creation.document(connection, document)?;
            if creation.is_complete() {
                self.creation.take().ok_or_else(invalid)?.finish()?;
            }
            return Ok(());
        }
        if self.fingerprint.is_some() {
            return self.fingerprint_marker(connection, document);
        }
        if self.remaining_values != 0 {
            let value = MetadataEffectStart::decode_value(document)?;
            super::super::super::metadata_state::value(
                connection,
                self.effect.as_ref().ok_or_else(invalid)?,
                self.value_index,
                &crate::metadata_codec::encode(&value)?,
            )?;
            self.value_index += 1;
            self.remaining_values -= 1;
        } else if self.effects < self.total {
            if self.media && self.media_effect(connection, document)? {
                self.effects += 1;
                return Ok(());
            }
            if !self.metadata {
                return Err(invalid().into());
            }
            let effect = MetadataEffectStart::from_document(document)?;
            self.require_media_target(connection, effect.target())?;
            let kind = if effect.value_count() == 0 {
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
            self.expect_observation(connection, &kind)?;
            super::super::super::metadata_state::start(connection, &effect, self.genesis)?;
            super::super::super::guard_state::recorded(
                connection,
                &SemanticConflictKey::MetadataProperty {
                    target: effect.target(),
                    property: effect.property().clone(),
                },
                self.sequence,
            )?;
            self.effects += 1;
            self.remaining_values = effect.value_count();
            self.value_index = 0;
            self.effect = Some(effect);
        } else {
            if self.events >= self.total_events {
                return Err(invalid().into());
            }
            let position = u32::try_from(self.events).map_err(|_| invalid())?;
            let event = decode_event(document)?;
            if event != observation(connection, self.revision, position)? {
                return Err(invalid().into());
            }
            self.events += 1;
        }
        Ok(())
    }

    pub(super) fn finish(self) -> ExchangeResult<()> {
        if self.effects != self.total
            || self.events != self.total_events
            || self.remaining_values != 0
            || self.expected_events != self.total_events
            || self.creation.is_some()
            || self.original.is_some()
            || self.fingerprint.is_some()
        {
            return Err(invalid().into());
        }
        Ok(())
    }

    fn expect_observation(
        &mut self,
        connection: &Connection,
        kind: &RevisionEventKind,
    ) -> ExchangeResult<()> {
        if self.expected_events >= self.total_events {
            return Err(invalid().into());
        }
        let expected = observation(
            connection,
            self.revision,
            u32::try_from(self.expected_events).map_err(|_| invalid())?,
        )?;
        if expected.kind() != kind {
            return Err(invalid().into());
        }
        self.expected_events += 1;
        Ok(())
    }
}
