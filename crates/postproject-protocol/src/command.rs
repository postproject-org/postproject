//! Ordered semantic intent; storage remains responsible for current-state guards.

use std::time::Duration;

use postproject_core::{
    Activity, AgentIdentity, Dependency, FileFacts, Job, JobFailure, JobId, Locator, LocatorId,
    MediaRoot, MediaRootId, MetadataProperty, MetadataValue, ObjectRef, OriginalMediaImport,
    PropertyId, RepresentationFingerprint, RepresentationId, RepresentationImport,
    ResourceFingerprint, ResourceId, ToolIdentity, VocabularyId,
};
use serde_json::{Value, json};

use crate::{
    Document, IdentifierAttachment, RecordFeature, Result,
    fields::{
        array, checked, decode_reference, encode_reference, malformed, object, text, unsupported,
    },
    metadata,
};

mod activity;
mod evidence;
mod jobs;
mod media;
mod observations;
mod prepared;

/// A checked domain command offered by the current development codec.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Command {
    /// Enqueues prepared requested work, without an executable claim.
    RequestJob(Job),
    /// Requests new ownership using authority time and a checked duration.
    ClaimJob {
        /// Stable job identity.
        job_id: JobId,
        /// Checked worker-tool attribution.
        tool: ToolIdentity,
        /// Optional checked agent attribution.
        agent: Option<AgentIdentity>,
        /// Positive whole microseconds, no longer than 24 hours.
        duration: Duration,
    },
    /// Requests renewal through separately supplied private ownership.
    RenewJob {
        /// Stable job identity.
        job_id: JobId,
        /// Positive whole microseconds, no longer than 24 hours.
        duration: Duration,
    },
    /// Releases work through separately supplied private ownership.
    ReleaseJob(JobId),
    /// Fails work through separately supplied private ownership.
    FailJob {
        /// Stable job identity.
        job_id: JobId,
        /// Checked bounded failure observation.
        failure: JobFailure,
    },
    /// Publishes prepared output/provenance through private ownership.
    CompleteJob {
        /// Stable job identity.
        job_id: JobId,
        /// Complete checked output measured before submission.
        output: Box<RepresentationImport>,
        /// Attribution and edges without storage-captured snapshots.
        activity: Box<Activity>,
    },
    /// Requests administrative cancellation without claiming ownership.
    CancelJob(JobId),
    /// Imports a complete checked aggregate measured before submission.
    ImportOriginal(OriginalMediaImport),
    /// Adds a complete prepared representation to an existing asset.
    AddRepresentation(RepresentationImport),
    /// Publishes attribution and edges with snapshots captured by storage.
    CreateActivity(Activity),
    /// Replaces a complete ordered dependency observation, including empty.
    RecordDependencySet {
        /// Representation whose prepared content was inspected.
        representation_id: RepresentationId,
        /// Exact ordered occurrences, with no assigned observation revision.
        dependencies: Vec<Dependency>,
    },
    /// Observes exact resource identity evidence under its native base guard.
    RecordResourceFingerprint {
        /// Stable resource identity.
        resource_id: ResourceId,
        /// Prepared algorithm/version/bytes, without an observation revision.
        fingerprint: ResourceFingerprint,
    },
    /// Observes exact representation evidence under its native base guard.
    RecordRepresentationFingerprint {
        /// Stable representation identity.
        representation_id: RepresentationId,
        /// Prepared algorithm/version/bytes, without an observation revision.
        fingerprint: RepresentationFingerprint,
    },
    /// Adds complete logical root configuration, without a local mapping.
    AddMediaRoot(MediaRoot),
    /// Changes an existing root under its native decision guard.
    SetMediaRootEnabled {
        /// Stable configured-root identity.
        root_id: MediaRootId,
        /// Requested resolver participation.
        enabled: bool,
    },
    /// Removes an existing logical root under its native decision guard.
    RemoveMediaRoot(MediaRootId),
    /// Adds complete prepared locator/naming observations.
    AddLocator(Locator),
    /// Retires an existing access route under its native decision guard.
    RetireLocator(LocatorId),
    /// Adds one exact identifier attachment.
    AddIdentifier(IdentifierAttachment),
    /// Removes one exact attachment under its native decision guard.
    RemoveIdentifier(IdentifierAttachment),
    /// Records measured resource evidence without measuring it on submission.
    RecordResourceFileFacts {
        /// Stable resource identity.
        resource_id: ResourceId,
        /// Exact prepared size/time observations.
        facts: FileFacts,
    },
    /// Appends one value while advancing the property's destructive-edit guard.
    AppendMetadata {
        /// Production-scoped assertion target.
        target: ObjectRef,
        /// Exact vocabulary/property identity.
        property: MetadataProperty,
        /// Validated exact value.
        value: MetadataValue,
    },
    /// Replaces the ordered values, including an explicit empty set.
    ReplaceMetadata {
        /// Production-scoped assertion target.
        target: ObjectRef,
        /// Exact vocabulary/property identity.
        property: MetadataProperty,
        /// Ordered replacement values.
        values: Vec<MetadataValue>,
    },
    /// Removes one property under its current semantic guard.
    RemoveMetadata {
        /// Production-scoped assertion target.
        target: ObjectRef,
        /// Exact vocabulary/property identity.
        property: MetadataProperty,
    },
}

