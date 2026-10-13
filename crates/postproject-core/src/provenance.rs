//! Extensible production-provenance vocabulary and participant identities.

use std::collections::BTreeSet;

use crate::{
    ActivityId, Error, ErrorKind, ExternalIdentifier, FingerprintSnapshot, RepresentationId,
    Result, Timestamp, uri::normalize_uri,
};

/// Maximum encoded length of an activity-kind identifier.
pub const MAX_ACTIVITY_KIND_BYTES: usize = 128;
/// Maximum encoded length of an activity-edge role identifier.
pub const MAX_ACTIVITY_ROLE_BYTES: usize = 128;
/// Maximum UTF-8 byte length of a tool or agent display name.
pub const MAX_PROVENANCE_NAME_BYTES: usize = 256;
/// Maximum UTF-8 byte length of an optional tool version.
pub const MAX_TOOL_VERSION_BYTES: usize = 128;
/// Maximum UTF-8 byte length of an optional tool or vendor URI.
pub const MAX_PROVENANCE_URI_BYTES: usize = 4_096;
/// Maximum number of input or output edges on one activity.
pub const MAX_ACTIVITY_EDGES: usize = 100_000;

/// A namespaced, open-world production activity kind.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActivityKind(String);

impl ActivityKind {
    /// Creates a kind such as `org.postproject:transcode`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when the value is not a bounded
    /// namespaced ASCII identifier.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        validate_namespaced_identifier("activity kind", value.into(), MAX_ACTIVITY_KIND_BYTES)
            .map(Self)
    }

    /// Returns the exact kind identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A namespaced, open-world role for an activity input or output.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActivityRole(String);

impl ActivityRole {
    /// Creates a role such as `org.postproject:input.primary-video`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when the value is not a bounded
    /// namespaced ASCII identifier.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        validate_namespaced_identifier("activity role", value.into(), MAX_ACTIVITY_ROLE_BYTES)
            .map(Self)
    }

    /// Returns the exact role identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The software or service that performed a production activity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolIdentity {
    name: String,
    version: Option<String>,
    uri: Option<String>,
}

impl ToolIdentity {
    /// Creates a bounded tool identity with an optional absolute URI.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for empty or oversized text, NUL
    /// bytes, or an invalid/relative URI.
    pub fn new(
        name: impl Into<String>,
        version: Option<String>,
        uri: Option<String>,
    ) -> Result<Self> {
        let name = name.into();
        validate_text("tool name", &name, MAX_PROVENANCE_NAME_BYTES)?;
        if let Some(version) = version.as_deref() {
            validate_text("tool version", version, MAX_TOOL_VERSION_BYTES)?;
        }
        let uri = uri.map(|value| normalize_uri(value, "tool")).transpose()?;
        if uri
            .as_ref()
            .is_some_and(|value| value.len() > MAX_PROVENANCE_URI_BYTES)
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("tool URI must not exceed {MAX_PROVENANCE_URI_BYTES} UTF-8 bytes"),
            ));
        }
        Ok(Self { name, version, uri })
    }

    /// Returns the user-facing tool name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the optional exact version or build string.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Returns the optional canonical absolute tool/vendor URI.
    #[must_use]
    pub fn uri(&self) -> Option<&str> {
        self.uri.as_deref()
    }
}

/// A person, organization, application instance, or service responsible for an activity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentIdentity {
    name: Option<String>,
    identifier: Option<ExternalIdentifier>,
}

/// One representation consumed by an activity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ActivityInput {
    representation_id: RepresentationId,
    role: Option<ActivityRole>,
    snapshot: Option<ActivityEdgeSnapshot>,
}

impl From<RepresentationId> for ActivityInput {
    fn from(representation_id: RepresentationId) -> Self {
        Self::new(representation_id, None)
    }
}

impl ActivityInput {
    /// Creates an activity input with an optional extensible role.
    #[must_use]
    pub const fn new(representation_id: RepresentationId, role: Option<ActivityRole>) -> Self {
        Self {
            representation_id,
            role,
            snapshot: None,
        }
    }

    /// Returns the consumed representation.
    #[must_use]
    pub const fn representation_id(&self) -> RepresentationId {
        self.representation_id
    }

    /// Returns the optional semantic input role.
    #[must_use]
    pub const fn role(&self) -> Option<&ActivityRole> {
        self.role.as_ref()
    }

