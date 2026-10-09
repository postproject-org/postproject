use postproject_core::{RevisionEventKind, RevisionId};
use postproject_protocol::{
    Document, MediaChange, MetadataEffectStart, RecordFeature, RecordManifest, decode_event,
};
use rusqlite::Connection;

use crate::ExchangeResult;

use super::super::{super::super::invalid, history::observation};

pub(super) struct RetainedEffects {
    revision: RevisionId,
    total: u64,
    total_events: u64,
    media: bool,
    metadata: bool,
    effects: u64,
    events: u64,
    remaining_values: u64,
    value_index: u64,
    effect: Option<MetadataEffectStart>,
    genesis: bool,
}

impl RetainedEffects {
    pub(super) fn new(manifest: &RecordManifest, genesis: bool) -> Self {
        Self {
            revision: manifest.revision().id(),
            total: manifest.effect_count(),
            total_events: manifest.event_count(),
            media: manifest
                .required_features()
                .any(|feature| feature == RecordFeature::Media),
            metadata: manifest
                .required_features()
                .any(|feature| feature == RecordFeature::Metadata),
            effects: 0,
            events: 0,
            remaining_values: 0,
            value_index: 0,
            effect: None,
            genesis,
        }
    }

    pub(super) fn document(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<()> {
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
            if matches!(
                document.kind()?,
                "root.added" | "root.enabled" | "root.removed"
            ) {
                if !self.media {
                    return Err(invalid().into());
                }
                let change = MediaChange::from_document(document)?;
                let expected = observation(
                    connection,
                    self.revision,
                    u32::try_from(self.effects).map_err(|_| invalid())?,
                )?;
                if expected.kind() != &change.observation() {
                    return Err(invalid().into());
                }
                super::super::super::root_state::change(connection, &change, self.genesis)?;
                self.effects += 1;
                return Ok(());
            }
            if !self.metadata {
                return Err(invalid().into());
            }
            let effect = MetadataEffectStart::from_document(document)?;
            let position = u32::try_from(self.effects).map_err(|_| invalid())?;
            let expected = observation(connection, self.revision, position)?;
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
            if expected.kind() != &kind {
                return Err(invalid().into());
            }
            super::super::super::metadata_state::start(connection, &effect, self.genesis)?;
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
        {
            return Err(invalid().into());
        }
        Ok(())
    }
}
