//! Scalar provenance identity and attribution, independent of edge collections.

mod edge;
mod path;
mod wire;
pub use edge::{ActivityEdgeHeader, ActivityEdgeSide};
pub use path::{ActivityPathHeader, ActivityPathSegment, ActivityPathStatus};

use crate::{Document, Result, fields::malformed};
use postproject_core::{
    Activity, ActivityId, ActivityKind, AgentIdentity, MAX_ACTIVITY_EDGES, Timestamp, ToolIdentity,
};

/// Checked scalar activity facts followed by its complete edge bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityHeader {
    id: ActivityId,
    kind: ActivityKind,
    started_at: Option<Timestamp>,
    finished_at: Option<Timestamp>,
    tool: Option<ToolIdentity>,
    agent: Option<AgentIdentity>,
    inputs: u64,
    outputs: u64,
}

impl ActivityHeader {
    /// Creates a header using native input/output cardinality limits.
    ///
    /// # Errors
    /// Rejects excessive edges or zero outputs. Storage validates actual edges.
    pub fn new(id: ActivityId, kind: ActivityKind, inputs: u64, outputs: u64) -> Result<Self> {
        let valid = |count| usize::try_from(count).is_ok_and(|count| count <= MAX_ACTIVITY_EDGES);
        if !valid(inputs) || !valid(outputs) || outputs == 0 {
            return Err(malformed());
        }
        Ok(Self {
            id,
            kind,
            started_at: None,
            finished_at: None,
            tool: None,
            agent: None,
            inputs,
            outputs,
        })
    }

    /// Copies authored scalar facts while borrowing the native edges.
    ///
    /// # Errors
    /// Rejects unsupported cardinalities or inconsistent timing.
    pub fn from_activity(activity: &Activity) -> Result<Self> {
        let mut header = Self::new(
            activity.id(),
            activity.kind().clone(),
            u64::try_from(activity.inputs().len()).map_err(|_| malformed())?,
            u64::try_from(activity.outputs().len()).map_err(|_| malformed())?,
        )?
        .with_timing(activity.started_at(), activity.finished_at())?;
        header.tool = activity.tool().cloned();
        header.agent = activity.agent().cloned();
        Ok(header)
    }

    /// Sets optional exact times using the native ordering rule.
    ///
    /// # Errors
    /// Rejects a finish before its recorded start.
    pub fn with_timing(
        mut self,
        started: Option<Timestamp>,
        finished: Option<Timestamp>,
    ) -> Result<Self> {
        if started
            .zip(finished)
            .is_some_and(|(started, finished)| finished < started)
        {
            return Err(malformed());
        }
        self.started_at = started;
        self.finished_at = finished;
        Ok(self)
    }

    /// Sets checked tool attribution.
    #[must_use]
    pub fn with_tool(mut self, tool: ToolIdentity) -> Self {
        self.tool = Some(tool);
        self
    }
    /// Sets checked agent attribution.
    #[must_use]
    pub fn with_agent(mut self, agent: AgentIdentity) -> Self {
        self.agent = Some(agent);
        self
    }
    /// Returns original identity.
    #[must_use]
    pub const fn id(&self) -> ActivityId {
        self.id
    }
    /// Returns the exact open-world kind.
    #[must_use]
    pub const fn kind(&self) -> &ActivityKind {
        &self.kind
    }
    /// Returns the optional recorded start.
    #[must_use]
    pub const fn started_at(&self) -> Option<Timestamp> {
        self.started_at
    }
    /// Returns the optional recorded finish.
    #[must_use]
    pub const fn finished_at(&self) -> Option<Timestamp> {
        self.finished_at
    }
    /// Returns optional tool attribution.
    #[must_use]
    pub const fn tool(&self) -> Option<&ToolIdentity> {
        self.tool.as_ref()
    }
    /// Returns optional agent attribution.
    #[must_use]
    pub const fn agent(&self) -> Option<&AgentIdentity> {
        self.agent.as_ref()
    }
    /// Returns the exact input edge count.
    #[must_use]
    pub const fn input_count(&self) -> u64 {
        self.inputs
    }
    /// Returns the exact output edge count.
    #[must_use]
    pub const fn output_count(&self) -> u64 {
        self.outputs
    }
    /// Encodes scalars without collecting edges or captured evidence.
    #[must_use]
    pub fn document(&self) -> Document {
        wire::encode(self)
    }
    /// Decodes attribution through native checked constructors.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds and invalid counts, identities or timing.
    pub fn from_document(document: &Document) -> Result<Self> {
        wire::decode(document)
    }
}
