//! Checked creation facts remain private until the enclosing record commits.

use postproject_core::{Asset, ObjectRef, Resource, RevisionEventKind};
use postproject_protocol::{
    CreationDecoder, CreationFact, Document, FingerprintObservation, RecordManifest,
    RepresentationCreationStart,
};
use rusqlite::{Transaction, params};

use super::{
    effects::invalid,
    facts::{observation, structural},
};
use crate::{
    ExchangeResult,
    transaction::{
        asset_exists, encode_representation_kind, encode_structure_kind, mutation_error,
        persist_content_structure, persist_locator, persist_resource,
    },
};

pub(super) fn asset(transaction: &Transaction<'_>, asset: &Asset) -> ExchangeResult<()> {
    structural(transaction.execute(
        "INSERT INTO assets (id, created_at_micros, display_name, import_source) VALUES (?1, ?2, ?3, ?4)",
        params![asset.id().as_bytes().as_slice(), asset.created_at().as_unix_micros(), asset.display_name(), asset.import_source()],
    ).map_err(mutation_error("stage replayed asset")))?;
    Ok(())
}

pub(super) struct CreationApply {
    header: RepresentationCreationStart,
    decoder: CreationDecoder,
    event_base: u64,
    resources: u64,
    locators: u64,
}

impl CreationApply {
    pub(super) fn new(
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        header: RepresentationCreationStart,
        events: &mut u64,
    ) -> ExchangeResult<Self> {
        let representation = header.representation();
        if !asset_exists(transaction, representation.asset_id())? {
            return Err(invalid().into());
        }
        let event_base = events.checked_add(1).ok_or_else(invalid)?;
        let total = header
            .resource_count()
            .checked_mul(2)
            .and_then(|count| count.checked_add(header.locator_count()))
            .and_then(|count| count.checked_add(event_base))
            .ok_or_else(invalid)?;
        if total > manifest.event_count() {
            return Err(invalid().into());
        }
        // The staging row starts with a provisional structure kind. The checked
        // structure continuation replaces it before the decoder can complete;
        // no prefix can become visible. Foreign keys are deferred until commit.
        structural(transaction.execute(
            "INSERT INTO representations (id, asset_id, kind, structure_kind) VALUES (?1, ?2, ?3, 0)",
            params![representation.id().as_bytes().as_slice(), representation.asset_id().as_bytes().as_slice(), encode_representation_kind(representation.kind())?],
        ).map_err(mutation_error("stage replayed representation")))?;
        observation(
            transaction,
            manifest,
            *events,
            &RevisionEventKind::RepresentationAdded {
                asset_id: representation.asset_id(),
                representation_id: representation.id(),
            },
        )?;
        *events = total;
        Ok(Self {
            header,
            decoder: CreationDecoder::new(header, manifest.revision().sequence())?,
            event_base,
            resources: 0,
            locators: 0,
        })
    }

    pub(super) fn document(
        &mut self,
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        document: &Document,
    ) -> ExchangeResult<()> {
        let representation = self.header.representation();
        match self.decoder.push(document)? {
            Some(
                CreationFact::RepresentationFingerprint(fact)
                | CreationFact::ResourceFingerprint(fact),
            ) => fingerprint(transaction, &fact)?,
            Some(CreationFact::Structure(structure)) => {
                transaction
                    .execute(
                        "UPDATE representations SET structure_kind = ?1 WHERE id = ?2",
                        params![
                            encode_structure_kind(structure.kind())?,
                            representation.id().as_bytes().as_slice()
                        ],
                    )
                    .map_err(mutation_error("stage replayed content kind"))?;
                structural(persist_content_structure(
                    transaction,
                    representation.id(),
                    &structure,
                ))?;
                for (position, resource_id) in structure.resource_ids().into_iter().enumerate() {
                    let position = u32::try_from(position).map_err(|_| invalid())?;
                    observation(
                        transaction,
                        manifest,
                        self.event_base + self.header.resource_count() + u64::from(position),
                        &RevisionEventKind::RepresentationResourceAdded {
                            representation_id: representation.id(),
                            resource_id,
                            position,
                        },
                    )?;
                }
            }
            Some(CreationFact::Resource(resource)) => {
                structural(persist_resource(
                    transaction,
                    &Resource::new(resource.id(), Vec::new(), resource.file_facts()),
                    i64::try_from(manifest.revision().sequence()).map_err(|_| invalid())?,
                ))?;
                observation(
                    transaction,
                    manifest,
                    self.event_base + self.resources,
                    &RevisionEventKind::ResourceAdded {
                        resource_id: resource.id(),
                    },
                )?;
                self.resources += 1;
            }
            Some(CreationFact::Locator(locator)) => {
                structural(persist_locator(transaction, &locator))?;
                observation(
                    transaction,
                    manifest,
                    self.event_base + 2 * self.header.resource_count() + self.locators,
                    &RevisionEventKind::LocatorAdded {
                        resource_id: locator.resource_id(),
                        locator_id: locator.id(),
                    },
                )?;
                self.locators += 1;
            }
            None => {}
        }
        Ok(())
    }

    pub(super) fn is_complete(&self) -> bool {
        self.decoder.is_complete()
    }

    pub(super) fn finish(self) -> ExchangeResult<()> {
        self.decoder.finish()?;
        Ok(())
    }
}

fn fingerprint(transaction: &Transaction<'_>, fact: &FingerprintObservation) -> ExchangeResult<()> {
    let (table, column, id) = match fact.target() {
        ObjectRef::Representation(id) => (
            "representation_fingerprints",
            "representation_id",
            id.into_bytes(),
        ),
        ObjectRef::Resource(id) => ("resource_fingerprints", "resource_id", id.into_bytes()),
        _ => return Err(invalid().into()),
    };
    let snapshot = fact.snapshot();
    let sequence = snapshot.observed_revision_sequence().ok_or_else(invalid)?;
    structural(transaction.execute(
        &format!("INSERT INTO {table} ({column}, algorithm, algorithm_version, value, observed_revision_sequence) VALUES (?1, ?2, ?3, ?4, ?5)"),
        params![id.as_slice(), snapshot.algorithm(), snapshot.version(), snapshot.value(), i64::try_from(sequence).map_err(|_| invalid())?],
    ).map_err(mutation_error("stage replayed initial fingerprint")))?;
    Ok(())
}