    /// Returns the storage-captured input state, or `None` for a migrated edge.
    #[must_use]
    pub const fn snapshot(&self) -> Option<&ActivityEdgeSnapshot> {
        self.snapshot.as_ref()
    }

    /// Attaches a snapshot loaded by a persistence backend.
    ///
    /// Callers do not use this when creating an activity: storage replaces any
    /// supplied value with a snapshot taken inside the creating transaction.
    #[doc(hidden)]
    #[must_use]
    pub fn with_snapshot(mut self, snapshot: ActivityEdgeSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }
}

/// One representation produced by an activity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ActivityOutput {
    representation_id: RepresentationId,
    role: Option<ActivityRole>,
    snapshot: Option<ActivityEdgeSnapshot>,
}

impl ActivityOutput {
    /// Creates an activity output with an optional extensible role.
    #[must_use]
    pub const fn new(representation_id: RepresentationId, role: Option<ActivityRole>) -> Self {
        Self {
            representation_id,
            role,
            snapshot: None,
        }
    }

    /// Returns the produced representation.
    #[must_use]
    pub const fn representation_id(&self) -> RepresentationId {
        self.representation_id
    }

    /// Returns the optional semantic output role.
    #[must_use]
    pub const fn role(&self) -> Option<&ActivityRole> {
        self.role.as_ref()
    }

    /// Returns the storage-captured output state, or `None` for a migrated edge.
    #[must_use]
    pub const fn snapshot(&self) -> Option<&ActivityEdgeSnapshot> {
        self.snapshot.as_ref()
    }

    /// Attaches a snapshot loaded by a persistence backend.
    #[doc(hidden)]
    #[must_use]
    pub fn with_snapshot(mut self, snapshot: ActivityEdgeSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }
}

/// Fingerprint evidence captured for one activity edge at commit time.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ActivityEdgeSnapshot {
    revision_sequence: u64,
    fingerprints: Vec<FingerprintSnapshot>,
}

impl ActivityEdgeSnapshot {
    /// Creates a canonical snapshot captured at `revision_sequence`.
    ///
    /// An empty fingerprint set explicitly records that the representation had
    /// no fingerprint evidence. This differs from an absent migrated snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for sequence zero or duplicate fingerprint domains.
    pub fn new(revision_sequence: u64, mut fingerprints: Vec<FingerprintSnapshot>) -> Result<Self> {
        if revision_sequence == 0 {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "activity-edge snapshot revision must be greater than zero",
            ));
        }
        fingerprints.sort_by(|left, right| {
            (left.algorithm(), left.version()).cmp(&(right.algorithm(), right.version()))
        });
        if fingerprints.windows(2).any(|pair| {
            (pair[0].algorithm(), pair[0].version()) == (pair[1].algorithm(), pair[1].version())
        }) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "activity-edge snapshot contains a duplicate fingerprint domain",
            ));
        }
        Ok(Self {
            revision_sequence,
            fingerprints,
        })
    }

    /// Returns the production revision current when storage captured the edge.
    #[must_use]
    pub const fn revision_sequence(&self) -> u64 {
        self.revision_sequence
    }

    /// Returns captured fingerprint domains in canonical order.
    #[must_use]
    pub fn fingerprints(&self) -> &[FingerprintSnapshot] {
        &self.fingerprints
    }
}

/// A production operation connecting input and output representations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Activity {
    id: ActivityId,
    kind: ActivityKind,
    started_at: Option<Timestamp>,
    finished_at: Option<Timestamp>,
    tool: Option<ToolIdentity>,
    agent: Option<AgentIdentity>,
    inputs: Vec<ActivityInput>,
    outputs: Vec<ActivityOutput>,
}