impl Command {
    /// Returns the job whose lifecycle this command observes in its result.
    #[must_use]
    pub const fn affected_job_id(&self) -> Option<JobId> {
        match self {
            Self::RequestJob(job) => Some(job.id()),
            Self::ClaimJob { job_id, .. }
            | Self::RenewJob { job_id, .. }
            | Self::ReleaseJob(job_id)
            | Self::FailJob { job_id, .. }
            | Self::CompleteJob { job_id, .. }
            | Self::CancelJob(job_id) => Some(*job_id),
            _ => None,
        }
    }

    /// Returns the domain codec required to decode this intent.
    #[must_use]
    pub const fn required_feature(&self) -> RecordFeature {
        match self {
            Self::RequestJob(_)
            | Self::ClaimJob { .. }
            | Self::RenewJob { .. }
            | Self::ReleaseJob(_)
            | Self::FailJob { .. }
            | Self::CompleteJob { .. }
            | Self::CancelJob(_) => RecordFeature::Jobs,
            Self::CreateActivity(_) => RecordFeature::Provenance,
            Self::RecordDependencySet { .. } => RecordFeature::Dependencies,
            Self::AppendMetadata { .. }
            | Self::ReplaceMetadata { .. }
            | Self::RemoveMetadata { .. } => RecordFeature::Metadata,
            Self::ImportOriginal(_)
            | Self::AddRepresentation(_)
            | Self::RecordResourceFingerprint { .. }
            | Self::RecordRepresentationFingerprint { .. }
            | Self::AddMediaRoot(_)
            | Self::SetMediaRootEnabled { .. }
            | Self::RemoveMediaRoot(_)
            | Self::AddLocator(_)
            | Self::RetireLocator(_)
            | Self::AddIdentifier(_)
            | Self::RemoveIdentifier(_)
            | Self::RecordResourceFileFacts { .. } => RecordFeature::Media,
        }
    }
    /// Encodes checked intent without authorizing a current-state mutation.
    ///
    /// # Errors
    /// Rejects unsupported future domain kinds.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: encode(self)?,
        })
    }

    /// Decodes checked domain values; storage must still verify state and scope.
    ///
    /// # Errors
    /// Rejects unknown operations/fields, malformed values or exceeded limits.
    pub fn from_document(document: &Document) -> Result<Self> {
        decode(&document.value)
    }
}

pub(crate) fn encode_property(property: &MetadataProperty) -> Value {
    json!({"vocabulary":property.vocabulary().as_str(),"property":property.property().as_str()})
}

pub(crate) fn decode_property(value: &Value) -> Result<MetadataProperty> {
    let fields = object(value, &["vocabulary", "property"])?;
    Ok(MetadataProperty::new(
        checked(VocabularyId::new(text(&fields["vocabulary"])?))?,
        checked(PropertyId::new(text(&fields["property"])?))?,
    ))
}

pub(crate) fn encode(command: &Command) -> Result<Value> {
    Ok(match command {
        Command::RequestJob(_)
        | Command::ClaimJob { .. }
        | Command::RenewJob { .. }
        | Command::ReleaseJob(_)
        | Command::FailJob { .. }
        | Command::CompleteJob { .. }
        | Command::CancelJob(_) => jobs::encode(command)?,
        Command::AppendMetadata {
            target,
            property,
            value,
        } => {
            json!({"kind":"metadata.append","target":encode_reference(*target)?,"property":encode_property(property),"value":metadata::encode(value)?})
        }
        Command::ReplaceMetadata {
            target,
            property,
            values,
        } => {
            json!({"kind":"metadata.replace","target":encode_reference(*target)?,"property":encode_property(property),"values":values.iter().map(metadata::encode).collect::<Result<Vec<_>>>()?})
        }
        Command::RemoveMetadata { target, property } => {
            json!({"kind":"metadata.remove","target":encode_reference(*target)?,"property":encode_property(property)})
        }
        Command::ImportOriginal(_) | Command::AddRepresentation(_) => prepared::encode(command)?,
        Command::CreateActivity(value) => activity::encode(value)?,
        Command::RecordDependencySet { .. }
        | Command::RecordResourceFingerprint { .. }
        | Command::RecordRepresentationFingerprint { .. } => observations::encode(command)?,
        other => media::encode(other)?,
    })
}

pub(crate) fn decode(value: &Value) -> Result<Command> {
    let kind = text(value.get("kind").ok_or_else(malformed)?)?;
    let keys = match kind {
        "job.request" | "job.claim" | "job.renew" | "job.release" | "job.fail" | "job.complete"
        | "job.cancel" => return jobs::decode(kind, value),
        "metadata.append" => &["kind", "target", "property", "value"][..],
        "metadata.replace" => &["kind", "target", "property", "values"][..],
        "metadata.remove" => &["kind", "target", "property"][..],
        "media.import-original" | "representation.add" => return prepared::decode(kind, value),
        "activity.create" => return activity::decode(value).map(Command::CreateActivity),
        "dependency.observe-set"
        | "resource.observe-fingerprint"
        | "representation.observe-fingerprint" => return observations::decode(kind, value),
        _ => return media::decode(kind, value),
    };
    let fields = object(value, keys)?;
    let target = decode_reference(&fields["target"])?;
    let property = decode_property(&fields["property"])?;
    Ok(match kind {
        "metadata.append" => Command::AppendMetadata {
            target,
            property,
            value: metadata::decode(&fields["value"], 1)?,
        },
        "metadata.replace" => Command::ReplaceMetadata {
            target,
            property,
            values: array(&fields["values"], 1_000_000)?
                .iter()
                .map(|v| metadata::decode(v, 1))
                .collect::<Result<_>>()?,
        },
        "metadata.remove" => Command::RemoveMetadata { target, property },
        _ => return Err(unsupported()),
    })
}
