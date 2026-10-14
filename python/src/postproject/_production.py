"""Owned production and transaction wrappers over the public C ABI."""

from __future__ import annotations

import ctypes
import os
import threading
import weakref
from _ctypes import _Pointer
from collections.abc import Callable, Iterable, Iterator, Mapping
from contextlib import ExitStack
from datetime import timedelta
from fractions import Fraction
from itertools import islice
from pathlib import Path
from types import TracebackType
from typing import Self, TypeVar, cast
from uuid import UUID

from . import _abi
from ._abi import (
    ActivityEdge as NativeActivityEdge,
)
from ._abi import (
    ActivitySet,
    AssetSet,
    DependencyQuerySet,
    Error,
    ExternalIdentifierSet,
    JobSet,
    KnownMediaSet,
    LocatorQuerySet,
    MediaRootSet,
    MetadataSet,
    ObjectQuerySet,
    ObjectRefSet,
    RegenerationPlanSet,
    RepresentationSet,
    ResolutionSet,
    RevisionEventSet,
    RevisionSet,
    Uuid,
)
from ._abi import (
    ArtifactEvaluation as NativeArtifactEvaluation,
)
from ._abi import (
    ArtifactReproducibility as NativeArtifactReproducibility,
)
from ._abi import CancelToken as NativeCancelToken
from ._abi import Dependency as NativeDependency
from ._abi import DependencyMatch as NativeDependencyMatch
from ._abi import (
    DependencySet as NativeDependencySet,
)
from ._abi import (
    FileResourceInput as NativeFileResourceInput,
)
from ._abi import Fingerprint as NativeFingerprint
from ._abi import Job as NativeJob
from ._abi import MediaSource as NativeMediaSource
from ._abi import (
    MetadataInput as NativeMetadataInput,
)
from ._abi import (
    MetadataValue as NativeMetadataValue,
)
from ._abi import (
    Production as NativeProduction,
)
from ._abi import ResolutionOptions as NativeResolutionOptions
from ._abi import (
    RevisionEvent as NativeRevisionEvent,
)
from ._abi import (
    RevisionWaiter as NativeRevisionWaiter,
)
from ._abi import (
    Transaction as NativeTransaction,
)
from ._artifact import read_evaluation, read_reproducibility
from ._errors import (
    ConflictError,
    ExchangeError,
    InternalError,
    InvalidArgumentError,
    PostProjectError,
    UnsupportedError,
)
from ._exchange import (
    ClaimedJobResult,
    ExchangeLimits,
    ExchangePosition,
    ExchangeRole,
    ExchangeState,
    FailedJobResult,
    JobResult,
    JobResultStatus,
    ProtocolBase,
    RequestIdentity,
    SubmissionOutcome,
    SubmissionResult,
)
from ._ids import ClientId, HistoryId, RequestId
from ._model import (
    ActiveJobLease,
    Activity,
    ActivityCreatedEvent,
    ActivityEdge,
    ActivityEdgeInput,
    ActivityEdgeSnapshot,
    ActivityId,
    ActivityInputAddedEvent,
    ActivityOutputAddedEvent,
    ActivityRef,
    ActivitySpec,
    AgentIdentity,
    AmbiguousResourceIssue,
    ArtifactEvaluation,
    ArtifactReproducibility,
    Asset,
    AssetId,
    AssetImportedEvent,
    AssetRef,
    AvailabilityIssue,
    AvailabilityIssueDetail,
    AvailabilityIssueKind,
    ClosedJobLease,
    CommitReceipt,
    CommittedRevision,
    ContentObservationOutcome,
    ContentStructureKind,
    ContentVerification,
    DecisionBase,
    Dependency,
    DependencyMatch,
    DependencySet,
    DependencySetRecordedEvent,
    DependencySetStatus,
    EvidenceKind,
    ExternalIdentifier,
    ExternalIdentifierAddedEvent,
    ExternalIdentifierRemovedEvent,
    FilteredRevisionPage,
    Fingerprint,
    FingerprintSnapshot,
    HostObjectBinding,
    ImageSequenceContent,
    ImageSequenceDescriptor,
    ImageSequenceSource,
    ImportedMedia,
    Job,
    JobCancelled,
    JobCancelledEvent,
    JobClaim,
    JobClaimedEvent,
    JobClaimReleasedEvent,
    JobClaimRenewedEvent,
    JobCompletion,
    JobFailedEvent,
    JobFailure,
    JobId,
    JobLeaseStatus,
    JobRef,
    JobRequest,
    JobRequested,
    JobRequestedEvent,
    JobState,
    JobStatus,
    JobSucceededEvent,
    KnownMediaMatch,
    Locator,
    LocatorAddedEvent,
    LocatorAvailability,
    LocatorId,
    LocatorIdentity,
    LocatorMatch,
    LocatorRetiredEvent,
    MediaRoot,
    MediaRootAddedEvent,
    MediaRootEnabledChangedEvent,
    MediaRootId,
    MediaRootRemovedEvent,
    MediaSource,
    MetadataAddedOrReplacedEvent,
    MetadataAssertion,
    MetadataBool,
    MetadataBytes,
    MetadataDecimal,
    MetadataI64,
    MetadataInput,
    MetadataKey,
    MetadataLanguageString,
    MetadataList,
    MetadataProperty,
    MetadataRational,
    MetadataReference,
    MetadataRemovedEvent,
    MetadataString,
    MetadataStruct,
    MetadataStructField,
    MetadataTimestamp,
    MetadataU64,
    MetadataUri,
    MetadataValue,
    MissingSequenceFrames,
    ObjectReference,
    OfflineResourceIssue,
    OrderedPartsContent,
    OrderedPartsSource,
    OriginIdentity,
    OutputSpec,
    PackageContent,
    PackageSource,
    PendingJobLease,
    ProductionId,
    ProductionRef,
    ProvenanceMatch,
    QueryPage,
    RegenerationJobPlan,
    RegisteredOutputs,
    RelinkLocation,
    RelinkSelection,
    Representation,
    RepresentationAddedEvent,
    RepresentationAvailability,
    RepresentationContent,
    RepresentationFingerprintObservedEvent,
    RepresentationId,
    RepresentationKind,
    RepresentationMember,
    RepresentationRef,
    RepresentationResolution,
    RepresentationResourceAddedEvent,
    ResolutionCandidate,
    ResolutionEvidence,
    ResolutionOutcome,
    Resource,
    ResourceAddedEvent,
    ResourceAmbiguous,
    ResourceErrorIssue,
    ResourceFileFactsObservedEvent,
    ResourceFingerprintObservedEvent,
    ResourceId,
    ResourceOffline,
    ResourceOnlineAtKnownLocator,
    ResourceRef,
    ResourceResolution,
    ResourceResolutionFailure,
    ResourceResolutionState,
    ResourceResolvedExact,
    ResourceResolvedProbable,
    Revision,
    RevisionContext,
    RevisionEvent,
    RevisionEventPayload,
    RevisionId,
    RevisionWait,
    RevisionWaitResult,
    SequenceNaming,
    SingleResourceContent,
    ToolIdentity,
    TransactionId,
    VerificationMode,
)
from ._native import NativeLibrary
from ._proposal_commands import ProposalCommand, StagedValue, _stage

_ObjectQueryItem = TypeVar("_ObjectQueryItem")


class _ActivitiesByRepresentation:
    def __init__(self, production: Production, direction: str) -> None:
        self._production = production
        self._direction = direction

    def __getitem__(self, representation_id: RepresentationId) -> tuple[Activity, ...]:
        return self._production._activities_for(self._direction, representation_id)


class _ProvenanceRepresentations:
    def __init__(self, production: Production, direction: str) -> None:
        self._production = production
        self._direction = direction

    def __getitem__(
        self, representation_id: RepresentationId
    ) -> tuple[RepresentationId, ...]:
        return self._production._provenance_representations(
            self._direction, representation_id
        )


class _ExternalIdentifiers:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(self, target: ObjectReference) -> tuple[ExternalIdentifier, ...]:
        return self._production._external_identifiers(target)


class _ObjectsByExternalIdentifier:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(
        self, key: tuple[str, str] | tuple[str, str, str]
    ) -> tuple[ObjectReference, ...]:
        if len(key) == 3:
            scheme, value, qualifier = key
            return self._production._find_by_external_identifier(
                scheme, value, qualifier
            )
        scheme, value = key
        return self._production._find_by_external_identifier(scheme, value, None)


class _Metadata:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(self, target: ObjectReference) -> tuple[MetadataAssertion, ...]:
        return self._production._metadata(target)


class _MetadataByProperty:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(self, property: MetadataKey) -> tuple[MetadataAssertion, ...]:
        return self._production._metadata_by_property(property)


class _RevisionEvents:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(self, revision_id: RevisionId) -> tuple[RevisionEvent, ...]:
        return self._production._revision_events(revision_id)


class _Resolutions:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(self, asset_id: AssetId) -> tuple[RepresentationResolution, ...]:
        return self._production.resolve(asset_id)


class _Representations:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(self, asset_id: AssetId) -> tuple[Representation, ...]:
        return self._production._representations(asset_id)


class _HostBindings:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(self, target: ObjectReference) -> str:
        production_id = _native_production_id(self._production.id)
        native_target = _native_object_reference(target)
        binding = ctypes.c_char_p()
        error = ctypes.POINTER(Error)()
        status = self._production._native.lib.pp_host_binding_format(
            production_id,
            ctypes.byref(native_target),
            ctypes.byref(binding),
            ctypes.byref(error),
        )
        self._production._native.check(status, error)
        try:
            return _decode_required(binding.value, "host binding")
        finally:
            self._production._native.lib.pp_string_release(binding)

    def parse(self, value: str) -> HostObjectBinding:
        production_id = _abi.ProductionId()
        target = _abi.ObjectRef()
        error = ctypes.POINTER(Error)()
        status = self._production._native.lib.pp_host_binding_parse(
            _utf8(value, "host binding"),
            ctypes.byref(production_id),
            ctypes.byref(target),
            ctypes.byref(error),
        )
        self._production._native.check(status, error)
        return HostObjectBinding(
            ProductionId(_uuid(production_id)), _object_reference(target)
        )


def _native_edit_context(
    origin: OriginIdentity | str | None, message: str | None
) -> _abi.EditContext:
    if origin is not None and not isinstance(origin, (str, OriginIdentity)):
        raise TypeError("origin must be a string or OriginIdentity")
    identity = OriginIdentity(origin) if isinstance(origin, str) else origin
    return _abi.EditContext(
        _optional_text(identity.name if identity else None),
        _optional_text(identity.version if identity else None),
        _optional_text(identity.uri if identity else None),
        _optional_text(message),
    )


