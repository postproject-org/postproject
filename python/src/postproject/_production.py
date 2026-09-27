"""Owned production and transaction wrappers over the public C ABI."""

from __future__ import annotations

import ctypes
import os
import threading
import weakref
from _ctypes import _Pointer
from collections.abc import Callable, Iterable, Iterator, Mapping
from pathlib import Path
from types import TracebackType
from typing import Self, TypeVar
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
from ._abi import Dependency as NativeDependency
from ._abi import DependencyMatch as NativeDependencyMatch
from ._abi import (
    DependencySet as NativeDependencySet,
)
from ._abi import (
    FileResourceInput as NativeFileResourceInput,
)
from ._abi import Job as NativeJob
from ._abi import (
    MediaRootMapping as NativeMediaRootMapping,
)
from ._abi import (
    MetadataInput as NativeMetadataInput,
)
from ._abi import (
    MetadataValue as NativeMetadataValue,
)
from ._abi import (
    Production as NativeProduction,
)
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
from ._model import (
    Activity,
    ActivityCreatedEvent,
    ActivityEdge,
    ActivityEdgeSnapshot,
    ActivityId,
    ActivityInputAddedEvent,
    ActivityOutputAddedEvent,
    ActivitySpec,
    AgentIdentity,
    ArtifactEvaluation,
    ArtifactReproducibility,
    Asset,
    AssetId,
    AssetImportedEvent,
    AvailabilityIssue,
    AvailabilityIssueKind,
    ContentStructureKind,
    Dependency,
    DependencyMatch,
    DependencySet,
    DependencySetRecordedEvent,
    DependencySetStatus,
    EvidenceKind,
    ExternalIdentifier,
    ExternalIdentifierAddedEvent,
    ExternalIdentifierRemovedEvent,
    FileResourceInput,
    FilteredRevisionPage,
    Fingerprint,
    FingerprintSnapshot,
    HostObjectBinding,
    ImageSequenceDescriptor,
    ImageSequenceInput,
    Job,
    JobCancelledEvent,
    JobClaim,
    JobClaimedEvent,
    JobClaimId,
    JobClaimReleasedEvent,
    JobClaimRenewedEvent,
    JobCompletion,
    JobFailedEvent,
    JobId,
    JobRequest,
    JobRequestedEvent,
    JobState,
    JobSucceededEvent,
    Locator,
    LocatorAddedEvent,
    LocatorAvailability,
    LocatorId,
    LocatorMatch,
    LocatorRetiredEvent,
    MediaRoot,
    MediaRootAddedEvent,
    MediaRootEnabledChangedEvent,
    MediaRootId,
    MediaRootRemovedEvent,
    MetadataAddedOrReplacedEvent,
    MetadataAssertion,
    MetadataBool,
    MetadataBytes,
    MetadataDecimal,
    MetadataI64,
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
    ObjectReference,
    OriginIdentity,
    ProductionId,
    ProvenanceMatch,
    QueryPage,
    RegenerationJobPlan,
    Representation,
    RepresentationAddedEvent,
    RepresentationAvailability,
    RepresentationFingerprintObservedEvent,
    RepresentationId,
    RepresentationKind,
    RepresentationMember,
    RepresentationResolution,
    RepresentationResourceAddedEvent,
    ResolutionCandidate,
    ResolutionEvidence,
    Resource,
    ResourceAddedEvent,
    ResourceFingerprintObservedEvent,
    ResourceId,
    ResourceResolution,
    ResourceResolutionState,
    Revision,
    RevisionContext,
    RevisionEvent,
    RevisionEventPayload,
    RevisionId,
    RevisionWait,
    RevisionWaitResult,
    ToolIdentity,
    TransactionId,
)
from ._native import NativeLibrary

_ObjectQueryItem = TypeVar("_ObjectQueryItem")


class _Assets:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __contains__(self, asset_id: object) -> bool:
        if not isinstance(asset_id, AssetId):
            return False
        return self._production._contains_asset(asset_id)

    def __iter__(self) -> Iterator[Asset]:
        return iter(self._production._assets())

    def __len__(self) -> int:
        return len(self._production._assets())


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

    def __getitem__(self, key: tuple[str, str]) -> tuple[ObjectReference, ...]:
        scheme, value = key
        return self._production._find_by_external_identifier(scheme, value)


class _Metadata:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(self, target: ObjectReference) -> tuple[MetadataAssertion, ...]:
        return self._production._metadata(target)


