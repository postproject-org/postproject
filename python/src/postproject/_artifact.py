"""Copy artifact-knowledge results out of owned native ABI handles."""

from __future__ import annotations

import ctypes
from _ctypes import _Pointer
from uuid import UUID

from . import _abi
from ._abi import ArtifactDependencyPathSegment as NativeArtifactDependencyPathSegment
from ._abi import ArtifactEvaluation as NativeArtifactEvaluation
from ._abi import ArtifactReason as NativeArtifactReason
from ._abi import ArtifactReproducibility as NativeArtifactReproducibility
from ._abi import (
    ArtifactReproducibilityIssue as NativeArtifactReproducibilityIssue,
)
from ._abi import Error, Uuid
from ._abi import ObjectRef as NativeObjectRef
from ._model import (
    ActivityId,
    ArtifactDependencyIssue,
    ArtifactDependencyPathSegment,
    ArtifactEdgeKind,
    ArtifactEvaluation,
    ArtifactKnowledgeState,
    ArtifactReason,
    ArtifactReasonKind,
    ArtifactReproducibility,
    ArtifactReproducibilityIssue,
    ArtifactReproducibilityIssueKind,
    ArtifactTraversalLimit,
    AssetId,
    AssetRef,
    ObjectReference,
    RepresentationId,
    RepresentationRef,
    ResourceId,
)
from ._native import NativeLibrary


