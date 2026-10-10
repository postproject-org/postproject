"""Typed local staging inputs; preparation produces the checked portable intent.

Sources and owning leases are local inputs only. Native staging measures sources
once and copies public job intent. Submission still requires explicit private
context; these values are never treated as wire documents.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from datetime import timedelta
from typing import TYPE_CHECKING, TypeAlias

from ._model import (
    ActivityId,
    ActivitySpec,
    AgentIdentity,
    AssetId,
    ContentObservationOutcome,
    Dependency,
    ExternalIdentifier,
    Fingerprint,
    JobId,
    JobRequest,
    LocatorId,
    MediaRootId,
    MediaSource,
    MetadataProperty,
    MetadataValue,
    ObjectReference,
    RepresentationId,
    RepresentationKind,
    ResourceId,
    SequenceNaming,
    ToolIdentity,
)

if TYPE_CHECKING:
    from ._production import JobLease


@dataclass(frozen=True, slots=True)
class ImportMediaCommand:
    source: MediaSource | str | os.PathLike[str]
    display_name: str | None = None


@dataclass(frozen=True, slots=True)
class AddRepresentationCommand:
    asset_id: AssetId
    kind: RepresentationKind
    source: MediaSource | str | os.PathLike[str]


@dataclass(frozen=True, slots=True)
class AddMediaRootCommand:
    name: str
    display_name: str | None = None
    priority: int = 0


@dataclass(frozen=True, slots=True)
class SetMediaRootEnabledCommand:
    root_id: MediaRootId
    enabled: bool


@dataclass(frozen=True, slots=True)
class RemoveMediaRootCommand:
    root_id: MediaRootId


@dataclass(frozen=True, slots=True)
class ConfirmLocatorCommand:
    resource_id: ResourceId
    uri: str
    media_root: str | None = None
    sequence_naming: SequenceNaming | None = None


@dataclass(frozen=True, slots=True)
class RetireLocatorCommand:
    locator_id: LocatorId


@dataclass(frozen=True, slots=True)
class RecordResourceFingerprintCommand:
    resource_id: ResourceId
    fingerprint: Fingerprint


@dataclass(frozen=True, slots=True)
class RecordRepresentationFingerprintCommand:
    representation_id: RepresentationId
    fingerprint: Fingerprint


@dataclass(frozen=True, slots=True)
class ObserveResourceContentCommand:
    resource_id: ResourceId
    path: str | os.PathLike[str]
    sequence_naming: SequenceNaming | None = None


@dataclass(frozen=True, slots=True)
class RecordDependencySetCommand:
    representation_id: RepresentationId
    dependencies: list[Dependency]


@dataclass(frozen=True, slots=True)
class AddIdentifierCommand:
    target: ObjectReference
    identifier: ExternalIdentifier


@dataclass(frozen=True, slots=True)
class RemoveIdentifierCommand:
    target: ObjectReference
    identifier: ExternalIdentifier


@dataclass(frozen=True, slots=True)
class AppendMetadataCommand:
    target: ObjectReference
    property: MetadataProperty
    value: MetadataValue


@dataclass(frozen=True, slots=True)
class ReplaceMetadataCommand:
    target: ObjectReference
    property: MetadataProperty
    values: list[MetadataValue]


@dataclass(frozen=True, slots=True)
class RemoveMetadataCommand:
    target: ObjectReference
    property: MetadataProperty


@dataclass(frozen=True, slots=True)
class CreateActivityCommand:
    spec: ActivitySpec


@dataclass(frozen=True, slots=True)
class RequestJobCommand:
    request: JobRequest


@dataclass(frozen=True, slots=True)
class ClaimJobCommand:
    job_id: JobId
    tool: ToolIdentity
    duration: timedelta
    agent: AgentIdentity | None = None


@dataclass(frozen=True, slots=True)
class RenewJobCommand:
    lease: JobLease
    duration: timedelta


@dataclass(frozen=True, slots=True)
class ReleaseJobCommand:
    lease: JobLease


@dataclass(frozen=True, slots=True)
class FailJobCommand:
    lease: JobLease
    diagnostic: str


@dataclass(frozen=True, slots=True)
class CompleteJobCommand:
    lease: JobLease
    output_representation_id: RepresentationId
    activity_id: ActivityId


@dataclass(frozen=True, slots=True)
class CancelJobCommand:
    job_id: JobId


ProposalCommand: TypeAlias = (
    ImportMediaCommand
    | AddRepresentationCommand
    | AddMediaRootCommand
    | SetMediaRootEnabledCommand
    | RemoveMediaRootCommand
    | ConfirmLocatorCommand
    | RetireLocatorCommand
    | RecordResourceFingerprintCommand
    | RecordRepresentationFingerprintCommand
    | ObserveResourceContentCommand
    | RecordDependencySetCommand
    | AddIdentifierCommand
    | RemoveIdentifierCommand
    | AppendMetadataCommand
    | ReplaceMetadataCommand
    | RemoveMetadataCommand
    | CreateActivityCommand
    | RequestJobCommand
    | ClaimJobCommand
    | RenewJobCommand
    | ReleaseJobCommand
    | FailJobCommand
    | CompleteJobCommand
    | CancelJobCommand
)


@dataclass(frozen=True, slots=True)
class PreparedAsset:
    asset_id: AssetId


@dataclass(frozen=True, slots=True)
class PreparedRepresentation:
    representation_id: RepresentationId


@dataclass(frozen=True, slots=True)
class PreparedMediaRoot:
    root_id: MediaRootId


@dataclass(frozen=True, slots=True)
class PreparedActivity:
    activity_id: ActivityId


@dataclass(frozen=True, slots=True)
class PreparedJob:
    job_id: JobId


StagedValue: TypeAlias = (
    PreparedAsset
    | PreparedRepresentation
    | PreparedMediaRoot
    | PreparedActivity
    | PreparedJob
    | ContentObservationOutcome
    | None
)