class _MetadataByProperty:
    def __init__(self, production: Production) -> None:
        self._production = production

    def __getitem__(self, property: MetadataProperty) -> tuple[MetadataAssertion, ...]:
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
        production_id = _native_uuid(self._production.id.value)
        native_target = _native_object_reference(target)
        binding = ctypes.c_char_p()
        error = ctypes.POINTER(Error)()
        status = self._production._native.lib.pp_host_binding_format(
            ctypes.byref(production_id),
            ctypes.byref(native_target),
            ctypes.byref(binding),
            ctypes.byref(error),
        )
        self._production._native.check(status, error)
        try:
            return _decode_required(binding.value, "host binding")
        finally:
            self._production._native.lib.pp_host_binding_release(binding)

    def parse(self, value: str) -> HostObjectBinding:
        production_id = Uuid()
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
            raise RuntimeError("native production creation returned no handle")
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
            raise RuntimeError("native production open returned no handle")
        return cls(native, handle)

    @property
    def id(self) -> ProductionId:
        """Return this production's stable identity."""

        self._require_open()
        value = Uuid()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_id(
            self._handle, ctypes.byref(value), ctypes.byref(error)
        )
        self._native.check(status, error)
        return ProductionId(_uuid(value))

    @property
    def assets(self) -> _Assets:
        """Return an iterable asset collection with identity membership checks."""

        self._require_open()
        return _Assets(self)

    @property
    def media_roots(self) -> tuple[MediaRoot, ...]:
        """Return configured resolver roots in priority order."""

        self._require_open()
        handle = ctypes.POINTER(MediaRootSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_media_roots(
            self._handle, ctypes.byref(handle), ctypes.byref(error)
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native media-root query returned no result set")
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
        native_id = _native_uuid(job_id.value)
        handle = ctypes.POINTER(JobSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_job(
            self._handle,
            ctypes.byref(native_id),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native job read returned no result set")
        try:
            if self._native.lib.pp_job_set_count(handle) != 1:
                raise RuntimeError("native job read returned no single job")
            return _job_at(self._native, handle, 0)
        finally:
            self._native.lib.pp_job_set_release(handle)

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
        handle = ctypes.POINTER(JobSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_jobs(
            self._handle,
            0 if state is None else _native_job_state(state),
            _optional_text(kind),
            limit,
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native job query returned no result set")
        try:
            count = self._native.lib.pp_job_set_count(handle)
            return QueryPage(
                tuple(
                    _job_at(self._native, handle, index) for index in range(int(count))
                ),
                _decode_optional(self._native.lib.pp_job_set_next_cursor(handle)),
            )
        finally:
            self._native.lib.pp_job_set_release(handle)

    def plan_regeneration(
        self, artifact_representation_ids: Iterable[RepresentationId]
    ) -> tuple[RegenerationJobPlan, ...]:
        """Derive job proposals without enqueuing or executing them."""

        self._require_open()
        artifact_ids = tuple(artifact_representation_ids)
        native_ids = (Uuid * len(artifact_ids))(
            *(_native_uuid(artifact_id.value) for artifact_id in artifact_ids)
        )
        handle = ctypes.POINTER(RegenerationPlanSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_plan_regeneration(
            self._handle,
            native_ids if artifact_ids else None,
            len(artifact_ids),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native regeneration query returned no result set")
        try:
            return tuple(
                _regeneration_plan_at(self._native, handle, index)
                for index in range(
                    int(self._native.lib.pp_regeneration_plan_set_count(handle))
                )
            )
        finally:
            self._native.lib.pp_regeneration_plan_set_release(handle)

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
        """Return object matches keyed by ``(scheme, value)``."""

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

    @property
    def resolutions(self) -> _Resolutions:
        """Return representation-resolution results keyed by asset identity."""

        self._require_open()
        return _Resolutions(self)

    def resolve(
        self,
        asset_id: AssetId,
        root_mappings: Mapping[str, str | os.PathLike[str]] | None = None,
    ) -> tuple[RepresentationResolution, ...]:
        """Resolve an asset using optional machine-local root mappings."""

        return self._resolve_asset(asset_id, root_mappings or {})

    def evaluate_artifact(
        self,
        representation_id: RepresentationId,
        *,
        max_depth: int = 64,
        max_representations: int = 1_000,
    ) -> ArtifactEvaluation:
        """Evaluate stored artifact evidence without accessing media files."""

        self._require_open()
        native_id = _native_uuid(representation_id.value)
        handle = ctypes.POINTER(NativeArtifactEvaluation)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_evaluate_artifact(
            self._handle,
            ctypes.byref(native_id),
            max_depth,
            max_representations,
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native artifact evaluation returned no result")
        try:
            return read_evaluation(self._native, handle)
        finally:
            self._native.lib.pp_artifact_evaluation_release(handle)

    def artifact_reproducibility(
        self, representation_id: RepresentationId
    ) -> ArtifactReproducibility:
        """Report whether stored knowledge is sufficient to reproduce an artifact."""

        self._require_open()
        native_id = _native_uuid(representation_id.value)
        handle = ctypes.POINTER(NativeArtifactReproducibility)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_artifact_reproducibility(
            self._handle,
            ctypes.byref(native_id),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native artifact reproducibility returned no report")
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
        native_id = _native_uuid(representation_id.value)
        handle = ctypes.POINTER(NativeDependencySet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_dependency_set(
            self._handle,
            ctypes.byref(native_id),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native dependency query returned no result set")
        try:
            present = ctypes.c_uint8()
            source_id = Uuid()
            revision = ctypes.c_uint64()
            set_status = _abi.DependencySetStatus()
            count = ctypes.c_uint64()
            summary_error = ctypes.POINTER(Error)()
            summary_status = self._native.lib.pp_dependency_set_get(
                handle,
                ctypes.byref(present),
                ctypes.byref(source_id),
                ctypes.byref(revision),
                ctypes.byref(set_status),
                ctypes.byref(count),
                ctypes.byref(summary_error),
            )
            self._native.check(summary_status, summary_error)
            if not present.value:
                return None
            return DependencySet(
                RepresentationId(_uuid(source_id)),
                int(revision.value),
                _dependency_set_status(int(set_status.value)),
                tuple(
                    _dependency_at(self._native, handle, index)
                    for index in range(int(count.value))
                ),
            )
        finally:
            self._native.lib.pp_dependency_set_release(handle)

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
        native_id = _native_uuid(representation_id.value)
        handle = ctypes.POINTER(DependencyQuerySet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_dependencies(
            self._handle,
            ctypes.byref(native_id),
            max_depth,
            max_representations,
            limit,
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native dependency query returned no result set")
        try:
            return _dependency_query_page(self._native, handle)
        finally:
            self._native.lib.pp_dependency_query_set_release(handle)

    def dependents(
        self,
        target: AssetId | RepresentationId,
        *,
        max_depth: int,
        max_representations: int,
        limit: int,
        cursor: str | None = None,
    ) -> QueryPage[DependencyMatch]:
        """Return one bounded page of direct or transitive dependents."""

        self._require_open()
        native_target = _native_object_reference(target)
        handle = ctypes.POINTER(DependencyQuerySet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_dependents(
            self._handle,
            ctypes.byref(native_target),
            max_depth,
            max_representations,
            limit,
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native dependents query returned no result set")
        try:
            return _dependency_query_page(self._native, handle)
        finally:
            self._native.lib.pp_dependency_query_set_release(handle)

    def asset(self, asset_id: AssetId) -> Asset:
        """Return one asset, raising ``NotFoundError`` when it is absent."""

        self._require_open()
        native_id = _native_uuid(asset_id.value)
        handle = ctypes.POINTER(AssetSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_asset(
            self._handle,
            ctypes.byref(native_id),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native asset read returned no result set")
        try:
            if self._native.lib.pp_asset_set_count(handle) != 1:
                raise RuntimeError("native asset read returned no single asset")
            return _asset_at(self._native, handle, 0)
        finally:
            self._native.lib.pp_asset_set_release(handle)

    def representation(self, representation_id: RepresentationId) -> Representation:
        """Return one representation, raising ``NotFoundError`` when absent."""

        self._require_open()
        native_id = _native_uuid(representation_id.value)
        page = self._representation_page(
            self._native.lib.pp_production_representation,
            self._handle,
            ctypes.byref(native_id),
        )
        if len(page.items) != 1:
            raise RuntimeError(
                "native representation read returned no single representation"
            )
        return page.items[0]

    def representations_using_resource(
        self, resource_id: ResourceId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[Representation]:
        """Return one bounded page of representations that use a resource."""

        self._require_open()
        native_id = _native_uuid(resource_id.value)
        return self._representation_page(
            self._native.lib.pp_production_representations_using_resource,
            self._handle,
            ctypes.byref(native_id),
            limit,
            _optional_text(cursor),
        )

    def assets_page(self, *, limit: int, cursor: str | None = None) -> QueryPage[Asset]:
        """Return one bounded asset page in creation and identity order."""

        self._require_open()
        handle = ctypes.POINTER(AssetSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_assets_page(
            self._handle,
            limit,
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native asset query returned no result set")
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
        native_id = _native_uuid(asset_id.value)
        return self._representation_page(
            self._native.lib.pp_production_representations_page,
            self._handle,
            ctypes.byref(native_id),
            limit,
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
            limit,
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
        native_id = _native_uuid(representation_id.value)
        return self._object_query_page(
            self._native.lib.pp_production_resources_page,
            _resource_match,
            self._handle,
            ctypes.byref(native_id),
            limit,
            _optional_text(cursor),
        )

    def locators_page(
        self, resource_id: ResourceId, *, limit: int, cursor: str | None = None
    ) -> QueryPage[LocatorMatch]:
        """Return one bounded page of locators belonging to a resource."""

        self._require_open()
        native_id = _native_uuid(resource_id.value)
        handle = ctypes.POINTER(LocatorQuerySet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_locators_page(
            self._handle,
            ctypes.byref(native_id),
            limit,
            _optional_text(cursor),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native locator query returned no result set")
        try:
            count = self._native.lib.pp_locator_query_set_count(handle)
            return QueryPage(
                tuple(
                    _locator_match_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(
                    self._native.lib.pp_locator_query_set_next_cursor(handle)
                ),
            )
        finally:
            self._native.lib.pp_locator_query_set_release(handle)

    def unresolved_media(
        self, *, limit: int, cursor: str | None = None
    ) -> QueryPage[RepresentationId]:
        """Return representations whose required resources lack locators."""

        self._require_open()
        return self._object_query_page(
            self._native.lib.pp_production_unresolved_media,
            _representation_match,
            self._handle,
            limit,
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
            sequence,
            limit,
            _optional_text(cursor),
        )

    def query_metadata(
        self,
        property: MetadataProperty,
        *,
        limit: int,
        cursor: str | None = None,
        value: MetadataValue | None = None,
    ) -> QueryPage[MetadataAssertion]:
        """Return one bounded assertion page with an optional exact scalar value."""

        self._require_open()
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
                limit,
                native_cursor,
                ctypes.byref(handle),
                ctypes.byref(error),
            )
        finally:
            if native_value is not None:
                self._native.lib.pp_metadata_input_release(native_value)
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native metadata query returned no result set")
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
        native_id = _native_uuid(representation_id.value)
        return self._activity_page(
            self._native.lib.pp_production_activities_producing_page,
            self._handle,
            ctypes.byref(native_id),
            limit,
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
        native_id = _native_uuid(representation_id.value)
        return self._activity_page(
            self._native.lib.pp_production_activities_consuming_page,
            self._handle,
            ctypes.byref(native_id),
            limit,
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
            limit,
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
            limit,
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
        native_id = _native_uuid(representation_id.value)
        return self._object_query_page(
            self._native.lib.pp_production_provenance_ancestors_page,
            _provenance_match,
            self._handle,
            ctypes.byref(native_id),
            max_depth,
            max_representations,
            limit,
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
        native_id = _native_uuid(representation_id.value)
        return self._object_query_page(
            self._native.lib.pp_production_provenance_descendants_page,
            _provenance_match,
            self._handle,
            ctypes.byref(native_id),
            max_depth,
            max_representations,
            limit,
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
        native_source = None if source is None else _native_uuid(source.value)
        return self._object_query_page(
            self._native.lib.pp_production_stale_artifacts,
            _representation_match,
            self._handle,
            None if native_source is None else ctypes.byref(native_source),
            max_depth,
            max_representations,
            limit,
            _optional_text(cursor),
        )

    @property
    def host_bindings(self) -> _HostBindings:
        """Return the portable host-binding formatter and parser."""

        self._require_open()
        return _HostBindings(self)

    def _contains_asset(self, asset_id: AssetId) -> bool:
        """Return whether an asset identity belongs to this production."""

        self._require_open()
        native_id = _native_uuid(asset_id.value)
        exists = ctypes.c_uint8()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_asset_exists(
            self._handle,
            ctypes.byref(native_id),
            ctypes.byref(exists),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return bool(exists.value)

    def _assets(self) -> tuple[Asset, ...]:
        self._require_open()
        handle = ctypes.POINTER(AssetSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_assets(
            self._handle, ctypes.byref(handle), ctypes.byref(error)
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native asset query returned no result set")
        try:
            count = self._native.lib.pp_asset_set_count(handle)
            return tuple(
                _asset_at(self._native, handle, index) for index in range(int(count))
            )
        finally:
            self._native.lib.pp_asset_set_release(handle)

    def _representations(self, asset_id: AssetId) -> tuple[Representation, ...]:
        self._require_open()
        native_id = _native_uuid(asset_id.value)
        handle = ctypes.POINTER(RepresentationSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_representations(
            self._handle,
            ctypes.byref(native_id),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native representation query returned no result set")
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
        native_id = _native_uuid(representation_id.value)
        return self._activity_set(function, self._handle, ctypes.byref(native_id))

    def _provenance_representations(
        self, direction: str, representation_id: RepresentationId
    ) -> tuple[RepresentationId, ...]:
        self._require_open()
        function = {
            "ancestors": self._native.lib.pp_production_provenance_ancestors,
            "descendants": self._native.lib.pp_production_provenance_descendants,
        }[direction]
        native_id = _native_uuid(representation_id.value)
        handle = ctypes.POINTER(ObjectRefSet)()
        error = ctypes.POINTER(Error)()
        status = function(
            self._handle,
            ctypes.byref(native_id),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native provenance query returned no result set")
        try:
            count = self._native.lib.pp_object_ref_set_count(handle)
            result: list[RepresentationId] = []
            for index in range(int(count)):
                reference = _object_reference_at(self._native, handle, index)
                if not isinstance(reference, RepresentationId):
                    raise RuntimeError(
                        "native provenance query returned a non-representation"
                    )
                result.append(reference)
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
            raise RuntimeError("native identifier query returned no result set")
        try:
            count = self._native.lib.pp_external_identifier_set_count(handle)
            return tuple(
                _external_identifier_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_external_identifier_set_release(handle)

    def _find_by_external_identifier(
        self, scheme: str, value: str
    ) -> tuple[ObjectReference, ...]:
        """Find objects carrying an exact external scheme and value."""

        self._require_open()
        handle = ctypes.POINTER(ObjectRefSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_find_by_external_identifier(
            self._handle,
            _utf8(scheme, "identifier scheme"),
            _utf8(value, "identifier value"),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native identifier lookup returned no result set")
        try:
            count = self._native.lib.pp_object_ref_set_count(handle)
            return tuple(
                _object_reference_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_object_ref_set_release(handle)

    def _metadata(self, target: ObjectReference) -> tuple[MetadataAssertion, ...]:
        self._require_open()
        native_target = _native_object_reference(target)
        return self._metadata_set(
            self._native.lib.pp_production_metadata,
            self._handle,
            ctypes.byref(native_target),
        )

    def _metadata_by_property(
        self, property: MetadataProperty
    ) -> tuple[MetadataAssertion, ...]:
        self._require_open()
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
            raise RuntimeError("native latest-revision query returned multiple values")
        return revisions[0] if revisions else None

    def changes_since(self, sequence: int, limit: int) -> tuple[Revision, ...]:
        """Return a bounded ascending page of revisions after ``sequence``."""

        self._require_open()
        return self._revision_set(
            self._native.lib.pp_production_changes_since,
            self._handle,
            sequence,
            limit,
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
            sequence,
            native_kinds,
            len(codes),
            limit,
            ctypes.byref(handle),
            ctypes.byref(through_sequence),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native filtered revision query returned no result set")
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
            raise RuntimeError("native revision waiter creation returned no handle")
        return RevisionWaiter(self._native, handle)

    def _revision_events(self, revision_id: RevisionId) -> tuple[RevisionEvent, ...]:
        """Return the ordered semantic events for one revision."""

        self._require_open()
        native_id = _native_uuid(revision_id.value)
        handle = ctypes.POINTER(RevisionEventSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_revision_events(
            self._handle,
            ctypes.byref(native_id),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native revision-event query returned no result set")
        try:
            count = self._native.lib.pp_revision_event_set_count(handle)
            return tuple(
                _revision_event_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_revision_event_set_release(handle)

    def _resolve_asset(
        self,
        asset_id: AssetId,
        root_mappings: Mapping[str, str | os.PathLike[str]],
    ) -> tuple[RepresentationResolution, ...]:
        self._require_open()
        native_id = _native_uuid(asset_id.value)
        mapping_values = sorted(root_mappings.items())
        mapping_names = [_utf8(name, "root name") for name, _ in mapping_values]
        mapping_directories = [_path_bytes(path) for _, path in mapping_values]
        native_mappings = (NativeMediaRootMapping * len(mapping_values))(
            *(
                NativeMediaRootMapping(name, directory)
                for name, directory in zip(
                    mapping_names, mapping_directories, strict=True
                )
            )
        )
        handle = ctypes.POINTER(ResolutionSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_resolve_asset(
            self._handle,
            ctypes.byref(native_id),
            native_mappings if mapping_values else None,
            len(mapping_values),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native resolution query returned no result set")
        try:
            count = self._native.lib.pp_resolution_set_representation_count(handle)
            return tuple(
                _representation_resolution_at(self._native, handle, index)
                for index in range(int(count))
            )
        finally:
            self._native.lib.pp_resolution_set_release(handle)

    def transaction(
        self,
        *,
        origin: OriginIdentity | str | None = None,
        message: str | None = None,
    ) -> Transaction:
        """Begin a transaction that commits on a clean context-manager exit."""

        self._require_open()
        handle = ctypes.POINTER(NativeTransaction)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_production_begin_transaction(
            self._handle, ctypes.byref(handle), ctypes.byref(error)
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native transaction creation returned no handle")
        transaction = Transaction(self._native, handle)
        if origin is not None or message is not None:
            identity = OriginIdentity(origin) if isinstance(origin, str) else origin
            transaction.set_revision_context(RevisionContext(identity, message))
        return transaction

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
            raise RuntimeError("production is closed")

    def _revision_set(
        self, function: Callable[..., int], *arguments: object
    ) -> tuple[Revision, ...]:
        handle = ctypes.POINTER(RevisionSet)()
        error = ctypes.POINTER(Error)()
        status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native revision query returned no result set")
        try:
            return tuple(
                _revision_at(self._native, handle, index)
                for index in range(int(self._native.lib.pp_revision_set_count(handle)))
            )
        finally:
            self._native.lib.pp_revision_set_release(handle)

    def _activity_set(
        self, function: Callable[..., int], *arguments: object
    ) -> tuple[Activity, ...]:
        handle = ctypes.POINTER(ActivitySet)()
        error = ctypes.POINTER(Error)()
        status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native activity query returned no result set")
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
        handle = ctypes.POINTER(MetadataSet)()
        error = ctypes.POINTER(Error)()
        status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native metadata query returned no result set")
        try:
            count = self._native.lib.pp_metadata_set_count(handle)
            return tuple(
                _metadata_at(self._native, handle, index) for index in range(int(count))
            )
        finally:
            self._native.lib.pp_metadata_set_release(handle)

    def _representation_page(
        self, function: Callable[..., int], *arguments: object
    ) -> QueryPage[Representation]:
        handle = ctypes.POINTER(RepresentationSet)()
        error = ctypes.POINTER(Error)()
        status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native representation query returned no result set")
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
        handle = ctypes.POINTER(ActivitySet)()
        error = ctypes.POINTER(Error)()
        status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native activity query returned no result set")
        try:
            count = self._native.lib.pp_activity_set_count(handle)
            return QueryPage(
                tuple(
                    _activity_at(self._native, handle, index)
                    for index in range(int(count))
                ),
                _decode_optional(self._native.lib.pp_activity_set_next_cursor(handle)),
            )
        finally:
            self._native.lib.pp_activity_set_release(handle)

    def _object_query_page(
        self,
        function: Callable[..., int],
        convert: Callable[[ObjectReference, int], _ObjectQueryItem],
        *arguments: object,
    ) -> QueryPage[_ObjectQueryItem]:
        handle = ctypes.POINTER(ObjectQuerySet)()
        error = ctypes.POINTER(Error)()
        status = function(*arguments, ctypes.byref(handle), ctypes.byref(error))
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native object query returned no result set")
        try:
            count = self._native.lib.pp_object_query_set_count(handle)
            items: list[_ObjectQueryItem] = []
            for index in range(int(count)):
                value = _abi.ObjectRef()
                depth = ctypes.c_uint32()
                item_error = ctypes.POINTER(Error)()
                item_status = self._native.lib.pp_object_query_set_get(
                    handle,
                    index,
                    ctypes.byref(value),
                    ctypes.byref(depth),
                    ctypes.byref(item_error),
                )
                self._native.check(item_status, item_error)
                items.append(convert(_object_reference(value), int(depth.value)))
            return QueryPage(
                tuple(items),
                _decode_optional(
                    self._native.lib.pp_object_query_set_next_cursor(handle)
                ),
                bool(self._native.lib.pp_object_query_set_traversal_truncated(handle)),
            )
        finally:
            self._native.lib.pp_object_query_set_release(handle)


_MAX_REVISION_WAIT_SECONDS = _abi.PP_REVISION_WAIT_MAX_TIMEOUT_MILLIS / 1000
_WAIT_RESULTS = {
    _abi.PP_REVISION_WAIT_REVISIONS: RevisionWaitResult.REVISIONS,
    _abi.PP_REVISION_WAIT_TIMED_OUT: RevisionWaitResult.TIMED_OUT,
    _abi.PP_REVISION_WAIT_CLOSED: RevisionWaitResult.CLOSED,
    _abi.PP_REVISION_WAIT_CANCELLED: RevisionWaitResult.CANCELLED,
}


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
            raise RuntimeError("revision waiter is closed")
        if not 0 <= timeout <= _MAX_REVISION_WAIT_SECONDS:
            raise ValueError("revision wait timeout must be 0-60 seconds")
        result = ctypes.c_uint32()
        handle = ctypes.POINTER(RevisionSet)()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_revision_waiter_wait(
            self._handle,
            after_sequence,
            limit,
            round(timeout * 1000),
            ctypes.byref(result),
            ctypes.byref(handle),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        if not handle:
            raise RuntimeError("native revision wait returned no result set")
        try:
            revisions = _revisions_in(self._native, handle)
        finally:
            self._native.lib.pp_revision_set_release(handle)
        try:
            outcome = _WAIT_RESULTS[result.value]
        except KeyError as error:
            raise RuntimeError(
                f"unknown revision wait result {result.value}"
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
        self._cursor = after_sequence
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


class Transaction:
    """A caller-serialized transaction with context-manager semantics."""

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

    def set_revision_context(self, context: RevisionContext) -> None:
        """Set optional origin and message fields for the future revision."""

        self._require_open()
        origin = context.origin
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_set_revision_context(
            self._handle,
            _optional_text(origin.name if origin else None),
            _optional_text(origin.version if origin else None),
            _optional_text(origin.uri if origin else None),
            _optional_text(context.message),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def import_media(
        self, path: str | os.PathLike[str], display_name: str | None = None
    ) -> AssetId:
        """Prepare and stage one original-media import."""

        self._require_open()
        asset_id = Uuid()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_import_media(
            self._handle,
            _path_bytes(path),
            _optional_text(display_name),
            ctypes.byref(asset_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return AssetId(_uuid(asset_id))

    def add_single_file_representation(
        self,
        asset_id: AssetId,
        kind: RepresentationKind,
        path: str | os.PathLike[str],
    ) -> RepresentationId:
        """Stage one single-file representation for an existing asset."""

        self._require_open()
        native_asset_id = _native_uuid(asset_id.value)
        representation_id = Uuid()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_add_single_file_representation(
            self._handle,
            ctypes.byref(native_asset_id),
            _native_representation_kind(kind),
            _path_bytes(path),
            ctypes.byref(representation_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return RepresentationId(_uuid(representation_id))

    def add_image_sequence_representation(
        self,
        asset_id: AssetId,
        kind: RepresentationKind,
        source: ImageSequenceInput,
    ) -> RepresentationId:
        """Stage one compact image-sequence representation."""

        self._require_open()
        native_asset_id = _native_uuid(asset_id.value)
        missing_type = ctypes.c_int64 * len(source.missing_frames)
        missing_frames = missing_type(*source.missing_frames)
        representation_id = Uuid()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_add_image_sequence_representation(
            self._handle,
            ctypes.byref(native_asset_id),
            _native_representation_kind(kind),
            _path_bytes(source.directory),
            _utf8(source.prefix, "image-sequence prefix"),
            _utf8(source.suffix, "image-sequence suffix"),
            source.padding,
            source.start,
            source.end,
            source.step,
            source.rate_numerator,
            source.rate_denominator,
            missing_frames,
            len(missing_frames),
            ctypes.byref(representation_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return RepresentationId(_uuid(representation_id))

    def add_ordered_parts_representation(
        self,
        asset_id: AssetId,
        kind: RepresentationKind,
        members: tuple[FileResourceInput, ...],
    ) -> RepresentationId:
        """Stage an ordered, fully required multi-file representation."""

        return self._add_file_collection_representation(
            self._native.lib.pp_transaction_add_ordered_parts_representation,
            asset_id,
            kind,
            members,
        )

    def add_package_representation(
        self,
        asset_id: AssetId,
        kind: RepresentationKind,
        members: tuple[FileResourceInput, ...],
    ) -> RepresentationId:
        """Stage a role-bearing package representation."""

        return self._add_file_collection_representation(
            self._native.lib.pp_transaction_add_package_representation,
            asset_id,
            kind,
            members,
        )

    def add_media_root(
        self,
        name: str,
        label: str | None = None,
        priority: int = 0,
    ) -> MediaRootId:
        """Stage a portable logical root used for resource discovery."""

        self._require_open()
        root_id = Uuid()
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
        """Stage a resolver root's enabled state."""

        self._require_open()
        native_id = _native_uuid(root_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_set_media_root_enabled(
            self._handle,
            ctypes.byref(native_id),
            int(enabled),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def remove_media_root(self, root_id: MediaRootId) -> None:
        """Stage removal of one resolver root."""

        self._require_open()
        native_id = _native_uuid(root_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_remove_media_root(
            self._handle, ctypes.byref(native_id), ctypes.byref(error)
        )
        self._native.check(status, error)

    def confirm_locator(self, resource_id: ResourceId, uri: str) -> None:
        """Stage explicit confirmation of one resource candidate URI."""

        self._require_open()
        native_id = _native_uuid(resource_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_confirm_locator(
            self._handle,
            ctypes.byref(native_id),
            _utf8(uri, "locator URI"),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def confirm_locator_under_root(
        self, resource_id: ResourceId, uri: str, root_name: str
    ) -> None:
        """Stage explicit confirmation of a URI under a logical media root.

        The root name is retained as query evidence and need not name a
        currently configured or enabled root.
        """

        self._require_open()
        native_id = _native_uuid(resource_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_confirm_locator_under_root(
            self._handle,
            ctypes.byref(native_id),
            _utf8(uri, "locator URI"),
            _utf8(root_name, "root name"),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def retire_locator(self, locator_id: LocatorId) -> None:
        """Stage retirement of one superseded resource locator."""

        self._require_open()
        native_id = _native_uuid(locator_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_retire_locator(
            self._handle, ctypes.byref(native_id), ctypes.byref(error)
        )
        self._native.check(status, error)

    def record_resource_fingerprint(
        self, resource_id: ResourceId, fingerprint: Fingerprint
    ) -> None:
        """Stage an explicit content-fingerprint observation for one resource."""

        self._require_open()
        native_id = _native_uuid(resource_id.value)
        value = (ctypes.c_uint8 * len(fingerprint.value)).from_buffer_copy(
            fingerprint.value
        )
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_record_resource_fingerprint(
            self._handle,
            ctypes.byref(native_id),
            _utf8(fingerprint.algorithm, "fingerprint algorithm"),
            fingerprint.version,
            value,
            len(fingerprint.value),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def record_representation_fingerprint(
        self, representation_id: RepresentationId, fingerprint: Fingerprint
    ) -> None:
        """Stage a structure-aware fingerprint observation for a representation."""

        self._require_open()
        native_id = _native_uuid(representation_id.value)
        value = (ctypes.c_uint8 * len(fingerprint.value)).from_buffer_copy(
            fingerprint.value
        )
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_record_representation_fingerprint(
            self._handle,
            ctypes.byref(native_id),
            _utf8(fingerprint.algorithm, "fingerprint algorithm"),
            fingerprint.version,
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
                        _native_uuid(value.source_resource_id.value)
                        if value.source_resource_id is not None
                        else Uuid()
                    ),
                    kinds[index],
                    _native_object_reference(value.target),
                    int(value.resolved_representation_id is not None),
                    (
                        _native_uuid(value.resolved_representation_id.value)
                        if value.resolved_representation_id is not None
                        else Uuid()
                    ),
                    int(value.required),
                    authored_references[index],
                )
                for index, value in enumerate(dependencies)
            )
        )
        native_id = _native_uuid(representation_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_record_dependency_set(
            self._handle,
            ctypes.byref(native_id),
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
        property: MetadataProperty,
        value: MetadataValue,
    ) -> None:
        """Stage one typed metadata assertion."""

        self._require_open()
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
        input_type = Uuid * len(request.inputs)
        inputs = input_type(*(_native_uuid(value.value) for value in request.inputs))
        output_asset_id = _native_uuid(request.output_asset_id.value)
        job_id = Uuid()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_request_job(
            self._handle,
            _utf8(request.kind, "job kind"),
            inputs if request.inputs else None,
            len(request.inputs),
            ctypes.byref(output_asset_id),
            _native_representation_kind(request.output_representation_kind),
            _optional_text(request.target_root),
            ctypes.byref(job_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return JobId(_uuid(job_id))

    def claim_job(
        self,
        job_id: JobId,
        tool: ToolIdentity,
        agent: AgentIdentity | None,
        now_unix_micros: int,
        expires_at_unix_micros: int,
    ) -> JobClaimId:
        """Stage an atomic claim and return its capability token."""

        self._require_open()
        native_job_id = _native_uuid(job_id.value)
        identifier = agent.identifier if agent is not None else None
        claim_id = Uuid()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_claim_job(
            self._handle,
            ctypes.byref(native_job_id),
            _utf8(tool.name, "tool name"),
            _optional_text(tool.version),
            _optional_text(tool.uri),
            _optional_text(agent.name if agent else None),
            _optional_text(identifier.scheme if identifier else None),
            _optional_text(identifier.value if identifier else None),
            _optional_text(identifier.qualifier if identifier else None),
            now_unix_micros,
            expires_at_unix_micros,
            ctypes.byref(claim_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return JobClaimId(_uuid(claim_id))

    def renew_job_claim(
        self,
        job_id: JobId,
        claim_id: JobClaimId,
        now_unix_micros: int,
        expires_at_unix_micros: int,
    ) -> None:
        """Stage renewal of an active job claim."""

        self._require_open()
        native_job_id = _native_uuid(job_id.value)
        native_claim_id = _native_uuid(claim_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_renew_job_claim(
            self._handle,
            ctypes.byref(native_job_id),
            ctypes.byref(native_claim_id),
            now_unix_micros,
            expires_at_unix_micros,
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def release_job_claim(self, job_id: JobId, claim_id: JobClaimId) -> None:
        """Stage release of an active job claim."""

        self._require_open()
        native_job_id = _native_uuid(job_id.value)
        native_claim_id = _native_uuid(claim_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_release_job_claim(
            self._handle,
            ctypes.byref(native_job_id),
            ctypes.byref(native_claim_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def complete_job(
        self,
        job_id: JobId,
        claim_id: JobClaimId,
        now_unix_micros: int,
        output_representation_id: RepresentationId,
        activity_id: ActivityId,
    ) -> None:
        """Bind a staged representation and activity into one completion."""

        self._require_open()
        native_job_id = _native_uuid(job_id.value)
        native_claim_id = _native_uuid(claim_id.value)
        native_output_id = _native_uuid(output_representation_id.value)
        native_activity_id = _native_uuid(activity_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_complete_job(
            self._handle,
            ctypes.byref(native_job_id),
            ctypes.byref(native_claim_id),
            now_unix_micros,
            ctypes.byref(native_output_id),
            ctypes.byref(native_activity_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def fail_job(
        self,
        job_id: JobId,
        claim_id: JobClaimId,
        now_unix_micros: int,
        diagnostic: str,
    ) -> None:
        """Stage failure of an active, unexpired job claim."""

        self._require_open()
        native_job_id = _native_uuid(job_id.value)
        native_claim_id = _native_uuid(claim_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_fail_job(
            self._handle,
            ctypes.byref(native_job_id),
            ctypes.byref(native_claim_id),
            now_unix_micros,
            _utf8(diagnostic, "job failure diagnostic"),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def cancel_job(self, job_id: JobId) -> None:
        """Stage administrative cancellation of a requested or claimed job."""

        self._require_open()
        native_job_id = _native_uuid(job_id.value)
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_cancel_job(
            self._handle,
            ctypes.byref(native_job_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)

    def create_activity(self, spec: ActivitySpec) -> ActivityId:
        """Stage one complete provenance activity."""

        self._require_open()
        inputs = _native_activity_edges(spec.inputs)
        outputs = _native_activity_edges(spec.outputs)
        started_at = _optional_i64(spec.started_at_unix_micros)
        finished_at = _optional_i64(spec.finished_at_unix_micros)
        tool = spec.tool
        agent = spec.agent
        identifier = agent.identifier if agent is not None else None
        activity_id = Uuid()
        error = ctypes.POINTER(Error)()
        status = self._native.lib.pp_transaction_create_activity(
            self._handle,
            _utf8(spec.kind, "activity kind"),
            inputs,
            len(inputs),
            outputs,
            len(outputs),
            None if started_at is None else ctypes.byref(started_at),
            None if finished_at is None else ctypes.byref(finished_at),
            _optional_text(tool.name if tool else None),
            _optional_text(tool.version if tool else None),
            _optional_text(tool.uri if tool else None),
            _optional_text(agent.name if agent else None),
            _optional_text(identifier.scheme if identifier else None),
            _optional_text(identifier.value if identifier else None),
            _optional_text(identifier.qualifier if identifier else None),
            ctypes.byref(activity_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return ActivityId(_uuid(activity_id))

    def remove_metadata_property(
        self, target: ObjectReference, property: MetadataProperty
    ) -> None:
        """Stage removal of every assertion for one target and property."""

        self._require_open()
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

    def commit(self) -> None:
        """Atomically persist every staged mutation."""

        self._finish(self._native.lib.pp_transaction_commit)

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
            if not self._finished:
                if exception_type is None:
                    self.commit()
                else:
                    self.rollback()
        finally:
            self.close()

    def _finish(self, function: Callable[..., int]) -> None:
        self._require_open()
        error = ctypes.POINTER(Error)()
        status = function(self._handle, ctypes.byref(error))
        self._native.check(status, error)
        self._finished = True

    def _add_file_collection_representation(
        self,
        function: Callable[..., int],
        asset_id: AssetId,
        kind: RepresentationKind,
        members: tuple[FileResourceInput, ...],
    ) -> RepresentationId:
        self._require_open()
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
        native_asset_id = _native_uuid(asset_id.value)
        representation_id = Uuid()
        error = ctypes.POINTER(Error)()
        status = function(
            self._handle,
            ctypes.byref(native_asset_id),
            _native_representation_kind(kind),
            native_members,
            len(native_members),
            ctypes.byref(representation_id),
            ctypes.byref(error),
        )
        self._native.check(status, error)
        return RepresentationId(_uuid(representation_id))

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
            raise RuntimeError("transaction is closed")
        if self._finished:
            raise RuntimeError("transaction is already finished")


def _path_bytes(path: str | os.PathLike[str]) -> bytes:
    return _utf8(str(Path(path)), "path")


def _optional_text(value: str | None) -> bytes | None:
    return None if value is None else _utf8(value, "text")


def _utf8(value: str, label: str) -> bytes:
    encoded = value.encode("utf-8")
    if b"\0" in encoded:
        raise ValueError(f"{label} must not contain NUL")
    return encoded


def _uuid(value: Uuid) -> UUID:
    return UUID(bytes=bytes(value.bytes))


def _native_uuid(value: UUID) -> Uuid:
    native = Uuid()
    native.bytes[:] = value.bytes
    return native


def _native_activity_edges(
    edges: tuple[ActivityEdge, ...],
) -> ctypes.Array[NativeActivityEdge]:
    array_type = NativeActivityEdge * len(edges)
    return array_type(
        *(
            NativeActivityEdge(
                _native_uuid(edge.representation_id.value), _optional_text(edge.role)
            )
            for edge in edges
        )
    )


def _optional_i64(value: int | None) -> ctypes.c_int64 | None:
    return None if value is None else ctypes.c_int64(value)


def _native_object_reference(value: ObjectReference) -> _abi.ObjectRef:
    native = _abi.ObjectRef()
    if isinstance(value, ProductionId):
        native.kind = _abi.PP_OBJECT_PRODUCTION
    elif isinstance(value, AssetId):
        native.kind = _abi.PP_OBJECT_ASSET
    elif isinstance(value, RepresentationId):
        native.kind = _abi.PP_OBJECT_REPRESENTATION
    elif isinstance(value, ResourceId):
        native.kind = _abi.PP_OBJECT_RESOURCE
    elif isinstance(value, ActivityId):
        native.kind = _abi.PP_OBJECT_ACTIVITY
    elif isinstance(value, JobId):
        native.kind = _abi.PP_OBJECT_JOB
    else:
        raise TypeError("unsupported object reference")
    native.id = _native_uuid(value.value)
    return native


def _native_representation_kind(value: RepresentationKind) -> int:
    return {
        RepresentationKind.ORIGINAL: _abi.PP_REPRESENTATION_ORIGINAL,
        RepresentationKind.PROXY: _abi.PP_REPRESENTATION_PROXY,
        RepresentationKind.OPTIMIZED: _abi.PP_REPRESENTATION_OPTIMIZED,
        RepresentationKind.DERIVED: _abi.PP_REPRESENTATION_DERIVED,
    }[value]


def _asset_at(native: NativeLibrary, assets: _Pointer[AssetSet], index: int) -> Asset:
    asset_id = Uuid()
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


def _media_root_at(
    native: NativeLibrary, roots: _Pointer[MediaRootSet], index: int
) -> MediaRoot:
    root_id = Uuid()
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
        raise RuntimeError("native metadata input creation returned no handle")
    return handle


def _metadata_input(
    native: NativeLibrary, value: MetadataValue
) -> _Pointer[NativeMetadataInput]:
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
            native, native.lib.pp_metadata_input_create_i64, value.value
        )
    if isinstance(value, MetadataU64):
        return _create_metadata_input(
            native, native.lib.pp_metadata_input_create_u64, value.value
        )
    if isinstance(value, MetadataDecimal):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_decimal,
            _utf8(str(value.coefficient), "metadata decimal coefficient"),
            value.scale,
        )
    if isinstance(value, MetadataBool):
        return _create_metadata_input(
            native, native.lib.pp_metadata_input_create_bool, int(value.value)
        )
    if isinstance(value, MetadataTimestamp):
        return _create_metadata_input(
            native,
            native.lib.pp_metadata_input_create_timestamp,
            value.unix_micros,
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
            value.numerator,
            value.denominator,
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
    if not isinstance(target, (AssetId, RepresentationId)):
        raise RuntimeError("native dependency has an unsupported target")
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
        input_id = Uuid()
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
    claim = None
    if state is JobState.CLAIMED:
        identifier = None
        scheme = _decode_optional(value.claim_agent_identifier_scheme)
        identifier_value = _decode_optional(value.claim_agent_identifier_value)
        if (scheme is None) != (identifier_value is None):
            raise RuntimeError(
                "native job claim returned an incomplete agent identifier"
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
        claim = JobClaim(
            JobClaimId(_uuid(value.claim_id)),
            ToolIdentity(
                _decode_required(value.claim_tool_name, "job claim tool name"),
                _decode_optional(value.claim_tool_version),
                _decode_optional(value.claim_tool_uri),
            ),
            agent,
            int(value.claim_expires_at_unix_micros),
        )
    completion = None
    if state is JobState.SUCCEEDED:
        completion = JobCompletion(
            ActivityId(_uuid(value.completion_activity_id)),
            RepresentationId(_uuid(value.completion_representation_id)),
        )
    return Job(
        id=JobId(_uuid(value.id)),
        kind=_decode_required(value.kind, "job kind"),
        inputs=tuple(inputs),
        output_asset_id=AssetId(_uuid(value.output_asset_id)),
        output_representation_kind=_representation_kind(
            int(value.output_representation_kind)
        ),
        target_root=_decode_optional(value.target_root),
        state=state,
        claim=claim,
        completion=completion,
        failure_diagnostic=_decode_optional(value.failure_diagnostic),
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
        if not isinstance(target, (AssetId, RepresentationId)):
            raise RuntimeError("native dependency query returned an invalid target")
        items.append(DependencyMatch(target, int(value.depth)))
    return QueryPage(
        tuple(items),
        _decode_optional(native.lib.pp_dependency_query_set_next_cursor(matches)),
        bool(native.lib.pp_dependency_query_set_traversal_truncated(matches)),
    )


def _resource_match(reference: ObjectReference, _depth: int) -> ResourceId:
    if not isinstance(reference, ResourceId):
        raise RuntimeError("native resource query returned a non-resource")
    return reference


def _representation_match(reference: ObjectReference, _depth: int) -> RepresentationId:
    if not isinstance(reference, RepresentationId):
        raise RuntimeError("native representation query returned a non-representation")
    return reference


def _object_match(reference: ObjectReference, _depth: int) -> ObjectReference:
    return reference


def _provenance_match(reference: ObjectReference, depth: int) -> ProvenanceMatch:
    if not isinstance(reference, RepresentationId):
        raise RuntimeError("native provenance query returned a non-representation")
    return ProvenanceMatch(reference, depth)


def _locator_match_at(
    native: NativeLibrary, locators: _Pointer[LocatorQuerySet], index: int
) -> LocatorMatch:
    locator_id = Uuid()
    resource_id = Uuid()
    uri = ctypes.c_char_p()
    availability = _abi.LocatorAvailability()
    has_last_seen = ctypes.c_uint8()
    last_seen = ctypes.c_int64()
    media_root = ctypes.c_char_p()
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
        ),
        _decode_optional(media_root.value),
    )


def _regeneration_plan_at(
    native: NativeLibrary,
    plans: _Pointer[RegenerationPlanSet],
    index: int,
) -> RegenerationJobPlan:
    artifact_id = Uuid()
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
        raise RuntimeError("native regeneration plan is incomplete")
    try:
        if native.lib.pp_job_set_count(jobs) != 1:
            raise RuntimeError("native regeneration plan must contain one job")
        job = _job_at(native, jobs, 0)
        assertions = tuple(
            _metadata_at(native, parameters, parameter_index)
            for parameter_index in range(
                int(native.lib.pp_metadata_set_count(parameters))
            )
        )
        if any(assertion.target != job.id for assertion in assertions):
            raise RuntimeError(
                "native regeneration parameter target does not match its job"
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
    activity_id = Uuid()
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
    representation_id = Uuid()
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
    representation_id = Uuid()
    asset_id = Uuid()
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
    return Representation(
        RepresentationId(_uuid(representation_id)),
        AssetId(_uuid(asset_id)),
        _representation_kind(int(kind.value)),
        structure,
        tuple(
            _representation_member_at(native, representations, index, member_index)
            for member_index in range(int(member_count.value))
        ),
        _image_sequence_at(native, representations, index)
        if structure is ContentStructureKind.IMAGE_SEQUENCE
        else None,
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
    resource_id = Uuid()
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
    prefix = ctypes.c_char_p()
    suffix = ctypes.c_char_p()
    padding = ctypes.c_uint8()
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
        ctypes.byref(prefix),
        ctypes.byref(suffix),
        ctypes.byref(padding),
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
        _decode_required(prefix.value, "image-sequence prefix"),
        _decode_required(suffix.value, "image-sequence suffix"),
        int(padding.value),
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
    resource_id = Uuid()
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
    locator_id = Uuid()
    uri = ctypes.c_char_p()
    availability = _abi.LocatorAvailability()
    has_last_seen = ctypes.c_uint8()
    last_seen = ctypes.c_int64()
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
        ctypes.byref(error),
    )
    native.check(status, error)
    return Locator(
        LocatorId(_uuid(locator_id)),
        _decode_required(uri.value, "locator URI"),
        _locator_availability(int(availability.value)),
        int(last_seen.value) if has_last_seen.value else None,
    )


def _representation_resolution_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
) -> RepresentationResolution:
    representation_id = Uuid()
    availability = _abi.RepresentationAvailability()
    resource_count = ctypes.c_uint64()
    issue_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_representation(
        resolutions,
        representation_index,
        ctypes.byref(representation_id),
        ctypes.byref(availability),
        ctypes.byref(resource_count),
        ctypes.byref(issue_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    return RepresentationResolution(
        RepresentationId(_uuid(representation_id)),
        _representation_availability(int(availability.value)),
        tuple(
            _resource_resolution_at(
                native, resolutions, representation_index, resource_index
            )
            for resource_index in range(int(resource_count.value))
        ),
        tuple(
            _availability_issue_at(
                native, resolutions, representation_index, issue_index
            )
            for issue_index in range(int(issue_count.value))
        ),
    )


def _resource_resolution_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
    resource_index: int,
) -> ResourceResolution:
    resource_id = Uuid()
    state = _abi.ResourceResolutionState()
    candidate_count = ctypes.c_uint64()
    evidence_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_resource(
        resolutions,
        representation_index,
        resource_index,
        ctypes.byref(resource_id),
        ctypes.byref(state),
        ctypes.byref(candidate_count),
        ctypes.byref(evidence_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ResourceResolution(
        ResourceId(_uuid(resource_id)),
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


def _resolution_candidate_at(
    native: NativeLibrary,
    resolutions: _Pointer[ResolutionSet],
    representation_index: int,
    resource_index: int,
    candidate_index: int,
) -> ResolutionCandidate:
    uri = ctypes.c_char_p()
    confidence = ctypes.c_uint16()
    evidence_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_resolution_set_get_candidate(
        resolutions,
        representation_index,
        resource_index,
        candidate_index,
        ctypes.byref(uri),
        ctypes.byref(confidence),
        ctypes.byref(evidence_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ResolutionCandidate(
        _decode_required(uri.value, "resolution candidate URI"),
        int(confidence.value),
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
    resource_id = Uuid()
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
    return AvailabilityIssue(
        ResourceId(_uuid(resource_id)),
        bool(required.value),
        _availability_issue_kind(int(kind.value)),
        tuple(
            _missing_frame_at(
                native, resolutions, representation_index, issue_index, frame_index
            )
            for frame_index in range(int(frame_count.value))
        ),
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
        raise RuntimeError("native metadata assertion is missing its value")
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
            raise RuntimeError("native metadata decimal is invalid") from exception
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
                raise RuntimeError("native metadata list item is missing")
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
                raise RuntimeError("native metadata structure field is missing")
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

    raise RuntimeError("metadata value has an unknown semantic kind")


def _revision_at(
    native: NativeLibrary,
    revisions: _Pointer[RevisionSet],
    index: int,
) -> Revision:
    revision_id = Uuid()
    sequence = ctypes.c_uint64()
    transaction_id = Uuid()
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
        raise RuntimeError("revision event has an unknown semantic kind")

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
        return ProductionId(object_id)
    if kind == _abi.PP_OBJECT_ASSET:
        return AssetId(object_id)
    if kind == _abi.PP_OBJECT_REPRESENTATION:
        return RepresentationId(object_id)
    if kind == _abi.PP_OBJECT_RESOURCE:
        return ResourceId(object_id)
    if kind == _abi.PP_OBJECT_ACTIVITY:
        return ActivityId(object_id)
    if kind == _abi.PP_OBJECT_JOB:
        return JobId(object_id)
    raise RuntimeError("revision event has an unknown object-reference kind")


def _representation_kind(value: int) -> RepresentationKind:
    result = {
        _abi.PP_REPRESENTATION_ORIGINAL: RepresentationKind.ORIGINAL,
        _abi.PP_REPRESENTATION_PROXY: RepresentationKind.PROXY,
        _abi.PP_REPRESENTATION_OPTIMIZED: RepresentationKind.OPTIMIZED,
        _abi.PP_REPRESENTATION_DERIVED: RepresentationKind.DERIVED,
    }.get(value)
    if result is None:
        raise RuntimeError("representation has an unknown kind")
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
        raise RuntimeError("job has an unknown state")
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
        raise RuntimeError("dependency set has an unknown status")
    return result


def _content_structure_kind(value: int) -> ContentStructureKind:
    result = {
        _abi.PP_CONTENT_SINGLE_RESOURCE: ContentStructureKind.SINGLE_RESOURCE,
        _abi.PP_CONTENT_IMAGE_SEQUENCE: ContentStructureKind.IMAGE_SEQUENCE,
        _abi.PP_CONTENT_ORDERED_PARTS: ContentStructureKind.ORDERED_PARTS,
        _abi.PP_CONTENT_PACKAGE: ContentStructureKind.PACKAGE,
    }.get(value)
    if result is None:
        raise RuntimeError("representation has an unknown content structure")
    return result


def _locator_availability(value: int) -> LocatorAvailability:
    result = {
        _abi.PP_LOCATOR_UNKNOWN: LocatorAvailability.UNKNOWN,
        _abi.PP_LOCATOR_ONLINE: LocatorAvailability.ONLINE,
        _abi.PP_LOCATOR_OFFLINE: LocatorAvailability.OFFLINE,
    }.get(value)
    if result is None:
        raise RuntimeError("locator has an unknown availability")
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
        raise RuntimeError("resolution has an unknown availability")
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
        raise RuntimeError("resolution has an unknown resource state")
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
        raise RuntimeError("resolution has an unknown availability issue")
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
    }.get(value)
    if result is None:
        raise RuntimeError("resolution has an unknown evidence kind")
    return result


def _decode_required(value: bytes | None, label: str) -> str:
    if value is None:
        raise RuntimeError(f"revision event is missing {label}")
    return value.decode("utf-8")


def _decode_optional(value: bytes | None) -> str | None:
    return None if value is None else value.decode("utf-8")
