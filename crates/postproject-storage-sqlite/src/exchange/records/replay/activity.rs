mod edge;
mod fingerprints;
mod path;

use postproject_core::RevisionEventKind;
use postproject_protocol::{
    ActivityEdgeHeader, ActivityEdgeSide, ActivityHeader, Document, RecordManifest,
};
use rusqlite::{Transaction, params};

use super::{effects::invalid, facts};
use crate::{ExchangeResult, sqlite_error};

pub(super) struct ActivityApply {
    header: ActivityHeader,
    edges: u64,
    edge: Option<edge::EdgeApply>,
    previous: Option<(
        postproject_core::RepresentationId,
        Option<postproject_core::ActivityRole>,
    )>,
}

impl ActivityApply {
    pub(super) fn begin(
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        header: ActivityHeader,
        event: &mut u64,
    ) -> ExchangeResult<Self> {
        let tool = header.tool();
        let agent = header.agent();
        let identifier = agent.and_then(postproject_core::AgentIdentity::identifier);
        transaction.execute("INSERT INTO activities (id, kind, started_at_micros, finished_at_micros, tool_name, tool_version, tool_uri, agent_name, agent_identifier_scheme, agent_identifier_value, agent_identifier_qualifier) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![header.id().as_bytes().as_slice(), header.kind().as_str(), header.started_at().map(postproject_core::Timestamp::as_unix_micros), header.finished_at().map(postproject_core::Timestamp::as_unix_micros), tool.map(postproject_core::ToolIdentity::name), tool.and_then(postproject_core::ToolIdentity::version), tool.and_then(postproject_core::ToolIdentity::uri), agent.and_then(postproject_core::AgentIdentity::name), identifier.map(|value|value.scheme().as_str()), identifier.map(postproject_core::ExternalIdentifier::value), identifier.and_then(postproject_core::ExternalIdentifier::qualifier)])
            .map_err(sqlite_error("stage original activity attribution"))?;
        facts::observation(
            transaction,
            manifest,
            *event,
            &RevisionEventKind::ActivityCreated {
                activity_id: header.id(),
                kind: header.kind().clone(),
            },
        )?;
        *event += 1;
        Ok(Self {
            header,
            edges: 0,
            edge: None,
            previous: None,
        })
    }

    pub(super) fn push(
        &mut self,
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        document: &Document,
        event: &mut u64,
    ) -> ExchangeResult<bool> {
        if let Some(edge) = self.edge.as_mut() {
            edge.push(transaction, manifest, document)?;
            if !edge.complete() {
                return Ok(false);
            }
            self.edge = None;
        } else {
            self.start_edge(transaction, manifest, document, event)?;
        }
        let complete = self.edges == self.header.input_count() + self.header.output_count()
            && self.edge.is_none();
        if complete {
            self.finish(transaction)?;
        }
        Ok(complete)
    }

    fn start_edge(
        &mut self,
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        document: &Document,
        event: &mut u64,
    ) -> ExchangeResult<()> {
        let header = ActivityEdgeHeader::from_document(document)?;
        let (side, position) = if self.edges < self.header.input_count() {
            (ActivityEdgeSide::Input, self.edges)
        } else {
            (
                ActivityEdgeSide::Output,
                self.edges - self.header.input_count(),
            )
        };
        if position == 0 {
            self.previous = None;
        }
        let key = (header.representation_id(), header.role().cloned());
        if header.activity_id() != self.header.id()
            || header.side() != side
            || header.position() != position
            || self
                .previous
                .as_ref()
                .is_some_and(|previous| previous >= &key)
        {
            return Err(invalid().into());
        }
        let pending = edge::EdgeApply::begin(transaction, manifest, header.clone())?;
        let observation = match side {
            ActivityEdgeSide::Input => RevisionEventKind::ActivityInputAdded {
                activity_id: self.header.id(),
                representation_id: header.representation_id(),
                role: header.role().cloned(),
            },
            ActivityEdgeSide::Output => RevisionEventKind::ActivityOutputAdded {
                activity_id: self.header.id(),
                representation_id: header.representation_id(),
                role: header.role().cloned(),
            },
        };
        facts::observation(transaction, manifest, *event, &observation)?;
        *event += 1;
        self.edges += 1;
        self.previous = Some(key);
        if !pending.complete() {
            self.edge = Some(pending);
        }
        Ok(())
    }

    fn finish(&self, transaction: &Transaction<'_>) -> ExchangeResult<()> {
        let cycle: bool = transaction.query_row("WITH RECURSIVE descendants(representation_id) AS (SELECT representation_id FROM activity_outputs WHERE activity_id = ?1 UNION SELECT outputs.representation_id FROM descendants JOIN activity_inputs inputs ON inputs.representation_id = descendants.representation_id JOIN activity_outputs outputs ON outputs.activity_id = inputs.activity_id) SELECT EXISTS(SELECT 1 FROM descendants JOIN activity_inputs inputs ON inputs.activity_id = ?1 AND inputs.representation_id = descendants.representation_id)", [self.header.id().as_bytes().as_slice()], |row|row.get(0)).map_err(sqlite_error("validate original provenance graph"))?;
        if cycle {
            return Err(invalid().into());
        }
        Ok(())
    }
}
