use postproject_core::{ObjectRef, RevisionEventKind, RevisionId};
use postproject_protocol::{CreationDecoder, CreationFact, Document, RepresentationCreationStart};
use rusqlite::Connection;

use crate::{
    ExchangeResult,
    exchange::checkpoint::import::{
        fingerprint_state, locator_state, media_state, recomputation_state,
    },
};

use super::{invalid, observation};

pub(super) struct CreationAudit {
    header: RepresentationCreationStart,
    decoder: CreationDecoder,
    revision: RevisionId,
    base: u64,
    resources: u64,
    locators: u64,
}

impl CreationAudit {
    pub(super) fn new(
        connection: &Connection,
        revision: RevisionId,
        sequence: u64,
        floor: u64,
        total: u64,
        header: RepresentationCreationStart,
        events: &mut u64,
    ) -> ExchangeResult<Self> {
        let representation = header.representation();
        media_state::require(
            connection,
            ObjectRef::Asset(representation.asset_id()),
            floor,
        )?;
        let base = events.checked_add(1).ok_or_else(invalid)?;
        let end = header
            .resource_count()
            .checked_mul(2)
            .and_then(|count| count.checked_add(header.locator_count()))
            .and_then(|count| count.checked_add(base))
            .ok_or_else(invalid)?;
        if end > total {
            return Err(invalid().into());
        }
        media_state::representation(connection, representation)?;
        recomputation_state::born(connection, representation.id())?;
        expect(
            connection,
            revision,
            *events,
            &RevisionEventKind::RepresentationAdded {
                asset_id: representation.asset_id(),
                representation_id: representation.id(),
            },
        )?;
        *events = end;
        Ok(Self {
            header,
            decoder: CreationDecoder::new(header, sequence)?,
            revision,
            base,
            resources: 0,
            locators: 0,
        })
    }

    pub(super) fn document(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<()> {
        let representation = self.header.representation().id();
        match self.decoder.push(document)? {
            Some(
                CreationFact::RepresentationFingerprint(fact)
                | CreationFact::ResourceFingerprint(fact),
            ) => fingerprint_state::initial(connection, &fact)?,
            Some(CreationFact::Structure(structure)) => {
                media_state::structure(connection, representation, &structure)?;
                for (position, resource_id) in structure.resource_ids().into_iter().enumerate() {
                    let position = u32::try_from(position).map_err(|_| invalid())?;
                    expect(
                        connection,
                        self.revision,
                        self.base + self.header.resource_count() + u64::from(position),
                        &RevisionEventKind::RepresentationResourceAdded {
                            representation_id: representation,
                            resource_id,
                            position,
                        },
                    )?;
                }
            }
            Some(CreationFact::Resource(resource)) => {
                media_state::resource(connection, resource)?;
                expect(
                    connection,
                    self.revision,
                    self.base + self.resources,
                    &RevisionEventKind::ResourceAdded {
                        resource_id: resource.id(),
                    },
                )?;
                self.resources += 1;
            }
            Some(CreationFact::Locator(locator)) => {
                locator_state::added(connection, &locator)?;
                expect(
                    connection,
                    self.revision,
                    self.base + 2 * self.header.resource_count() + self.locators,
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
        Ok(self.decoder.finish()?)
    }
}

fn expect(
    connection: &Connection,
    revision: RevisionId,
    position: u64,
    kind: &RevisionEventKind,
) -> ExchangeResult<()> {
    if observation(
        connection,
        revision,
        u32::try_from(position).map_err(|_| invalid())?,
    )?
    .kind()
        != kind
    {
        return Err(invalid().into());
    }
    Ok(())
}
