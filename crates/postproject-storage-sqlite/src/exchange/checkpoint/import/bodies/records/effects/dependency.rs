use postproject_core::{RevisionEventKind, SemanticConflictKey};
use postproject_protocol::{DependencySetHeader, Document};
use rusqlite::Connection;

use crate::{
    ExchangeResult,
    exchange::checkpoint::import::{dependency_state, guard_state},
};

use super::RetainedEffects;

impl RetainedEffects {
    pub(super) fn start_dependency(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<()> {
        let header = DependencySetHeader::from_document(document)?;
        let source = header.source_representation_id();
        let pending =
            dependency_state::Replacement::begin(connection, header, self.sequence, self.floor)?;
        self.expect_observation(
            connection,
            &RevisionEventKind::DependencySetRecorded {
                representation_id: source,
            },
        )?;
        guard_state::recorded(
            connection,
            &SemanticConflictKey::DependencySet(source),
            self.sequence,
        )?;
        if header.occurrence_count() == 0 {
            pending.finish(connection)?;
        } else {
            self.dependency = Some(pending);
        }
        Ok(())
    }
}