impl Activity {
    /// Creates a complete production activity and canonicalizes its edges.
    ///
    /// An activity may have no inputs, but must produce at least one output
    /// because this model does not expose an in-progress lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when edges are empty or
    /// excessive, an identical edge is duplicated, or one representation
    /// appears on both sides of the activity.
    pub fn new(
        id: ActivityId,
        kind: ActivityKind,
        mut inputs: Vec<ActivityInput>,
        mut outputs: Vec<ActivityOutput>,
    ) -> Result<Self> {
        validate_edge_count("input", inputs.len(), true)?;
        validate_edge_count("output", outputs.len(), false)?;
        inputs.sort();
        outputs.sort();
        if inputs.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "activity contains a duplicate input edge",
            ));
        }
        if outputs.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "activity contains a duplicate output edge",
            ));
        }
        let input_representations: BTreeSet<_> = inputs
            .iter()
            .map(ActivityInput::representation_id)
            .collect();
        if outputs
            .iter()
            .any(|output| input_representations.contains(&output.representation_id()))
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "an activity cannot consume and produce the same representation",
            ));
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

    /// Adds optional activity timing.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when both times are present and
    /// the finish precedes the start.
    pub fn with_timing(
        mut self,
        started_at: Option<Timestamp>,
        finished_at: Option<Timestamp>,
    ) -> Result<Self> {
        if started_at
            .zip(finished_at)
            .is_some_and(|(started, finished)| finished < started)
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "activity finish time must not precede its start time",
            ));
        }
        self.started_at = started_at;
        self.finished_at = finished_at;
        Ok(self)
    }

    /// Adds the optional tool that performed the activity.
    #[must_use]
    pub fn with_tool(mut self, tool: ToolIdentity) -> Self {
        self.tool = Some(tool);
        self
    }

    /// Adds the optional agent responsible for the activity.
    #[must_use]
    pub fn with_agent(mut self, agent: AgentIdentity) -> Self {
        self.agent = Some(agent);
        self
    }

    /// Returns the activity's stable identity.
    #[must_use]
    pub const fn id(&self) -> ActivityId {
        self.id
    }

    /// Returns the extensible activity kind.
    #[must_use]
    pub const fn kind(&self) -> &ActivityKind {
        &self.kind
    }

    /// Returns the optional start time.
    #[must_use]
    pub const fn started_at(&self) -> Option<Timestamp> {
        self.started_at
    }

    /// Returns the optional finish time.
    #[must_use]
    pub const fn finished_at(&self) -> Option<Timestamp> {
        self.finished_at
    }

    /// Returns the optional responsible tool.
    #[must_use]
    pub const fn tool(&self) -> Option<&ToolIdentity> {
        self.tool.as_ref()
    }

    /// Returns the optional responsible agent.
    #[must_use]
    pub const fn agent(&self) -> Option<&AgentIdentity> {
        self.agent.as_ref()
    }

    /// Returns inputs in canonical representation/role order.
    #[must_use]
    pub fn inputs(&self) -> &[ActivityInput] {
        &self.inputs
    }

    /// Returns outputs in canonical representation/role order.
    #[must_use]
    pub fn outputs(&self) -> &[ActivityOutput] {
        &self.outputs
    }
}

impl AgentIdentity {
    /// Creates an agent described by a name, an external identifier, or both.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when neither identity is present
    /// or the name is empty, oversized, or contains NUL.
    pub fn new(name: Option<String>, identifier: Option<ExternalIdentifier>) -> Result<Self> {
        if let Some(name) = name.as_deref() {
            validate_text("agent name", name, MAX_PROVENANCE_NAME_BYTES)?;
        }
        if name.is_none() && identifier.is_none() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "agent identity requires a name or external identifier",
            ));
        }
        Ok(Self { name, identifier })
    }

    /// Returns the optional user-facing agent name.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the optional external agent identity.
    #[must_use]
    pub const fn identifier(&self) -> Option<&ExternalIdentifier> {
        self.identifier.as_ref()
    }
}

fn validate_namespaced_identifier(label: &str, value: String, maximum: usize) -> Result<String> {
    let valid_bytes = value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'));
    let valid_namespace = value
        .split_once(':')
        .is_some_and(|(namespace, local)| !namespace.is_empty() && !local.is_empty());
    if value.len() > maximum || !valid_bytes || !valid_namespace {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("{label} must be a namespaced identifier of at most {maximum} ASCII bytes"),
        ));
    }
    Ok(value)
}

fn validate_text(label: &str, value: &str, maximum: usize) -> Result<()> {
    if value.is_empty() || value.len() > maximum || value.contains('\0') {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("{label} must contain 1-{maximum} UTF-8 bytes without NUL"),
        ));
    }
    Ok(())
}

