use postproject_protocol::{ActivityEdgeHeader, ActivityEdgeSide, Document, RecordManifest};
use rusqlite::{Transaction, params};

use super::super::effects::invalid;
use super::{fingerprints, path::PathApply};
use crate::{
    ExchangeResult, dependency_snapshot::validate_dependency_paths, sqlite_error,
    transaction::representation_exists,
};

pub(super) struct EdgeApply {
    id: i64,
    header: ActivityEdgeHeader,
    fingerprints: u64,
    previous_domain: Option<(String, u16)>,
    paths: u64,
    path: Option<PathApply>,
}

impl EdgeApply {
    pub(super) fn begin(
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        header: ActivityEdgeHeader,
    ) -> ExchangeResult<Self> {
        if header.snapshot_revision_sequence() != Some(manifest.revision().sequence())
            || (header.side() == ActivityEdgeSide::Input && !header.has_dependency_snapshot())
            || !representation_exists(transaction, header.representation_id())?
        {
            return Err(invalid().into());
        }
        if header.side() == ActivityEdgeSide::Output {
            let overlaps: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM activity_inputs WHERE activity_id = ?1 AND representation_id = ?2)", params![header.activity_id().as_bytes().as_slice(), header.representation_id().as_bytes().as_slice()], |row|row.get(0)).map_err(sqlite_error("validate disjoint activity sides"))?;
            if overlaps {
                return Err(invalid().into());
            }
        }
        fingerprints::count(
            transaction,
            header.representation_id(),
            header.fingerprint_count(),
        )?;
        let table = match header.side() {
            ActivityEdgeSide::Input => "activity_inputs",
            ActivityEdgeSide::Output => "activity_outputs",
        };
        transaction.execute(&format!("INSERT INTO {table} (activity_id, representation_id, role, snapshot_revision_sequence) VALUES (?1, ?2, ?3, ?4)"), params![header.activity_id().as_bytes().as_slice(), header.representation_id().as_bytes().as_slice(), header.role().map(postproject_core::ActivityRole::as_str), i64::try_from(manifest.revision().sequence()).map_err(|_|invalid())?])
            .map_err(sqlite_error("stage original activity edge"))?;
        let id = transaction.last_insert_rowid();
        if header.has_dependency_snapshot() {
            transaction.execute("INSERT INTO activity_input_dependency_snapshots (activity_input_id) VALUES (?1)", [id]).map_err(sqlite_error("stage original dependency snapshot marker"))?;
        }
        if header.has_dependency_snapshot() && header.dependency_path_count() == 0 {
            super::super::facts::structural(validate_dependency_paths(
                transaction,
                id,
                header.representation_id(),
            ))?;
        }
        Ok(Self {
            id,
            header,
            fingerprints: 0,
            previous_domain: None,
            paths: 0,
            path: None,
        })
    }

    pub(super) fn push(
        &mut self,
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        document: &Document,
    ) -> ExchangeResult<()> {
        if self.fingerprints < self.header.fingerprint_count() {
            let (table, column) = match self.header.side() {
                ActivityEdgeSide::Input => {
                    ("activity_input_fingerprint_snapshots", "activity_input_id")
                }
                ActivityEdgeSide::Output => (
                    "activity_output_fingerprint_snapshots",
                    "activity_output_id",
                ),
            };
            fingerprints::push(
                transaction,
                manifest,
                document,
                fingerprints::Owner {
                    table,
                    column,
                    id: self.id,
                    subject: self.header.representation_id(),
                },
                &mut self.previous_domain,
            )?;
            self.fingerprints += 1;
        } else if let Some(path) = self.path.as_mut() {
            path.push(transaction, manifest, document)?;
            if path.complete() {
                self.path = None;
            }
        } else if self.paths < self.header.dependency_path_count() {
            let path = PathApply::begin(
                transaction,
                document,
                self.id,
                self.paths,
                self.header.representation_id(),
            )?;
            self.paths += 1;
            if !path.complete() {
                self.path = Some(path);
            }
        } else {
            return Err(invalid().into());
        }
        if self.complete() && self.header.has_dependency_snapshot() {
            super::super::facts::structural(validate_dependency_paths(
                transaction,
                self.id,
                self.header.representation_id(),
            ))?;
        }
        Ok(())
    }

    pub(super) fn complete(&self) -> bool {
        self.fingerprints == self.header.fingerprint_count()
            && self.paths == self.header.dependency_path_count()
            && self.path.is_none()
    }
}
