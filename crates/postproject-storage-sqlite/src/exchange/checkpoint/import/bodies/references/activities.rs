use postproject_core::RevisionEventKind;
use rusqlite::params;

use crate::{ExchangeResult, exchange::checkpoint::activity_facts, sqlite_error};

use super::{Bodies, invalid};

impl Bodies<'_, '_> {
    pub(super) fn activity_event(&self, event: &RevisionEventKind) -> ExchangeResult<()> {
        match event {
            RevisionEventKind::ActivityCreated { activity_id, kind } => {
                if activity_facts::header(self.transaction, *activity_id)?.kind() != kind {
                    return Err(invalid().into());
                }
            }
            RevisionEventKind::ActivityInputAdded {
                activity_id,
                representation_id,
                role,
            }
            | RevisionEventKind::ActivityOutputAdded {
                activity_id,
                representation_id,
                role,
            } => {
                let table = if matches!(event, RevisionEventKind::ActivityInputAdded { .. }) {
                    "activity_inputs"
                } else {
                    "activity_outputs"
                };
                let matches: bool = self.transaction.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE activity_id = ?1 AND representation_id = ?2 AND role IS ?3 AND (snapshot_revision_sequence IS NULL OR snapshot_revision_sequence = ?4))"), params![activity_id.as_bytes().as_slice(), representation_id.as_bytes().as_slice(), role.as_ref().map(postproject_core::ActivityRole::as_str), i64::try_from(self.event_sequence).map_err(|_|invalid())?], |row|row.get(0))
                    .map_err(sqlite_error("validate immutable activity edge observation"))?;
                if !matches {
                    return Err(invalid().into());
                }
            }
            _ => return Err(invalid().into()),
        }
        Ok(())
    }

    pub(super) fn validate_activities(&self) -> ExchangeResult<()> {
        for (table, kind) in [
            (
                "activity_inputs",
                postproject_core::RevisionEventType::ActivityInputAdded,
            ),
            (
                "activity_outputs",
                postproject_core::RevisionEventType::ActivityOutputAdded,
            ),
        ] {
            let missing: bool = self.transaction.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} a WHERE a.snapshot_revision_sequence IS NOT NULL AND NOT EXISTS(SELECT 1 FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.kind = ?1 AND e.primary_id = a.activity_id AND e.secondary_id = a.representation_id AND e.role IS a.role AND r.sequence = a.snapshot_revision_sequence))"), [crate::stored_revision_event_kind(kind)?], |row|row.get(0))
                .map_err(sqlite_error("validate original activity snapshot boundary"))?;
            if missing {
                return Err(invalid().into());
            }
        }
        let inconsistent: bool = self.transaction.query_row("SELECT EXISTS(SELECT 1 FROM activity_inputs a JOIN activity_outputs b ON a.activity_id = b.activity_id AND a.representation_id = b.representation_id UNION ALL SELECT 1 FROM activities a WHERE NOT EXISTS(SELECT 1 FROM activity_outputs b WHERE b.activity_id = a.id))", [], |row|row.get(0))
            .map_err(sqlite_error("validate complete disjoint activity edges"))?;
        if inconsistent {
            return Err(invalid().into());
        }
        Ok(())
    }
}
