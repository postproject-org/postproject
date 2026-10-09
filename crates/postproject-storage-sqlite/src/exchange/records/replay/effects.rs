use postproject_core::{AssetId, RepresentationKind, RevisionEventKind, SemanticConflictKey};
use postproject_protocol::{
    DependencySetHeader, Document, FailureKind, FingerprintChangeStart, IdentifierChange,
    MediaChange, MetadataEffectStart, MetadataOperation, ProtocolError, RecordFeature,
    RecordManifest, RepresentationCreationStart, decode_event, decode_original_creation_start,
};
use rusqlite::{Transaction, params};

use crate::{
    ExchangeResult, encode_metadata_target, metadata_codec, sqlite_error,
    transaction::{
        delete_metadata_property, ensure_metadata_target_exists, insert_metadata_value,
        next_metadata_position,
    },
};

pub(super) struct ApplyEffects<'a, 'connection> {
    transaction: &'a Transaction<'connection>,
    manifest: &'a RecordManifest,
    effect: Option<Pending>,
    effects: u64,
    expected_events: u64,
    events: u64,
}

enum Pending {
    Metadata(MetadataEffectStart, u64),
    Original(AssetId),
    Creation(Box<super::creation::CreationApply>),
    Fingerprint(Box<super::fingerprint_change::FingerprintApply>),
    Dependency(super::dependency::DependencyApply),
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
            expected_events: 0,
            events: 0,
        }
    }

    pub(super) fn document(&mut self, document: &Document) -> ExchangeResult<()> {
        match self.effect.as_ref() {
            Some(Pending::Metadata(..)) => return self.value(document),
            Some(Pending::Original(_)) => return self.start_creation(document),
            Some(Pending::Creation(_)) => return self.creation_document(document),
            Some(Pending::Fingerprint(_)) => return self.fingerprint_document(document),
            Some(Pending::Dependency(_)) => return self.dependency_document(document),
            None => {}
        }
        if self.effects < self.manifest.effect_count() {
            return match document.kind()? {
                "metadata.effect" => {
                    self.require_feature(RecordFeature::Metadata)?;
                    self.start(MetadataEffectStart::from_document(document)?)
                }
                "original.creation" => {
                    self.require_feature(RecordFeature::Media)?;
                    let asset = decode_original_creation_start(document)?;
                    super::creation::asset(self.transaction, &asset)?;
                    super::facts::observation(
                        self.transaction,
                        self.manifest,
                        self.expected_events,
                        &RevisionEventKind::AssetImported {
                            asset_id: asset.id(),
                        },
                    )?;
                    self.expected_events += 1;
                    self.effects += 1;
                    self.effect = Some(Pending::Original(asset.id()));
                    Ok(())
                }
                "representation.creation" => self.start_creation(document),
                "fingerprint.change" => self.start_fingerprint(document),
                "dependency.set" => self.start_dependency(document),
                "identifier.added" | "identifier.removed" => self.identifier_document(document),
                "resource.file-facts"
                | "root.added"
                | "root.enabled"
                | "root.removed"
                | "locator.added"
                | "locator.retired" => {
                    self.require_feature(RecordFeature::Media)?;
                    let change = MediaChange::from_document(document)?;
                    super::media_change::apply(self.transaction, &change)?;
                    super::facts::changed(self.transaction, self.manifest, &change.conflict_key())?;
                    super::facts::observation(
                        self.transaction,
                        self.manifest,
                        self.expected_events,
                        &change.observation(),
                    )?;
                    self.expected_events += 1;
                    self.effects += 1;
                    Ok(())
                }
                _ => Err(ProtocolError::new(
                    FailureKind::Unsupported,
                    "unsupported authored effect",
                )
                .into()),
            };
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

    fn start_dependency(&mut self, document: &Document) -> ExchangeResult<()> {
        self.require_feature(RecordFeature::Dependencies)?;
        let header = DependencySetHeader::from_document(document)?;
        let source = header.source_representation_id();
        let pending =
            super::dependency::DependencyApply::begin(self.transaction, self.manifest, header)?;
        super::facts::changed(
            self.transaction,
            self.manifest,
            &SemanticConflictKey::DependencySet(source),
        )?;
        super::facts::observation(
            self.transaction,
            self.manifest,
            self.expected_events,
            &RevisionEventKind::DependencySetRecorded {
                representation_id: source,
            },
        )?;
        self.expected_events += 1;
        self.effects += 1;
        if header.occurrence_count() != 0 {
            self.effect = Some(Pending::Dependency(pending));
        }
        Ok(())
    }

    fn dependency_document(&mut self, document: &Document) -> ExchangeResult<()> {
        let Some(Pending::Dependency(pending)) = self.effect.as_mut() else {
            return Err(invalid().into());
        };
        if pending.push(self.transaction, document)? {
            self.effect = None;
        }
        Ok(())
    }

    fn identifier_document(&mut self, document: &Document) -> ExchangeResult<()> {
        self.require_feature(RecordFeature::Media)?;
        let change = IdentifierChange::from_document(document)?;
        super::identifier_change::apply(self.transaction, &change)?;
        super::facts::changed(self.transaction, self.manifest, &change.conflict_key())?;
        super::facts::observation(
            self.transaction,
            self.manifest,
            self.expected_events,
            &change.observation(),
        )?;
        self.expected_events += 1;
        self.effects += 1;
        Ok(())
    }

    fn start_fingerprint(&mut self, document: &Document) -> ExchangeResult<()> {
        self.require_feature(RecordFeature::Media)?;
        let start = FingerprintChangeStart::from_document(document)?;
        let markers = start.marker_count();
        let observation = start.observation();
        let key = start.conflict_key();
        let pending = super::fingerprint_change::FingerprintApply::begin(
            self.transaction,
            self.manifest,
            start,
        )?;
        super::facts::changed(self.transaction, self.manifest, &key)?;
        super::facts::observation(
            self.transaction,
            self.manifest,
            self.expected_events,
            &observation,
        )?;
        self.expected_events += 1;
        self.effects += 1;
        if markers != 0 {
            self.effect = Some(Pending::Fingerprint(Box::new(pending)));
        }
        Ok(())
    }

    fn fingerprint_document(&mut self, document: &Document) -> ExchangeResult<()> {
        let Some(Pending::Fingerprint(pending)) = self.effect.as_mut() else {
            return Err(invalid().into());
        };
        if pending.push(self.transaction, self.manifest, document)? {
            self.effect = None;
        }
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
        super::facts::changed(
            self.transaction,
            self.manifest,
            &SemanticConflictKey::MetadataProperty {
                target: effect.target(),
                property: effect.property().clone(),
            },
        )?;
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
        super::facts::observation(
            self.transaction,
            self.manifest,
            self.expected_events,
            &event,
        )?;
        self.expected_events += 1;
        self.effects += 1;
        if effect.value_count() > 0 {
            self.effect = Some(Pending::Metadata(effect, 0));
        }
        Ok(())
    }

    fn value(&mut self, document: &Document) -> ExchangeResult<()> {
        let Some(Pending::Metadata(effect, index)) = self.effect.as_mut() else {
            return Err(invalid().into());
        };
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

    fn require_feature(&self, feature: RecordFeature) -> ExchangeResult<()> {
        if !self
            .manifest
            .required_features()
            .any(|required| required == feature)
        {
            return Err(invalid().into());
        }
        Ok(())
    }

    fn start_creation(&mut self, document: &Document) -> ExchangeResult<()> {
        self.require_feature(RecordFeature::Media)?;
        let header = RepresentationCreationStart::from_document(document)?;
        if let Some(Pending::Original(asset)) = self.effect.take() {
            if header.representation().asset_id() != asset
                || header.representation().kind() != RepresentationKind::Original
            {
                return Err(invalid().into());
            }
        } else {
            self.effects += 1;
        }
        let creation = super::creation::CreationApply::new(
            self.transaction,
            self.manifest,
            header,
            &mut self.expected_events,
        )?;
        self.effect = Some(Pending::Creation(Box::new(creation)));
        Ok(())
    }

    fn creation_document(&mut self, document: &Document) -> ExchangeResult<()> {
        let Some(Pending::Creation(mut creation)) = self.effect.take() else {
            return Err(invalid().into());
        };
        creation.document(self.transaction, self.manifest, document)?;
        if creation.is_complete() {
            creation.finish()?;
        } else {
            self.effect = Some(Pending::Creation(creation));
        }
        Ok(())
    }

    pub(super) fn finish(self) -> ExchangeResult<()> {
        if self.effect.is_some()
            || self.effects != self.manifest.effect_count()
            || self.events != self.manifest.event_count()
            || self.expected_events != self.events
        {
            return Err(invalid().into());
        }
        Ok(())
    }
}

pub(super) fn invalid() -> ProtocolError {
    ProtocolError::new(
        FailureKind::Integrity,
        "record effects do not match structural state or observations",
    )
}
