use postproject_core::{ActivityId, ActivityRole, ObjectRef, RepresentationId, RevisionEventKind};
use postproject_protocol::{
    ActivityEdgeHeader, ActivityEdgeSide, ActivityHeader, ActivityPathHeader, ActivityPathSegment,
    ActivityPathStatus, Document, decode_fingerprint_snapshot,
};
use rusqlite::Connection;

use crate::{
    ExchangeResult,
    exchange::checkpoint::import::{
        activity_state, dependency_state, fingerprint_state, media_state,
    },
    sqlite_error,
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
    input: Option<(RepresentationId, Option<ActivityRole>)>,
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
            input: None,
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
                self.finish_input(connection)?;
                let edge = ActivityEdgeHeader::from_document(document)?;
                if edge.snapshot_revision_sequence() != Some(self.sequence) {
                    return Err(invalid().into());
                }
                if edge.side() == ActivityEdgeSide::Input {
                    if !edge.has_dependency_snapshot() {
                        return Err(invalid().into());
                    }
                    self.input = Some((edge.representation_id(), edge.role().cloned()));
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
            "activity.dependency-path" => {
                let path = ActivityPathHeader::from_document(document)?;
                let subject = path.subject_representation_id();
                media_state::require(connection, ObjectRef::Representation(subject), self.floor)?;
                dependency_state::subject(connection, subject, path.status(), self.floor)?;
                if path.status() == ActivityPathStatus::Recorded {
                    fingerprint_state::snapshot_count(
                        connection,
                        subject,
                        path.fingerprint_count(),
                        self.floor,
                    )?;
                }
                self.owner = Some(subject);
                None
            }
            "activity.dependency-segment" => {
                dependency_state::segment(
                    connection,
                    &ActivityPathSegment::from_document(document)?,
                    self.floor,
                )?;
                None
            }
            _ => return Err(invalid().into()),
        };
        if complete {
            self.finish_input(connection)?;
        }
        Ok((complete, observation))
    }

    fn finish_input(&mut self, connection: &Connection) -> ExchangeResult<()> {
        if let Some((owner, role)) = self.input.take() {
            let input = connection.query_row("SELECT id FROM activity_inputs WHERE activity_id = ?1 AND representation_id = ?2 AND role IS ?3", rusqlite::params![self.id.as_bytes().as_slice(), owner.as_bytes().as_slice(), role.as_ref().map(ActivityRole::as_str)], |row| row.get(0))
                .map_err(sqlite_error("read original activity input identity"))?;
            dependency_state::validate_paths(connection, input, owner, self.floor)?;
        }
        Ok(())
    }
}
