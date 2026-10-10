use postproject_core::{Dependency, DependencySetStatus};
use postproject_protocol::{DependencyOccurrence, DependencySetHeader, Document, RecordManifest};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{effects::invalid, facts::structural};
use crate::{
    ExchangeResult, decode_dependency, sqlite_error,
    transaction::{dependency_persist, representation_exists, validate_dependency_references},
};

pub(in crate::exchange) struct DependencyApply {
    header: DependencySetHeader,
    position: u64,
    unchanged: bool,
    checkpoint: bool,
}

impl DependencyApply {
    pub(super) fn begin(
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        header: DependencySetHeader,
    ) -> ExchangeResult<Self> {
        let source = header.source_representation_id();
        if header.status() != DependencySetStatus::Current
            || header.recorded_at_revision() != manifest.revision().sequence()
            || !representation_exists(transaction, source)?
        {
            return Err(invalid().into());
        }
        let prior = transaction.query_row(
            "SELECT needs_extraction, (SELECT COUNT(*) FROM dependencies WHERE source_representation_id = ?1) FROM dependency_sets WHERE source_representation_id = ?1",
            [source.as_bytes().as_slice()], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        ).optional().map_err(sqlite_error("read prior dependency observation"))?;
        let unchanged = prior.is_some_and(|(status, count)| {
            status == 0 && u64::try_from(count).ok() == Some(header.occurrence_count())
        });
        transaction.execute(
            "INSERT INTO dependency_sets (source_representation_id, recorded_revision_sequence, needs_extraction) VALUES (?1, ?2, 0) ON CONFLICT(source_representation_id) DO UPDATE SET recorded_revision_sequence = excluded.recorded_revision_sequence, needs_extraction = 0",
            params![source.as_bytes().as_slice(), i64::try_from(header.recorded_at_revision()).map_err(|_| invalid())?],
        ).map_err(sqlite_error("stage authored dependency header"))?;
        let state = Self {
            header,
            position: 0,
            unchanged,
            checkpoint: false,
        };
        if header.occurrence_count() == 0 {
            state.finish(transaction)?;
        }
        Ok(state)
    }

    pub(in crate::exchange) fn push(
        &mut self,
        transaction: &Transaction<'_>,
        document: &Document,
    ) -> ExchangeResult<bool> {
        let occurrence = DependencyOccurrence::from_document(document)?;
        let source = self.header.source_representation_id();
        if occurrence.source_representation_id() != source
            || occurrence.position() != self.position
            || self.position >= self.header.occurrence_count()
        {
            return Err(invalid().into());
        }
        structural(validate_dependency_references(
            transaction,
            source,
            occurrence.dependency(),
        ))?;
        let position = i64::try_from(self.position).map_err(|_| invalid())?;
        if self.unchanged {
            self.unchanged = prior_occurrence(transaction, source, position)?.as_ref()
                == Some(occurrence.dependency());
        }
        if !self.checkpoint {
            transaction
            .execute(
                "DELETE FROM dependencies WHERE source_representation_id = ?1 AND position = ?2",
                params![source.as_bytes().as_slice(), position],
            )
            .map_err(sqlite_error("replace authored dependency occurrence"))?;
        }
        dependency_persist::occurrence(transaction, source, position, occurrence.dependency())?;
        self.position += 1;
        let complete = self.position == self.header.occurrence_count();
        if complete {
            self.finish(transaction)?;
        }
        Ok(complete)
    }

    fn finish(&self, transaction: &Transaction<'_>) -> ExchangeResult<()> {
        if self.unchanged || self.position != self.header.occurrence_count() {
            return Err(invalid().into());
        }
        transaction
            .execute(
                "DELETE FROM dependencies WHERE source_representation_id = ?1 AND position >= ?2",
                params![
                    self.header.source_representation_id().as_bytes().as_slice(),
                    i64::try_from(self.position).map_err(|_| invalid())?
                ],
            )
            .map_err(sqlite_error("finish authored dependency replacement"))?;
        Ok(())
    }

    pub(in crate::exchange) fn begin_checkpoint(
        transaction: &Transaction<'_>,
        head: u64,
        header: DependencySetHeader,
    ) -> ExchangeResult<Self> {
        if header.recorded_at_revision() > head
            || !representation_exists(transaction, header.source_representation_id())?
        {
            return Err(invalid().into());
        }
        transaction.execute("INSERT INTO dependency_sets (source_representation_id, recorded_revision_sequence, needs_extraction) VALUES (?1, ?2, ?3)", params![header.source_representation_id().as_bytes().as_slice(), i64::try_from(header.recorded_at_revision()).map_err(|_|invalid())?, header.status() == DependencySetStatus::NeedsExtraction])
            .map_err(sqlite_error("stage checkpoint dependency header"))?;
        let state = Self {
            header,
            position: 0,
            unchanged: false,
            checkpoint: true,
        };
        if header.occurrence_count() == 0 {
            state.finish(transaction)?;
        }
        Ok(state)
    }
}

fn prior_occurrence(
    transaction: &Transaction<'_>,
    source: postproject_core::RepresentationId,
    position: i64,
) -> ExchangeResult<Option<Dependency>> {
    let prior = transaction.query_row("SELECT source_resource_id, kind, target_kind, target_id, resolved_representation_id, required, authored_reference FROM dependencies WHERE source_representation_id = ?1 AND position = ?2", params![source.as_bytes().as_slice(), position], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))).optional().map_err(sqlite_error("compare authored dependency occurrence"))?;
    prior
        .map(
            |(resource, kind, target_kind, target, resolved, required, authored)| {
                decode_dependency(
                    resource,
                    kind,
                    target_kind,
                    target,
                    resolved,
                    required,
                    authored,
                )
                .map_err(Into::into)
            },
        )
        .transpose()
}