fn validate_edge_count(label: &str, count: usize, may_be_empty: bool) -> Result<()> {
    if count > MAX_ACTIVITY_EDGES || (!may_be_empty && count == 0) {
        let minimum = usize::from(!may_be_empty);
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("activity must have {minimum}-{MAX_ACTIVITY_EDGES} {label} edges"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IdentifierScheme;

    #[test]
    fn activity_vocabulary_is_extensible_and_namespaced() {
        let kind = ActivityKind::new("vendor.example:vfx-render").expect("valid kind");
        let role = ActivityRole::new("vendor.example:input.plate").expect("valid role");

        assert_eq!(kind.as_str(), "vendor.example:vfx-render");
        assert_eq!(role.as_str(), "vendor.example:input.plate");
        assert!(ActivityKind::new("transcode").is_err());
        assert!(ActivityRole::new("vendor.example:bad role").is_err());
    }

    #[test]
    fn tool_and_agent_identity_preserve_supplied_detail() {
        let tool = ToolIdentity::new(
            "FFmpeg",
            Some("8.0-custom".to_owned()),
            Some("https://ffmpeg.org".to_owned()),
        )
        .expect("valid tool");
        let identifier = ExternalIdentifier::new(
            IdentifierScheme::new("com.example.worker").expect("valid scheme"),
            "worker-42",
            None,
        )
        .expect("valid identifier");
        let agent = AgentIdentity::new(Some("Render worker".to_owned()), Some(identifier.clone()))
            .expect("valid agent");

        assert_eq!(tool.name(), "FFmpeg");
        assert_eq!(tool.version(), Some("8.0-custom"));
        assert_eq!(tool.uri(), Some("https://ffmpeg.org/"));
        assert_eq!(agent.name(), Some("Render worker"));
        assert_eq!(agent.identifier(), Some(&identifier));
    }

    #[test]
    fn participant_identity_is_bounded() {
        assert!(ToolIdentity::new("", None, None).is_err());
        assert!(ToolIdentity::new("tool", Some(String::new()), None).is_err());
        assert!(ToolIdentity::new("tool", None, Some("relative".to_owned())).is_err());
        let oversized_uri = format!(
            "https://example.com/{}",
            "x".repeat(MAX_PROVENANCE_URI_BYTES)
        );
        assert!(ToolIdentity::new("tool", None, Some(oversized_uri)).is_err());
        assert!(AgentIdentity::new(None, None).is_err());
        assert!(AgentIdentity::new(Some("bad\0name".to_owned()), None).is_err());
    }

    #[test]
    fn activity_supports_canonical_fan_in_and_fan_out() {
        let first_input = RepresentationId::from_bytes([2; 16]);
        let second_input = RepresentationId::from_bytes([1; 16]);
        let first_output = RepresentationId::from_bytes([4; 16]);
        let second_output = RepresentationId::from_bytes([3; 16]);
        let activity = Activity::new(
            ActivityId::new(),
            ActivityKind::new("org.postproject:transcode").expect("valid kind"),
            vec![
                ActivityInput::new(first_input, None),
                ActivityInput::new(second_input, None),
            ],
            vec![
                ActivityOutput::new(first_output, None),
                ActivityOutput::new(second_output, None),
            ],
        )
        .expect("valid activity")
        .with_timing(
            Some(Timestamp::from_unix_micros(10)),
            Some(Timestamp::from_unix_micros(20)),
        )
        .expect("valid timing");

        assert_eq!(activity.inputs()[0].representation_id(), second_input);
        assert_eq!(activity.inputs()[1].representation_id(), first_input);
        assert_eq!(activity.outputs()[0].representation_id(), second_output);
        assert_eq!(activity.outputs()[1].representation_id(), first_output);
    }

    #[test]
    fn activity_rejects_incomplete_or_ambiguous_graph_facts() {
        let representation = RepresentationId::new();
        let kind = || ActivityKind::new("org.postproject:vfx-render").expect("valid kind");
        let input = || ActivityInput::new(representation, None);
        let output = || ActivityOutput::new(representation, None);

        let no_outputs = Activity::new(ActivityId::new(), kind(), vec![input()], Vec::new());
        assert!(no_outputs.is_err());

        let duplicate = Activity::new(
            ActivityId::new(),
            kind(),
            Vec::new(),
            vec![output(), output()],
        );
        assert!(duplicate.is_err());

        let self_edge = Activity::new(ActivityId::new(), kind(), vec![input()], vec![output()]);
        assert!(self_edge.is_err());

        let reversed_time = Activity::new(
            ActivityId::new(),
            kind(),
            Vec::new(),
            vec![ActivityOutput::new(RepresentationId::new(), None)],
        )
        .expect("valid activity")
        .with_timing(
            Some(Timestamp::from_unix_micros(2)),
            Some(Timestamp::from_unix_micros(1)),
        );
        assert!(reversed_time.is_err());
    }
}
