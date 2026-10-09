mod segment;

use postproject_core::RepresentationId;
use postproject_protocol::{ActivityPathHeader, ActivityPathSegment, ActivityPathStatus, Document};
use rusqlite::{OptionalExtension, Transaction, params};

use super::super::effects::invalid;
use super::{context::Context, fingerprints};
use crate::{ExchangeResult, sqlite_error, transaction::representation_exists};

pub(super) struct PathApply {
    id: i64,
    header: ActivityPathHeader,
    segments: u64,
    next_source: Option<RepresentationId>,
    last_source: RepresentationId,
    fingerprints: u64,
    previous_domain: Option<(String, u16)>,
}

impl PathApply {
    pub(super) fn begin(
        transaction: &Transaction<'_>,
        context: Context<'_>,
        document: &Document,
        input: i64,
        position: u64,
        root: RepresentationId,
    ) -> ExchangeResult<Self> {
        let header = ActivityPathHeader::from_document(document)?;
        if header.position() != position
            || !representation_exists(transaction, header.subject_representation_id())?
        {
            return Err(invalid().into());
        }
        if context.prefix() && header.status() == ActivityPathStatus::Recorded {
            fingerprints::count(
                transaction,
                header.subject_representation_id(),
                header.fingerprint_count(),
            )?;
        }
        let status = match header.status() {
            ActivityPathStatus::Recorded => 0,
            ActivityPathStatus::NeedsExtraction => 1,
            ActivityPathStatus::Unresolved => 2,
            ActivityPathStatus::DepthTruncated => 3,
            ActivityPathStatus::RepresentationsTruncated => 4,
        };
        transaction.execute("INSERT INTO activity_input_dependency_paths (activity_input_id, position, status, subject_representation_id) VALUES (?1, ?2, ?3, ?4)", params![input, i64::try_from(position).map_err(|_|invalid())?, status, header.subject_representation_id().as_bytes().as_slice()]).map_err(sqlite_error("stage original dependency path"))?;
        let state = Self {
            id: transaction.last_insert_rowid(),
            header,
            segments: 0,
            next_source: Some(root),
            last_source: root,
            fingerprints: 0,
            previous_domain: None,
        };
        if state.header.segment_count() == 0 {
            state.validate_subject(transaction, context)?;
        }
        Ok(state)
    }

    pub(super) fn push(
        &mut self,
        transaction: &Transaction<'_>,
        context: Context<'_>,
        document: &Document,
    ) -> ExchangeResult<()> {
        if self.segments < self.header.segment_count() {
            let segment = ActivityPathSegment::from_document(document)?;
            let occurrence = segment.occurrence();
            if segment.position() != self.segments
                || Some(occurrence.source_representation_id()) != self.next_source
            {
                return Err(invalid().into());
            }
            segment::persist(transaction, self.id, &segment, context.prefix())?;
            self.last_source = occurrence.source_representation_id();
            self.next_source = match occurrence.dependency().target() {
                postproject_core::DependencyTarget::Representation(id) => Some(id),
                postproject_core::DependencyTarget::Asset(_) => {
                    occurrence.dependency().resolved_representation_id()
                }
                _ => return Err(invalid().into()),
            };
            self.segments += 1;
            if self.segments == self.header.segment_count() {
                self.validate_subject(transaction, context)?;
            }
        } else if self.fingerprints < self.header.fingerprint_count() {
            fingerprints::push(
                transaction,
                context,
                document,
                fingerprints::Owner {
                    table: "activity_input_dependency_fingerprint_snapshots",
                    column: "path_id",
                    id: self.id,
                    subject: self.header.subject_representation_id(),
                },
                &mut self.previous_domain,
            )?;
            self.fingerprints += 1;
        } else {
            return Err(invalid().into());
        }
        Ok(())
    }

    fn validate_subject(
        &self,
        transaction: &Transaction<'_>,
        context: Context<'_>,
    ) -> ExchangeResult<()> {
        let subject = self.header.subject_representation_id();
        if self.header.status() == ActivityPathStatus::Unresolved {
            if self.next_source.is_some() || subject != self.last_source {
                return Err(invalid().into());
            }
        } else if self.next_source != Some(subject) {
            return Err(invalid().into());
        }
        if !context.prefix() {
            return Ok(());
        }
        let needs_extraction = transaction
            .query_row(
                "SELECT needs_extraction FROM dependency_sets WHERE source_representation_id = ?1",
                [subject.as_bytes().as_slice()],
                |row| row.get::<_, bool>(0),
            )
            .optional()
            .map_err(sqlite_error("validate captured dependency status"))?;
        match self.header.status() {
            ActivityPathStatus::NeedsExtraction if needs_extraction != Some(true) => {
                return Err(invalid().into());
            }
            ActivityPathStatus::Recorded | ActivityPathStatus::DepthTruncated
                if needs_extraction == Some(true) =>
            {
                return Err(invalid().into());
            }
            ActivityPathStatus::DepthTruncated => {
                let children: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM dependencies WHERE source_representation_id = ?1 AND required = 1)", [subject.as_bytes().as_slice()], |row|row.get(0)).map_err(sqlite_error("validate captured dependency depth truncation"))?;
                if !children {
                    return Err(invalid().into());
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn complete(&self) -> bool {
        self.segments == self.header.segment_count()
            && self.fingerprints == self.header.fingerprint_count()
    }
}