def read_evaluation(
    native: NativeLibrary,
    evaluation: _Pointer[NativeArtifactEvaluation],
) -> ArtifactEvaluation:
    representation_id = _abi.RepresentationId()
    state = _abi.ArtifactKnowledgeState()
    visited_representations = ctypes.c_uint32()
    truncated = ctypes.c_uint8()
    reason_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_artifact_evaluation_get(
        evaluation,
        ctypes.byref(representation_id),
        ctypes.byref(state),
        ctypes.byref(visited_representations),
        ctypes.byref(truncated),
        ctypes.byref(reason_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ArtifactEvaluation(
        RepresentationId(_uuid(representation_id)),
        _knowledge_state(state.value),
        int(visited_representations.value),
        bool(truncated.value),
        tuple(
            _reason_at(native, evaluation, index)
            for index in range(int(reason_count.value))
        ),
    )


def _reason_at(
    native: NativeLibrary,
    evaluation: _Pointer[NativeArtifactEvaluation],
    index: int,
) -> ArtifactReason:
    value = NativeArtifactReason()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_artifact_evaluation_get_reason(
        evaluation, index, ctypes.byref(value), ctypes.byref(error)
    )
    native.check(status, error)
    kind = _reason_kind(value.kind)
    dependency_kinds = {
        ArtifactReasonKind.DEPENDENCY_SNAPSHOT_ABSENT,
        ArtifactReasonKind.DEPENDENCY_KNOWLEDGE_INCOMPLETE,
        ArtifactReasonKind.DEPENDENCY_PATH_CHANGED,
        ArtifactReasonKind.DEPENDENCY_FINGERPRINT_CHANGED,
        ArtifactReasonKind.DEPENDENCY_FINGERPRINT_RECOMPUTATION_PENDING,
        ArtifactReasonKind.DEPENDENCY_FINGERPRINT_EVIDENCE_MISSING,
    }
    has_activity = (
        kind
        in {
            ArtifactReasonKind.SNAPSHOT_ABSENT,
            ArtifactReasonKind.FINGERPRINT_EVIDENCE_MISSING,
            ArtifactReasonKind.FINGERPRINT_CHANGED,
            ArtifactReasonKind.FINGERPRINT_RECOMPUTATION_PENDING,
        }
        | dependency_kinds
    )
    has_edge = has_activity and kind not in dependency_kinds
    return ArtifactReason(
        kind=kind,
        representation_id=RepresentationId(_uuid(value.representation_id)),
        activity_id=ActivityId(_uuid(value.activity_id)) if has_activity else None,
        edge_kind=_edge_kind(value.edge_kind) if has_edge else None,
        upstream_state=(
            _knowledge_state(value.upstream_state)
            if kind is ArtifactReasonKind.UPSTREAM_NOT_CURRENT
            else None
        ),
        traversal_limit=(
            _traversal_limit(value.traversal_limit)
            if kind is ArtifactReasonKind.TRAVERSAL_TRUNCATED
            else None
        ),
        activity_count=(
            int(value.activity_count)
            if kind is ArtifactReasonKind.PRODUCING_ACTIVITY_AMBIGUOUS
            else None
        ),
        fingerprint_algorithm=_decode_optional(value.fingerprint_algorithm),
        fingerprint_version=(
            int(value.fingerprint_version)
            if value.fingerprint_algorithm is not None
            else None
        ),
        snapshot_value=_optional_bytes(
            value.has_snapshot_value,
            value.snapshot_value,
            value.snapshot_value_length,
        ),
        current_value=_optional_bytes(
            value.has_current_value,
            value.current_value,
            value.current_value_length,
        ),
        input_representation_id=(
            RepresentationId(_uuid(value.input_representation_id))
            if kind in dependency_kinds
            else None
        ),
        dependency_issue=(
            _dependency_issue(value.dependency_issue)
            if kind is ArtifactReasonKind.DEPENDENCY_KNOWLEDGE_INCOMPLETE
            else None
        ),
        dependency_path=tuple(
            _dependency_path_segment(value.dependency_path[index])
            for index in range(int(value.dependency_path_length))
        ),
    )


def _dependency_path_segment(
    segment: NativeArtifactDependencyPathSegment,
) -> ArtifactDependencyPathSegment:
    return ArtifactDependencyPathSegment(
        source_representation_id=RepresentationId(
            _uuid(segment.source_representation_id)
        ),
        dependency_position=int(segment.dependency_position),
        source_resource_id=(
            ResourceId(_uuid(segment.source_resource_id))
            if segment.has_source_resource
            else None
        ),
        kind=_decode_required(segment.kind),
        target=_dependency_target(segment.target),
        resolved_representation_id=(
            RepresentationId(_uuid(segment.resolved_representation_id))
            if segment.has_resolved_representation
            else None
        ),
        authored_reference=_decode_required(segment.authored_reference),
    )


def _dependency_target(value: NativeObjectRef) -> ObjectReference:
    if value.kind == _abi.PP_OBJECT_ASSET:
        return AssetRef(AssetId(_uuid(value.id)))
    if value.kind == _abi.PP_OBJECT_REPRESENTATION:
        return RepresentationRef(RepresentationId(_uuid(value.id)))
    raise RuntimeError("artifact dependency path has an unknown target kind")


def _dependency_issue(value: int) -> ArtifactDependencyIssue:
    result = {
        _abi.PP_ARTIFACT_DEPENDENCY_NEEDS_EXTRACTION: (
            ArtifactDependencyIssue.NEEDS_EXTRACTION
        ),
        _abi.PP_ARTIFACT_DEPENDENCY_UNRESOLVED: ArtifactDependencyIssue.UNRESOLVED,
        _abi.PP_ARTIFACT_DEPENDENCY_DEPTH_TRUNCATED: (
            ArtifactDependencyIssue.DEPTH_TRUNCATED
        ),
        _abi.PP_ARTIFACT_DEPENDENCY_REPRESENTATIONS_TRUNCATED: (
            ArtifactDependencyIssue.REPRESENTATIONS_TRUNCATED
        ),
    }.get(value)
    if result is None:
        raise RuntimeError("artifact evaluation has an unknown dependency issue")
    return result


def read_reproducibility(
    native: NativeLibrary,
    report: _Pointer[NativeArtifactReproducibility],
) -> ArtifactReproducibility:
    representation_id = _abi.RepresentationId()
    reproducible = ctypes.c_uint8()
    has_producing_activity = ctypes.c_uint8()
    producing_activity_id = _abi.ActivityId()
    activity_kind = ctypes.c_char_p()
    issue_count = ctypes.c_uint64()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_artifact_reproducibility_get(
        report,
        ctypes.byref(representation_id),
        ctypes.byref(reproducible),
        ctypes.byref(has_producing_activity),
        ctypes.byref(producing_activity_id),
        ctypes.byref(activity_kind),
        ctypes.byref(issue_count),
        ctypes.byref(error),
    )
    native.check(status, error)
    return ArtifactReproducibility(
        RepresentationId(_uuid(representation_id)),
        bool(reproducible.value),
        (
            ActivityId(_uuid(producing_activity_id))
            if has_producing_activity.value
            else None
        ),
        _decode_optional(activity_kind.value),
        tuple(
            _reproducibility_issue_at(native, report, index)
            for index in range(int(issue_count.value))
        ),
    )


def _reproducibility_issue_at(
    native: NativeLibrary,
    report: _Pointer[NativeArtifactReproducibility],
    index: int,
) -> ArtifactReproducibilityIssue:
    value = NativeArtifactReproducibilityIssue()
    error = ctypes.POINTER(Error)()
    status = native.lib.pp_artifact_reproducibility_get_issue(
        report, index, ctypes.byref(value), ctypes.byref(error)
    )
    native.check(status, error)
    kind = _reproducibility_issue_kind(value.kind)
    has_activity = kind in {
        ArtifactReproducibilityIssueKind.TOOL_IDENTITY_MISSING,
        ArtifactReproducibilityIssueKind.PARAMETERS_MISSING,
        ArtifactReproducibilityIssueKind.INPUT_REPRESENTATION_MISSING,
    }
    return ArtifactReproducibilityIssue(
        kind,
        ActivityId(_uuid(value.activity_id)) if has_activity else None,
        (
            RepresentationId(_uuid(value.representation_id))
            if kind is ArtifactReproducibilityIssueKind.INPUT_REPRESENTATION_MISSING
            else None
        ),
        (
            int(value.activity_count)
            if kind is ArtifactReproducibilityIssueKind.PRODUCING_ACTIVITY_AMBIGUOUS
            else None
        ),
    )


def _knowledge_state(value: int) -> ArtifactKnowledgeState:
    result = {
        _abi.PP_ARTIFACT_CURRENT: ArtifactKnowledgeState.CURRENT,
        _abi.PP_ARTIFACT_STALE: ArtifactKnowledgeState.STALE,
        _abi.PP_ARTIFACT_INDETERMINATE: ArtifactKnowledgeState.INDETERMINATE,
        _abi.PP_ARTIFACT_DIVERGED: ArtifactKnowledgeState.DIVERGED,
    }.get(value)
    if result is None:
        raise RuntimeError("artifact evaluation has an unknown state")
    return result


def _edge_kind(value: int) -> ArtifactEdgeKind:
    result = {
        _abi.PP_ARTIFACT_EDGE_INPUT: ArtifactEdgeKind.INPUT,
        _abi.PP_ARTIFACT_EDGE_OUTPUT: ArtifactEdgeKind.OUTPUT,
    }.get(value)
    if result is None:
        raise RuntimeError("artifact evaluation has an unknown edge kind")
    return result


def _reason_kind(value: int) -> ArtifactReasonKind:
    result = {
        _abi.PP_ARTIFACT_REASON_PRODUCING_ACTIVITY_MISSING: (
            ArtifactReasonKind.PRODUCING_ACTIVITY_MISSING
        ),
        _abi.PP_ARTIFACT_REASON_PRODUCING_ACTIVITY_AMBIGUOUS: (
            ArtifactReasonKind.PRODUCING_ACTIVITY_AMBIGUOUS
        ),
        _abi.PP_ARTIFACT_REASON_SNAPSHOT_ABSENT: ArtifactReasonKind.SNAPSHOT_ABSENT,
        _abi.PP_ARTIFACT_REASON_FINGERPRINT_EVIDENCE_MISSING: (
            ArtifactReasonKind.FINGERPRINT_EVIDENCE_MISSING
        ),
        _abi.PP_ARTIFACT_REASON_FINGERPRINT_CHANGED: (
            ArtifactReasonKind.FINGERPRINT_CHANGED
        ),
        _abi.PP_ARTIFACT_REASON_FINGERPRINT_RECOMPUTATION_PENDING: (
            ArtifactReasonKind.FINGERPRINT_RECOMPUTATION_PENDING
        ),
        _abi.PP_ARTIFACT_REASON_UPSTREAM_NOT_CURRENT: (
            ArtifactReasonKind.UPSTREAM_NOT_CURRENT
        ),
        _abi.PP_ARTIFACT_REASON_TRAVERSAL_TRUNCATED: (
            ArtifactReasonKind.TRAVERSAL_TRUNCATED
        ),
        _abi.PP_ARTIFACT_REASON_DEPENDENCY_SNAPSHOT_ABSENT: (
            ArtifactReasonKind.DEPENDENCY_SNAPSHOT_ABSENT
        ),
        _abi.PP_ARTIFACT_REASON_DEPENDENCY_KNOWLEDGE_INCOMPLETE: (
            ArtifactReasonKind.DEPENDENCY_KNOWLEDGE_INCOMPLETE
        ),
        _abi.PP_ARTIFACT_REASON_DEPENDENCY_PATH_CHANGED: (
            ArtifactReasonKind.DEPENDENCY_PATH_CHANGED
        ),
        _abi.PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_CHANGED: (
            ArtifactReasonKind.DEPENDENCY_FINGERPRINT_CHANGED
        ),
        _abi.PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_RECOMPUTATION_PENDING: (
            ArtifactReasonKind.DEPENDENCY_FINGERPRINT_RECOMPUTATION_PENDING
        ),
        _abi.PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_EVIDENCE_MISSING: (
            ArtifactReasonKind.DEPENDENCY_FINGERPRINT_EVIDENCE_MISSING
        ),
    }.get(value)
    if result is None:
        raise RuntimeError("artifact evaluation has an unknown reason kind")
    return result


def _traversal_limit(value: int) -> ArtifactTraversalLimit:
    result = {
        _abi.PP_ARTIFACT_TRAVERSAL_DEPTH: ArtifactTraversalLimit.DEPTH,
        _abi.PP_ARTIFACT_TRAVERSAL_REPRESENTATIONS: (
            ArtifactTraversalLimit.REPRESENTATIONS
        ),
    }.get(value)
    if result is None:
        raise RuntimeError("artifact evaluation has an unknown traversal limit")
    return result


def _reproducibility_issue_kind(value: int) -> ArtifactReproducibilityIssueKind:
    result = {
        _abi.PP_ARTIFACT_REPRODUCIBILITY_PRODUCING_ACTIVITY_MISSING: (
            ArtifactReproducibilityIssueKind.PRODUCING_ACTIVITY_MISSING
        ),
        _abi.PP_ARTIFACT_REPRODUCIBILITY_PRODUCING_ACTIVITY_AMBIGUOUS: (
            ArtifactReproducibilityIssueKind.PRODUCING_ACTIVITY_AMBIGUOUS
        ),
        _abi.PP_ARTIFACT_REPRODUCIBILITY_TOOL_IDENTITY_MISSING: (
            ArtifactReproducibilityIssueKind.TOOL_IDENTITY_MISSING
        ),
        _abi.PP_ARTIFACT_REPRODUCIBILITY_PARAMETERS_MISSING: (
            ArtifactReproducibilityIssueKind.PARAMETERS_MISSING
        ),
        _abi.PP_ARTIFACT_REPRODUCIBILITY_INPUT_REPRESENTATION_MISSING: (
            ArtifactReproducibilityIssueKind.INPUT_REPRESENTATION_MISSING
        ),
    }.get(value)
    if result is None:
        raise RuntimeError("artifact report has an unknown issue kind")
    return result


def _optional_bytes(
    present: int, value: _Pointer[ctypes.c_uint8], length: int
) -> bytes | None:
    if not present:
        return None
    if length and not value:
        raise RuntimeError("artifact evaluation returned a null byte span")
    return bytes(value[:length])


def _uuid(value: Uuid | _abi.ActivityId | _abi.RepresentationId) -> UUID:
    return UUID(bytes=bytes(value.bytes))


def _decode_optional(value: bytes | None) -> str | None:
    return None if value is None else value.decode("utf-8")


def _decode_required(value: bytes | None) -> str:
    if value is None:
        raise RuntimeError("artifact evaluation returned a null string")
    return value.decode("utf-8")
