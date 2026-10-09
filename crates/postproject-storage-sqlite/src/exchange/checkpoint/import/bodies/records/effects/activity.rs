use postproject_core::{ActivityId, ObjectRef, RepresentationId, RevisionEventKind};
use postproject_protocol::{
    ActivityEdgeHeader, ActivityEdgeSide, ActivityHeader, Document, decode_fingerprint_snapshot,
};
use rusqlite::Connection;

use crate::{
    ExchangeResult,
    exchange::checkpoint::import::{activity_state, fingerprint_state, media_state},
};

use super::invalid;

impl super::RetainedEffects {
    pub(super) fn start_activity(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<()> {
        let header = ActivityHeader::from_document(document)?;
        self.activity = Some(ActivityAudit::begin(
            connection,
            &header,
            self.sequence,
            self.floor,
            document,
        )?);
        self.expect_observation(
            connection,
            &RevisionEventKind::ActivityCreated {
                activity_id: header.id(),
                kind: header.kind().clone(),
            },
        )
    }
}

pub(super) struct ActivityAudit {
    id: ActivityId,
    sequence: u64,
    floor: u64,
    position: u64,
    owner: Option<RepresentationId>,
}

impl ActivityAudit {
    pub(super) fn begin(
        connection: &Connection,
        header: &ActivityHeader,
        sequence: u64,
        floor: u64,
        document: &Document,
    ) -> ExchangeResult<Self> {
        if activity_state::frame(connection, header.id(), 0, document)? {
            return Err(invalid().into());
        }
        activity_state::created(connection, header.id())?;
        Ok(Self {
            id: header.id(),
            sequence,
            floor,
            position: 1,
            owner: None,
        })
    }

    pub(super) fn document(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<(bool, Option<RevisionEventKind>)> {
        let complete = activity_state::frame(connection, self.id, self.position, document)?;
        self.position = self.position.checked_add(1).ok_or_else(invalid)?;
        let observation = match document.kind()? {
            "activity.edge" => {
                let edge = ActivityEdgeHeader::from_document(document)?;
                if edge.snapshot_revision_sequence() != Some(self.sequence) {
                    return Err(invalid().into());
                }
                if edge.dependency_path_count() != 0 {
                    return Err(postproject_protocol::ProtocolError::new(
                        postproject_protocol::FailureKind::Unsupported,
                        "checkpoint retained dependency-path audit is not supported yet",
                    )
                    .into());
                }
                media_state::require(
                    connection,
                    ObjectRef::Representation(edge.representation_id()),
                    self.floor,
                )?;
                fingerprint_state::snapshot_count(
                    connection,
                    edge.representation_id(),
                    edge.fingerprint_count(),
                    self.floor,
                )?;
                self.owner = Some(edge.representation_id());
                Some(match edge.side() {
                    ActivityEdgeSide::Input => RevisionEventKind::ActivityInputAdded {
                        activity_id: self.id,
                        representation_id: edge.representation_id(),
                        role: edge.role().cloned(),
                    },
                    ActivityEdgeSide::Output => RevisionEventKind::ActivityOutputAdded {
                        activity_id: self.id,
                        representation_id: edge.representation_id(),
                        role: edge.role().cloned(),
                    },
                })
            }
            "fingerprint.snapshot" => {
                fingerprint_state::snapshot(
                    connection,
                    self.owner.ok_or_else(invalid)?,
                    &decode_fingerprint_snapshot(document)?,
                    self.floor,
                )?;
                None
            }
            _ => return Err(invalid().into()),
        };
        Ok((complete, observation))
    }
}
