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
    MetadataInput,
    MetadataKey,
    ObjectReference,
    RepresentationId,
    RepresentationKind,
    ResourceId,
    SequenceNaming,
    ToolIdentity,
)

if TYPE_CHECKING:
    from ._production import Edit, JobLease


@dataclass(frozen=True, slots=True)
class ImportMediaCommand:
    source: MediaSource
    display_name: str | None = None


@dataclass(frozen=True, slots=True)
class AddRepresentationCommand:
    asset_id: AssetId
    kind: RepresentationKind
    source: MediaSource


@dataclass(frozen=True, slots=True)
class AddMediaRootCommand:
    name: str
    label: str | None = None
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
    property: MetadataKey
    value: MetadataInput


@dataclass(frozen=True, slots=True)
class ReplaceMetadataCommand:
    target: ObjectReference
    property: MetadataKey
    values: list[MetadataInput]


@dataclass(frozen=True, slots=True)
class RemoveMetadataCommand:
    target: ObjectReference
    property: MetadataKey


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


def _stage(transaction: Edit, command: ProposalCommand) -> StagedValue:
    """Reuse typed native validators; earlier staging remains on later failure."""
    if isinstance(command, ImportMediaCommand):
        return PreparedAsset(
            transaction.import_media(command.source, command.display_name)
        )
    if isinstance(command, AddRepresentationCommand):
        return PreparedRepresentation(
            transaction.add_representation(
                command.asset_id, command.kind, command.source
            )
        )
    if isinstance(command, AddMediaRootCommand):
        return PreparedMediaRoot(
            transaction.add_media_root(
                command.name,
                label=command.label,
                priority=command.priority,
            )
        )
    if isinstance(command, SetMediaRootEnabledCommand):
        transaction.set_media_root_enabled(command.root_id, command.enabled)
    elif isinstance(command, RemoveMediaRootCommand):
        transaction.remove_media_root(command.root_id)
    elif isinstance(command, ConfirmLocatorCommand):
        transaction.confirm_locator(
            command.resource_id,
            command.uri,
            media_root=command.media_root,
            sequence_naming=command.sequence_naming,
        )
    elif isinstance(command, RetireLocatorCommand):
        transaction.retire_locator(command.locator_id)
    elif isinstance(command, RecordResourceFingerprintCommand):
        transaction.record_resource_fingerprint(
            command.resource_id, command.fingerprint
        )
    elif isinstance(command, RecordRepresentationFingerprintCommand):
        transaction.record_representation_fingerprint(
            command.representation_id, command.fingerprint
        )
    elif isinstance(command, ObserveResourceContentCommand):
        return transaction.observe_resource_content(
            command.resource_id, command.path, sequence_naming=command.sequence_naming
        )
    elif isinstance(command, RecordDependencySetCommand):
        transaction.record_dependency_set(
            command.representation_id, tuple(command.dependencies)
        )
    elif isinstance(command, AddIdentifierCommand):
        transaction.add_external_identifier(command.target, command.identifier)
    elif isinstance(command, RemoveIdentifierCommand):
        transaction.remove_external_identifier(command.target, command.identifier)
    elif isinstance(command, AppendMetadataCommand):
        transaction.add_metadata(command.target, command.property, command.value)
    elif isinstance(command, ReplaceMetadataCommand):
        transaction.remove_metadata_property(command.target, command.property)
        for value in command.values:
            transaction.add_metadata(command.target, command.property, value)
    elif isinstance(command, RemoveMetadataCommand):
        transaction.remove_metadata_property(command.target, command.property)
    elif isinstance(command, CreateActivityCommand):
        return PreparedActivity(transaction.create_activity(command.spec))
    elif isinstance(command, RequestJobCommand):
        return PreparedJob(transaction.request_job(command.request))
    elif isinstance(command, ClaimJobCommand):
        # Pending staging grants no worker ownership. Only fresh submission
        # delivers a new owning capability; discard this temporary local handle.
        with transaction.claim_job_lease(
            command.job_id, command.tool, command.duration, agent=command.agent
        ):
            pass
    elif isinstance(command, RenewJobCommand):
        transaction.renew_job_lease(command.lease, command.duration)
    elif isinstance(command, ReleaseJobCommand):
        transaction.release_job_lease(command.lease)
    elif isinstance(command, FailJobCommand):
        transaction.fail_job_lease(command.lease, command.diagnostic)
    elif isinstance(command, CompleteJobCommand):
        transaction.complete_job_lease(
            command.lease, command.output_representation_id, command.activity_id
        )
    elif isinstance(command, CancelJobCommand):
        transaction.cancel_job(command.job_id)
    else:
        raise TypeError("unknown typed proposal command")
    return None