class Production:
    """An owned native production handle supporting concurrent operations.

    Calls may run from multiple threads, but ``close()`` must not overlap them.
    """

    def __init__(
        self,
        native: NativeLibrary,
        handle: _Pointer[NativeProduction],
    ) -> None:
        self._native = native
        self._handle = handle
        self._finalizer = weakref.finalize(
            self, native.lib.pp_production_release, handle
        )

    def proposal_builder(
        self,
        client_id: ClientId,
        request_id: RequestId,
        *,
        base: DecisionBase | ProtocolBase | None = None,
        context: RevisionContext | None = None,
    ) -> ProposalBuilder:
        """Stage typed command collections; closing never writes a revision."""
        if isinstance(base, ProtocolBase):
            full_base = base
            transaction = self.edit(
                base.decision,
                origin=context.origin if context else None,
                message=context.message if context else None,
            )
        else:
            full_base = None
            transaction = self.edit(
                base,
                origin=context.origin if context else None,
                message=context.message if context else None,
            )
        try:
            return ProposalBuilder(transaction, client_id, request_id, full_base)
        except BaseException:
            transaction.close()
            raise

    def submit_proposal(
        self,
        proposal: Proposal,
        *,
        leases: Iterable[JobLease] = (),
        private_tokens: Iterable[str] = (),
    ) -> SubmissionResult:
        """Submit once; recover public facts separately from fresh worker delivery.

        Private context is borrowed explicitly, without exporting a token or
        checking current ownership before the retained native request lookup.
        """
        self._require_open()
        native_proposal = proposal._for_native(self._native)
        borrowed = list(islice(leases, 1001))
        tokens = list(islice(private_tokens, max(0, 1001 - len(borrowed))))
        if len(borrowed) + len(tokens) > 1000:
            raise ExchangeError(
                _abi.PP_ERROR_LIMIT_EXCEEDED, "too many private submission entries"
            )
        handles = (ctypes.POINTER(_abi.JobLease) * len(borrowed))(
            *(_job_lease_handle(lease, self._native) for lease in borrowed)
        )
        buffers = []
        for token in tokens:
            if not isinstance(token, str) or len(token) != 115 or not token.isascii():
                raise InvalidArgumentError(
                    _abi.PP_ERROR_INVALID_ARGUMENT, "invalid private lease token"
                )
            buffers.append(
                (ctypes.c_uint8 * 115).from_buffer_copy(token.encode("ascii"))
            )
        native_tokens = (_abi.PrivateLeaseToken * len(buffers))(
            *(_abi.PrivateLeaseToken(buffer, 115) for buffer in buffers)
        )
        handle = ctypes.POINTER(_abi.Submission)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_submit_proposal(
            self._handle,
            native_proposal,
            handles,
            len(handles),
            native_tokens,
            len(native_tokens),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        delivered: list[JobLease] = []
        try:
            outcome = ctypes.POINTER(_abi.Outcome)()
            status = self._native.lib.pp_submission_outcome(
                handle, ctypes.byref(outcome), ctypes.byref(error)
            )
            self._native.check(status, error)
            facts = _submission_outcome(self._native, outcome)
            count = int(self._native.lib.pp_submission_lease_count(handle))
            if count > 1000:
                raise InternalError(
                    _abi.PP_ERROR_INTERNAL, "invalid delivered lease count"
                )
            for index in range(count):
                lease = ctypes.POINTER(_abi.JobLease)()
                status = self._native.lib.pp_submission_take_lease(
                    handle, index, ctypes.byref(lease), ctypes.byref(error)
                )
                self._native.check(status, error)
                delivered.append(JobLease(self._native, lease))
            return SubmissionResult(facts, delivered)
        except BaseException:
            for lease in delivered:
                lease.close()
            raise
        finally:
            self._native.lib.pp_submission_release(handle)

    def submission_outcome(self, identity: RequestIdentity) -> SubmissionOutcome:
        """Copy the original public result; absent request raises NotFoundError."""
        self._require_open()
        value = _abi.RequestIdentity()
        value.production_id = _native_production_id(identity.production_id)
        value.history_id.bytes = _native_uuid(identity.history_id).bytes
        value.client_id.bytes = _native_uuid(identity.client_id).bytes
        value.request_id.bytes = _native_uuid(identity.request_id).bytes
        outcome = ctypes.POINTER(_abi.Outcome)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_submission_outcome(
            self._handle,
            ctypes.byref(value),
            ctypes.byref(outcome),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return _submission_outcome(self._native, outcome)

    @property
    def exchange_state(self) -> ExchangeState:
        """Copy coherent role, source scope, limits, floor and available head."""
        self._require_open()
        state = _abi.ExchangeState()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_exchange_state(
            self._handle, ctypes.byref(state), ctypes.byref(error)
        )
        self._native.check(status, error)
        try:
            role = ExchangeRole(state.role)
        except ValueError as error:
            raise UnsupportedError(
                _abi.PP_ERROR_UNSUPPORTED, "unknown exchange role"
            ) from error
        if state.has_head not in (0, 1):
            raise UnsupportedError(
                _abi.PP_ERROR_UNSUPPORTED, "unknown exchange head presence"
            )
        limits = state.limits
        return ExchangeState(
            state.protocol_version,
            role,
            state.supported_features,
            _protocol_base(state.observed_base),
            _exchange_position(state.floor),
            _exchange_position(state.head) if state.has_head else None,
            ExchangeLimits(
                limits.file_bytes,
                limits.disk_bytes,
                limits.frames,
                limits.document_bytes,
                limits.document_depth,
                limits.document_nodes,
            ),
        )

    def export_checkpoint(self, path: str | os.PathLike[str]) -> ExchangePosition:
        """Exclusively publish a sealed checkpoint from one pinned view."""
        self._require_open()
        position = _abi.ExchangePosition()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_export_checkpoint(
            self._handle, _path_bytes(path), ctypes.byref(position), ctypes.byref(error)
        )
        self._native.check(status, error)
        return _exchange_position(position)

    @classmethod
    def validate_checkpoint(
        cls,
        path: str | os.PathLike[str],
        limits: ExchangeLimits | None = None,
        *,
        library_path: str | os.PathLike[str] | None = None,
    ) -> ExchangePosition:
        """Validate all domain facts in private temporary storage."""
        native = NativeLibrary(library_path)
        native_limits = _native_exchange_limits(limits)
        position = _abi.ExchangePosition()
        error = ctypes.POINTER(Error)()
        status = native.lib.pp_checkpoint_validate_file(
            _path_bytes(path),
            None if native_limits is None else ctypes.byref(native_limits),
            ctypes.byref(position),
            ctypes.byref(error),
        )
        native.check(status, error)
        return _exchange_position(position)

    @classmethod
    def import_checkpoint(
        cls,
        path: str | os.PathLike[str],
        destination: str | os.PathLike[str],
        limits: ExchangeLimits | None = None,
        *,
        library_path: str | os.PathLike[str] | None = None,
    ) -> Self:
        """Create a passive mirror without overwriting an existing destination."""
        native = NativeLibrary(library_path)
        native_limits = _native_exchange_limits(limits)
        handle = ctypes.POINTER(NativeProduction)()
        error = ctypes.POINTER(Error)()
        status = native.lib.pp_checkpoint_import_file(
            _path_bytes(path),
            _path_bytes(destination),
            None if native_limits is None else ctypes.byref(native_limits),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        native.check(status, error)
        return cls(native, handle)

    def export_changes(
        self, position: ExchangePosition, path: str | os.PathLike[str]
    ) -> ExchangePosition:
        """Publish exact complete records through one pinned source head."""
        self._require_open()
        native_from = _native_exchange_position(position)
        through = _abi.ExchangePosition()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_export_changes(
            self._handle,
            ctypes.byref(native_from),
            _path_bytes(path),
            ctypes.byref(through),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return _exchange_position(through)

    def apply_changes(
        self, path: str | os.PathLike[str], limits: ExchangeLimits | None = None
    ) -> ExchangePosition:
        """Apply atomic records to a passive mirror; explicitly retry after loss.

        Failure may retain an earlier complete prefix. Inspect after reopening
        and retry the same sealed file; verified duplicates create no revision.
        """
        self._require_open()
        native_limits = _native_exchange_limits(limits)
        through = _abi.ExchangePosition()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_apply_changes(
            self._handle,
            _path_bytes(path),
            None if native_limits is None else ctypes.byref(native_limits),
            ctypes.byref(through),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return _exchange_position(through)

    def resynchronize_history(
        self, base: ProtocolBase, *, records: int, encoded_bytes: int
    ) -> ExchangePosition:
        """Explicitly recover incomplete authority history, preserving old evidence."""
        self._require_open()
        native_base = _native_protocol_base(base)
        floor = _abi.ExchangePosition()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_resynchronize_history(
            self._handle,
            ctypes.byref(native_base),
            _unsigned(records, 64, "records"),
            _unsigned(encoded_bytes, 64, "encoded_bytes"),
            ctypes.byref(floor),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return _exchange_position(floor)

    @classmethod
    def create(
        cls,
        path: str | os.PathLike[str],
        display_name: str | None = None,
        *,
        library_path: str | os.PathLike[str] | None = None,
    ) -> Self:
        """Create a new production without overwriting an existing path."""

        native = NativeLibrary(library_path)
        handle = ctypes.POINTER(NativeProduction)()
        error = ctypes.POINTER(Error)()
        status = native.lib.pp_production_create(
            _path_bytes(path),
            _optional_text(display_name),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native production creation returned no handle"
            )
        return cls(native, handle)

    @classmethod
    def open(
        cls,
        path: str | os.PathLike[str],
        *,
        library_path: str | os.PathLike[str] | None = None,
    ) -> Self:
        """Open an existing production."""

        native = NativeLibrary(library_path)
        handle = ctypes.POINTER(NativeProduction)()
        error = ctypes.POINTER(Error)()
        status = native.lib.pp_production_open(
            _path_bytes(path), ctypes.byref(handle), ctypes.byref(error)
        )
        native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native production open returned no handle"
            )
        return cls(native, handle)

    @property
    def id(self) -> ProductionId:
        """Return this production's stable identity."""

        self._require_open()
        value = _abi.ProductionId()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_id(
            self._handle, ctypes.byref(value), ctypes.byref(error)
        )
        self._native.check(status, error)
        return ProductionId(_uuid(value))

    def media_roots_page(
        self, *, limit: int, cursor: str | None = None
    ) -> QueryPage[MediaRoot]:
        """Read a bounded root page in priority and identity order."""

        self._require_open()
        handle = ctypes.POINTER(MediaRootSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_media_roots_page(
            self._handle,
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return _media_root_page(self._native, handle)

    @property
    def media_roots(self) -> tuple[MediaRoot, ...]:
        """Return resolver roots in priority order, with a 1000-root cap."""

        self._require_open()
        handle = ctypes.POINTER(MediaRootSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_media_roots(
            self._handle, ctypes.byref(handle), ctypes.byref(error)
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native media-root query returned no result set"
            )
        try:
            count = self._native.lib.pp_media_root_set_count(handle)
            return tuple(
                _media_root_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_media_root_set_release(handle)

    @property
    def activities(self) -> tuple[Activity, ...]:
        """Return every provenance activity in deterministic order."""

        self._require_open()
        return self._activity_set(
            self._native.lib.pp_production_activities, self._handle
        )

    def job(self, job_id: JobId) -> Job:
        """Return one durable job, raising ``NotFoundError`` when absent."""

        self._require_open()
        native_id = _native_job_id(job_id)
        page = _read_jobs(
            self._native,
            self._native.lib.pp_production_job,
            self._handle,
            native_id,
        )
        if len(page.items) != 1:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native job read returned no single job"
            )
        return page.items[0]

    def jobs(
        self,
        *,
        limit: int,
        cursor: str | None = None,
        state: JobState | None = None,
        kind: str | None = None,
    ) -> QueryPage[Job]:
        """Return one bounded job page with optional exact predicates."""

        self._require_open()
        return _read_jobs(
            self._native,
            self._native.lib.pp_production_jobs,
            self._handle,
            0 if state is None else _native_job_state(state),
            _optional_text(kind),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def plan_regeneration(
        self, artifact_representation_ids: Iterable[RepresentationId]
    ) -> tuple[RegenerationJobPlan, ...]:
        """Derive job proposals without enqueuing or executing them."""

        self._require_open()
        return _read_regeneration_plans(
            self._native,
            self._native.lib.pp_production_plan_regeneration,
            self._handle,
            artifact_representation_ids,
        )

    @property
    def activities_producing(self) -> _ActivitiesByRepresentation:
        """Return producing activities keyed by representation identity."""

        self._require_open()
        return _ActivitiesByRepresentation(self, "producing")

    @property
    def activities_consuming(self) -> _ActivitiesByRepresentation:
        """Return consuming activities keyed by representation identity."""

        self._require_open()
        return _ActivitiesByRepresentation(self, "consuming")

    @property
    def provenance_ancestors(self) -> _ProvenanceRepresentations:
        """Return transitive ancestors keyed by representation identity."""

        self._require_open()
        return _ProvenanceRepresentations(self, "ancestors")

    @property
    def provenance_descendants(self) -> _ProvenanceRepresentations:
        """Return transitive descendants keyed by representation identity."""

        self._require_open()
        return _ProvenanceRepresentations(self, "descendants")

    @property
    def external_identifiers(self) -> _ExternalIdentifiers:
        """Return external identifiers keyed by their target object."""

        self._require_open()
        return _ExternalIdentifiers(self)

    @property
    def objects_by_external_identifier(self) -> _ObjectsByExternalIdentifier:
        """Return object matches keyed by ``(scheme, value)``.

        A ``(scheme, value, qualifier)`` key matches only identifiers with
        exactly that qualifier; a two-element key matches any qualifier.
        """

        self._require_open()
        return _ObjectsByExternalIdentifier(self)

    @property
    def metadata(self) -> _Metadata:
        """Return metadata assertions keyed by their target object."""

        self._require_open()
        return _Metadata(self)

    @property
    def metadata_by_property(self) -> _MetadataByProperty:
        """Return metadata assertions keyed by vocabulary-qualified property."""

        self._require_open()
        return _MetadataByProperty(self)

    @property
    def revision_events(self) -> _RevisionEvents:
        """Return semantic event lists keyed by revision identity."""

        self._require_open()
        return _RevisionEvents(self)

    def revision_events_page(
        self, revision_id: RevisionId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[RevisionEvent]:
        """Return one bounded page of immutable events in position order."""

        self._require_open()
        native_id = _native_revision_id(revision_id)
        return _read_revision_event_page(
            self._native,
            self._native.lib.pp_production_revision_events_page,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    @property
    def resolutions(self) -> _Resolutions:
        """Return representation-resolution results keyed by asset identity."""

        self._require_open()
        return _Resolutions(self)

    def verify_resource(
        self,
        resource_id: ResourceId,
        path: str | os.PathLike[str],
        *,
        sequence_naming: SequenceNaming | None = None,
    ) -> ContentVerification:
        """Compare the content at path with the resource's stored fingerprints.

        Only fingerprint domains PostProject computes are compared; a resource
        with only foreign fingerprints is ``NOT_COMPARABLE``. For an image
        sequence, path is its directory and ``sequence_naming`` names its
        files; ``None`` means the naming recorded for that directory.
        """

        self._require_open()
        return _verify_resource(
            self._native,
            self._native.lib.pp_production_verify_resource,
            self._handle,
            resource_id,
            path,
            sequence_naming,
        )

    def resolve(
        self,
        asset_ids: AssetId | Iterable[AssetId],
        root_mappings: Mapping[str, str | os.PathLike[str]] | None = None,
        *,
        search_directories: Iterable[str | os.PathLike[str]] = (),
        verification: VerificationMode = VerificationMode.PRESENCE,
        max_depth: int = 64,
        max_entries_per_directory: int = 100_000,
        cancel_token: CancelToken | None = None,
    ) -> tuple[RepresentationResolution, ...]:
        """Resolve one or more assets, scanning the search scope once.

        ``root_mappings`` map logical roots to this machine's directories and
        ``search_directories`` are unnamed places searched after them; neither
        is recorded. Results cover every representation of each asset, in
        asset order. A cancelled ``cancel_token`` raises ``CancelledError``.
        """

        self._require_open()
        return _resolve(
            self._native,
            self._native.lib.pp_production_resolve_assets,
            self._handle,
            asset_ids,
            root_mappings,
            search_directories,
            verification,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_entries_per_directory, 64, "max_entries_per_directory"),
            cancel_token,
        )

    def evaluate_artifact(
        self,
        representation_id: RepresentationId,
        *,
        max_depth: int = 64,
        max_representations: int = 1_000,
    ) -> ArtifactEvaluation:
        """Evaluate stored artifact evidence without accessing media files."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        handle = ctypes.POINTER(NativeArtifactEvaluation)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_evaluate_artifact(
            self._handle,
            native_id,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native artifact evaluation returned no result"
            )
        try:
            return read_evaluation(self._native, handle)
        finally:
            self._native.lib.pp_artifact_evaluation_release(handle)

    def artifact_reproducibility(
        self, representation_id: RepresentationId
    ) -> ArtifactReproducibility:
        """Report whether stored knowledge is sufficient to reproduce an artifact."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        handle = ctypes.POINTER(NativeArtifactReproducibility)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_artifact_reproducibility(
            self._handle,
            native_id,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native artifact reproducibility returned no report",
            )
        try:
            return read_reproducibility(self._native, handle)
        finally:
            self._native.lib.pp_artifact_reproducibility_release(handle)

    @property
    def representations(self) -> _Representations:
        """Return immutable representation snapshots keyed by asset identity."""

        self._require_open()
        return _Representations(self)

    def dependency_set(
        self, representation_id: RepresentationId
    ) -> DependencySet | None:
        """Return recorded dependency knowledge, preserving absent versus empty."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return _read_dependency_set(
            self._native,
            self._native.lib.pp_production_dependency_set,
            self._handle,
            native_id,
        )

    def dependencies(
        self,
        representation_id: RepresentationId,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[DependencyMatch]:
        """Return one bounded page of direct or transitive dependencies."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return _read_dependency_query(
            self._native,
            self._native.lib.pp_production_dependencies,
            self._handle,
            native_id,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def dependents(
        self,
        target: AssetRef | RepresentationRef,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[DependencyMatch]:
        """Return one bounded page of direct or transitive dependents."""

        self._require_open()
        native_target = _native_object_reference(target)
        return _read_dependency_query(
            self._native,
            self._native.lib.pp_production_dependents,
            self._handle,
            ctypes.byref(native_target),
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def asset(self, asset_id: AssetId) -> Asset:
        """Return one asset, raising ``NotFoundError`` when it is absent."""

        self._require_open()
        native_id = _native_asset_id(asset_id)
        handle = ctypes.POINTER(AssetSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_asset(
            self._handle,
            native_id,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native asset read returned no result set"
            )
        try:
            if self._native.lib.pp_asset_set_count(handle) != 1:
                raise InternalError(
                    _abi.PP_ERROR_INTERNAL, "native asset read returned no single asset"
                )
            return _asset_at(self._native, handle, 0)
        finally:
            self._native.lib.pp_asset_set_release(handle)

    def representation(self, representation_id: RepresentationId) -> Representation:
        """Return one representation, raising ``NotFoundError`` when absent."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        page = self._representation_page(
            self._native.lib.pp_production_representation,
            self._handle,
            native_id,
        )
        if len(page.items) != 1:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native representation read returned no single representation",
            )
        return page.items[0]

    def representations_using_resource(
        self, resource_id: ResourceId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[Representation]:
        """Return one bounded page of representations that use a resource."""

        self._require_open()
        native_id = _native_resource_id(resource_id)
        return self._representation_page(
            self._native.lib.pp_production_representations_using_resource,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def assets_page(self, *, limit: int, cursor: str | None = None) -> QueryPage[Asset]:
        """Return one bounded asset page in creation and identity order."""

        self._require_open()
        handle = ctypes.POINTER(AssetSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_assets_page(
            self._handle,
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native asset query returned no result set"
            )
        try:
            count = self._native.lib.pp_asset_set_count(handle)
            return QueryPage(
                tuple(
                    _asset_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(self._native.lib.pp_asset_set_next_cursor(handle)),
            )
        finally:
            self._native.lib.pp_asset_set_release(handle)

    def representations_page(
        self, asset_id: AssetId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[Representation]:
        """Return one bounded page of representations belonging to an asset."""

        self._require_open()
        native_id = _native_asset_id(asset_id)
        return self._representation_page(
            self._native.lib.pp_production_representations_page,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def representations_under_media_root(
        self, root_name: str, *, limit: int, cursor: str | None = None
    ) -> QueryPage[Representation]:
        """Return one bounded page of representations located under a root."""

        self._require_open()
        return self._representation_page(
            self._native.lib.pp_production_representations_under_media_root,
            self._handle,
            _utf8(root_name, "root name"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def resources_page(
        self,
        representation_id: RepresentationId,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[ResourceId]:
        """Return one bounded page of resources in representation order."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return self._object_query_page(
            self._native.lib.pp_production_resources_page,
            _resource_match,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def locators_page(
        self, resource_id: ResourceId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[LocatorMatch]:
        """Return one bounded page of locators belonging to a resource."""

        self._require_open()
        native_id = _native_resource_id(resource_id)
        handle = ctypes.POINTER(LocatorQuerySet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_locators_page(
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return _locator_page(self._native, handle)

    def find_known_media(
        self,
        source: str | os.PathLike[str] | LocatorIdentity | Fingerprint,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[KnownMediaMatch]:
        """Find ownership candidates by file path, exact locator or fingerprint.

        Paths must exist for canonicalization; lookup never hashes their bytes.
        Sequences require a LocatorIdentity with exact naming. Follow the page
        continuation before treating candidates as complete; lookup never
        adopts or merges identity. Strings are paths, not guessed URI inputs.
        """
        self._require_open()
        if isinstance(source, Fingerprint):
            return self._find_known_media_by_fingerprint(
                source, limit=limit, cursor=cursor
            )
        if isinstance(source, (str, os.PathLike)):
            source = LocatorIdentity(_file_locator(self._native, source))
        if not isinstance(source, LocatorIdentity):
            raise TypeError("source must be a path, LocatorIdentity or Fingerprint")
        return self._find_known_media_by_locator(source, limit=limit, cursor=cursor)

    def _find_known_media_by_locator(
        self,
        locator: LocatorIdentity,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[KnownMediaMatch]:
        """Find every current ownership candidate at an exact locator.

        The query is read-only. For an image sequence, ``locator`` must carry
        its exact directory naming; directory-only lookup does not match it.
        """

        self._require_open()
        handle = ctypes.POINTER(KnownMediaSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_find_known_media_by_locator(
            self._handle,
            _utf8(locator.uri, "locator URI"),
            _native_naming(locator.sequence_naming),
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return self._known_media_page(handle)

    def _find_known_media_by_fingerprint(
        self,
        fingerprint: Fingerprint,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[KnownMediaMatch]:
        """Find resources with this exact current effective fingerprint.

        Every candidate is returned; content equality does not prove logical
        asset identity and the query never adopts or merges media.
        """

        self._require_open()
        value = (ctypes.c_uint8 * len(fingerprint.value)).from_buffer_copy(
            fingerprint.value
        )
        handle = ctypes.POINTER(KnownMediaSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_find_known_media_by_fingerprint(
            self._handle,
            _utf8(fingerprint.algorithm, "fingerprint algorithm"),
            _unsigned(fingerprint.version, 16, "fingerprint version"),
            value,
            len(fingerprint.value),
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return self._known_media_page(handle)

    def unresolved_media(
        self, *, limit: int, cursor: str | None = None
    ) -> QueryPage[RepresentationId]:
        """Return representations whose required resources lack locators."""

        self._require_open()
        return self._object_query_page(
            self._native.lib.pp_production_unresolved_media,
            _representation_match,
            self._handle,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def objects_changed_since(
        self, sequence: int, *, limit: int, cursor: str | None = None
    ) -> QueryPage[ObjectReference]:
        """Return distinct semantic objects touched after revision ``sequence``."""

        self._require_open()
        return self._object_query_page(
            self._native.lib.pp_production_objects_changed_since,
            _object_match,
            self._handle,
            _unsigned(sequence, 64, "sequence"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def query_metadata(
        self,
        property: MetadataKey,
        *,
        limit: int,
        cursor: str | None = None,
        value: MetadataInput | None = None,
    ) -> QueryPage[MetadataAssertion]:
        """Return one bounded assertion page with an optional exact scalar value."""

        self._require_open()
        property = _metadata_key(property)
        vocabulary = _utf8(property.vocabulary, "metadata vocabulary")
        property_name = _utf8(property.property, "metadata property")
        native_cursor = _optional_text(cursor)
        handle = ctypes.POINTER(MetadataSet)()
        error = ctypes.POINTER(Error)()
        native_value = None if value is None else _metadata_input(self._native, value)
        try:
            status = self._native.lib.pp_production_query_metadata(
                self._handle,
                vocabulary,
                property_name,
                native_value,
                _page_limit(limit),
                native_cursor,
                ctypes.byref(handle),
                ctypes.byref(error),
            )
        finally:
            if native_value is not None:
                self._native.lib.pp_metadata_input_release(native_value)
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native metadata query returned no result set"
            )
        try:
            count = self._native.lib.pp_metadata_set_count(handle)
            return QueryPage(
                tuple(
                    _metadata_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(self._native.lib.pp_metadata_set_next_cursor(handle)),
            )
        finally:
            self._native.lib.pp_metadata_set_release(handle)

    def activities_producing_page(
        self,
        representation_id: RepresentationId,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[Activity]:
        """Return one bounded page of activities producing a representation."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return self._activity_page(
            self._native.lib.pp_production_activities_producing_page,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def activities_consuming_page(
        self,
        representation_id: RepresentationId,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[Activity]:
        """Return one bounded page of activities consuming a representation."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return self._activity_page(
            self._native.lib.pp_production_activities_consuming_page,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def outputs_by_activity_kind(
        self, kind: str, *, limit: int, cursor: str | None = None
    ) -> QueryPage[RepresentationId]:
        """Return outputs produced by activities of one exact kind."""

        self._require_open()
        return self._object_query_page(
            self._native.lib.pp_production_outputs_by_activity_kind,
            _representation_match,
            self._handle,
            _utf8(kind, "activity kind"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def outputs_by_tool(
        self, tool: ToolIdentity, *, limit: int, cursor: str | None = None
    ) -> QueryPage[RepresentationId]:
        """Return outputs produced by activities with one exact tool identity."""

        self._require_open()
        return self._object_query_page(
            self._native.lib.pp_production_outputs_by_tool,
            _representation_match,
            self._handle,
            _utf8(tool.name, "tool name"),
            _optional_text(tool.version),
            _optional_text(tool.uri),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def provenance_ancestors_page(
        self,
        representation_id: RepresentationId,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[ProvenanceMatch]:
        """Return one bounded page of shortest-depth provenance ancestors."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return self._object_query_page(
            self._native.lib.pp_production_provenance_ancestors_page,
            _provenance_match,
            self._handle,
            native_id,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def provenance_descendants_page(
        self,
        representation_id: RepresentationId,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[ProvenanceMatch]:
        """Return one bounded page of shortest-depth provenance descendants."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return self._object_query_page(
            self._native.lib.pp_production_provenance_descendants_page,
            _provenance_match,
            self._handle,
            native_id,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def stale_artifacts(
        self,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
        source: RepresentationId | None = None,
    ) -> QueryPage[RepresentationId]:
        """Return produced representations currently evaluated as stale.

        ``source`` restricts the query to its provenance descendants; the depth
        and representation bounds apply to each artifact evaluation.
        """

        self._require_open()
        native_source = None if source is None else _native_representation_id(source)
        return self._object_query_page(
            self._native.lib.pp_production_stale_artifacts,
            _representation_match,
            self._handle,
            None if native_source is None else ctypes.byref(native_source),
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    @property
    def host_bindings(self) -> _HostBindings:
        """Return the portable host-binding formatter and parser."""

        self._require_open()
        return _HostBindings(self)

    def asset_exists(self, asset_id: AssetId) -> bool:
        """Return whether an asset identity belongs to this production."""

        self._require_open()
        native_id = _native_asset_id(asset_id)
        exists = ctypes.c_uint8()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_asset_exists(
            self._handle,
            native_id,
            ctypes.byref(exists),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return bool(exists.value)

    def _representations(self, asset_id: AssetId) -> tuple[Representation, ...]:
        self._require_open()
        native_id = _native_asset_id(asset_id)
        handle = ctypes.POINTER(RepresentationSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_representations(
            self._handle,
            native_id,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native representation query returned no result set",
            )
        try:
            count = self._native.lib.pp_representation_set_count(handle)
            return tuple(
                _representation_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_representation_set_release(handle)

    def _activities_for(
        self, direction: str, representation_id: RepresentationId
    ) -> tuple[Activity, ...]:
        self._require_open()
        function = {
            "producing": self._native.lib.pp_production_activities_producing,
            "consuming": self._native.lib.pp_production_activities_consuming,
        }[direction]
        native_id = _native_representation_id(representation_id)
        return self._activity_set(function, self._handle, native_id)

    def _provenance_representations(
        self, direction: str, representation_id: RepresentationId
    ) -> tuple[RepresentationId, ...]:
        self._require_open()
        function = {
            "ancestors": self._native.lib.pp_production_provenance_ancestors,
            "descendants": self._native.lib.pp_production_provenance_descendants,
        }[direction]
        native_id = _native_representation_id(representation_id)
        handle = ctypes.POINTER(ObjectRefSet)()
        error = ctypes.POINTER(Error)()
        status = function(
            self._handle,
            native_id,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native provenance query returned no result set"
            )
        try:
            count = self._native.lib.pp_object_ref_set_count(handle)
            result: list[RepresentationId] = []
            for index in range(int(count)):
                reference = _object_reference_at(self._native, handle, index)
                if not isinstance(reference, RepresentationRef):
                    raise InternalError(
                        _abi.PP_ERROR_INTERNAL,
                        "native provenance query returned a non-representation",
                    )
                result.append(reference.id)
            return tuple(result)
        finally:
            self._native.lib.pp_object_ref_set_release(handle)

    def _external_identifiers(
        self, target: ObjectReference
    ) -> tuple[ExternalIdentifier, ...]:
        """Return every external identifier attached to ``target``."""

        self._require_open()
        native_target = _native_object_reference(target)
        handle = ctypes.POINTER(ExternalIdentifierSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_external_identifiers(
            self._handle,
            ctypes.byref(native_target),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native identifier query returned no result set"
            )
        try:
            count = self._native.lib.pp_external_identifier_set_count(handle)
            return tuple(
                _external_identifier_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_external_identifier_set_release(handle)

    def _find_by_external_identifier(
        self, scheme: str, value: str, qualifier: str | None
    ) -> tuple[ObjectReference, ...]:
        """Find objects carrying an exact external scheme, value, and qualifier."""

        self._require_open()
        handle = ctypes.POINTER(ObjectRefSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_find_by_external_identifier(
            self._handle,
            _utf8(scheme, "identifier scheme"),
            _utf8(value, "identifier value"),
            None if qualifier is None else _utf8(qualifier, "identifier qualifier"),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native identifier lookup returned no result set",
            )
        try:
            count = self._native.lib.pp_object_ref_set_count(handle)
            return tuple(
                _object_reference_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_object_ref_set_release(handle)

    def _known_media_page(
        self, handle: _Pointer[KnownMediaSet]
    ) -> QueryPage[KnownMediaMatch]:
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native known-media query returned no result set",
            )
        try:
            count = self._native.lib.pp_known_media_set_count(handle)
            return QueryPage(
                tuple(
                    _known_media_match_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(
                    self._native.lib.pp_known_media_set_next_cursor(handle)
                ),
            )
        finally:
            self._native.lib.pp_known_media_set_release(handle)

    def _metadata(self, target: ObjectReference) -> tuple[MetadataAssertion, ...]:
        self._require_open()
        native_target = _native_object_reference(target)
        return self._metadata_set(
            self._native.lib.pp_production_metadata,
            self._handle,
            ctypes.byref(native_target),
        )

    def _metadata_by_property(
        self, property: MetadataKey
    ) -> tuple[MetadataAssertion, ...]:
        self._require_open()
        property = _metadata_key(property)
        return self._metadata_set(
            self._native.lib.pp_production_find_metadata,
            self._handle,
            _utf8(property.vocabulary, "metadata vocabulary"),
            _utf8(property.property, "metadata property"),
        )

    @property
    def latest_revision(self) -> Revision | None:
        """Return the newest committed revision, if one exists."""

        self._require_open()
        revisions = self._revision_set(
            self._native.lib.pp_production_latest_revision, self._handle
        )
        if len(revisions) > 1:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native latest-revision query returned multiple values",
            )
        return revisions[0] if revisions else None

    def changes_since(self, sequence: int, limit: int) -> tuple[Revision, ...]:
        """Return a bounded ascending page of revisions after ``sequence``."""

        self._require_open()
        return self._revision_set(
            self._native.lib.pp_production_changes_since,
            self._handle,
            _unsigned(sequence, 64, "sequence"),
            _page_limit(limit),
        )

    def changes_since_filtered(
        self,
        sequence: int,
        kinds: Iterable[type[RevisionEventPayload]],
        limit: int = 100,
    ) -> FilteredRevisionPage:
        """Return revisions after ``sequence`` with an event of one of ``kinds``.

        ``kinds`` are event payload classes such as ``JobSucceededEvent``.
        Continue from the returned ``through_sequence``, which skips unrelated
        revisions without reading them.
        """

        self._require_open()
        codes = [_revision_event_kind(kind) for kind in kinds]
        native_kinds = (ctypes.c_uint32 * len(codes))(*codes)
        handle = ctypes.POINTER(RevisionSet)()
        through_sequence = ctypes.c_uint64()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_changes_since_filtered(
            self._handle,
            _unsigned(sequence, 64, "sequence"),
            native_kinds,
            len(codes),
            _page_limit(limit),
            ctypes.byref(handle),
            ctypes.byref(through_sequence),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native filtered revision query returned no result set",
            )
        try:
            revisions = _revisions_in(self._native, handle)
        finally:
            self._native.lib.pp_revision_set_release(handle)
        return FilteredRevisionPage(revisions, int(through_sequence.value))

    def revision_waiter(self) -> RevisionWaiter:
        """Create a waiter that blocks until new revisions are committed.

        The waiter observes commits from this production immediately and
        from other processes by polling. Closing the production closes it.
        """

        self._require_open()
        handle = ctypes.POINTER(NativeRevisionWaiter)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_revision_waiter_create(
            self._handle, ctypes.byref(handle), ctypes.byref(error)
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native revision waiter creation returned no handle",
            )
        return RevisionWaiter(self._native, handle)

    def _revision_events(self, revision_id: RevisionId) -> tuple[RevisionEvent, ...]:
        """Return the ordered semantic events for one revision."""

        self._require_open()
        native_id = _native_revision_id(revision_id)
        handle = ctypes.POINTER(RevisionEventSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_revision_events(
            self._handle,
            native_id,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native revision-event query returned no result set",
            )
        try:
            count = self._native.lib.pp_revision_event_set_count(handle)
            return tuple(
                _revision_event_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_revision_event_set_release(handle)

    def import_job_lease(self, token: str) -> JobLease:
        """Import an explicit worker credential and check current authority."""
        self._require_open()
        if not isinstance(token, str):
            raise TypeError("job lease token must be str")
        if len(token) != 115 or not token.isascii():
            raise ValueError("invalid scoped job lease token")
        handle = ctypes.POINTER(_abi.JobLease)()
        encoded = (ctypes.c_uint8 * 115).from_buffer_copy(token.encode("ascii"))
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_import_job_lease(
            self._handle,
            encoded,
            115,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return JobLease(self._native, handle)

    def read_session(self) -> ReadSession:
        """Open a pinned view with its production and revision captured together."""
        self._require_open()
        handle = ctypes.POINTER(_abi.ReadSession)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_read_session(
            self._handle, ctypes.byref(handle), ctypes.byref(error)
        )
        self._native.check(status, error)
        return ReadSession(self._native, handle)

    def edit(
        self,
        base: DecisionBase | None = None,
        *,
        origin: OriginIdentity | str | None = None,
        message: str | None = None,
    ) -> Edit:
        """Begin an additive or detached decision edit requiring explicit commit.

        Omit ``base`` for additive writes. Pass the ``decision_base`` copied
        from a read session for decisions after closing that view. Use
        ``view.edit()`` when staging needs the exact retained knowledge.
        Uncommitted context exit rolls back; a failed commit is terminal.
        """
        self._require_open()
        handle = ctypes.POINTER(NativeTransaction)()
        error = ctypes.POINTER(Error)()
        value = _native_decision_base(base) if base is not None else None
        context = _native_edit_context(origin, message)
        status = self._native.lib.pp_production_begin_edit(
            self._handle,
            ctypes.byref(value) if value is not None else None,
            ctypes.byref(context),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native edit creation returned no handle"
            )
        return Edit(self._native, handle)

    def close(self) -> None:
        """Release the native handle. Repeated calls are harmless."""

        self._finalizer()
        self._handle = ctypes.POINTER(NativeProduction)()

    def __enter__(self) -> Self:
        self._require_open()
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self.close()

    def _require_open(self) -> None:
        if not self._finalizer.alive:
            raise InvalidArgumentError(
                _abi.PP_ERROR_INVALID_ARGUMENT, "production is closed"
            )

    def _revision_set(
        self, function: Callable[..., int], *arguments: object
    ) -> tuple[Revision, ...]:
        return _read_revisions(self._native, function, *arguments)

    def _activity_set(
        self, function: Callable[..., int], *arguments: object
    ) -> tuple[Activity, ...]:
        handle = ctypes.POINTER(ActivitySet)()
        error = ctypes.POINTER(Error)()
        status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native activity query returned no result set"
            )
        try:
            count = self._native.lib.pp_activity_set_count(handle)
            return tuple(
                _activity_at(self._native, handle, index) for index in range(int(count))
            )
        finally:
            self._native.lib.pp_activity_set_release(handle)

    def _metadata_set(
        self, function: Callable[..., int], *arguments: object
    ) -> tuple[MetadataAssertion, ...]:
        return _metadata_set(self._native, function, *arguments)

    def _representation_page(
        self, function: Callable[..., int], *arguments: object
    ) -> QueryPage[Representation]:
        handle = ctypes.POINTER(RepresentationSet)()
        error = ctypes.POINTER(Error)()
        status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native representation query returned no result set",
            )
        try:
            count = self._native.lib.pp_representation_set_count(handle)
            return QueryPage(
                tuple(
                    _representation_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(
                    self._native.lib.pp_representation_set_next_cursor(handle)
                ),
            )
        finally:
            self._native.lib.pp_representation_set_release(handle)

    def _activity_page(
        self, function: Callable[..., int], *arguments: object
    ) -> QueryPage[Activity]:
        return _read_activity_page(self._native, function, *arguments)

    def _object_query_page(
        self,
        function: Callable[..., int],
        convert: Callable[[ObjectReference, int], _ObjectQueryItem],
        *arguments: object,
    ) -> QueryPage[_ObjectQueryItem]:
        return _object_query_page(self._native, function, convert, *arguments)


def _read_jobs(
    native: NativeLibrary, function: Callable[..., int], *arguments: object
) -> QueryPage[Job]:
    handle = ctypes.POINTER(JobSet)()
    error = ctypes.POINTER(Error)()
    status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native job query returned no result set"
        )
    try:
        count = native.lib.pp_job_set_count(handle)
        return QueryPage(
            tuple(_job_at(native, handle, index) for index in range(int(count))),
            _decode_optional(native.lib.pp_job_set_next_cursor(handle)),
        )
    finally:
        native.lib.pp_job_set_release(handle)


def _submission_outcome(
    native: NativeLibrary, handle: _Pointer[_abi.Outcome]
) -> SubmissionOutcome:
    """Copy complete public facts and release the native outcome on every path."""
    try:
        state = ctypes.c_uint32()
        error = ctypes.POINTER(Error)()
        native.check(
            native.lib.pp_outcome_status(
                handle, ctypes.byref(state), ctypes.byref(error)
            ),
            error,
        )
        terminal: CommitReceipt | PostProjectError
        if state.value == _abi.PP_OUTCOME_ACCEPTED:
            receipt = _abi.CommitReceipt()
            native.check(
                native.lib.pp_outcome_receipt(
                    handle, ctypes.byref(receipt), ctypes.byref(error)
                ),
                error,
            )
            if receipt.outcome not in (
                _abi.PP_COMMIT_NO_CHANGE,
                _abi.PP_COMMIT_REVISION_CREATED,
            ):
                raise UnsupportedError(
                    _abi.PP_ERROR_UNSUPPORTED, "unknown submission receipt"
                )
            terminal = CommitReceipt(
                ProductionId(_uuid(receipt.production_id)),
                CommittedRevision(
                    RevisionId(_uuid(receipt.revision_id)), receipt.revision_sequence
                )
                if receipt.outcome == _abi.PP_COMMIT_REVISION_CREATED
                else None,
            )
        elif state.value == _abi.PP_OUTCOME_REJECTED:
            rejection = ctypes.POINTER(Error)()
            native.check(
                native.lib.pp_outcome_rejection(
                    handle, ctypes.byref(rejection), ctypes.byref(error)
                ),
                error,
            )
            if not rejection:
                raise InternalError(
                    _abi.PP_ERROR_INTERNAL, "rejected outcome has no rejection"
                )
            try:
                native.check(native.lib.pp_error_code(rejection), rejection)
            except PostProjectError as rejected:
                terminal = rejected
            else:
                raise InternalError(
                    _abi.PP_ERROR_INTERNAL, "rejected outcome has no error code"
                )
        else:
            raise UnsupportedError(
                _abi.PP_ERROR_UNSUPPORTED, "unknown submission outcome"
            )
        jobs = []
        count = int(native.lib.pp_outcome_job_count(handle))
        if count > 1000:
            raise InternalError(_abi.PP_ERROR_INTERNAL, "invalid outcome job count")
        for index in range(count):
            identifier, job_state = _abi.JobId(), ctypes.c_uint32()
            native.check(
                native.lib.pp_outcome_job_get(
                    handle,
                    index,
                    ctypes.byref(identifier),
                    ctypes.byref(job_state),
                    ctypes.byref(error),
                ),
                error,
            )
            observed: JobResultStatus
            if job_state.value == _abi.PP_JOB_CLAIMED:
                expiry = ctypes.c_int64()
                native.check(
                    native.lib.pp_outcome_job_expiry(
                        handle, index, ctypes.byref(expiry), ctypes.byref(error)
                    ),
                    error,
                )
                observed = ClaimedJobResult(expiry.value)
            elif job_state.value == _abi.PP_JOB_SUCCEEDED:
                completion = _abi.JobCompletion()
                native.check(
                    native.lib.pp_outcome_job_completion(
                        handle, index, ctypes.byref(completion), ctypes.byref(error)
                    ),
                    error,
                )
                observed = JobCompletion(
                    ActivityId(_uuid(completion.activity_id)),
                    RepresentationId(_uuid(completion.representation_id)),
                )
            elif job_state.value == _abi.PP_JOB_REQUESTED:
                observed = JobRequested()
            elif job_state.value == _abi.PP_JOB_FAILED:
                observed = FailedJobResult()
            elif job_state.value == _abi.PP_JOB_CANCELLED:
                observed = JobCancelled()
            else:
                raise UnsupportedError(
                    _abi.PP_ERROR_UNSUPPORTED, "unknown outcome job state"
                )
            jobs.append(JobResult(JobId(_uuid(identifier)), observed))
        data = ctypes.POINTER(ctypes.c_uint8)()
        length = ctypes.c_uint64()
        native.check(
            native.lib.pp_outcome_document(
                handle, ctypes.byref(data), ctypes.byref(length), ctypes.byref(error)
            ),
            error,
        )
        return SubmissionOutcome(terminal, jobs, ctypes.string_at(data, length.value))
    finally:
        native.lib.pp_outcome_release(handle)


def _protocol_base(value: _abi.ProtocolBase) -> ProtocolBase:
    return ProtocolBase(
        _decision_base(value.decision),
        HistoryId(UUID(bytes=bytes(value.history_id.bytes))),
    )


def _exchange_position(value: _abi.ExchangePosition) -> ExchangePosition:
    return ExchangePosition(_protocol_base(value.base), bytes(value.digest))


def _native_exchange_position(position: ExchangePosition) -> _abi.ExchangePosition:
    value = _abi.ExchangePosition()
    value.base = _native_protocol_base(position.base)
    value.digest[:] = position.digest
    return value


def _native_exchange_limits(
    limits: ExchangeLimits | None,
) -> _abi.ExchangeLimits | None:
    if limits is None:
        return None
    if not isinstance(limits, ExchangeLimits):
        raise TypeError("receiver limits must be ExchangeLimits")
    return _abi.ExchangeLimits(
        limits.file_bytes,
        limits.disk_bytes,
        limits.frames,
        limits.document_bytes,
        limits.document_depth,
        limits.document_nodes,
    )


def _native_protocol_base(base: ProtocolBase) -> _abi.ProtocolBase:
    value = _abi.ProtocolBase()
    value.decision = _native_decision_base(base.decision)
    value.history_id.bytes = _native_uuid(base.history_id).bytes
    return value


def _native_decision_base(base: DecisionBase) -> _abi.DecisionBase:
    value = _abi.DecisionBase()
    value.production_id = _native_production_id(base.production_id)
    if base.revision is not None:
        if (
            type(base.revision.sequence) is not int
            or not 0 < base.revision.sequence <= 2**64 - 1
        ):
            raise ValueError("decision sequence must be a positive uint64")
        value.has_revision = 1
        value.revision_id = _native_revision_id(base.revision.id)
        value.revision_sequence = base.revision.sequence
    return value


def _decision_base(value: _abi.DecisionBase) -> DecisionBase:
    revision = (
        CommittedRevision(
            RevisionId(_uuid(value.revision_id)), int(value.revision_sequence)
        )
        if value.has_revision
        else None
    )
    return DecisionBase(ProductionId(_uuid(value.production_id)), revision)


def _parse_decision_base(
    token: str, library_path: str | os.PathLike[str] | None
) -> DecisionBase:
    native = NativeLibrary(library_path)
    base = _abi.DecisionBase()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_decision_base_parse(
        _utf8(token, "decision-base token"), ctypes.byref(base), ctypes.byref(error)
    )
    native.check(status, error)
    return _decision_base(base)


def _format_decision_base(
    base: DecisionBase, library_path: str | os.PathLike[str] | None
) -> str:
    native = NativeLibrary(library_path)
    value = _native_decision_base(base)
    token = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_decision_base_format(
        ctypes.byref(value), ctypes.byref(token), ctypes.byref(error)
    )
    native.check(status, error)
    try:
        if token.value is None:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native decision-base formatter returned no token",
            )
        return token.value.decode("utf-8")
    finally:
        native.lib.pp_string_release(token)


def _verify_resource(
    native: NativeLibrary,
    function: Callable[..., int],
    handle: object,
    resource_id: ResourceId,
    path: str | os.PathLike[str],
    sequence_naming: SequenceNaming | None,
) -> ContentVerification:
    native_id = _native_resource_id(resource_id)
    verification = ctypes.c_uint32()
    error = ctypes.POINTER(Error)()
    status = function(
        handle,
        native_id,
        _path_bytes(path),
        _native_naming(sequence_naming),
        ctypes.byref(verification),
        ctypes.byref(error),
    )
    native.check(status, error)
    try:
        return _CONTENT_VERIFICATIONS[verification.value]
    except KeyError as error:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "unknown content verification"
        ) from error


def _resolve(
    native: NativeLibrary,
    function: Callable[..., int],
    handle: object,
    asset_ids: AssetId | Iterable[AssetId],
    root_mappings: Mapping[str, str | os.PathLike[str]] | None,
    search_directories: Iterable[str | os.PathLike[str]],
    verification: VerificationMode,
    max_depth: int,
    max_entries_per_directory: int,
    cancel_token: CancelToken | None,
) -> tuple[RepresentationResolution, ...]:
    ids = (AssetId(asset_ids),) if isinstance(asset_ids, UUID) else tuple(asset_ids)
    options = _ResolutionOptions(
        native,
        root_mappings,
        search_directories,
        verification,
        max_depth,
        max_entries_per_directory,
        cancel_token,
    )
    try:
        return _resolve_assets(native, function, handle, ids, options)
    finally:
        options.close()


def _resolve_assets(
    native: NativeLibrary,
    function: Callable[..., int],
    reader: object,
    asset_ids: tuple[AssetId, ...],
    options: _ResolutionOptions,
) -> tuple[RepresentationResolution, ...]:
    native_ids = (_abi.AssetId * len(asset_ids))(
        *(_native_asset_id(asset_id) for asset_id in asset_ids)
    )
    handle = ctypes.POINTER(ResolutionSet)()
    error = ctypes.POINTER(Error)()
    status = function(
        reader,
        native_ids if asset_ids else None,
        len(asset_ids),
        options.handle,
        ctypes.byref(handle),
        ctypes.byref(error),
    )
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native resolution query returned no result set"
        )
    try:
        count = native.lib.pp_resolution_set_representation_count(handle)
        return tuple(
            _representation_resolution_at(native, handle, index)
            for index in range(int(count))
        )
    finally:
        native.lib.pp_resolution_set_release(handle)


def _object_query_page(
    native: NativeLibrary,
    function: Callable[..., int],
    convert: Callable[[ObjectReference, int], _ObjectQueryItem],
    *arguments: object,
) -> QueryPage[_ObjectQueryItem]:
    handle = ctypes.POINTER(ObjectQuerySet)()
    error = ctypes.POINTER(Error)()
    status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native object query returned no result set"
        )
    try:
        count = native.lib.pp_object_query_set_count(handle)
        items: list[_ObjectQueryItem] = []
        for index in range(int(count)):
            value = _abi.ObjectRef()
            depth = ctypes.c_uint32()
            item_error = ctypes.POINTER(Error)()
            item_status = native.lib.pp_object_query_set_get(
                handle,
                index,
                ctypes.byref(value),
                ctypes.byref(depth),
                ctypes.byref(item_error),
            )
            native.check(item_status, item_error)
            items.append(convert(_object_reference(value), int(depth.value)))
        return QueryPage(
            tuple(items),
            _decode_optional(native.lib.pp_object_query_set_next_cursor(handle)),
            bool(native.lib.pp_object_query_set_traversal_truncated(handle)),
        )
    finally:
        native.lib.pp_object_query_set_release(handle)


_MAX_REVISION_WAIT_SECONDS = _abi.PP_REVISION_WAIT_MAX_TIMEOUT_MILLIS / 1000
_WAIT_RESULTS = {
    _abi.PP_REVISION_WAIT_REVISIONS: RevisionWaitResult.REVISIONS,
    _abi.PP_REVISION_WAIT_TIMED_OUT: RevisionWaitResult.TIMED_OUT,
    _abi.PP_REVISION_WAIT_CLOSED: RevisionWaitResult.CLOSED,
    _abi.PP_REVISION_WAIT_CANCELLED: RevisionWaitResult.CANCELLED,
}


def _locator_page(
    native: NativeLibrary, handle: _Pointer[LocatorQuerySet]
) -> QueryPage[LocatorMatch]:
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native locator query returned no result set"
        )
    try:
        count = native.lib.pp_locator_query_set_count(handle)
        return QueryPage(
            tuple(
                _locator_match_at(native, handle, index) for index in range(int(count))
            ),
            _decode_optional(native.lib.pp_locator_query_set_next_cursor(handle)),
        )
    finally:
        native.lib.pp_locator_query_set_release(handle)


def _metadata_set(
    native: NativeLibrary, function: Callable[..., int], *arguments: object
) -> tuple[MetadataAssertion, ...]:
    handle = ctypes.POINTER(MetadataSet)()
    error = ctypes.POINTER(Error)()
    status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native metadata query returned no result set"
        )
    try:
        count = native.lib.pp_metadata_set_count(handle)
        return tuple(_metadata_at(native, handle, index) for index in range(int(count)))
    finally:
        native.lib.pp_metadata_set_release(handle)


def _read_regeneration_plans(
    native: NativeLibrary,
    function: Callable[..., int],
    handle: object,
    artifact_representation_ids: Iterable[RepresentationId],
) -> tuple[RegenerationJobPlan, ...]:
    artifact_ids = tuple(
        islice(artifact_representation_ids, _abi.PP_MAX_REGENERATION_PLANS + 1)
    )
    if len(artifact_ids) > _abi.PP_MAX_REGENERATION_PLANS:
        raise ValueError("too many artifacts for regeneration planning")
    native_ids = (_abi.RepresentationId * len(artifact_ids))(
        *(_native_representation_id(artifact_id) for artifact_id in artifact_ids)
    )
    result_handle = ctypes.POINTER(RegenerationPlanSet)()
    error = ctypes.POINTER(Error)()
    status = function(
        handle,
        native_ids if artifact_ids else None,
        len(artifact_ids),
        ctypes.byref(result_handle),
        ctypes.byref(error),
    )
    native.check(status, error)
    if not result_handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native regeneration query returned no result set"
        )
    try:
        return tuple(
            _regeneration_plan_at(native, result_handle, index)
            for index in range(
                int(native.lib.pp_regeneration_plan_set_count(result_handle))
            )
        )
    finally:
        native.lib.pp_regeneration_plan_set_release(result_handle)


def _read_revision_event_page(
    native: NativeLibrary, function: Callable[..., int], *arguments: object
) -> QueryPage[RevisionEvent]:
    handle = ctypes.POINTER(RevisionEventSet)()
    error = ctypes.POINTER(Error)()
    status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native revision-event query returned no result set"
        )
    try:
        count = native.lib.pp_revision_event_set_count(handle)
        return QueryPage(
            tuple(
                _revision_event_at(native, handle, index) for index in range(int(count))
            ),
            _decode_optional(native.lib.pp_revision_event_set_next_cursor(handle)),
        )
    finally:
        native.lib.pp_revision_event_set_release(handle)


def _read_revisions(
    native: NativeLibrary, function: Callable[..., int], *arguments: object
) -> tuple[Revision, ...]:
    handle = ctypes.POINTER(RevisionSet)()
    error = ctypes.POINTER(Error)()
    status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native revision query returned no result set"
        )
    try:
        return tuple(
            _revision_at(native, handle, index)
            for index in range(int(native.lib.pp_revision_set_count(handle)))
        )
    finally:
        native.lib.pp_revision_set_release(handle)


def _read_activity_page(
    native: NativeLibrary, function: Callable[..., int], *arguments: object
) -> QueryPage[Activity]:
    handle = ctypes.POINTER(ActivitySet)()
    error = ctypes.POINTER(Error)()
    status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native activity query returned no result set"
        )
    try:
        count = native.lib.pp_activity_set_count(handle)
        return QueryPage(
            tuple(_activity_at(native, handle, index) for index in range(int(count))),
            _decode_optional(native.lib.pp_activity_set_next_cursor(handle)),
        )
    finally:
        native.lib.pp_activity_set_release(handle)


def _read_dependency_query(
    native: NativeLibrary, function: Callable[..., int], *arguments: object
) -> QueryPage[DependencyMatch]:
    handle = ctypes.POINTER(DependencyQuerySet)()
    error = ctypes.POINTER(Error)()
    status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native dependency query returned no result set"
        )
    try:
        return _dependency_query_page(native, handle)
    finally:
        native.lib.pp_dependency_query_set_release(handle)


def _read_dependency_set(
    native: NativeLibrary, function: Callable[..., int], *arguments: object
) -> DependencySet | None:
    handle = ctypes.POINTER(NativeDependencySet)()
    error = ctypes.POINTER(Error)()
    status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native dependency query returned no result set"
        )
    try:
        present = ctypes.c_uint8()
        source_id = _abi.RepresentationId()
        revision = ctypes.c_uint64()
        set_status = _abi.DependencySetStatus()
        count = ctypes.c_uint64()
        summary_error = ctypes.POINTER(Error)()
        summary_status = native.lib.pp_dependency_set_get(
            handle,
            ctypes.byref(present),
            ctypes.byref(source_id),
            ctypes.byref(revision),
            ctypes.byref(set_status),
            ctypes.byref(count),
            ctypes.byref(summary_error),
        )
        native.check(summary_status, summary_error)
        if not present.value:
            return None
        return DependencySet(
            RepresentationId(_uuid(source_id)),
            int(revision.value),
            _dependency_set_status(int(set_status.value)),
            tuple(
                _dependency_at(native, handle, index)
                for index in range(int(count.value))
            ),
        )
    finally:
        native.lib.pp_dependency_set_release(handle)


class ReadSession:
    """Owned pinned view; copied results outlive close, further reads reject."""

    def __init__(
        self, native: NativeLibrary, handle: _Pointer[_abi.ReadSession]
    ) -> None:
        self._native = native
        self._handle = handle
        self._finalizer = weakref.finalize(
            self, native.lib.pp_read_session_release, handle
        )

    def plan_regeneration(
        self, artifact_representation_ids: Iterable[RepresentationId]
    ) -> tuple[RegenerationJobPlan, ...]:
        """Derive job proposals from this view without enqueuing or executing them."""

        self._require_open()
        return _read_regeneration_plans(
            self._native,
            self._native.lib.pp_read_session_plan_regeneration,
            self._handle,
            artifact_representation_ids,
        )

    def revision_events_page(
        self, revision_id: RevisionId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[RevisionEvent]:
        """Return one bounded page of events visible in this retained view."""

        self._require_open()
        native_id = _native_revision_id(revision_id)
        return _read_revision_event_page(
            self._native,
            self._native.lib.pp_read_session_revision_events_page,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def representations_under_media_root(
        self, root_name: str, *, limit: int, cursor: str | None = None
    ) -> QueryPage[Representation]:
        """Return one bounded page of representations located under a root."""

        self._require_open()
        return self._representation_page(
            self._native.lib.pp_read_session_representations_under_media_root,
            self._handle,
            _utf8(root_name, "root name"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def unresolved_media(
        self, *, limit: int, cursor: str | None = None
    ) -> QueryPage[RepresentationId]:
        """Return representations whose required resources lack locators."""

        self._require_open()
        return _object_query_page(
            self._native,
            self._native.lib.pp_read_session_unresolved_media,
            _representation_match,
            self._handle,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def objects_changed_since(
        self, sequence: int, *, limit: int, cursor: str | None = None
    ) -> QueryPage[ObjectReference]:
        """Return distinct semantic objects touched after revision ``sequence``."""

        self._require_open()
        return _object_query_page(
            self._native,
            self._native.lib.pp_read_session_objects_changed_since,
            _object_match,
            self._handle,
            _unsigned(sequence, 64, "sequence"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def provenance_ancestors_page(
        self,
        representation_id: RepresentationId,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[ProvenanceMatch]:
        """Return one bounded page of shortest-depth provenance ancestors."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return _object_query_page(
            self._native,
            self._native.lib.pp_read_session_provenance_ancestors_page,
            _provenance_match,
            self._handle,
            native_id,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def provenance_descendants_page(
        self,
        representation_id: RepresentationId,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[ProvenanceMatch]:
        """Return one bounded page of shortest-depth provenance descendants."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return _object_query_page(
            self._native,
            self._native.lib.pp_read_session_provenance_descendants_page,
            _provenance_match,
            self._handle,
            native_id,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def stale_artifacts(
        self,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
        source: RepresentationId | None = None,
    ) -> QueryPage[RepresentationId]:
        """Return produced representations currently evaluated as stale.

        ``source`` restricts the query to its provenance descendants; the depth
        and representation bounds apply to each artifact evaluation.
        """

        self._require_open()
        native_source = None if source is None else _native_representation_id(source)
        return _object_query_page(
            self._native,
            self._native.lib.pp_read_session_stale_artifacts,
            _representation_match,
            self._handle,
            None if native_source is None else ctypes.byref(native_source),
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def outputs_by_activity_kind(
        self, kind: str, *, limit: int, cursor: str | None = None
    ) -> QueryPage[RepresentationId]:
        """Return outputs produced by activities of one exact kind."""

        self._require_open()
        return _object_query_page(
            self._native,
            self._native.lib.pp_read_session_outputs_by_activity_kind,
            _representation_match,
            self._handle,
            _utf8(kind, "activity kind"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def outputs_by_tool(
        self, tool: ToolIdentity, *, limit: int, cursor: str | None = None
    ) -> QueryPage[RepresentationId]:
        """Return outputs produced by activities with one exact tool identity."""

        self._require_open()
        return _object_query_page(
            self._native,
            self._native.lib.pp_read_session_outputs_by_tool,
            _representation_match,
            self._handle,
            _utf8(tool.name, "tool name"),
            _optional_text(tool.version),
            _optional_text(tool.uri),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def activities_producing_page(
        self,
        representation_id: RepresentationId,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[Activity]:
        """Return one bounded page of activities producing a representation."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return _read_activity_page(
            self._native,
            self._native.lib.pp_read_session_activities_producing_page,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def activities_consuming_page(
        self,
        representation_id: RepresentationId,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[Activity]:
        """Return one bounded page of activities consuming a representation."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return _read_activity_page(
            self._native,
            self._native.lib.pp_read_session_activities_consuming_page,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def changes_since_filtered(
        self,
        sequence: int,
        kinds: Iterable[type[RevisionEventPayload]],
        limit: int = 100,
    ) -> FilteredRevisionPage:
        """Return revisions after ``sequence`` with an event of one of ``kinds``.

        ``kinds`` are event payload classes such as ``JobSucceededEvent``.
        Continue from the returned ``through_sequence``, which skips unrelated
        revisions without reading them.
        """

        self._require_open()
        codes = [_revision_event_kind(kind) for kind in kinds]
        native_kinds = (ctypes.c_uint32 * len(codes))(*codes)
        handle = ctypes.POINTER(RevisionSet)()
        through_sequence = ctypes.c_uint64()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_changes_since_filtered(
            self._handle,
            _unsigned(sequence, 64, "sequence"),
            native_kinds,
            len(codes),
            _page_limit(limit),
            ctypes.byref(handle),
            ctypes.byref(through_sequence),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native filtered revision query returned no result set",
            )
        try:
            revisions = _revisions_in(self._native, handle)
        finally:
            self._native.lib.pp_revision_set_release(handle)
        return FilteredRevisionPage(revisions, int(through_sequence.value))

    @property
    def latest_revision(self) -> Revision | None:
        """Return the newest committed revision, if one exists."""

        self._require_open()
        revisions = _read_revisions(
            self._native, self._native.lib.pp_read_session_latest_revision, self._handle
        )
        if len(revisions) > 1:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native latest-revision query returned multiple values",
            )
        return revisions[0] if revisions else None

    def changes_since(self, sequence: int, limit: int) -> tuple[Revision, ...]:
        """Return a bounded ascending page of revisions after ``sequence``."""

        self._require_open()
        return _read_revisions(
            self._native,
            self._native.lib.pp_read_session_changes_since,
            self._handle,
            _unsigned(sequence, 64, "sequence"),
            _page_limit(limit),
        )

    def revisions(self, *, after: int = 0, page_size: int = 100) -> Iterator[Revision]:
        """Scan copied revision summaries in ascending order within this view.

        This is a finite scan of the pinned history, not a live subscription.
        Page size bounds each native read; the complete scan has no total cap.
        Closing the session invalidates unstarted and buffered advances.
        """
        self._require_open()
        sequence = _unsigned(after, 64, "after")
        limit = _page_limit(page_size)

        def scan() -> Iterator[Revision]:
            position = sequence
            while True:
                self._require_open()
                page = self.changes_since(position, limit)
                for revision in page:
                    self._require_open()
                    yield revision
                self._require_open()
                if len(page) < limit:
                    return
                position = page[-1].sequence

        return scan()

    def dependency_set(
        self, representation_id: RepresentationId
    ) -> DependencySet | None:
        """Return recorded dependency knowledge, preserving absent versus empty."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return _read_dependency_set(
            self._native,
            self._native.lib.pp_read_session_dependency_set,
            self._handle,
            native_id,
        )

    def dependencies(
        self,
        representation_id: RepresentationId,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[DependencyMatch]:
        """Return one bounded page of direct or transitive dependencies."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return _read_dependency_query(
            self._native,
            self._native.lib.pp_read_session_dependencies,
            self._handle,
            native_id,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def dependents(
        self,
        target: AssetRef | RepresentationRef,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[DependencyMatch]:
        """Return one bounded page of direct or transitive dependents."""

        self._require_open()
        native_target = _native_object_reference(target)
        return _read_dependency_query(
            self._native,
            self._native.lib.pp_read_session_dependents,
            self._handle,
            ctypes.byref(native_target),
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def verify_resource(
        self,
        resource_id: ResourceId,
        path: str | os.PathLike[str],
        *,
        sequence_naming: SequenceNaming | None = None,
    ) -> ContentVerification:
        """Compare current files with resource fingerprints in this view."""
        self._require_open()
        return _verify_resource(
            self._native,
            self._native.lib.pp_read_session_verify_resource,
            self._handle,
            resource_id,
            path,
            sequence_naming,
        )

    def resolve(
        self,
        asset_ids: AssetId | Iterable[AssetId],
        root_mappings: Mapping[str, str | os.PathLike[str]] | None = None,
        *,
        search_directories: Iterable[str | os.PathLike[str]] = (),
        verification: VerificationMode = VerificationMode.PRESENCE,
        max_depth: int = 64,
        max_entries_per_directory: int = 100_000,
        cancel_token: CancelToken | None = None,
    ) -> tuple[RepresentationResolution, ...]:
        """Resolve current files using this view's stored media knowledge."""
        self._require_open()
        return _resolve(
            self._native,
            self._native.lib.pp_read_session_resolve_assets,
            self._handle,
            asset_ids,
            root_mappings,
            search_directories,
            verification,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_entries_per_directory, 64, "max_entries_per_directory"),
            cancel_token,
        )

    def job(self, job_id: JobId) -> Job:
        """Return one durable job, raising ``NotFoundError`` when absent."""

        self._require_open()
        native_id = _native_job_id(job_id)
        page = _read_jobs(
            self._native,
            self._native.lib.pp_read_session_job,
            self._handle,
            native_id,
        )
        if len(page.items) != 1:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native job read returned no single job"
            )
        return page.items[0]

    def jobs(
        self,
        *,
        limit: int,
        cursor: str | None = None,
        state: JobState | None = None,
        kind: str | None = None,
    ) -> QueryPage[Job]:
        """Return one bounded job page with optional exact predicates."""

        self._require_open()
        return _read_jobs(
            self._native,
            self._native.lib.pp_read_session_jobs,
            self._handle,
            0 if state is None else _native_job_state(state),
            _optional_text(kind),
            _page_limit(limit),
            _optional_text(cursor),
        )

    def evaluate_artifact(
        self,
        representation_id: RepresentationId,
        *,
        max_depth: int = 64,
        max_representations: int = 1_000,
    ) -> ArtifactEvaluation:
        """Evaluate stored artifact evidence without accessing media files."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        handle = ctypes.POINTER(NativeArtifactEvaluation)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_evaluate_artifact(
            self._handle,
            native_id,
            _unsigned(max_depth, 32, "max_depth"),
            _unsigned(max_representations, 32, "max_representations"),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native artifact evaluation returned no result"
            )
        try:
            return read_evaluation(self._native, handle)
        finally:
            self._native.lib.pp_artifact_evaluation_release(handle)

    def artifact_reproducibility(
        self, representation_id: RepresentationId
    ) -> ArtifactReproducibility:
        """Report whether stored knowledge is sufficient to reproduce an artifact."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        handle = ctypes.POINTER(NativeArtifactReproducibility)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_artifact_reproducibility(
            self._handle,
            native_id,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native artifact reproducibility returned no report",
            )
        try:
            return read_reproducibility(self._native, handle)
        finally:
            self._native.lib.pp_artifact_reproducibility_release(handle)

    @property
    def decision_base(self) -> DecisionBase:
        """Detach the revision and production captured with this view."""
        self._require_open()
        value = _abi.DecisionBase()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_decision_base(
            self._handle, ctypes.byref(value), ctypes.byref(error)
        )
        self._native.check(status, error)
        return _decision_base(value)

    def edit(
        self,
        *,
        origin: OriginIdentity | str | None = None,
        message: str | None = None,
    ) -> Edit:
        """Begin a decision edit retaining this view's exact read knowledge."""
        self._require_open()
        handle = ctypes.POINTER(NativeTransaction)()
        error = ctypes.POINTER(Error)()
        context = _native_edit_context(origin, message)
        status = self._native.lib.pp_read_session_begin_edit(
            self._handle,
            ctypes.byref(context),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return Edit(self._native, handle)

    def asset(self, asset_id: AssetId) -> Asset:
        """Return one asset, raising ``NotFoundError`` when it is absent."""

        self._require_open()
        native_id = _native_asset_id(asset_id)
        handle = ctypes.POINTER(AssetSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_asset(
            self._handle,
            native_id,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native asset read returned no result set"
            )
        try:
            if self._native.lib.pp_asset_set_count(handle) != 1:
                raise InternalError(
                    _abi.PP_ERROR_INTERNAL, "native asset read returned no single asset"
                )
            return _asset_at(self._native, handle, 0)
        finally:
            self._native.lib.pp_asset_set_release(handle)

    def representation(self, representation_id: RepresentationId) -> Representation:
        """Return one representation, raising ``NotFoundError`` when absent."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        page = self._representation_page(
            self._native.lib.pp_read_session_representation,
            self._handle,
            native_id,
        )
        if len(page.items) != 1:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native representation read returned no single representation",
            )
        return page.items[0]

    def assets(self, *, page_size: int = 100) -> Iterator[Asset]:
        """Iterate bounded asset pages in this explicitly owned snapshot.

        Page size is validated immediately. Each advance requires an open
        session, including advances through buffered values. Copied assets
        outlive close; closing this iterator does not close the session.
        """

        self._require_open()
        _page_limit(page_size)

        def iterate() -> Iterator[Asset]:
            cursor = None
            while True:
                page = self.assets_page(limit=page_size, cursor=cursor)
                for asset in page.items:
                    self._require_open()
                    yield asset
                self._require_open()
                cursor = page.next_cursor
                if cursor is None:
                    return

        return iterate()

    def assets_page(self, *, limit: int, cursor: str | None = None) -> QueryPage[Asset]:
        """Return one bounded asset page in creation and identity order."""

        self._require_open()
        handle = ctypes.POINTER(AssetSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_assets_page(
            self._handle,
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native asset query returned no result set"
            )
        try:
            count = self._native.lib.pp_asset_set_count(handle)
            return QueryPage(
                tuple(
                    _asset_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(self._native.lib.pp_asset_set_next_cursor(handle)),
            )
        finally:
            self._native.lib.pp_asset_set_release(handle)

    def representations_page(
        self, asset_id: AssetId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[Representation]:
        """Return one bounded page of representations belonging to an asset."""

        self._require_open()
        native_id = _native_asset_id(asset_id)
        return self._representation_page(
            self._native.lib.pp_read_session_representations_page,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def representations_using_resource(
        self, resource_id: ResourceId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[Representation]:
        """Return one bounded page of representations that use a resource."""

        self._require_open()
        native_id = _native_resource_id(resource_id)
        return self._representation_page(
            self._native.lib.pp_read_session_representations_using_resource,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def resources_page(
        self,
        representation_id: RepresentationId,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[ResourceId]:
        """Return one bounded page of resources in representation order."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        return _object_query_page(
            self._native,
            self._native.lib.pp_read_session_resources_page,
            _resource_match,
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
        )

    def locators_page(
        self, resource_id: ResourceId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[LocatorMatch]:
        """Return one bounded page of locators belonging to a resource."""

        self._require_open()
        native_id = _native_resource_id(resource_id)
        handle = ctypes.POINTER(LocatorQuerySet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_locators_page(
            self._handle,
            native_id,
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return _locator_page(self._native, handle)

    def media_roots_page(
        self, *, limit: int, cursor: str | None = None
    ) -> QueryPage[MediaRoot]:
        """Read a bounded root page in priority and identity order."""

        self._require_open()
        handle = ctypes.POINTER(MediaRootSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_media_roots_page(
            self._handle,
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return _media_root_page(self._native, handle)

    @property
    def media_roots(self) -> tuple[MediaRoot, ...]:
        """Return resolver roots in priority order, with a 1000-root cap."""

        self._require_open()
        handle = ctypes.POINTER(MediaRootSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_media_roots(
            self._handle, ctypes.byref(handle), ctypes.byref(error)
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native media-root query returned no result set"
            )
        try:
            count = self._native.lib.pp_media_root_set_count(handle)
            return tuple(
                _media_root_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_media_root_set_release(handle)

    def find_known_media(
        self,
        source: str | os.PathLike[str] | LocatorIdentity | Fingerprint,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[KnownMediaMatch]:
        """Find ownership candidates by file path, exact locator or fingerprint.

        Paths must exist for canonicalization; lookup never hashes their bytes.
        Sequences require a LocatorIdentity with exact naming. Follow the page
        continuation before treating candidates as complete; lookup never
        adopts or merges identity. Strings are paths, not guessed URI inputs.
        """
        self._require_open()
        if isinstance(source, Fingerprint):
            return self._find_known_media_by_fingerprint(
                source, limit=limit, cursor=cursor
            )
        if isinstance(source, (str, os.PathLike)):
            source = LocatorIdentity(_file_locator(self._native, source))
        if not isinstance(source, LocatorIdentity):
            raise TypeError("source must be a path, LocatorIdentity or Fingerprint")
        return self._find_known_media_by_locator(source, limit=limit, cursor=cursor)

    def _find_known_media_by_locator(
        self,
        locator: LocatorIdentity,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[KnownMediaMatch]:
        """Find every current ownership candidate at an exact locator.

        The query is read-only. For an image sequence, ``locator`` must carry
        its exact directory naming; directory-only lookup does not match it.
        """

        self._require_open()
        handle = ctypes.POINTER(KnownMediaSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_find_known_media_by_locator(
            self._handle,
            _utf8(locator.uri, "locator URI"),
            _native_naming(locator.sequence_naming),
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return self._known_media_page(handle)

    def _find_known_media_by_fingerprint(
        self,
        fingerprint: Fingerprint,
        *,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[KnownMediaMatch]:
        """Find resources with this exact current effective fingerprint.

        Every candidate is returned; content equality does not prove logical
        asset identity and the query never adopts or merges media.
        """

        self._require_open()
        value = (ctypes.c_uint8 * len(fingerprint.value)).from_buffer_copy(
            fingerprint.value
        )
        handle = ctypes.POINTER(KnownMediaSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_find_known_media_by_fingerprint(
            self._handle,
            _utf8(fingerprint.algorithm, "fingerprint algorithm"),
            _unsigned(fingerprint.version, 16, "fingerprint version"),
            value,
            len(fingerprint.value),
            _page_limit(limit),
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return self._known_media_page(handle)

    def metadata(self, target: ObjectReference) -> tuple[MetadataAssertion, ...]:
        """Return copied metadata assertions for one object in this view."""

        self._require_open()
        native_target = _native_object_reference(target)
        return _metadata_set(
            self._native,
            self._native.lib.pp_read_session_metadata,
            self._handle,
            ctypes.byref(native_target),
        )

    def metadata_by_property(
        self, property: MetadataKey
    ) -> tuple[MetadataAssertion, ...]:
        """Return copied assertions of one property in this view."""

        self._require_open()
        property = _metadata_key(property)
        return _metadata_set(
            self._native,
            self._native.lib.pp_read_session_find_metadata,
            self._handle,
            _utf8(property.vocabulary, "metadata vocabulary"),
            _utf8(property.property, "metadata property"),
        )

    def query_metadata(
        self,
        property: MetadataKey,
        *,
        limit: int,
        cursor: str | None = None,
        value: MetadataInput | None = None,
    ) -> QueryPage[MetadataAssertion]:
        """Return one bounded assertion page with an optional exact scalar value."""

        self._require_open()
        property = _metadata_key(property)
        vocabulary = _utf8(property.vocabulary, "metadata vocabulary")
        property_name = _utf8(property.property, "metadata property")
        native_cursor = _optional_text(cursor)
        handle = ctypes.POINTER(MetadataSet)()
        error = ctypes.POINTER(Error)()
        native_value = None if value is None else _metadata_input(self._native, value)
        try:
            status = self._native.lib.pp_read_session_query_metadata(
                self._handle,
                vocabulary,
                property_name,
                native_value,
                _page_limit(limit),
                native_cursor,
                ctypes.byref(handle),
                ctypes.byref(error),
            )
        finally:
            if native_value is not None:
                self._native.lib.pp_metadata_input_release(native_value)
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native metadata query returned no result set"
            )
        try:
            count = self._native.lib.pp_metadata_set_count(handle)
            return QueryPage(
                tuple(
                    _metadata_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(self._native.lib.pp_metadata_set_next_cursor(handle)),
            )
        finally:
            self._native.lib.pp_metadata_set_release(handle)

    def external_identifiers(
        self, target: ObjectReference
    ) -> tuple[ExternalIdentifier, ...]:
        """Return every external identifier attached to ``target``."""

        self._require_open()
        native_target = _native_object_reference(target)
        handle = ctypes.POINTER(ExternalIdentifierSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_external_identifiers(
            self._handle,
            ctypes.byref(native_target),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native identifier query returned no result set"
            )
        try:
            count = self._native.lib.pp_external_identifier_set_count(handle)
            return tuple(
                _external_identifier_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_external_identifier_set_release(handle)

    def find_by_external_identifier(
        self, scheme: str, value: str, qualifier: str | None
    ) -> tuple[ObjectReference, ...]:
        """Find objects carrying an exact external scheme, value, and qualifier."""

        self._require_open()
        handle = ctypes.POINTER(ObjectRefSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_read_session_find_by_external_identifier(
            self._handle,
            _utf8(scheme, "identifier scheme"),
            _utf8(value, "identifier value"),
            None if qualifier is None else _utf8(qualifier, "identifier qualifier"),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native identifier lookup returned no result set",
            )
        try:
            count = self._native.lib.pp_object_ref_set_count(handle)
            return tuple(
                _object_reference_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_object_ref_set_release(handle)

    def _known_media_page(
        self, handle: _Pointer[KnownMediaSet]
    ) -> QueryPage[KnownMediaMatch]:
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native known-media query returned no result set",
            )
        try:
            count = self._native.lib.pp_known_media_set_count(handle)
            return QueryPage(
                tuple(
                    _known_media_match_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(
                    self._native.lib.pp_known_media_set_next_cursor(handle)
                ),
            )
        finally:
            self._native.lib.pp_known_media_set_release(handle)

    def _representation_page(
        self, function: Callable[..., int], *arguments: object
    ) -> QueryPage[Representation]:
        handle = ctypes.POINTER(RepresentationSet)()
        error = ctypes.POINTER(Error)()
        status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native representation query returned no result set",
            )
        try:
            count = self._native.lib.pp_representation_set_count(handle)
            return QueryPage(
                tuple(
                    _representation_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(
                    self._native.lib.pp_representation_set_next_cursor(handle)
                ),
            )
        finally:
            self._native.lib.pp_representation_set_release(handle)

    def close(self) -> None:
        """Release the pinned view; repeated calls are harmless."""
        self._finalizer()
        self._handle = ctypes.POINTER(_abi.ReadSession)()

    def _require_open(self) -> None:
        if not self._finalizer.alive:
            raise InvalidArgumentError(
                _abi.PP_ERROR_INVALID_ARGUMENT, "read session is closed"
            )

    def __enter__(self) -> Self:
        self._require_open()
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self.close()


class RevisionWaiter:
    """Blocks until revisions after a sequence are committed.

    The waiter owns its own connection to the production file, so a wait never
    blocks other calls on the production. Waits must not overlap one another or
    ``close()``; ``cancel()`` may be called from any thread at any time.
    """

    def __init__(
        self, native: NativeLibrary, handle: _Pointer[NativeRevisionWaiter]
    ) -> None:
        self._native = native
        self._handle = handle
        self._finalizer = weakref.finalize(
            self, native.lib.pp_revision_waiter_release, handle
        )

    def wait(
        self,
        after_sequence: int,
        *,
        limit: int = 100,
        timeout: float = _MAX_REVISION_WAIT_SECONDS,
    ) -> RevisionWait:
        """Wait up to ``timeout`` seconds (at most 60) for revisions.

        A zero timeout checks once. The GIL is released while waiting.
        """

        if not self._finalizer.alive:
            raise InvalidArgumentError(
                _abi.PP_ERROR_INVALID_ARGUMENT, "revision waiter is closed"
            )
        if not 0 <= timeout <= _MAX_REVISION_WAIT_SECONDS:
            raise ValueError("revision wait timeout must be 0-60 seconds")
        result = ctypes.c_uint32()
        handle = ctypes.POINTER(RevisionSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_revision_waiter_wait(
            self._handle,
            _unsigned(after_sequence, 64, "after_sequence"),
            _page_limit(limit),
            round(timeout * 1000),
            ctypes.byref(result),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native revision wait returned no result set"
            )
        try:
            revisions = _revisions_in(self._native, handle)
        finally:
            self._native.lib.pp_revision_set_release(handle)
        try:
            outcome = _WAIT_RESULTS[result.value]
        except KeyError as error:
            raise UnsupportedError(
                _abi.PP_ERROR_UNSUPPORTED,
                f"unknown revision wait result {result.value}",
            ) from error
        return RevisionWait(outcome, revisions)

    def cancel(self) -> None:
        """End the current wait; later waits return ``CANCELLED``."""

        if self._finalizer.alive:
            self._native.lib.pp_revision_waiter_cancel(self._handle)

    def close(self) -> None:
        """Release the native waiter. Repeated calls are harmless."""

        self._finalizer()
        self._handle = ctypes.POINTER(NativeRevisionWaiter)()

    def __enter__(self) -> Self:
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self.close()


class RevisionObserver:
    """Delivers new revisions to ``callback`` on a thread the observer owns.

    ``callback(revision, events)`` runs on the observer thread for each
    revision after ``after_sequence``; with ``kinds``, only for revisions that
    contain an event of one of those payload classes. Stop the observer before
    closing the production. An exception from the callback or a query ends
    observation and is kept in ``error``.
    """

    _PAGE_SIZE = 100

    def __init__(
        self,
        production: Production,
        callback: Callable[[Revision, tuple[RevisionEvent, ...]], object],
        *,
        after_sequence: int = 0,
        kinds: Iterable[type[RevisionEventPayload]] | None = None,
    ) -> None:
        self._production = production
        self._callback = callback
        self._kinds = None if kinds is None else tuple(kinds)
        if self._kinds is not None:
            for kind in self._kinds:
                _revision_event_kind(kind)
        self._cursor = _unsigned(after_sequence, 64, "after_sequence")
        self._error: BaseException | None = None
        self._waiter = production.revision_waiter()
        self._thread = threading.Thread(
            target=self._run, name="postproject-revision-observer", daemon=True
        )
        self._thread.start()

    @property
    def cursor(self) -> int:
        """Sequence of the last fully delivered revision or filtered page."""

        return self._cursor

    @property
    def error(self) -> BaseException | None:
        """The exception that ended observation, if any."""

        return self._error

    def stop(self) -> None:
        """Cancel the wait and join the observer thread.

        Called from the callback, it only cancels.
        """

        self._waiter.cancel()
        if threading.current_thread() is not self._thread:
            self._thread.join()
            self._waiter.close()

    def __enter__(self) -> Self:
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self.stop()

    def _run(self) -> None:
        try:
            while True:
                wait = self._waiter.wait(self._cursor, limit=self._PAGE_SIZE)
                if wait.result is RevisionWaitResult.TIMED_OUT:
                    continue
                if wait.result is not RevisionWaitResult.REVISIONS:
                    return
                if self._kinds is None:
                    for revision in wait.revisions:
                        self._deliver(revision)
                        self._cursor = revision.sequence
                    continue
                while True:
                    page = self._production.changes_since_filtered(
                        self._cursor, self._kinds, self._PAGE_SIZE
                    )
                    for revision in page.revisions:
                        self._deliver(revision)
                    self._cursor = page.through_sequence
                    if len(page.revisions) < self._PAGE_SIZE:
                        break
        except BaseException as error:
            self._error = error

    def _deliver(self, revision: Revision) -> None:
        self._callback(revision, self._production.revision_events[revision.id])


class Edit:
    """A caller-serialized edit; uncommitted context work rolls back."""

    def __init__(
        self,
        native: NativeLibrary,
        handle: _Pointer[NativeTransaction],
    ) -> None:
        self._native = native
        self._handle = handle
        self._finished = False
        self._finalizer = weakref.finalize(
            self, native.lib.pp_transaction_release, handle
        )

    def import_media(
        self,
        source: MediaSource,
        display_name: str | None = None,
    ) -> ImportedMedia:
        """Stage a new asset whose original representation has the source's
        structure. A path imports a single file."""

        self._require_open()
        asset_id = _abi.AssetId()
        representation_id = _abi.RepresentationId()
        error = ctypes.POINTER(Error)()
        if isinstance(source, (str, os.PathLike)):
            status = self._native.lib.pp_transaction_import_file(
                self._handle,
                _path_bytes(source),
                _optional_text(display_name),
                ctypes.byref(asset_id),
                ctypes.byref(representation_id),
                ctypes.byref(error),
            )
        else:
            with _NativeMediaSource(self._native, source) as native_source:
                status = self._native.lib.pp_transaction_import_media(
                    self._handle,
                    native_source.handle,
                    _optional_text(display_name),
                    ctypes.byref(asset_id),
                    ctypes.byref(representation_id),
                    ctypes.byref(error),
                )
        self._native.check(status, error)
        return ImportedMedia(
            AssetId(_uuid(asset_id)), RepresentationId(_uuid(representation_id))
        )

    def add_representation(
        self,
        asset_id: AssetId,
        kind: RepresentationKind,
        source: MediaSource,
    ) -> RepresentationId:
        """Stage a representation of ``kind`` with the source's structure for an
        existing asset. A path adds a single file."""

        self._require_open()
        native_asset_id = _native_asset_id(asset_id)
        representation_id = _abi.RepresentationId()
        native_kind = _native_representation_kind(kind)
        error = ctypes.POINTER(Error)()
        if isinstance(source, (str, os.PathLike)):
            status = self._native.lib.pp_transaction_add_file_representation(
                self._handle,
                native_asset_id,
                native_kind,
                _path_bytes(source),
                ctypes.byref(representation_id),
                ctypes.byref(error),
            )
        else:
            with _NativeMediaSource(self._native, source) as native_source:
                status = self._native.lib.pp_transaction_add_representation(
                    self._handle,
                    native_asset_id,
                    native_kind,
                    native_source.handle,
                    ctypes.byref(representation_id),
                    ctypes.byref(error),
                )
        self._native.check(status, error)
        return RepresentationId(_uuid(representation_id))

    def add_media_root(
        self,
        name: str,
        label: str | None = None,
        priority: int = 0,
    ) -> MediaRootId:
        """Stage a portable logical root used for resource discovery."""

        self._require_open()
        _signed(priority, 32, "root priority")
        root_id = _abi.MediaRootId()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_add_media_root(
            self._handle,
            _utf8(name, "root name"),
            _optional_text(label),
            priority,
            ctypes.byref(root_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return MediaRootId(_uuid(root_id))

    def set_media_root_enabled(self, root_id: MediaRootId, enabled: bool) -> None:
        """Stage a resolver root's enabled state from a read-bound edit.

        An unbased transaction rejects before staging and remains open.
        """

        self._require_open()
        if not isinstance(enabled, bool):
            raise TypeError("enabled must be a bool")
        native_id = _native_media_root_id(root_id)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_set_media_root_enabled(
            self._handle,
            native_id,
            int(enabled),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def remove_media_root(self, root_id: MediaRootId) -> None:
        """Stage removal of one resolver root from a read-bound edit.

        An unbased transaction rejects before staging and remains open.
        """

        self._require_open()
        native_id = _native_media_root_id(root_id)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_remove_media_root(
            self._handle, native_id, ctypes.byref(error)
        )
        self._native.check(status, error)

    def confirm_selection(self, selection: RelinkSelection) -> None:
        """Stage every location from the original read-bound selection atomically.

        Open this edit with ``base=selection.base``. A mismatch or malformed
        location leaves staged work unchanged. Files are not rechecked here;
        the host still checks association/generation before applying results.
        Commit remains explicit.
        """
        self._require_open()
        if not isinstance(selection, RelinkSelection):
            raise TypeError("selection must be a RelinkSelection")
        locations = (_abi.RelinkLocation * len(selection.locations))()
        for index, item in enumerate(selection.locations):
            locations[index].resource_id = _native_resource_id(item.resource_id)
            locations[index].uri = _utf8(item.candidate.uri, "locator URI")
            locations[index].media_root = (
                None
                if item.candidate.media_root is None
                else _utf8(item.candidate.media_root, "root name")
            )
            locations[index].has_sequence_naming = int(
                item.candidate.sequence_naming is not None
            )
            naming = _native_naming(item.candidate.sequence_naming)
            if naming is not None:
                locations[index].sequence_naming = naming.contents
        native_selection = _abi.RelinkSelection(
            _native_decision_base(selection.base),
            _native_representation_id(selection.representation_id),
            locations,
            len(locations),
        )
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_confirm_selection(
            self._handle, ctypes.byref(native_selection), ctypes.byref(error)
        )
        self._native.check(status, error)

    def confirm_locator(
        self,
        resource_id: ResourceId,
        uri: str,
        *,
        media_root: str | None = None,
        sequence_naming: SequenceNaming | None = None,
    ) -> None:
        """Stage explicit confirmation of one resource candidate URI.

        ``media_root`` records the logical root the URI was found under; it is
        retained as query evidence and need not name a configured or enabled
        root. ``sequence_naming`` names the files at a locator of an
        image-sequence resource: required for such a resource and rejected at
        commit for any other. Pass a candidate's ``media_root`` and
        ``sequence_naming`` to record it as found.
        """

        self._require_open()
        native_id = _native_resource_id(resource_id)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_confirm_locator(
            self._handle,
            native_id,
            _utf8(uri, "locator URI"),
            None if media_root is None else _utf8(media_root, "root name"),
            _native_naming(sequence_naming),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def retire_locator(self, locator_id: LocatorId) -> None:
        """Stage retirement of one superseded resource locator."""

        self._require_open()
        native_id = _native_locator_id(locator_id)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_retire_locator(
            self._handle, native_id, ctypes.byref(error)
        )
        self._native.check(status, error)

    def record_resource_fingerprint(
        self, resource_id: ResourceId, fingerprint: Fingerprint
    ) -> None:
        """Stage a resource fingerprint from a decision-bound edit."""

        self._require_open()
        native_id = _native_resource_id(resource_id)
        value = (ctypes.c_uint8 * len(fingerprint.value)).from_buffer_copy(
            fingerprint.value
        )
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_record_resource_fingerprint(
            self._handle,
            native_id,
            _utf8(fingerprint.algorithm, "fingerprint algorithm"),
            _unsigned(fingerprint.version, 16, "fingerprint version"),
            value,
            len(fingerprint.value),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def observe_resource_content(
        self,
        resource_id: ResourceId,
        path: str | os.PathLike[str],
        *,
        sequence_naming: SequenceNaming | None = None,
    ) -> ContentObservationOutcome:
        """Stage the content at path as the resource's new observation.

        Every representation using the resource is recomputed and staged too,
        so commit leaves no representation pending recomputation. The outcome
        says whether the content changed; changed file facts can still create
        a revision when it is ``UNCHANGED``. Requires a decision-bound edit;
        session edits retain their original view. Detached edits must match
        the head when this operation pins its view.
        ``sequence_naming`` is as for :meth:`Production.verify_resource`.
        """

        self._require_open()
        native_id = _native_resource_id(resource_id)
        outcome = ctypes.c_uint32()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_observe_resource_content(
            self._handle,
            native_id,
            _path_bytes(path),
            _native_naming(sequence_naming),
            ctypes.byref(outcome),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        try:
            return _CONTENT_OBSERVATION_OUTCOMES[outcome.value]
        except KeyError as error:
            raise UnsupportedError(
                _abi.PP_ERROR_UNSUPPORTED, "unknown content observation outcome"
            ) from error

    def record_representation_fingerprint(
        self, representation_id: RepresentationId, fingerprint: Fingerprint
    ) -> None:
        """Stage a representation fingerprint from a decision-bound edit."""

        self._require_open()
        native_id = _native_representation_id(representation_id)
        value = (ctypes.c_uint8 * len(fingerprint.value)).from_buffer_copy(
            fingerprint.value
        )
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_record_representation_fingerprint(
            self._handle,
            native_id,
            _utf8(fingerprint.algorithm, "fingerprint algorithm"),
            _unsigned(fingerprint.version, 16, "fingerprint version"),
            value,
            len(fingerprint.value),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def record_dependency_set(
        self,
        representation_id: RepresentationId,
        dependencies: tuple[Dependency, ...],
    ) -> None:
        """Stage replacement of one complete ordered dependency observation."""

        self._require_open()
        kinds = tuple(_utf8(value.kind, "dependency kind") for value in dependencies)
        authored_references = tuple(
            _utf8(value.authored_reference, "authored dependency reference")
            for value in dependencies
        )
        array_type = NativeDependency * len(dependencies)
        native_dependencies = array_type(
            *(
                NativeDependency(
                    int(value.source_resource_id is not None),
                    (
                        _native_resource_id(value.source_resource_id)
                        if value.source_resource_id is not None
                        else _abi.ResourceId()
                    ),
                    kinds[index],
                    _native_object_reference(value.target),
                    int(value.resolved_representation_id is not None),
                    (
                        _native_representation_id(value.resolved_representation_id)
                        if value.resolved_representation_id is not None
                        else _abi.RepresentationId()
                    ),
                    int(value.required),
                    authored_references[index],
                )
                for index, value in enumerate(dependencies)
            )
        )
        native_id = _native_representation_id(representation_id)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_record_dependency_set(
            self._handle,
            native_id,
            native_dependencies if dependencies else None,
            len(native_dependencies),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def add_external_identifier(
        self, target: ObjectReference, identifier: ExternalIdentifier
    ) -> None:
        """Stage an external identifier attachment."""

        self._mutate_external_identifier(False, target, identifier)

    def remove_external_identifier(
        self, target: ObjectReference, identifier: ExternalIdentifier
    ) -> None:
        """Stage removal of one exact external identifier attachment."""

        self._mutate_external_identifier(True, target, identifier)

    def add_metadata(
        self,
        target: ObjectReference,
        property: MetadataKey,
        value: MetadataInput,
    ) -> None:
        """Stage one assertion using native scalars or an explicit metadata kind."""

        self._require_open()
        property = _metadata_key(property)
        native_target = _native_object_reference(target)
        native_value = _metadata_input(self._native, value)
        try:
            error = ctypes.POINTER(Error)()
            status = self._native.lib.pp_transaction_add_metadata_value(
                self._handle,
                ctypes.byref(native_target),
                _utf8(property.vocabulary, "metadata vocabulary"),
                _utf8(property.property, "metadata property"),
                native_value,
                ctypes.byref(error),
            )
            self._native.check(status, error)
        finally:
            self._native.lib.pp_metadata_input_release(native_value)

    def request_job(self, request: JobRequest) -> JobId:
        """Stage one durable requested job."""

        self._require_open()
        input_type = _abi.RepresentationId * len(request.inputs)
        inputs = input_type(
            *(_native_representation_id(value) for value in request.inputs)
        )
        output_asset_id = _native_asset_id(request.output_asset_id)
        job_id = _abi.JobId()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_request_job(
            self._handle,
            _utf8(request.kind, "job kind"),
            inputs if request.inputs else None,
            len(request.inputs),
            output_asset_id,
            _native_representation_kind(request.output_representation_kind),
            _optional_text(request.target_root),
            ctypes.byref(job_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return JobId(_uuid(job_id))

    def claim_job_lease(
        self,
        job_id: JobId,
        tool: ToolIdentity,
        duration: timedelta,
        *,
        agent: AgentIdentity | None = None,
    ) -> JobLease:
        """Claim work with library-controlled time; ownership activates on commit."""
        self._require_open()
        micros = _lease_duration_micros(duration)
        identifier = agent.identifier if agent else None
        handle = ctypes.POINTER(_abi.JobLease)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_claim_job_lease(
            self._handle,
            _native_job_id(job_id),
            _utf8(tool.name, "tool name"),
            _optional_text(tool.version),
            _optional_text(tool.uri),
            _optional_text(agent.name if agent else None),
            _optional_text(identifier.scheme if identifier else None),
            _optional_text(identifier.value if identifier else None),
            _optional_text(identifier.qualifier if identifier else None),
            micros,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return JobLease(self._native, handle)

    def renew_job_lease(self, lease: JobLease, duration: timedelta) -> None:
        """Stage renewal from current authority time for a checked duration."""
        self._require_open()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_renew_job_lease(
            self._handle,
            _job_lease_handle(lease, self._native),
            _lease_duration_micros(duration),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def release_job_lease(self, lease: JobLease) -> None:
        """Stage explicit release of a current unexpired claim."""
        self._require_open()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_release_job_lease(
            self._handle,
            _job_lease_handle(lease, self._native),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def complete_job_lease(
        self,
        lease: JobLease,
        output_representation_id: RepresentationId,
        activity_id: ActivityId,
    ) -> None:
        """Bind staged output and provenance into one guarded atomic completion."""
        self._require_open()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_complete_job_lease(
            self._handle,
            _job_lease_handle(lease, self._native),
            _native_representation_id(output_representation_id),
            _native_activity_id(activity_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def fail_job_lease(self, lease: JobLease, diagnostic: str) -> None:
        """Stage claimant failure through current unexpired ownership."""
        self._require_open()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_fail_job_lease(
            self._handle,
            _job_lease_handle(lease, self._native),
            _utf8(diagnostic, "job failure diagnostic"),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def cancel_job(self, job_id: JobId) -> None:
        """Stage administrative cancellation of a requested or claimed job."""

        self._require_open()
        native_job_id = _native_job_id(job_id)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_cancel_job(
            self._handle,
            native_job_id,
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def create_activity(self, spec: ActivitySpec) -> ActivityId:
        """Stage an activity describing existing outputs and its actual parameters."""
        self._require_open()
        outputs = _native_activity_edges(spec.outputs)
        activity_id = _abi.ActivityId()
        error = ctypes.POINTER(Error)()
        with _NativeActivityRecipe(self._native, spec) as recipe:
            status = self._native.lib.pp_transaction_create_activity(
                self._handle,
                ctypes.byref(recipe.value),
                outputs,
                len(outputs),
                ctypes.byref(activity_id),
                ctypes.byref(error),
            )
            self._native.check(status, error)
        return ActivityId(_uuid(activity_id))

    def register_outputs(
        self, outputs: Iterable[OutputSpec], spec: ActivitySpec
    ) -> RegisteredOutputs:
        """Stage produced sources, one actual activity and its parameters together.

        Each output is ``(asset_or_None, kind, source[, role[, display_name]])``.
        None explicitly creates a new original asset. The recipe's outputs must
        be empty; its inputs take representation IDs or ``(id, role)`` pairs.
        No job or dependency observation is created. Commit stays explicit.
        """
        self._require_open()
        if spec.outputs:
            raise ValueError(
                "register_outputs derives activity outputs from its sources"
            )
        values = _bounded_tuple(outputs, 100_000, "output sources")
        if not values:
            raise ValueError("register_outputs requires at least one output")
        activity_id = _abi.ActivityId()
        error = ctypes.POINTER(Error)()
        result = (_abi.OutputIdentity * len(values))()
        with ExitStack() as stack:
            recipe = stack.enter_context(_NativeActivityRecipe(self._native, spec))
            native_outputs = []
            for value in values:
                if not isinstance(value, tuple) or not 3 <= len(value) <= 5:
                    raise TypeError(
                        "output requires (asset_or_None, kind, source[, role[, name]])"
                    )
                asset, kind, source = value[:3]
                native_asset = _native_asset_id(asset) if asset is not None else None
                path = None
                native_source = None
                if isinstance(source, (str, os.PathLike)):
                    path = _path_bytes(source)
                else:
                    native_source = stack.enter_context(
                        _NativeMediaSource(self._native, source)
                    ).handle
                native_outputs.append(
                    _abi.OutputSource(
                        ctypes.pointer(native_asset)
                        if native_asset is not None
                        else None,
                        _native_representation_kind(kind),
                        path,
                        native_source,
                        _optional_text(value[3] if len(value) >= 4 else None),
                        _optional_text(value[4] if len(value) >= 5 else None),
                    )
                )
            array = (_abi.OutputSource * len(native_outputs))(*native_outputs)
            status = self._native.lib.pp_transaction_register_outputs(
                self._handle,
                array,
                len(array),
                ctypes.byref(recipe.value),
                result,
                ctypes.byref(activity_id),
                ctypes.byref(error),
            )
            self._native.check(status, error)
        return RegisteredOutputs(
            ActivityId(_uuid(activity_id)),
            tuple(
                ImportedMedia(
                    AssetId(_uuid(value.asset_id)),
                    RepresentationId(_uuid(value.representation_id)),
                )
                for value in result
            ),
        )

    def remove_metadata_property(
        self, target: ObjectReference, property: MetadataKey
    ) -> None:
        """Remove every assertion for a target/property in a read-bound edit.

        An unbased transaction rejects before staging and remains open.
        Intervening property changes, including appends, conflict at commit.
        """

        self._require_open()
        property = _metadata_key(property)
        native_target = _native_object_reference(target)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_remove_metadata_property(
            self._handle,
            ctypes.byref(native_target),
            _utf8(property.vocabulary, "metadata vocabulary"),
            _utf8(property.property, "metadata property"),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def prepare_proposal(
        self,
        client_id: ClientId,
        request_id: RequestId,
        base: ProtocolBase | None = None,
    ) -> Proposal:
        """Close staged intent without committing; retain its original identities.

        Failure leaves the edit open. A mirror-derived decision needs its full
        protocol base. Only a fresh successful submission delivers owning leases.
        """
        self._require_open()
        native_base = None if base is None else _native_protocol_base(base)
        client = _abi.ClientId()
        client.bytes = _native_uuid(client_id).bytes
        request = _abi.RequestId()
        request.bytes = _native_uuid(request_id).bytes
        proposal = ctypes.POINTER(_abi.Proposal)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_prepare_proposal(
            self._handle,
            client,
            request,
            None if native_base is None else ctypes.byref(native_base),
            ctypes.byref(proposal),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        self._finished = True
        return Proposal(self._native, proposal)

    def commit(self) -> CommitReceipt:
        """Persist staged mutations and return this commit's atomic receipt.

        The attempt is terminal, including failure. A receipt with no revision
        creates no journal entry and does not identify another writer's head.
        """

        self._require_open()
        receipt = _abi.CommitReceipt()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_commit(
            self._handle, ctypes.byref(receipt), ctypes.byref(error)
        )
        self._finished = True
        self._native.check(status, error)
        revision = None
        if receipt.outcome == _abi.PP_COMMIT_REVISION_CREATED:
            if receipt.revision_sequence == 0:
                raise InternalError(
                    _abi.PP_ERROR_INTERNAL, "commit receipt has no revision sequence"
                )
            revision = CommittedRevision(
                RevisionId(_uuid(receipt.revision_id)), int(receipt.revision_sequence)
            )
        elif receipt.outcome != _abi.PP_COMMIT_NO_CHANGE:
            raise UnsupportedError(
                _abi.PP_ERROR_UNSUPPORTED,
                "unknown commit outcome; the edit is terminal",
            )
        return CommitReceipt(ProductionId(_uuid(receipt.production_id)), revision)

    def rollback(self) -> None:
        """Discard every staged mutation."""

        self._finish(self._native.lib.pp_transaction_rollback)

    def close(self) -> None:
        """Release the handle, implicitly rolling back if still open."""

        self._finalizer()
        self._handle = ctypes.POINTER(NativeTransaction)()

    def __enter__(self) -> Self:
        self._require_open()
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        try:
            if not self._finished and self._finalizer.alive:
                self.rollback()
        finally:
            self.close()

    def _finish(self, function: Callable[..., int]) -> None:
        self._require_open()
        error = ctypes.POINTER(Error)()
        status = function(self._handle, ctypes.byref(error))
        self._finished = True
        self._native.check(status, error)

    def _mutate_external_identifier(
        self,
        remove: bool,
        target: ObjectReference,
        identifier: ExternalIdentifier,
    ) -> None:
        self._require_open()
        native_target = _native_object_reference(target)
        error = ctypes.POINTER(Error)()
        function = (
            self._native.lib.pp_transaction_remove_external_identifier
            if remove
            else self._native.lib.pp_transaction_add_external_identifier
        )
        status = function(
            self._handle,
            ctypes.byref(native_target),
            _utf8(identifier.scheme, "identifier scheme"),
            _utf8(identifier.value, "identifier value"),
            None
            if identifier.qualifier is None
            else _utf8(identifier.qualifier, "identifier qualifier"),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def _require_open(self) -> None:
        if not self._finalizer.alive:
            raise InvalidArgumentError(
                _abi.PP_ERROR_INVALID_ARGUMENT, "transaction is closed"
            )
        if self._finished:
            raise InvalidArgumentError(
                _abi.PP_ERROR_INVALID_ARGUMENT, "transaction is already finished"
            )


class ProposalBuilder:
    """Caller-serialized staging for typed local commands and exact prepared IDs.

    Preparation closes staging without a commit. Closing/discarding the builder
    never submits. If staging fails, discard it before deciding on a new intent.
    """

    def __init__(
        self,
        transaction: Edit,
        client: ClientId,
        request: RequestId,
        base: ProtocolBase | None,
    ) -> None:
        self._transaction = transaction
        self._client = client
        self._request = request
        self._base = base
        self._staged = 0

    def stage(self, commands: Iterable[ProposalCommand]) -> list[StagedValue]:
        """Stage an ordered collection; copy checked values and return prepared IDs."""
        self._transaction._require_open()
        bounded = list(islice(commands, 1001 - self._staged))
        if len(bounded) + self._staged > 1000:
            raise ExchangeError(
                _abi.PP_ERROR_LIMIT_EXCEEDED, "too many proposal commands"
            )
        result = []
        for command in bounded:
            result.append(_stage(self._transaction, command))
            self._staged += 1
        return result

    def prepare(self) -> Proposal:
        """Freeze checked ordered intent; failure leaves native staging open."""
        return self._transaction.prepare_proposal(
            self._client, self._request, self._base
        )

    def close(self) -> None:
        """Discard local staging only; repeated calls are harmless."""
        self._transaction.close()

    def __enter__(self) -> Self:
        self._transaction._require_open()
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


class Proposal:
    """Own immutable prepared intent; closing never submits or commits it."""

    def __init__(self, native: NativeLibrary, handle: _Pointer[_abi.Proposal]) -> None:
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native preparation returned no proposal"
            )
        self._native = native
        self._handle = handle
        self._finalizer = weakref.finalize(self, native.lib.pp_proposal_release, handle)

    @classmethod
    def parse(
        cls,
        document: bytes,
        *,
        library_path: str | os.PathLike[str] | None = None,
    ) -> Self:
        """Read bounded public transport; ordinary commands use typed staging."""
        if not isinstance(document, bytes):
            raise TypeError("proposal document must be bytes")
        if len(document) > 64 << 20:
            raise ExchangeError(
                _abi.PP_ERROR_LIMIT_EXCEEDED, "proposal document exceeds 64 MiB"
            )
        native = NativeLibrary(library_path)
        buffer = (ctypes.c_uint8 * len(document)).from_buffer_copy(document)
        handle = ctypes.POINTER(_abi.Proposal)()
        error = ctypes.POINTER(Error)()
        status = native.lib.pp_proposal_parse(
            buffer, len(document), ctypes.byref(handle), ctypes.byref(error)
        )
        native.check(status, error)
        return cls(native, handle)

    @property
    def identity(self) -> RequestIdentity:
        """Copy the complete public recovery scope, independently of this handle."""
        self._require_open()
        value = _abi.RequestIdentity()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_proposal_identity(
            self._handle, ctypes.byref(value), ctypes.byref(error)
        )
        self._native.check(status, error)
        return RequestIdentity(
            ProductionId(_uuid(value.production_id)),
            HistoryId(UUID(bytes=bytes(value.history_id.bytes))),
            ClientId(UUID(bytes=bytes(value.client_id.bytes))),
            RequestId(UUID(bytes=bytes(value.request_id.bytes))),
        )

    @property
    def document(self) -> bytes:
        """Copy canonical portable bytes containing no private capability."""
        self._require_open()
        data = ctypes.POINTER(ctypes.c_uint8)()
        length = ctypes.c_uint64()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_proposal_document(
            self._handle, ctypes.byref(data), ctypes.byref(length), ctypes.byref(error)
        )
        self._native.check(status, error)
        return ctypes.string_at(data, length.value)

    def close(self) -> None:
        """Release local immutable intent; repeated calls are harmless."""
        self._finalizer()
        self._handle = ctypes.POINTER(_abi.Proposal)()

    def _require_open(self) -> None:
        if not self._finalizer.alive:
            raise InvalidArgumentError(
                _abi.PP_ERROR_INVALID_ARGUMENT, "proposal is closed"
            )

    def _for_native(self, native: NativeLibrary) -> _Pointer[_abi.Proposal]:
        self._require_open()
        if self._native.lib._handle != native.lib._handle:
            raise ValueError("proposal belongs to another native library")
        return self._handle

    def __copy__(self) -> Self:
        raise TypeError("an owning proposal cannot be copied; transport its document")

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        raise TypeError("an owning proposal cannot be copied; transport its document")

    def __enter__(self) -> Self:
        self._require_open()
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


class JobLease:
    """Owns a worker capability; closing/freeing it never writes to storage.

    Pending ownership belongs to its claiming edit. Successful commit activates
    it; rollback closes it. Every worker mutation still validates the store.
    """

    def __init__(self, native: NativeLibrary, handle: _Pointer[_abi.JobLease]) -> None:
        if not handle:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native claim returned no lease handle"
            )
        self._native = native
        self._handle = handle
        self._finalizer = weakref.finalize(self, native.lib.pp_job_lease_free, handle)

    def _info(self) -> tuple[ProductionId, JobId, JobLeaseStatus]:
        self._require_open()
        production = _abi.ProductionId()
        job = _abi.JobId()
        state = ctypes.c_uint32()
        expiry = ctypes.c_int64()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_job_lease_get(
            self._handle,
            ctypes.byref(production),
            ctypes.byref(job),
            ctypes.byref(state),
            ctypes.byref(expiry),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if state.value not in (
            _abi.PP_JOB_LEASE_ACTIVE,
            _abi.PP_JOB_LEASE_PENDING,
            _abi.PP_JOB_LEASE_CLOSED,
        ):
            raise UnsupportedError(
                _abi.PP_ERROR_UNSUPPORTED, "native lease has an unknown state"
            )
        ownership: JobLeaseStatus
        if state.value == _abi.PP_JOB_LEASE_ACTIVE:
            ownership = ActiveJobLease(expiry.value)
        elif state.value == _abi.PP_JOB_LEASE_PENDING and expiry.value == 0:
            ownership = PendingJobLease()
        elif state.value == _abi.PP_JOB_LEASE_CLOSED and expiry.value == 0:
            ownership = ClosedJobLease()
        else:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native lease has an unknown or inconsistent state",
            )
        return ProductionId(_uuid(production)), JobId(_uuid(job)), ownership

    @property
    def production_id(self) -> ProductionId:
        """Return production scope without credentials."""
        return self._info()[0]

    @property
    def job_id(self) -> JobId:
        """Return the claimed job without credentials."""
        return self._info()[1]

    @property
    def state(self) -> JobLeaseStatus:
        """Return local pending, active-with-expiry or closed ownership."""
        return self._info()[2]

    def export_token(self) -> str:
        """Explicitly export a secret for a protected file/pipe, never logs."""
        self._require_open()
        token = ctypes.c_char_p()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_job_lease_export_token(
            self._handle,
            ctypes.byref(token),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        try:
            value = token.value
            if value is None:
                raise InternalError(
                    _abi.PP_ERROR_INTERNAL, "native lease export returned no token"
                )
            return value.decode("ascii")
        finally:
            self._native.lib.pp_string_release(token)

    def close(self) -> None:
        """Free local ownership only; repeated calls are harmless."""
        self._finalizer()
        self._handle = ctypes.POINTER(_abi.JobLease)()

    def _require_open(self) -> None:
        if not self._finalizer.alive:
            raise InvalidArgumentError(
                _abi.PP_ERROR_INVALID_ARGUMENT, "job lease handle is closed"
            )

    def _for_native(self, native: NativeLibrary) -> _Pointer[_abi.JobLease]:
        self._require_open()
        if self._native.lib._handle != native.lib._handle:
            raise ValueError("job lease belongs to another native library")
        return self._handle

    def __enter__(self) -> Self:
        self._require_open()
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self.close()


def _lease_duration_micros(duration: timedelta) -> int:
    if not isinstance(duration, timedelta):
        raise TypeError("job lease duration must be datetime.timedelta")
    # Avoid float total_seconds(), which can lose microsecond precision.
    micros = (
        duration.days * 86_400 + duration.seconds
    ) * 1_000_000 + duration.microseconds
    if not 1 <= micros <= 86_400_000_000:
        raise ValueError("job lease duration must be between 1 us and 24 h")
    return micros


def _job_lease_handle(
    lease: JobLease, native: NativeLibrary
) -> _Pointer[_abi.JobLease]:
    if not isinstance(lease, JobLease):
        raise TypeError("worker transition requires a JobLease")
    return lease._for_native(native)


def fingerprint_file(
    path: str | os.PathLike[str],
    *,
    library_path: str | os.PathLike[str] | None = None,
) -> Fingerprint:
    """Compute the fingerprint import records for a regular file."""

    native = NativeLibrary(library_path)
    handle = ctypes.POINTER(NativeFingerprint)()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_fingerprint_file(
        _path_bytes(path), ctypes.byref(handle), ctypes.byref(error)
    )
    native.check(status, error)
    try:
        algorithm = ctypes.c_char_p()
        version = ctypes.c_uint16()
        value = ctypes.POINTER(ctypes.c_uint8)()
        length = ctypes.c_uint64()
        error = ctypes.POINTER(Error)()
        status = native.lib.pp_fingerprint_get(
            handle,
            ctypes.byref(algorithm),
            ctypes.byref(version),
            ctypes.byref(value),
            ctypes.byref(length),
            ctypes.byref(error),
        )
        native.check(status, error)
        return Fingerprint(
            _decode_required(algorithm.value, "fingerprint algorithm"),
            version.value,
            ctypes.string_at(value, length.value),
        )
    finally:
        native.lib.pp_fingerprint_release(handle)


class CancelToken:
    """A cancellation flag shared with running operations.

    ``cancel()`` may be called from any thread, including while
    ``Production.resolve`` runs on another; that call then raises
    ``CancelledError``. Cancellation cannot be undone.
    """

    def __init__(self, *, library_path: str | os.PathLike[str] | None = None) -> None:
        native = NativeLibrary(library_path)
        handle = ctypes.POINTER(NativeCancelToken)()
        error = ctypes.POINTER(Error)()
        status = native.lib.pp_cancel_token_create(
            ctypes.byref(handle), ctypes.byref(error)
        )
        native.check(status, error)
        self._native = native
        self.handle = handle
        self._finalizer = weakref.finalize(
            self, native.lib.pp_cancel_token_release, handle
        )

    def cancel(self) -> None:
        """Request cancellation of every operation observing this token."""

        self._native.lib.pp_cancel_token_cancel(self.handle)


class _NativeMediaSource:
    """Owned native media source, released when the ``with`` block ends."""

    def __init__(
        self,
        native: NativeLibrary,
        source: MediaSource,
    ) -> None:
        self._native = native
        self.handle = ctypes.POINTER(NativeMediaSource)()
        error = ctypes.POINTER(Error)()
        if isinstance(source, ImageSequenceSource):
            if not isinstance(source.rate, Fraction):
                raise TypeError("sequence rate must be fractions.Fraction")
            missing_type = ctypes.c_int64 * len(source.missing_frames)
            missing_frames = missing_type(
                *(
                    _signed(frame, 64, "missing frame")
                    for frame in source.missing_frames
                )
            )
            status = native.lib.pp_media_source_create_image_sequence(
                _path_bytes(source.directory),
                _native_naming(source.naming),
                _signed(source.start, 64, "start frame"),
                _signed(source.end, 64, "end frame"),
                _unsigned(source.step, 32, "frame step"),
                _unsigned(source.rate.numerator, 32, "rate numerator"),
                _unsigned(source.rate.denominator, 32, "rate denominator"),
                missing_frames,
                len(missing_frames),
                ctypes.byref(self.handle),
                ctypes.byref(error),
            )
        elif isinstance(source, (OrderedPartsSource, PackageSource)):
            members = (
                source.parts
                if isinstance(source, OrderedPartsSource)
                else source.members
            )
            function = (
                native.lib.pp_media_source_create_ordered_parts
                if isinstance(source, OrderedPartsSource)
                else native.lib.pp_media_source_create_package
            )
            paths = tuple(_path_bytes(member.path) for member in members)
            roles = tuple(_utf8(member.role, "resource role") for member in members)
            array_type = NativeFileResourceInput * len(members)
            native_members = array_type(
                *(
                    NativeFileResourceInput(
                        paths[index], roles[index], int(member.required)
                    )
                    for index, member in enumerate(members)
                )
            )
            status = function(
                native_members,
                len(native_members),
                ctypes.byref(self.handle),
                ctypes.byref(error),
            )
        else:
            raise TypeError(
                "compound media source must describe a sequence, parts or package"
            )
        native.check(status, error)

    def __enter__(self) -> Self:
        return self

    def __exit__(self, *_: object) -> None:
        self._native.lib.pp_media_source_release(self.handle)
        self.handle = ctypes.POINTER(NativeMediaSource)()


class _ResolutionOptions:
    """Owned native resolution options for one resolve call."""

    def __init__(
        self,
        native: NativeLibrary,
        root_mappings: Mapping[str, str | os.PathLike[str]] | None,
        search_directories: Iterable[str | os.PathLike[str]],
        verification: VerificationMode,
        max_depth: int,
        max_entries_per_directory: int,
        cancel_token: CancelToken | None,
    ) -> None:
        mappings = tuple(sorted((root_mappings or {}).items()))
        native_mappings = (_abi.RootMapping * len(mappings))(
            *(
                _abi.RootMapping(_utf8(name, "root name"), _path_bytes(path))
                for name, path in mappings
            )
        )
        directories = tuple(_path_bytes(path) for path in search_directories)
        native_directories = (ctypes.c_char_p * len(directories))(*directories)
        config = _abi.ResolutionConfig(
            native_mappings,
            len(native_mappings),
            native_directories,
            len(directories),
            _VERIFICATION_MODES[verification],
            max_depth,
            max_entries_per_directory,
            None if cancel_token is None else cancel_token.handle,
        )
        handle = ctypes.POINTER(NativeResolutionOptions)()
        error = ctypes.POINTER(Error)()
        status = native.lib.pp_resolution_options_create(
            ctypes.byref(config), ctypes.byref(handle), ctypes.byref(error)
        )
        native.check(status, error)
        self.handle = handle
        self._finalizer = weakref.finalize(
            self, native.lib.pp_resolution_options_release, handle
        )

    def close(self) -> None:
        self._finalizer()
        self.handle = ctypes.POINTER(NativeResolutionOptions)()


_VERIFICATION_MODES = {
    VerificationMode.PRESENCE: _abi.PP_VERIFY_PRESENCE,
    VerificationMode.CONTENT: _abi.PP_VERIFY_CONTENT,
}


def file_locator(
    path: str | os.PathLike[str],
    *,
    library_path: str | os.PathLike[str] | None = None,
) -> str:
    """Return the canonical ``file:`` locator URI import records for a path.

    Compare locators only through URIs returned here or read from a production.
    """

    return _file_locator(NativeLibrary(library_path), path)


def _file_locator(native: NativeLibrary, path: str | os.PathLike[str]) -> str:
    uri = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_file_path_to_locator(
        _path_bytes(path), ctypes.byref(uri), ctypes.byref(error)
    )
    native.check(status, error)
    try:
        return _decode_required(uri.value, "locator URI")
    finally:
        native.lib.pp_string_release(uri)


def locator_file_path(
    uri: str, *, library_path: str | os.PathLike[str] | None = None
) -> Path:
    """Convert a local ``file:`` locator URI to a path, which need not exist."""

    native = NativeLibrary(library_path)
    path = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_locator_to_file_path(
        _utf8(uri, "locator URI"), ctypes.byref(path), ctypes.byref(error)
    )
    native.check(status, error)
    try:
        return Path(_decode_required(path.value, "locator path"))
    finally:
        native.lib.pp_string_release(path)


_CONTENT_VERIFICATIONS = {
    _abi.PP_CONTENT_MATCHES: ContentVerification.MATCHES,
    _abi.PP_CONTENT_DIFFERS: ContentVerification.DIFFERS,
    _abi.PP_CONTENT_NOT_COMPARABLE: ContentVerification.NOT_COMPARABLE,
}
_CONTENT_OBSERVATION_OUTCOMES = {
    _abi.PP_OBSERVATION_UNCHANGED: ContentObservationOutcome.UNCHANGED,
    _abi.PP_OBSERVATION_CHANGED: ContentObservationOutcome.CHANGED,
    _abi.PP_OBSERVATION_FIRST: ContentObservationOutcome.FIRST,
}


def _path_bytes(path: str | os.PathLike[str]) -> bytes:
    return _utf8(str(Path(path)), "path")


def _optional_text(value: str | None) -> bytes | None:
    return None if value is None else _utf8(value, "text")


def _native_naming(
    naming: SequenceNaming | None,
) -> _Pointer[_abi.SequenceNaming] | None:
    """Return a borrowed native naming, or ``None`` for no naming."""

    if naming is None:
        return None
    return ctypes.pointer(
        _abi.SequenceNaming(
            _utf8(naming.prefix, "sequence naming prefix"),
            _utf8(naming.suffix, "sequence naming suffix"),
            _unsigned(naming.padding, 8, "sequence padding"),
        )
    )


def _sequence_naming(
    present: ctypes.c_uint8, naming: _abi.SequenceNaming
) -> SequenceNaming | None:
    if not present.value:
        return None
    return SequenceNaming(
        _decode_required(naming.prefix, "sequence naming prefix"),
        _decode_required(naming.suffix, "sequence naming suffix"),
        int(naming.padding),
    )


def _utf8(value: str, label: str) -> bytes:
    encoded = value.encode("utf-8")
    if b"\0" in encoded:
        raise ValueError(f"{label} must not contain NUL")
    return encoded


def _uuid(
    value: Uuid
    | _abi.ResourceId
    | _abi.RepresentationId
    | _abi.ActivityId
    | _abi.AssetId
    | _abi.JobId
    | _abi.LocatorId
    | _abi.MediaRootId
    | _abi.ProductionId
    | _abi.RevisionId
    | _abi.TransactionId,
) -> UUID:
    return UUID(bytes=bytes(value.bytes))


def _native_asset_id(value: AssetId) -> _abi.AssetId:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = _abi.AssetId()
    native.bytes[:] = value.bytes
    return native


def _native_media_root_id(value: MediaRootId) -> _abi.MediaRootId:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = _abi.MediaRootId()
    native.bytes[:] = value.bytes
    return native


def _native_locator_id(value: LocatorId) -> _abi.LocatorId:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = _abi.LocatorId()
    native.bytes[:] = value.bytes
    return native


def _native_job_id(value: JobId) -> _abi.JobId:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = _abi.JobId()
    native.bytes[:] = value.bytes
    return native


def _native_activity_id(value: ActivityId) -> _abi.ActivityId:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = _abi.ActivityId()
    native.bytes[:] = value.bytes
    return native


def _native_representation_id(value: RepresentationId) -> _abi.RepresentationId:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = _abi.RepresentationId()
    native.bytes[:] = value.bytes
    return native


def _native_resource_id(value: ResourceId) -> _abi.ResourceId:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = _abi.ResourceId()
    native.bytes[:] = value.bytes
    return native


def _native_production_id(value: ProductionId) -> _abi.ProductionId:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = _abi.ProductionId()
    native.bytes[:] = value.bytes
    return native


def _native_revision_id(value: RevisionId) -> _abi.RevisionId:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = _abi.RevisionId()
    native.bytes[:] = value.bytes
    return native


def _native_uuid(value: UUID) -> Uuid:
    if not isinstance(value, UUID):
        raise TypeError("identity must be a uuid.UUID")
    native = Uuid()
    native.bytes[:] = value.bytes
    return native


def _bounded_tuple(
    values: Iterable[OutputSpec], limit: int, label: str
) -> tuple[OutputSpec, ...]:
    copied = tuple(islice(values, limit + 1))
    if len(copied) > limit:
        raise ValueError(f"{label} exceeds {limit} items")
    return copied


class _NativeActivityRecipe:
    """Borrowed recipe retaining owned parameter inputs only for the native call."""

    def __init__(self, native: NativeLibrary, spec: ActivitySpec) -> None:
        self._native = native
        self._values: list[_Pointer[NativeMetadataInput]] = []
        try:
            inputs = _native_activity_edges(spec.inputs)
            started = _optional_i64(spec.started_at_unix_micros)
            finished = _optional_i64(spec.finished_at_unix_micros)
            tool, agent = spec.tool, spec.agent
            if agent is not None and agent.name is None and agent.identifier is None:
                raise ValueError("an agent needs a name or identifier")
            identifier = agent.identifier if agent is not None else None
            parameters = []
            supplied = (
                cast(Mapping[MetadataKey, MetadataInput], spec.parameters).items()
                if isinstance(spec.parameters, Mapping)
                else spec.parameters
            )
            for index, (key, value) in enumerate(supplied):
                if index == 100_000:
                    raise ValueError("too many activity parameters")
                property = _metadata_key(key)
                handle = _metadata_input(native, value)
                self._values.append(handle)
                parameters.append(
                    _abi.ActivityParameter(
                        _utf8(property.vocabulary, "parameter vocabulary"),
                        _utf8(property.property, "parameter property"),
                        handle,
                    )
                )
            array = (_abi.ActivityParameter * len(parameters))(*parameters)
            self.value = _abi.ActivityRecipe(
                _utf8(spec.kind, "activity kind"),
                inputs,
                len(inputs),
                ctypes.pointer(started) if started is not None else None,
                ctypes.pointer(finished) if finished is not None else None,
                _utf8(tool.name, "tool name") if tool else None,
                _optional_text(tool.version if tool else None),
                _optional_text(tool.uri if tool else None),
                _optional_text(agent.name if agent else None),
                _optional_text(identifier.scheme if identifier else None),
                _optional_text(identifier.value if identifier else None),
                _optional_text(identifier.qualifier if identifier else None),
                array,
                len(array),
            )
        except BaseException:
            self.close()
            raise

    def close(self) -> None:
        for value in self._values:
            self._native.lib.pp_metadata_input_release(value)
        self._values = []

    def __enter__(self) -> Self:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


def _native_activity_edges(
    edges: tuple[ActivityEdgeInput, ...],
) -> ctypes.Array[NativeActivityEdge]:
    if len(edges) > 100_000:
        raise ValueError("too many activity edges")
    values = []
    for edge in edges:
        if isinstance(edge, UUID):
            identity, role = edge, None
        elif isinstance(edge, tuple) and len(edge) == 2:
            identity, role = edge
        else:
            raise TypeError(
                "activity edges require representation IDs or (id, role) pairs"
            )
        values.append(
            NativeActivityEdge(
                _native_representation_id(identity), _optional_text(role)
            )
        )
    return (NativeActivityEdge * len(values))(*values)


def _optional_i64(value: int | None) -> ctypes.c_int64 | None:
    return None if value is None else ctypes.c_int64(_signed(value, 64, "timestamp"))


def _native_object_reference(value: ObjectReference) -> _abi.ObjectRef:
    native = _abi.ObjectRef()
    if isinstance(value, ProductionRef):
        native.kind = _abi.PP_OBJECT_PRODUCTION
    elif isinstance(value, AssetRef):
        native.kind = _abi.PP_OBJECT_ASSET
    elif isinstance(value, RepresentationRef):
        native.kind = _abi.PP_OBJECT_REPRESENTATION
    elif isinstance(value, ResourceRef):
        native.kind = _abi.PP_OBJECT_RESOURCE
    elif isinstance(value, ActivityRef):
        native.kind = _abi.PP_OBJECT_ACTIVITY
    elif isinstance(value, JobRef):
        native.kind = _abi.PP_OBJECT_JOB
    else:
        raise TypeError("unsupported object reference")
    native.id = _native_uuid(value.id)
    return native


def _native_representation_kind(value: RepresentationKind) -> int:
    return {
        RepresentationKind.ORIGINAL: _abi.PP_REPRESENTATION_ORIGINAL,
        RepresentationKind.PROXY: _abi.PP_REPRESENTATION_PROXY,
        RepresentationKind.OPTIMIZED: _abi.PP_REPRESENTATION_OPTIMIZED,
        RepresentationKind.DERIVED: _abi.PP_REPRESENTATION_DERIVED,
    }[value]


def _asset_at(native: NativeLibrary, assets: _Pointer[AssetSet], index: int) -> Asset:
    asset_id = _abi.AssetId()
    created_at = ctypes.c_int64()
    display_name = ctypes.c_char_p()
    import_source = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_asset_set_get(
        assets,
        index,
        ctypes.byref(asset_id),
        ctypes.byref(created_at),
        ctypes.byref(display_name),
        ctypes.byref(import_source),
        ctypes.byref(error),
    )
    native.check(status, error)
    return Asset(
        AssetId(_uuid(asset_id)),
        int(created_at.value),
        _decode_optional(display_name.value),
        _decode_optional(import_source.value),
    )


def _unsigned(value: int, bits: int, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an int")
    if not 0 <= value < 2**bits:
        raise InvalidArgumentError(
            _abi.PP_ERROR_INVALID_ARGUMENT,
            f"{name} is outside its unsigned {bits}-bit range",
        )
    return value


def _signed(value: int, bits: int, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an int")
    if not -(2 ** (bits - 1)) <= value < 2 ** (bits - 1):
        raise InvalidArgumentError(
            _abi.PP_ERROR_INVALID_ARGUMENT,
            f"{name} is outside its signed {bits}-bit range",
        )
    return value


def _page_limit(limit: int) -> int:
    _unsigned(limit, 32, "page limit")
    if not 1 <= limit <= 1000:
        raise InvalidArgumentError(
            _abi.PP_ERROR_INVALID_ARGUMENT, "page limit must be between 1 and 1000"
        )
    return limit


def _media_root_page(
    native: NativeLibrary, handle: _Pointer[MediaRootSet]
) -> QueryPage[MediaRoot]:
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native media-root query returned no result set"
        )
    try:
        count = native.lib.pp_media_root_set_count(handle)
        items = tuple(
            _media_root_at(native, handle, index) for index in range(int(count))
        )
        cursor = _decode_optional(native.lib.pp_media_root_set_next_cursor(handle))
        return QueryPage(items, cursor)
    finally:
        native.lib.pp_media_root_set_release(handle)


def _media_root_at(
    native: NativeLibrary, roots: _Pointer[MediaRootSet], index: int
) -> MediaRoot:
    root_id = _abi.MediaRootId()
    name = ctypes.c_char_p()
    label = ctypes.c_char_p()
    legacy_uri = ctypes.c_char_p()
    priority = ctypes.c_int32()
    enabled = ctypes.c_uint8()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_media_root_set_get(
        roots,
        index,
        ctypes.byref(root_id),
        ctypes.byref(name),
        ctypes.byref(label),
        ctypes.byref(legacy_uri),
        ctypes.byref(priority),
        ctypes.byref(enabled),
        ctypes.byref(error),
    )
    native.check(status, error)
    return MediaRoot(
        MediaRootId(_uuid(root_id)),
        _decode_required(name.value, "media root name"),
        _decode_optional(label.value),
        _decode_optional(legacy_uri.value),
        int(priority.value),
        bool(enabled.value),
    )


def _create_metadata_input(
    native: NativeLibrary, function: Callable[..., int], *arguments: object
) -> _Pointer[NativeMetadataInput]:
    handle = ctypes.POINTER(NativeMetadataInput)()
    error = ctypes.POINTER(Error)()
    status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
    native.check(status, error)
    if not handle:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native metadata input creation returned no handle"
        )
    return handle


def _metadata_key(key: MetadataKey) -> MetadataProperty:
    if isinstance(key, MetadataProperty):
        return key
    if (
        not isinstance(key, tuple)
        or len(key) != 2
        or not all(isinstance(part, str) for part in key)
    ):
        raise TypeError("metadata key must be a (vocabulary, property) string pair")
    return MetadataProperty(*key)


def _metadata_input(
    native: NativeLibrary, value: MetadataInput
) -> _Pointer[NativeMetadataInput]:
    if isinstance(value, str):
        value = MetadataString(value)
    elif isinstance(value, bool):
        value = MetadataBool(value)
    elif isinstance(value, int):
        value = MetadataI64(value)
    elif isinstance(value, bytes):
        value = MetadataBytes(value)
    elif isinstance(value, Fraction):
        value = MetadataRational(value.numerator, value.denominator)
    if isinstance(value, MetadataString):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_string,
            _utf8(value.value, "metadata text"),
            None,
        )
    if isinstance(value, MetadataLanguageString):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_string,
            _utf8(value.value, "metadata text"),
            _utf8(value.language, "metadata language"),
        )
    if isinstance(value, MetadataI64):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_i64,
            _signed(value.value, 64, "metadata integer"),
        )
    if isinstance(value, MetadataU64):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_u64,
            _unsigned(value.value, 64, "metadata integer"),
        )
    if isinstance(value, MetadataDecimal):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_decimal,
            _utf8(str(value.coefficient), "metadata decimal coefficient"),
            _unsigned(value.scale, 32, "metadata decimal scale"),
        )
    if isinstance(value, MetadataBool):
        return _create_metadata_input(
            native, native.lib.pp_metadata_input_create_bool, int(value.value)
        )
    if isinstance(value, MetadataTimestamp):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_timestamp,
            _signed(value.unix_micros, 64, "metadata timestamp"),
        )
    if isinstance(value, MetadataUri):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_uri,
            _utf8(value.value, "metadata URI"),
        )
    if isinstance(value, MetadataBytes):
        buffer = (ctypes.c_uint8 * len(value.value)).from_buffer_copy(value.value)
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_bytes,
            buffer,
            len(value.value),
        )
    if isinstance(value, MetadataRational):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_rational,
            _signed(value.numerator, 64, "metadata rational numerator"),
            _unsigned(value.denominator, 64, "metadata rational denominator"),
        )
    if isinstance(value, MetadataReference):
        target = _native_object_reference(value.target)
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_reference,
            ctypes.byref(target),
        )
    if isinstance(value, MetadataList):
        children: list[_Pointer[NativeMetadataInput]] = []
        try:
            children.extend(_metadata_input(native, item) for item in value.values)
            array_type = ctypes.POINTER(NativeMetadataInput) * len(children)
            items = array_type(*children)
            return _create_metadata_input(
                native,
                native.lib.pp_metadata_input_create_list,
                items,
                len(items),
            )
        finally:
            for child in children:
                native.lib.pp_metadata_input_release(child)
    if isinstance(value, MetadataStruct):
        children = []
        try:
            children.extend(
                _metadata_input(native, field.value) for field in value.fields
            )
            names_type = ctypes.c_char_p * len(value.fields)
            names = names_type(
                *(
                    _utf8(field.name, "metadata structure field name")
                    for field in value.fields
                )
            )
            values_type = ctypes.POINTER(NativeMetadataInput) * len(children)
            values = values_type(*children)
            return _create_metadata_input(
                native,
                native.lib.pp_metadata_input_create_struct,
                names,
                values,
                len(values),
            )
        finally:
            for child in children:
                native.lib.pp_metadata_input_release(child)
    raise TypeError("unsupported metadata value")


def _external_identifier_at(
    native: NativeLibrary,
    identifiers: _Pointer[ExternalIdentifierSet],
    index: int,
) -> ExternalIdentifier:
    scheme = ctypes.c_char_p()
    value = ctypes.c_char_p()
    qualifier = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_external_identifier_set_get(
        identifiers,
        index,
        ctypes.byref(scheme),
        ctypes.byref(value),
        ctypes.byref(qualifier),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ExternalIdentifier(
        _decode_required(scheme.value, "identifier scheme"),
        _decode_required(value.value, "identifier value"),
        _decode_optional(qualifier.value),
    )


def _object_reference_at(
    native: NativeLibrary,
    objects: _Pointer[ObjectRefSet],
    index: int,
) -> ObjectReference:
    value = _abi.ObjectRef()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_object_ref_set_get(
        objects, index, ctypes.byref(value), ctypes.byref(error)
    )
    native.check(status, error)
    return _object_reference(value)


def _dependency_at(
    native: NativeLibrary,
    dependencies: _Pointer[NativeDependencySet],
    index: int,
) -> Dependency:
    value = NativeDependency()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_dependency_set_get_dependency(
        dependencies, index, ctypes.byref(value), ctypes.byref(error)
    )
    native.check(status, error)
    target = _object_reference(value.target)
    if not isinstance(target, (AssetRef, RepresentationRef)):
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native dependency has an unsupported target"
        )
    return Dependency(
        kind=_decode_required(value.kind, "dependency kind"),
        target=target,
        authored_reference=_decode_required(
            value.authored_reference, "authored dependency reference"
        ),
        required=bool(value.required),
        source_resource_id=(
            ResourceId(_uuid(value.source_resource_id))
            if value.has_source_resource
            else None
        ),
        resolved_representation_id=(
            RepresentationId(_uuid(value.resolved_representation_id))
            if value.has_resolved_representation
            else None
        ),
    )


def _job_at(native: NativeLibrary, jobs: _Pointer[JobSet], index: int) -> Job:
    value = NativeJob()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_job_set_get(
        jobs, index, ctypes.byref(value), ctypes.byref(error)
    )
    native.check(status, error)
    inputs: list[RepresentationId] = []
    for input_index in range(int(value.input_count)):
        input_id = _abi.RepresentationId()
        input_error = ctypes.POINTER(Error)()
        input_status = native.lib.pp_job_set_get_input(
            jobs,
            index,
            input_index,
            ctypes.byref(input_id),
            ctypes.byref(input_error),
        )
        native.check(input_status, input_error)
        inputs.append(RepresentationId(_uuid(input_id)))

    state = _job_state(int(value.state))
    job_status: JobStatus = JobRequested()
    if state is JobState.CLAIMED:
        identifier = None
        scheme = _decode_optional(value.claim_agent_identifier_scheme)
        identifier_value = _decode_optional(value.claim_agent_identifier_value)
        if (scheme is None) != (identifier_value is None):
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native job claim returned an incomplete agent identifier",
            )
        if scheme is not None and identifier_value is not None:
            identifier = ExternalIdentifier(
                scheme,
                identifier_value,
                _decode_optional(value.claim_agent_identifier_qualifier),
            )
        agent_name = _decode_optional(value.claim_agent_name)
        agent = (
            AgentIdentity(agent_name, identifier)
            if agent_name is not None or identifier is not None
            else None
        )
        job_status = JobClaim(
            ToolIdentity(
                _decode_required(value.claim_tool_name, "job claim tool name"),
                _decode_optional(value.claim_tool_version),
                _decode_optional(value.claim_tool_uri),
            ),
            agent,
            int(value.claim_expires_at_unix_micros),
        )
    if state is JobState.SUCCEEDED:
        job_status = JobCompletion(
            ActivityId(_uuid(value.completion_activity_id)),
            RepresentationId(_uuid(value.completion_representation_id)),
        )
    if state is JobState.FAILED:
        job_status = JobFailure(
            _decode_required(value.failure_diagnostic, "job failure diagnostic")
        )
    elif state is JobState.CANCELLED:
        job_status = JobCancelled()
    return Job(
        id=JobId(_uuid(value.id)),
        kind=_decode_required(value.kind, "job kind"),
        inputs=tuple(inputs),
        output_asset_id=AssetId(_uuid(value.output_asset_id)),
        output_representation_kind=_representation_kind(
            int(value.output_representation_kind)
        ),
        target_root=_decode_optional(value.target_root),
        status=job_status,
    )


def _dependency_query_page(
    native: NativeLibrary, matches: _Pointer[DependencyQuerySet]
) -> QueryPage[DependencyMatch]:
    count = native.lib.pp_dependency_query_set_count(matches)
    items: list[DependencyMatch] = []
    for index in range(int(count)):
        value = NativeDependencyMatch()
        error = ctypes.POINTER(Error)()
        status = native.lib.pp_dependency_query_set_get(
            matches, index, ctypes.byref(value), ctypes.byref(error)
        )
        native.check(status, error)
        target = _object_reference(value.target)
        if not isinstance(target, (AssetRef, RepresentationRef)):
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native dependency query returned an invalid target",
            )
        items.append(DependencyMatch(target, int(value.depth)))
    return QueryPage(
        tuple(items),
        _decode_optional(native.lib.pp_dependency_query_set_next_cursor(matches)),
        bool(native.lib.pp_dependency_query_set_traversal_truncated(matches)),
    )


def _resource_match(reference: ObjectReference, _depth: int) -> ResourceId:
    if not isinstance(reference, ResourceRef):
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native resource query returned a non-resource"
        )
    return reference.id


def _representation_match(reference: ObjectReference, _depth: int) -> RepresentationId:
    if not isinstance(reference, RepresentationRef):
        raise InternalError(
            _abi.PP_ERROR_INTERNAL,
            "native representation query returned a non-representation",
        )
    return reference.id


def _object_match(reference: ObjectReference, _depth: int) -> ObjectReference:
    return reference


def _provenance_match(reference: ObjectReference, depth: int) -> ProvenanceMatch:
    if not isinstance(reference, RepresentationRef):
        raise InternalError(
            _abi.PP_ERROR_INTERNAL,
            "native provenance query returned a non-representation",
        )
    return ProvenanceMatch(reference.id, depth)


def _locator_match_at(
    native: NativeLibrary, locators: _Pointer[LocatorQuerySet], index: int
) -> LocatorMatch:
    locator_id = _abi.LocatorId()
    resource_id = _abi.ResourceId()
    uri = ctypes.c_char_p()
    availability = _abi.LocatorAvailability()
    has_last_seen = ctypes.c_uint8()
    last_seen = ctypes.c_int64()
    media_root = ctypes.c_char_p()
    has_naming = ctypes.c_uint8()
    naming = _abi.SequenceNaming()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_locator_query_set_get(
        locators,
        index,
        ctypes.byref(locator_id),
        ctypes.byref(resource_id),
        ctypes.byref(uri),
        ctypes.byref(availability),
        ctypes.byref(has_last_seen),
        ctypes.byref(last_seen),
        ctypes.byref(media_root),
        ctypes.byref(has_naming),
        ctypes.byref(naming),
        ctypes.byref(error),
    )
    native.check(status, error)
    return LocatorMatch(
        ResourceId(_uuid(resource_id)),
        Locator(
            LocatorId(_uuid(locator_id)),
            _decode_required(uri.value, "locator URI"),
            _locator_availability(int(availability.value)),
            int(last_seen.value) if has_last_seen.value else None,
            _sequence_naming(has_naming, naming),
        ),
        _decode_optional(media_root.value),
    )


def _known_media_match_at(
    native: NativeLibrary, matches: _Pointer[KnownMediaSet], index: int
) -> KnownMediaMatch:
    asset_id = _abi.AssetId()
    representation_id = _abi.RepresentationId()
    resource_id = _abi.ResourceId()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_known_media_set_get(
        matches,
        index,
        ctypes.byref(asset_id),
        ctypes.byref(representation_id),
        ctypes.byref(resource_id),
        ctypes.byref(error),
    )
    native.check(status, error)
    return KnownMediaMatch(
        AssetId(_uuid(asset_id)),
        RepresentationId(_uuid(representation_id)),
        ResourceId(_uuid(resource_id)),
    )


def _regeneration_plan_at(
    native: NativeLibrary,
    plans: _Pointer[RegenerationPlanSet],
    index: int,
) -> RegenerationJobPlan:
    artifact_id = _abi.RepresentationId()
    jobs = ctypes.POINTER(JobSet)()
    parameters = ctypes.POINTER(MetadataSet)()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_regeneration_plan_set_get(
        plans,
        index,
        ctypes.byref(artifact_id),
        ctypes.byref(jobs),
        ctypes.byref(parameters),
        ctypes.byref(error),
    )
    native.check(status, error)
    if not jobs or not parameters:
        if jobs:
            native.lib.pp_job_set_release(jobs)
        if parameters:
            native.lib.pp_metadata_set_release(parameters)
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native regeneration plan is incomplete"
        )
    try:
        if native.lib.pp_job_set_count(jobs) != 1:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native regeneration plan must contain one job"
            )
        job = _job_at(native, jobs, 0)
        assertions = tuple(
            _metadata_at(native, parameters, parameter_index)
            for parameter_index in range(
                int(native.lib.pp_metadata_set_count(parameters))
            )
        )
        if any(assertion.target != JobRef(job.id) for assertion in assertions):
            raise InternalError(
                _abi.PP_ERROR_INTERNAL,
                "native regeneration parameter target does not match its job",
            )
        return RegenerationJobPlan(
            RepresentationId(_uuid(artifact_id)), job, assertions
        )
    finally:
        native.lib.pp_metadata_set_release(parameters)
        native.lib.pp_job_set_release(jobs)


def _activity_at(
    native: NativeLibrary,
    activities: _Pointer[ActivitySet],
    index: int,
) -> Activity:
    activity_id = _abi.ActivityId()
    kind = ctypes.c_char_p()
    has_started_at = ctypes.c_uint8()
    started_at = ctypes.c_int64()
    has_finished_at = ctypes.c_uint8()
    finished_at = ctypes.c_int64()
    input_count = ctypes.c_uint64()
    output_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_activity_set_get(
        activities,
        index,
        ctypes.byref(activity_id),
        ctypes.byref(kind),
        ctypes.byref(has_started_at),
        ctypes.byref(started_at),
        ctypes.byref(has_finished_at),
        ctypes.byref(finished_at),
        ctypes.byref(input_count),
        ctypes.byref(output_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    return Activity(
        ActivityId(_uuid(activity_id)),
        _decode_required(kind.value, "activity kind"),
        int(started_at.value) if has_started_at.value else None,
        int(finished_at.value) if has_finished_at.value else None,
        _activity_tool(native, activities, index),
        _activity_agent(native, activities, index),
        tuple(
            _activity_edge(native, activities, index, edge_index, False)
            for edge_index in range(int(input_count.value))
        ),
        tuple(
            _activity_edge(native, activities, index, edge_index, True)
            for edge_index in range(int(output_count.value))
        ),
    )


def _activity_tool(
    native: NativeLibrary, activities: _Pointer[ActivitySet], index: int
) -> ToolIdentity | None:
    name = ctypes.c_char_p()
    version = ctypes.c_char_p()
    uri = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_activity_set_get_tool(
        activities,
        index,
        ctypes.byref(name),
        ctypes.byref(version),
        ctypes.byref(uri),
        ctypes.byref(error),
    )
    native.check(status, error)
    if name.value is None and version.value is None and uri.value is None:
        return None
    return ToolIdentity(
        _decode_required(name.value, "activity tool name"),
        _decode_optional(version.value),
        _decode_optional(uri.value),
    )


def _activity_agent(
    native: NativeLibrary, activities: _Pointer[ActivitySet], index: int
) -> AgentIdentity | None:
    name = ctypes.c_char_p()
    scheme = ctypes.c_char_p()
    value = ctypes.c_char_p()
    qualifier = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_activity_set_get_agent(
        activities,
        index,
        ctypes.byref(name),
        ctypes.byref(scheme),
        ctypes.byref(value),
        ctypes.byref(qualifier),
        ctypes.byref(error),
    )
    native.check(status, error)
    if name.value is None and scheme.value is None and value.value is None:
        return None
    identifier = None
    if scheme.value is not None or value.value is not None:
        identifier = ExternalIdentifier(
            _decode_required(scheme.value, "agent identifier scheme"),
            _decode_required(value.value, "agent identifier value"),
            _decode_optional(qualifier.value),
        )
    return AgentIdentity(_decode_optional(name.value), identifier)


def _activity_edge(
    native: NativeLibrary,
    activities: _Pointer[ActivitySet],
    activity_index: int,
    edge_index: int,
    output: bool,
) -> ActivityEdge:
    representation_id = _abi.RepresentationId()
    role = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    function = (
        native.lib.pp_activity_set_get_output
        if output
        else native.lib.pp_activity_set_get_input
    )
    status = function(
        activities,
        activity_index,
        edge_index,
        ctypes.byref(representation_id),
        ctypes.byref(role),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ActivityEdge(
        RepresentationId(_uuid(representation_id)),
        _decode_optional(role.value),
        _activity_edge_snapshot(native, activities, activity_index, edge_index, output),
    )


def _activity_edge_snapshot(
    native: NativeLibrary,
    activities: _Pointer[ActivitySet],
    activity_index: int,
    edge_index: int,
    output: bool,
) -> ActivityEdgeSnapshot | None:
    has_snapshot = ctypes.c_uint8()
    revision_sequence = ctypes.c_uint64()
    fingerprint_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    summary = (
        native.lib.pp_activity_set_get_output_snapshot
        if output
        else native.lib.pp_activity_set_get_input_snapshot
    )
    status = summary(
        activities,
        activity_index,
        edge_index,
        ctypes.byref(has_snapshot),
        ctypes.byref(revision_sequence),
        ctypes.byref(fingerprint_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    if not has_snapshot.value:
        return None
    fingerprints = tuple(
        _activity_snapshot_fingerprint(
            native, activities, activity_index, edge_index, index, output
        )
        for index in range(int(fingerprint_count.value))
    )
    return ActivityEdgeSnapshot(int(revision_sequence.value), fingerprints)


def _activity_snapshot_fingerprint(
    native: NativeLibrary,
    activities: _Pointer[ActivitySet],
    activity_index: int,
    edge_index: int,
    fingerprint_index: int,
    output: bool,
) -> FingerprintSnapshot:
    algorithm = ctypes.c_char_p()
    version = ctypes.c_uint16()
    value = ctypes.POINTER(ctypes.c_uint8)()
    value_length = ctypes.c_uint64()
    has_observed_revision = ctypes.c_uint8()
    observed_revision_sequence = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    accessor = (
        native.lib.pp_activity_set_get_output_snapshot_fingerprint
        if output
        else native.lib.pp_activity_set_get_input_snapshot_fingerprint
    )
    status = accessor(
        activities,
        activity_index,
        edge_index,
        fingerprint_index,
        ctypes.byref(algorithm),
        ctypes.byref(version),
        ctypes.byref(value),
        ctypes.byref(value_length),
        ctypes.byref(has_observed_revision),
        ctypes.byref(observed_revision_sequence),
        ctypes.byref(error),
    )
    native.check(status, error)
    return FingerprintSnapshot(
        _decode_required(algorithm.value, "snapshot fingerprint algorithm"),
        int(version.value),
        bytes(value[: value_length.value]),
        int(observed_revision_sequence.value) if has_observed_revision.value else None,
    )


def _representation_at(
    native: NativeLibrary,
    representations: _Pointer[RepresentationSet],
    index: int,
) -> Representation:
    representation_id = _abi.RepresentationId()
    asset_id = _abi.AssetId()
    kind = _abi.RepresentationKind()
    structure_kind = _abi.ContentStructureKind()
    member_count = ctypes.c_uint64()
    resource_count = ctypes.c_uint64()
    fingerprint_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_representation_set_get(
        representations,
        index,
        ctypes.byref(representation_id),
        ctypes.byref(asset_id),
        ctypes.byref(kind),
        ctypes.byref(structure_kind),
        ctypes.byref(member_count),
        ctypes.byref(resource_count),
        ctypes.byref(fingerprint_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    structure = _content_structure_kind(int(structure_kind.value))
    members = tuple(
        _representation_member_at(native, representations, index, member_index)
        for member_index in range(int(member_count.value))
    )
    content: RepresentationContent
    if structure is ContentStructureKind.SINGLE_RESOURCE:
        if len(members) != 1 or members[0].role is not None or not members[0].required:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "invalid single-resource projection"
            )
        content = SingleResourceContent(members[0].resource_id)
    elif structure is ContentStructureKind.IMAGE_SEQUENCE:
        if len(members) != 1 or members[0].role is not None or not members[0].required:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "invalid image-sequence membership projection"
            )
        content = ImageSequenceContent(
            members[0].resource_id, _image_sequence_at(native, representations, index)
        )
    elif structure is ContentStructureKind.ORDERED_PARTS:
        content = OrderedPartsContent(members)
    else:
        content = PackageContent(members)
    return Representation(
        RepresentationId(_uuid(representation_id)),
        AssetId(_uuid(asset_id)),
        _representation_kind(int(kind.value)),
        content,
        tuple(
            _representation_fingerprint_at(
                native, representations, index, fingerprint_index
            )
            for fingerprint_index in range(int(fingerprint_count.value))
        ),
        tuple(
            _resource_at(native, representations, index, resource_index)
            for resource_index in range(int(resource_count.value))
        ),
    )


def _representation_member_at(
    native: NativeLibrary,
    representations: _Pointer[RepresentationSet],
    representation_index: int,
    member_index: int,
) -> RepresentationMember:
    resource_id = _abi.ResourceId()
    role = ctypes.c_char_p()
    required = ctypes.c_uint8()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_representation_set_get_member(
        representations,
        representation_index,
        member_index,
        ctypes.byref(resource_id),
        ctypes.byref(role),
        ctypes.byref(required),
        ctypes.byref(error),
    )
    native.check(status, error)
    return RepresentationMember(
        ResourceId(_uuid(resource_id)),
        _decode_optional(role.value),
        bool(required.value),
    )


def _image_sequence_at(
    native: NativeLibrary,
    representations: _Pointer[RepresentationSet],
    representation_index: int,
) -> ImageSequenceDescriptor:
    start = ctypes.c_int64()
    end = ctypes.c_int64()
    step = ctypes.c_uint32()
    rate_numerator = ctypes.c_uint32()
    rate_denominator = ctypes.c_uint32()
    missing_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_representation_set_get_sequence(
        representations,
        representation_index,
        ctypes.byref(start),
        ctypes.byref(end),
        ctypes.byref(step),
        ctypes.byref(rate_numerator),
        ctypes.byref(rate_denominator),
        ctypes.byref(missing_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ImageSequenceDescriptor(
        int(start.value),
        int(end.value),
        int(step.value),
        int(rate_numerator.value),
        int(rate_denominator.value),
        tuple(
            _sequence_missing_frame_at(
                native, representations, representation_index, index
            )
            for index in range(int(missing_count.value))
        ),
    )


def _sequence_missing_frame_at(
    native: NativeLibrary,
    representations: _Pointer[RepresentationSet],
    representation_index: int,
    frame_index: int,
) -> int:
    frame = ctypes.c_int64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_representation_set_get_sequence_missing_frame(
        representations,
        representation_index,
        frame_index,
        ctypes.byref(frame),
        ctypes.byref(error),
    )
    native.check(status, error)
    return int(frame.value)


def _representation_fingerprint_at(
    native: NativeLibrary,
    representations: _Pointer[RepresentationSet],
    representation_index: int,
    fingerprint_index: int,
) -> Fingerprint:
    algorithm = ctypes.c_char_p()
    version = ctypes.c_uint16()
    value = ctypes.POINTER(ctypes.c_uint8)()
    value_length = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_representation_set_get_fingerprint(
        representations,
        representation_index,
        fingerprint_index,
        ctypes.byref(algorithm),
        ctypes.byref(version),
        ctypes.byref(value),
        ctypes.byref(value_length),
        ctypes.byref(error),
    )
    native.check(status, error)
    return _fingerprint(algorithm, version, value, value_length)


def _resource_at(
    native: NativeLibrary,
    representations: _Pointer[RepresentationSet],
    representation_index: int,
    resource_index: int,
) -> Resource:
    resource_id = _abi.ResourceId()
    has_file_facts = ctypes.c_uint8()
    file_size = ctypes.c_uint64()
    has_modified_at = ctypes.c_uint8()
    modified_at = ctypes.c_int64()
    locator_count = ctypes.c_uint64()
    fingerprint_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_representation_set_get_resource(
        representations,
        representation_index,
        resource_index,
        ctypes.byref(resource_id),
        ctypes.byref(has_file_facts),
        ctypes.byref(file_size),
        ctypes.byref(has_modified_at),
        ctypes.byref(modified_at),
        ctypes.byref(locator_count),
        ctypes.byref(fingerprint_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    return Resource(
        ResourceId(_uuid(resource_id)),
        int(file_size.value) if has_file_facts.value else None,
        int(modified_at.value) if has_modified_at.value else None,
        tuple(
            _resource_fingerprint_at(
                native,
                representations,
                representation_index,
                resource_index,
                fingerprint_index,
            )
            for fingerprint_index in range(int(fingerprint_count.value))
        ),
        tuple(
            _locator_at(
                native,
                representations,
                representation_index,
                resource_index,
                locator_index,
            )
            for locator_index in range(int(locator_count.value))
        ),
    )


def _resource_fingerprint_at(
    native: NativeLibrary,
    representations: _Pointer[RepresentationSet],
    representation_index: int,
    resource_index: int,
    fingerprint_index: int,
) -> Fingerprint:
    algorithm = ctypes.c_char_p()
    version = ctypes.c_uint16()
    value = ctypes.POINTER(ctypes.c_uint8)()
    value_length = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_representation_set_get_resource_fingerprint(
        representations,
        representation_index,
        resource_index,
        fingerprint_index,
        ctypes.byref(algorithm),
        ctypes.byref(version),
        ctypes.byref(value),
        ctypes.byref(value_length),
        ctypes.byref(error),
    )
    native.check(status, error)
    return _fingerprint(algorithm, version, value, value_length)


def _fingerprint(
    algorithm: ctypes.c_char_p,
    version: ctypes.c_uint16,
    value: _Pointer[ctypes.c_uint8],
    value_length: ctypes.c_uint64,
) -> Fingerprint:
    return Fingerprint(
        _decode_required(algorithm.value, "fingerprint algorithm"),
        int(version.value),
        ctypes.string_at(value, int(value_length.value)),
    )


def _locator_at(
    native: NativeLibrary,
    representations: _Pointer[RepresentationSet],
    representation_index: int,
    resource_index: int,
    locator_index: int,
) -> Locator:
    locator_id = _abi.LocatorId()
    uri = ctypes.c_char_p()
    availability = _abi.LocatorAvailability()
    has_last_seen = ctypes.c_uint8()
    last_seen = ctypes.c_int64()
    has_naming = ctypes.c_uint8()
    naming = _abi.SequenceNaming()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_representation_set_get_locator(
        representations,
        representation_index,
        resource_index,
        locator_index,
        ctypes.byref(locator_id),
        ctypes.byref(uri),
        ctypes.byref(availability),
        ctypes.byref(has_last_seen),
        ctypes.byref(last_seen),
        ctypes.byref(has_naming),
        ctypes.byref(naming),
        ctypes.byref(error),
    )
    native.check(status, error)
    return Locator(
        LocatorId(_uuid(locator_id)),
        _decode_required(uri.value, "locator URI"),
        _locator_availability(int(availability.value)),
        int(last_seen.value) if has_last_seen.value else None,
        _sequence_naming(has_naming, naming),
    )


def _relink_selection_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
    resources: tuple[ResourceResolution, ...],
) -> tuple[RelinkSelection | None, str | None]:
    capacity = len(resources)
    locations = (_abi.RelinkLocation * capacity)()
    value = _abi.RelinkSelection()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_select_verified(
        resolutions,
        representation_index,
        locations,
        capacity,
        ctypes.byref(value),
        ctypes.byref(error),
    )
    try:
        native.check(status, error)
    except ConflictError as failure:
        return None, str(failure)
    if value.location_count > capacity:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "selection exceeds location storage"
        )
    copied: list[RelinkLocation] = []
    for item in locations[: value.location_count]:
        if item.resource_index >= capacity:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "selection has invalid resource index"
            )
        resource = resources[item.resource_index]
        if (
            resource.resource_id != _uuid(item.resource_id)
            or len(resource.candidates) != 1
        ):
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "selection does not match resource evidence"
            )
        copied.append(RelinkLocation(resource.resource_id, resource.candidates[0]))
    return RelinkSelection(
        _decision_base(value.base),
        RepresentationId(_uuid(value.representation_id)),
        tuple(copied),
    ), None


def _representation_resolution_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
) -> RepresentationResolution:
    asset_id = _abi.AssetId()
    representation_id = _abi.RepresentationId()
    availability = _abi.RepresentationAvailability()
    resource_count = ctypes.c_uint64()
    issue_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_representation(
        resolutions,
        representation_index,
        ctypes.byref(asset_id),
        ctypes.byref(representation_id),
        ctypes.byref(availability),
        ctypes.byref(resource_count),
        ctypes.byref(issue_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    resources = tuple(
        _resource_resolution_at(
            native, resolutions, representation_index, resource_index
        )
        for resource_index in range(int(resource_count.value))
    )
    selection, selection_error = _relink_selection_at(
        native, resolutions, representation_index, resources
    )
    return RepresentationResolution(
        AssetId(_uuid(asset_id)),
        RepresentationId(_uuid(representation_id)),
        _representation_availability(int(availability.value)),
        resources,
        tuple(
            _availability_issue_at(
                native, resolutions, representation_index, issue_index
            )
            for issue_index in range(int(issue_count.value))
        ),
        selection,
        selection_error,
    )


def _resource_resolution_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
    resource_index: int,
) -> ResourceResolution:
    resource_id = _abi.ResourceId()
    state = _abi.ResourceResolutionState()
    candidate_count = ctypes.c_uint64()
    evidence_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_resource_state(
        resolutions,
        representation_index,
        resource_index,
        ctypes.byref(state),
        ctypes.byref(error),
    )
    native.check(status, error)
    status = native.lib.pp_resolution_set_get_resource(
        resolutions,
        representation_index,
        resource_index,
        state.value,
        ctypes.byref(resource_id),
        ctypes.byref(state),
        ctypes.byref(candidate_count),
        ctypes.byref(evidence_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    outcome = _resolution_outcome(
        _resource_resolution_state(int(state.value)),
        tuple(
            _resolution_candidate_at(
                native,
                resolutions,
                representation_index,
                resource_index,
                candidate_index,
            )
            for candidate_index in range(int(candidate_count.value))
        ),
    )
    return ResourceResolution(
        ResourceId(_uuid(resource_id)),
        outcome,
        tuple(
            _resource_evidence_at(
                native,
                resolutions,
                representation_index,
                resource_index,
                evidence_index,
            )
            for evidence_index in range(int(evidence_count.value))
        ),
    )


def _resolution_outcome(
    state: ResourceResolutionState, candidates: tuple[ResolutionCandidate, ...]
) -> ResolutionOutcome:
    if (
        state
        in (
            ResourceResolutionState.ONLINE_AT_KNOWN_LOCATOR,
            ResourceResolutionState.RESOLVED_EXACT,
            ResourceResolutionState.RESOLVED_PROBABLE,
        )
        and len(candidates) == 1
    ):
        (candidate,) = candidates
        if state is ResourceResolutionState.ONLINE_AT_KNOWN_LOCATOR:
            return ResourceOnlineAtKnownLocator(candidate)
        if state is ResourceResolutionState.RESOLVED_EXACT:
            return ResourceResolvedExact(candidate)
        return ResourceResolvedProbable(candidate)
    if state is ResourceResolutionState.AMBIGUOUS and len(candidates) >= 2:
        return ResourceAmbiguous(candidates)
    if not candidates:
        if state is ResourceResolutionState.OFFLINE:
            return ResourceOffline()
        if state is ResourceResolutionState.ERROR:
            return ResourceResolutionFailure()
    raise InternalError(
        _abi.PP_ERROR_INTERNAL, "resolution state and candidate count disagree"
    )


def _resolution_candidate_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
    resource_index: int,
    candidate_index: int,
) -> ResolutionCandidate:
    uri = ctypes.c_char_p()
    confidence = ctypes.c_uint16()
    media_root = ctypes.c_char_p()
    has_naming = ctypes.c_uint8()
    naming = _abi.SequenceNaming()
    evidence_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_candidate(
        resolutions,
        representation_index,
        resource_index,
        candidate_index,
        ctypes.byref(uri),
        ctypes.byref(confidence),
        ctypes.byref(media_root),
        ctypes.byref(has_naming),
        ctypes.byref(naming),
        ctypes.byref(evidence_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ResolutionCandidate(
        _decode_required(uri.value, "resolution candidate URI"),
        int(confidence.value),
        _decode_optional(media_root.value),
        _sequence_naming(has_naming, naming),
        tuple(
            _candidate_evidence_at(
                native,
                resolutions,
                representation_index,
                resource_index,
                candidate_index,
                evidence_index,
            )
            for evidence_index in range(int(evidence_count.value))
        ),
    )


def _resource_evidence_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
    resource_index: int,
    evidence_index: int,
) -> ResolutionEvidence:
    kind = _abi.EvidenceKind()
    detail = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_resource_evidence(
        resolutions,
        representation_index,
        resource_index,
        evidence_index,
        ctypes.byref(kind),
        ctypes.byref(detail),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ResolutionEvidence(
        _evidence_kind(int(kind.value)), _decode_optional(detail.value)
    )


def _candidate_evidence_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
    resource_index: int,
    candidate_index: int,
    evidence_index: int,
) -> ResolutionEvidence:
    kind = _abi.EvidenceKind()
    detail = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_candidate_evidence(
        resolutions,
        representation_index,
        resource_index,
        candidate_index,
        evidence_index,
        ctypes.byref(kind),
        ctypes.byref(detail),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ResolutionEvidence(
        _evidence_kind(int(kind.value)), _decode_optional(detail.value)
    )


def _availability_issue_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
    issue_index: int,
) -> AvailabilityIssue:
    resource_id = _abi.ResourceId()
    required = ctypes.c_uint8()
    kind = _abi.AvailabilityIssueKind()
    frame_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_issue(
        resolutions,
        representation_index,
        issue_index,
        ctypes.byref(resource_id),
        ctypes.byref(required),
        ctypes.byref(kind),
        ctypes.byref(frame_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    category = _availability_issue_kind(int(kind.value))
    if frame_count.value > 100_000:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "availability frame count exceeds bounds"
        )
    frames = tuple(
        _missing_frame_at(
            native, resolutions, representation_index, issue_index, frame_index
        )
        for frame_index in range(int(frame_count.value))
    )
    detail: AvailabilityIssueDetail
    if category is AvailabilityIssueKind.MISSING_FRAMES:
        if not frames:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "missing-frame issue has no frames"
            )
        detail = MissingSequenceFrames(frames)
    else:
        if frames:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "resource issue carries unrelated frames"
            )
        if category is AvailabilityIssueKind.OFFLINE_RESOURCE:
            detail = OfflineResourceIssue()
        elif category is AvailabilityIssueKind.AMBIGUOUS_RESOURCE:
            detail = AmbiguousResourceIssue()
        else:
            detail = ResourceErrorIssue()
    return AvailabilityIssue(
        ResourceId(_uuid(resource_id)), bool(required.value), detail
    )


def _missing_frame_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
    issue_index: int,
    frame_index: int,
) -> int:
    frame = ctypes.c_int64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_issue_frame(
        resolutions,
        representation_index,
        issue_index,
        frame_index,
        ctypes.byref(frame),
        ctypes.byref(error),
    )
    native.check(status, error)
    return int(frame.value)


def _metadata_at(
    native: NativeLibrary,
    metadata: _Pointer[MetadataSet],
    index: int,
) -> MetadataAssertion:
    target = _abi.ObjectRef()
    vocabulary = ctypes.c_char_p()
    property_name = ctypes.c_char_p()
    value = ctypes.POINTER(NativeMetadataValue)()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_metadata_set_get(
        metadata,
        index,
        ctypes.byref(target),
        ctypes.byref(vocabulary),
        ctypes.byref(property_name),
        ctypes.byref(value),
        ctypes.byref(error),
    )
    native.check(status, error)
    if not value:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native metadata assertion is missing its value"
        )
    return MetadataAssertion(
        _object_reference(target),
        MetadataProperty(
            _decode_required(vocabulary.value, "metadata vocabulary"),
            _decode_required(property_name.value, "metadata property"),
        ),
        _metadata_value(native, value),
    )


def _metadata_value(
    native: NativeLibrary, value: _Pointer[NativeMetadataValue]
) -> MetadataValue:
    kind = int(native.lib.pp_metadata_value_kind(value))
    error = ctypes.POINTER(Error)()

    if kind in (_abi.PP_METADATA_STRING, _abi.PP_METADATA_LANG_STRING):
        text = ctypes.c_char_p()
        language = ctypes.c_char_p()
        status = native.lib.pp_metadata_value_get_string(
            value, ctypes.byref(text), ctypes.byref(language), ctypes.byref(error)
        )
        native.check(status, error)
        decoded = _decode_required(text.value, "metadata text")
        if kind == _abi.PP_METADATA_STRING:
            return MetadataString(decoded)
        return MetadataLanguageString(
            decoded, _decode_required(language.value, "metadata language")
        )

    if kind == _abi.PP_METADATA_I64:
        result = ctypes.c_int64()
        status = native.lib.pp_metadata_value_get_i64(
            value, ctypes.byref(result), ctypes.byref(error)
        )
        native.check(status, error)
        return MetadataI64(int(result.value))

    if kind == _abi.PP_METADATA_U64:
        result = ctypes.c_uint64()
        status = native.lib.pp_metadata_value_get_u64(
            value, ctypes.byref(result), ctypes.byref(error)
        )
        native.check(status, error)
        return MetadataU64(int(result.value))

    if kind == _abi.PP_METADATA_DECIMAL:
        coefficient = ctypes.c_char_p()
        scale = ctypes.c_uint32()
        status = native.lib.pp_metadata_value_get_decimal(
            value,
            ctypes.byref(coefficient),
            ctypes.byref(scale),
            ctypes.byref(error),
        )
        native.check(status, error)
        coefficient_text = _decode_required(
            coefficient.value, "metadata decimal coefficient"
        )
        try:
            parsed_coefficient = int(coefficient_text)
        except ValueError as exception:
            raise InternalError(
                _abi.PP_ERROR_INTERNAL, "native metadata decimal is invalid"
            ) from exception
        return MetadataDecimal(parsed_coefficient, int(scale.value))

    if kind == _abi.PP_METADATA_BOOL:
        result = ctypes.c_uint8()
        status = native.lib.pp_metadata_value_get_bool(
            value, ctypes.byref(result), ctypes.byref(error)
        )
        native.check(status, error)
        return MetadataBool(bool(result.value))

    if kind == _abi.PP_METADATA_TIMESTAMP:
        result = ctypes.c_int64()
        status = native.lib.pp_metadata_value_get_timestamp(
            value, ctypes.byref(result), ctypes.byref(error)
        )
        native.check(status, error)
        return MetadataTimestamp(int(result.value))

    if kind == _abi.PP_METADATA_URI:
        result = ctypes.c_char_p()
        status = native.lib.pp_metadata_value_get_uri(
            value, ctypes.byref(result), ctypes.byref(error)
        )
        native.check(status, error)
        return MetadataUri(_decode_required(result.value, "metadata URI"))

    if kind == _abi.PP_METADATA_BYTES:
        result = ctypes.POINTER(ctypes.c_uint8)()
        length = ctypes.c_uint64()
        status = native.lib.pp_metadata_value_get_bytes(
            value,
            ctypes.byref(result),
            ctypes.byref(length),
            ctypes.byref(error),
        )
        native.check(status, error)
        return MetadataBytes(bytes(result[: length.value]) if length.value else b"")

    if kind == _abi.PP_METADATA_RATIONAL:
        numerator = ctypes.c_int64()
        denominator = ctypes.c_uint64()
        status = native.lib.pp_metadata_value_get_rational(
            value,
            ctypes.byref(numerator),
            ctypes.byref(denominator),
            ctypes.byref(error),
        )
        native.check(status, error)
        return MetadataRational(int(numerator.value), int(denominator.value))

    if kind == _abi.PP_METADATA_LIST:
        count = native.lib.pp_metadata_value_list_count(value)
        items: list[MetadataValue] = []
        for index in range(int(count)):
            item = ctypes.POINTER(NativeMetadataValue)()
            status = native.lib.pp_metadata_value_list_get(
                value, index, ctypes.byref(item), ctypes.byref(error)
            )
            native.check(status, error)
            if not item:
                raise InternalError(
                    _abi.PP_ERROR_INTERNAL, "native metadata list item is missing"
                )
            items.append(_metadata_value(native, item))
        return MetadataList(tuple(items))

    if kind == _abi.PP_METADATA_STRUCT:
        count = native.lib.pp_metadata_value_struct_count(value)
        fields: list[MetadataStructField] = []
        for index in range(int(count)):
            name = ctypes.c_char_p()
            field_value = ctypes.POINTER(NativeMetadataValue)()
            status = native.lib.pp_metadata_value_struct_get(
                value,
                index,
                ctypes.byref(name),
                ctypes.byref(field_value),
                ctypes.byref(error),
            )
            native.check(status, error)
            if not field_value:
                raise InternalError(
                    _abi.PP_ERROR_INTERNAL, "native metadata structure field is missing"
                )
            fields.append(
                MetadataStructField(
                    _decode_required(name.value, "metadata field name"),
                    _metadata_value(native, field_value),
                )
            )
        return MetadataStruct(tuple(fields))

    if kind == _abi.PP_METADATA_REFERENCE:
        target = _abi.ObjectRef()
        status = native.lib.pp_metadata_value_get_reference(
            value, ctypes.byref(target), ctypes.byref(error)
        )
        native.check(status, error)
        return MetadataReference(_object_reference(target))

    raise UnsupportedError(
        _abi.PP_ERROR_UNSUPPORTED, "metadata value has an unknown semantic kind"
    )


def _revision_at(
    native: NativeLibrary,
    revisions: _Pointer[RevisionSet],
    index: int,
) -> Revision:
    revision_id = _abi.RevisionId()
    sequence = ctypes.c_uint64()
    transaction_id = _abi.TransactionId()
    committed_at = ctypes.c_int64()
    origin_name = ctypes.c_char_p()
    origin_version = ctypes.c_char_p()
    origin_uri = ctypes.c_char_p()
    message = ctypes.c_char_p()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_revision_set_get(
        revisions,
        index,
        ctypes.byref(revision_id),
        ctypes.byref(sequence),
        ctypes.byref(transaction_id),
        ctypes.byref(committed_at),
        ctypes.byref(origin_name),
        ctypes.byref(origin_version),
        ctypes.byref(origin_uri),
        ctypes.byref(message),
        ctypes.byref(error),
    )
    native.check(status, error)
    name = _decode_optional(origin_name.value)
    origin = None
    if name is not None:
        origin = OriginIdentity(
            name,
            _decode_optional(origin_version.value),
            _decode_optional(origin_uri.value),
        )
    return Revision(
        RevisionId(_uuid(revision_id)),
        int(sequence.value),
        TransactionId(_uuid(transaction_id)),
        int(committed_at.value),
        origin,
        _decode_optional(message.value),
    )


_REVISION_EVENT_KINDS: dict[type, int] = {
    AssetImportedEvent: _abi.PP_REVISION_ASSET_IMPORTED,
    RepresentationAddedEvent: _abi.PP_REVISION_REPRESENTATION_ADDED,
    ResourceAddedEvent: _abi.PP_REVISION_RESOURCE_ADDED,
    RepresentationResourceAddedEvent: _abi.PP_REVISION_REPRESENTATION_RESOURCE_ADDED,
    LocatorAddedEvent: _abi.PP_REVISION_LOCATOR_ADDED,
    LocatorRetiredEvent: _abi.PP_REVISION_LOCATOR_RETIRED,
    MediaRootAddedEvent: _abi.PP_REVISION_MEDIA_ROOT_ADDED,
    MediaRootEnabledChangedEvent: _abi.PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED,
    MediaRootRemovedEvent: _abi.PP_REVISION_MEDIA_ROOT_REMOVED,
    ExternalIdentifierAddedEvent: _abi.PP_REVISION_EXTERNAL_IDENTIFIER_ADDED,
    ExternalIdentifierRemovedEvent: _abi.PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED,
    MetadataAddedOrReplacedEvent: _abi.PP_REVISION_METADATA_ADDED_OR_REPLACED,
    MetadataRemovedEvent: _abi.PP_REVISION_METADATA_REMOVED,
    ActivityCreatedEvent: _abi.PP_REVISION_ACTIVITY_CREATED,
    ActivityInputAddedEvent: _abi.PP_REVISION_ACTIVITY_INPUT_ADDED,
    ActivityOutputAddedEvent: _abi.PP_REVISION_ACTIVITY_OUTPUT_ADDED,
    ResourceFileFactsObservedEvent: _abi.PP_REVISION_RESOURCE_FILE_FACTS_OBSERVED,
    ResourceFingerprintObservedEvent: _abi.PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED,
    RepresentationFingerprintObservedEvent: (
        _abi.PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED
    ),
    DependencySetRecordedEvent: _abi.PP_REVISION_DEPENDENCY_SET_RECORDED,
    JobRequestedEvent: _abi.PP_REVISION_JOB_REQUESTED,
    JobClaimedEvent: _abi.PP_REVISION_JOB_CLAIMED,
    JobClaimRenewedEvent: _abi.PP_REVISION_JOB_CLAIM_RENEWED,
    JobClaimReleasedEvent: _abi.PP_REVISION_JOB_CLAIM_RELEASED,
    JobSucceededEvent: _abi.PP_REVISION_JOB_SUCCEEDED,
    JobFailedEvent: _abi.PP_REVISION_JOB_FAILED,
    JobCancelledEvent: _abi.PP_REVISION_JOB_CANCELLED,
}


def _revision_event_kind(kind: type) -> int:
    try:
        return _REVISION_EVENT_KINDS[kind]
    except (KeyError, TypeError) as error:
        raise ValueError(f"not a revision event payload class: {kind!r}") from error


def _revisions_in(
    native: NativeLibrary, handle: _Pointer[RevisionSet]
) -> tuple[Revision, ...]:
    return tuple(
        _revision_at(native, handle, index)
        for index in range(int(native.lib.pp_revision_set_count(handle)))
    )


def _revision_event_at(
    native: NativeLibrary,
    events: _Pointer[RevisionEventSet],
    index: int,
) -> RevisionEvent:
    event = NativeRevisionEvent()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_revision_event_set_get(
        events, index, ctypes.byref(event), ctypes.byref(error)
    )
    native.check(status, error)

    kind = int(event.kind)
    if kind == _abi.PP_REVISION_ASSET_IMPORTED:
        payload = AssetImportedEvent(AssetId(_uuid(event.asset_id)))
    elif kind == _abi.PP_REVISION_REPRESENTATION_ADDED:
        payload = RepresentationAddedEvent(
            AssetId(_uuid(event.asset_id)),
            RepresentationId(_uuid(event.representation_id)),
        )
    elif kind == _abi.PP_REVISION_RESOURCE_ADDED:
        payload = ResourceAddedEvent(ResourceId(_uuid(event.resource_id)))
    elif kind == _abi.PP_REVISION_REPRESENTATION_RESOURCE_ADDED:
        payload = RepresentationResourceAddedEvent(
            RepresentationId(_uuid(event.representation_id)),
            ResourceId(_uuid(event.resource_id)),
            int(event.structural_position),
        )
    elif kind == _abi.PP_REVISION_LOCATOR_ADDED:
        payload = LocatorAddedEvent(
            ResourceId(_uuid(event.resource_id)),
            LocatorId(_uuid(event.locator_id)),
        )
    elif kind == _abi.PP_REVISION_MEDIA_ROOT_ADDED:
        payload = MediaRootAddedEvent(MediaRootId(_uuid(event.media_root_id)))
    elif kind == _abi.PP_REVISION_LOCATOR_RETIRED:
        payload = LocatorRetiredEvent(
            ResourceId(_uuid(event.resource_id)), LocatorId(_uuid(event.locator_id))
        )
    elif kind == _abi.PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED:
        payload = MediaRootEnabledChangedEvent(
            MediaRootId(_uuid(event.media_root_id)), bool(event.enabled)
        )
    elif kind == _abi.PP_REVISION_MEDIA_ROOT_REMOVED:
        payload = MediaRootRemovedEvent(MediaRootId(_uuid(event.media_root_id)))
    elif kind == _abi.PP_REVISION_EXTERNAL_IDENTIFIER_ADDED:
        payload = ExternalIdentifierAddedEvent(
            _object_reference(event.target), _external_identifier(event)
        )
    elif kind == _abi.PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED:
        payload = ExternalIdentifierRemovedEvent(
            _object_reference(event.target), _external_identifier(event)
        )
    elif kind == _abi.PP_REVISION_METADATA_ADDED_OR_REPLACED:
        payload = MetadataAddedOrReplacedEvent(
            _object_reference(event.target), _metadata_property(event)
        )
    elif kind == _abi.PP_REVISION_METADATA_REMOVED:
        payload = MetadataRemovedEvent(
            _object_reference(event.target), _metadata_property(event)
        )
    elif kind == _abi.PP_REVISION_ACTIVITY_CREATED:
        payload = ActivityCreatedEvent(
            ActivityId(_uuid(event.activity_id)),
            _decode_required(event.activity_kind, "activity kind"),
        )
    elif kind == _abi.PP_REVISION_ACTIVITY_INPUT_ADDED:
        payload = ActivityInputAddedEvent(
            ActivityId(_uuid(event.activity_id)),
            RepresentationId(_uuid(event.representation_id)),
            _decode_optional(event.role),
        )
    elif kind == _abi.PP_REVISION_ACTIVITY_OUTPUT_ADDED:
        payload = ActivityOutputAddedEvent(
            ActivityId(_uuid(event.activity_id)),
            RepresentationId(_uuid(event.representation_id)),
            _decode_optional(event.role),
        )
    elif kind == _abi.PP_REVISION_RESOURCE_FILE_FACTS_OBSERVED:
        payload = ResourceFileFactsObservedEvent(ResourceId(_uuid(event.resource_id)))
    elif kind == _abi.PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED:
        payload = ResourceFingerprintObservedEvent(
            ResourceId(_uuid(event.resource_id)),
            _decode_required(event.fingerprint_algorithm, "fingerprint algorithm"),
            int(event.fingerprint_version),
        )
    elif kind == _abi.PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED:
        payload = RepresentationFingerprintObservedEvent(
            RepresentationId(_uuid(event.representation_id)),
            _decode_required(event.fingerprint_algorithm, "fingerprint algorithm"),
            int(event.fingerprint_version),
        )
    elif kind == _abi.PP_REVISION_DEPENDENCY_SET_RECORDED:
        payload = DependencySetRecordedEvent(
            RepresentationId(_uuid(event.representation_id))
        )
    elif kind == _abi.PP_REVISION_JOB_REQUESTED:
        payload = JobRequestedEvent(JobId(_uuid(event.job_id)))
    elif kind == _abi.PP_REVISION_JOB_CLAIMED:
        payload = JobClaimedEvent(JobId(_uuid(event.job_id)))
    elif kind == _abi.PP_REVISION_JOB_CLAIM_RENEWED:
        payload = JobClaimRenewedEvent(JobId(_uuid(event.job_id)))
    elif kind == _abi.PP_REVISION_JOB_CLAIM_RELEASED:
        payload = JobClaimReleasedEvent(JobId(_uuid(event.job_id)))
    elif kind == _abi.PP_REVISION_JOB_SUCCEEDED:
        payload = JobSucceededEvent(JobId(_uuid(event.job_id)))
    elif kind == _abi.PP_REVISION_JOB_FAILED:
        payload = JobFailedEvent(JobId(_uuid(event.job_id)))
    elif kind == _abi.PP_REVISION_JOB_CANCELLED:
        payload = JobCancelledEvent(JobId(_uuid(event.job_id)))
    else:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "revision event has an unknown semantic kind"
        )

    return RevisionEvent(int(event.position), payload)


def _external_identifier(event: NativeRevisionEvent) -> ExternalIdentifier:
    return ExternalIdentifier(
        _decode_required(event.identifier_scheme, "identifier scheme"),
        _decode_required(event.identifier_value, "identifier value"),
        _decode_optional(event.identifier_qualifier),
    )


def _metadata_property(event: NativeRevisionEvent) -> MetadataProperty:
    return MetadataProperty(
        _decode_required(event.vocabulary, "metadata vocabulary"),
        _decode_required(event.property, "metadata property"),
    )


def _object_reference(value: _abi.ObjectRef) -> ObjectReference:
    object_id = _uuid(value.id)
    kind = int(value.kind)
    if kind == _abi.PP_OBJECT_PRODUCTION:
        return ProductionRef(ProductionId(object_id))
    if kind == _abi.PP_OBJECT_ASSET:
        return AssetRef(AssetId(object_id))
    if kind == _abi.PP_OBJECT_REPRESENTATION:
        return RepresentationRef(RepresentationId(object_id))
    if kind == _abi.PP_OBJECT_RESOURCE:
        return ResourceRef(ResourceId(object_id))
    if kind == _abi.PP_OBJECT_ACTIVITY:
        return ActivityRef(ActivityId(object_id))
    if kind == _abi.PP_OBJECT_JOB:
        return JobRef(JobId(object_id))
    raise UnsupportedError(
        _abi.PP_ERROR_UNSUPPORTED, "revision event has an unknown object-reference kind"
    )


def _representation_kind(value: int) -> RepresentationKind:
    result = {
        _abi.PP_REPRESENTATION_ORIGINAL: RepresentationKind.ORIGINAL,
        _abi.PP_REPRESENTATION_PROXY: RepresentationKind.PROXY,
        _abi.PP_REPRESENTATION_OPTIMIZED: RepresentationKind.OPTIMIZED,
        _abi.PP_REPRESENTATION_DERIVED: RepresentationKind.DERIVED,
    }.get(value)
    if result is None:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "representation has an unknown kind"
        )
    return result


def _job_state(value: int) -> JobState:
    result = {
        _abi.PP_JOB_REQUESTED: JobState.REQUESTED,
        _abi.PP_JOB_CLAIMED: JobState.CLAIMED,
        _abi.PP_JOB_SUCCEEDED: JobState.SUCCEEDED,
        _abi.PP_JOB_FAILED: JobState.FAILED,
        _abi.PP_JOB_CANCELLED: JobState.CANCELLED,
    }.get(value)
    if result is None:
        raise UnsupportedError(_abi.PP_ERROR_UNSUPPORTED, "job has an unknown state")
    return result


def _native_job_state(value: JobState) -> int:
    return {
        JobState.REQUESTED: _abi.PP_JOB_REQUESTED,
        JobState.CLAIMED: _abi.PP_JOB_CLAIMED,
        JobState.SUCCEEDED: _abi.PP_JOB_SUCCEEDED,
        JobState.FAILED: _abi.PP_JOB_FAILED,
        JobState.CANCELLED: _abi.PP_JOB_CANCELLED,
    }[value]


def _dependency_set_status(value: int) -> DependencySetStatus:
    result = {
        _abi.PP_DEPENDENCY_SET_CURRENT: DependencySetStatus.CURRENT,
        _abi.PP_DEPENDENCY_SET_NEEDS_EXTRACTION: DependencySetStatus.NEEDS_EXTRACTION,
    }.get(value)
    if result is None:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "dependency set has an unknown status"
        )
    return result


def _content_structure_kind(value: int) -> ContentStructureKind:
    result = {
        _abi.PP_CONTENT_SINGLE_RESOURCE: ContentStructureKind.SINGLE_RESOURCE,
        _abi.PP_CONTENT_IMAGE_SEQUENCE: ContentStructureKind.IMAGE_SEQUENCE,
        _abi.PP_CONTENT_ORDERED_PARTS: ContentStructureKind.ORDERED_PARTS,
        _abi.PP_CONTENT_PACKAGE: ContentStructureKind.PACKAGE,
    }.get(value)
    if result is None:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "representation has an unknown content structure"
        )
    return result


def _locator_availability(value: int) -> LocatorAvailability:
    result = {
        _abi.PP_LOCATOR_UNKNOWN: LocatorAvailability.UNKNOWN,
        _abi.PP_LOCATOR_ONLINE: LocatorAvailability.ONLINE,
        _abi.PP_LOCATOR_OFFLINE: LocatorAvailability.OFFLINE,
    }.get(value)
    if result is None:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "locator has an unknown availability"
        )
    return result


def _representation_availability(value: int) -> RepresentationAvailability:
    result = {
        _abi.PP_AVAILABILITY_ONLINE: RepresentationAvailability.ONLINE,
        _abi.PP_AVAILABILITY_PARTIAL: RepresentationAvailability.PARTIAL,
        _abi.PP_AVAILABILITY_OFFLINE: RepresentationAvailability.OFFLINE,
        _abi.PP_AVAILABILITY_AMBIGUOUS: RepresentationAvailability.AMBIGUOUS,
        _abi.PP_AVAILABILITY_ERROR: RepresentationAvailability.ERROR,
    }.get(value)
    if result is None:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "resolution has an unknown availability"
        )
    return result


def _resource_resolution_state(value: int) -> ResourceResolutionState:
    result = {
        _abi.PP_RESOURCE_ONLINE_AT_KNOWN_LOCATOR: (
            ResourceResolutionState.ONLINE_AT_KNOWN_LOCATOR
        ),
        _abi.PP_RESOURCE_RESOLVED_EXACT: ResourceResolutionState.RESOLVED_EXACT,
        _abi.PP_RESOURCE_RESOLVED_PROBABLE: (ResourceResolutionState.RESOLVED_PROBABLE),
        _abi.PP_RESOURCE_OFFLINE: ResourceResolutionState.OFFLINE,
        _abi.PP_RESOURCE_AMBIGUOUS: ResourceResolutionState.AMBIGUOUS,
        _abi.PP_RESOURCE_RESOLUTION_ERROR: ResourceResolutionState.ERROR,
    }.get(value)
    if result is None:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "resolution has an unknown resource state"
        )
    return result


def _availability_issue_kind(value: int) -> AvailabilityIssueKind:
    result = {
        _abi.PP_AVAILABILITY_ISSUE_OFFLINE_RESOURCE: (
            AvailabilityIssueKind.OFFLINE_RESOURCE
        ),
        _abi.PP_AVAILABILITY_ISSUE_AMBIGUOUS_RESOURCE: (
            AvailabilityIssueKind.AMBIGUOUS_RESOURCE
        ),
        _abi.PP_AVAILABILITY_ISSUE_RESOURCE_ERROR: (
            AvailabilityIssueKind.RESOURCE_ERROR
        ),
        _abi.PP_AVAILABILITY_ISSUE_MISSING_FRAMES: (
            AvailabilityIssueKind.MISSING_FRAMES
        ),
    }.get(value)
    if result is None:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "resolution has an unknown availability issue"
        )
    return result


def _evidence_kind(value: int) -> EvidenceKind:
    result = {
        _abi.PP_EVIDENCE_KNOWN_LOCATOR_AVAILABLE: EvidenceKind.KNOWN_LOCATOR_AVAILABLE,
        _abi.PP_EVIDENCE_EXACT_FINGERPRINT_MATCH: EvidenceKind.EXACT_FINGERPRINT_MATCH,
        _abi.PP_EVIDENCE_FULL_HASH_MATCH: EvidenceKind.FULL_HASH_MATCH,
        _abi.PP_EVIDENCE_PARTIAL_FINGERPRINT_MATCH: (
            EvidenceKind.PARTIAL_FINGERPRINT_MATCH
        ),
        _abi.PP_EVIDENCE_FILE_SIZE_MATCH: EvidenceKind.FILE_SIZE_MATCH,
        _abi.PP_EVIDENCE_FILE_NAME_MATCH: EvidenceKind.FILE_NAME_MATCH,
        _abi.PP_EVIDENCE_RELATIVE_PATH_SIMILARITY: (
            EvidenceKind.RELATIVE_PATH_SIMILARITY
        ),
        _abi.PP_EVIDENCE_MEDIA_ROOT_RELATION: EvidenceKind.MEDIA_ROOT_RELATION,
        _abi.PP_EVIDENCE_MEDIA_ROOT_UNMAPPED: EvidenceKind.MEDIA_ROOT_UNMAPPED,
        _abi.PP_EVIDENCE_MEDIA_ROOT_UNAVAILABLE: EvidenceKind.MEDIA_ROOT_UNAVAILABLE,
        _abi.PP_EVIDENCE_CONFLICTING_CANDIDATE: EvidenceKind.CONFLICTING_CANDIDATE,
        _abi.PP_EVIDENCE_DISCOVERY_ERROR: EvidenceKind.DISCOVERY_ERROR,
        _abi.PP_EVIDENCE_SEARCH_TRUNCATED: EvidenceKind.SEARCH_TRUNCATED,
        _abi.PP_EVIDENCE_FINGERPRINT_MISMATCH: EvidenceKind.FINGERPRINT_MISMATCH,
        _abi.PP_EVIDENCE_FINGERPRINT_NOT_VERIFIED: (
            EvidenceKind.FINGERPRINT_NOT_VERIFIED
        ),
    }.get(value)
    if result is None:
        raise UnsupportedError(
            _abi.PP_ERROR_UNSUPPORTED, "resolution has an unknown evidence kind"
        )
    return result


def _decode_required(value: bytes | None, label: str) -> str:
    if value is None:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, f"revision event is missing {label}"
        )
    return _decode_text(value)


def _decode_optional(value: bytes | None) -> str | None:
    return None if value is None else _decode_text(value)


def _decode_text(value: bytes) -> str:
    try:
        return value.decode("utf-8")
    except UnicodeDecodeError as error:
        raise InternalError(
            _abi.PP_ERROR_INTERNAL, "native text is not valid UTF-8"
        ) from error
