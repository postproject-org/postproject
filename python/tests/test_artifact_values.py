"""Artifact observations retain only the payload of their actual case."""

import unittest
from dataclasses import replace
from uuid import UUID

from postproject import (
    ActivityId,
    ArtifactDependencyPathChanged,
    ArtifactDependencyPathSegment,
    ArtifactEdgeKind,
    ArtifactFingerprintChanged,
    ArtifactKnowledgeState,
    ArtifactProducerAmbiguous,
    ArtifactProducerMissing,
    ArtifactReproducibilityIssueKind,
    ArtifactUpstreamNotCurrent,
    RepresentationId,
    RepresentationRef,
    ReproducibilityInputMissing,
    ReproducibilityParametersMissing,
    ReproducibilityProducerAmbiguous,
    ReproducibilityProducerMissing,
    ReproducibilityToolMissing,
    UnsupportedError,
)


class ArtifactValueTests(unittest.TestCase):
    def test_reason_cases_own_bytes_and_require_applicable_fields(self) -> None:
        activity = ActivityId(UUID(int=1))
        representation = RepresentationId(UUID(int=2))
        changed = ArtifactFingerprintChanged(
            activity,
            representation,
            ArtifactEdgeKind.INPUT,
            "foreign_digest",
            7,
            b"old",
            b"new",
        )
        supplied = bytearray(b"old")
        copied = replace(changed, snapshot_value=supplied)
        supplied.clear()
        self.assertEqual(copied.snapshot_value, b"old")
        self.assertEqual(copied.fingerprint_algorithm, "foreign_digest")
        for changes in (
            dict(activity_id=None),
            dict(edge_kind=None),
            dict(snapshot_value=None),
        ):
            with self.assertRaises(TypeError):
                replace(changed, **changes)
        with self.assertRaises(ValueError):
            replace(changed, current_value=b"")
        with self.assertRaises(TypeError):
            replace(
                ArtifactProducerMissing(representation), snapshot_value=b"extraneous"
            )
        with self.assertRaises(ValueError):
            ArtifactProducerAmbiguous(representation, 1)
        with self.assertRaises(ValueError):
            ArtifactUpstreamNotCurrent(representation, ArtifactKnowledgeState.CURRENT)

    def test_dependency_reason_owns_its_path_without_a_fabricated_subject(self) -> None:
        activity = ActivityId(UUID(int=1))
        representation = RepresentationId(UUID(int=2))
        segment = ArtifactDependencyPathSegment(
            representation,
            0,
            None,
            "vendor:reference",
            RepresentationRef(representation),
            None,
            "../opaque%20spelling",
        )
        path = [segment]
        changed = replace(
            ArtifactDependencyPathChanged(activity, representation, ()),
            dependency_path=path,
        )
        path.clear()
        self.assertEqual(changed.dependency_path, (segment,))
        self.assertEqual(
            changed.dependency_path[0].authored_reference, "../opaque%20spelling"
        )
        with self.assertRaises(TypeError):
            replace(changed, representation_id=representation)
        from postproject import _abi
        from postproject._artifact import _reason_payload

        unknown = _abi.ArtifactReason()
        unknown.kind = 999
        with self.assertRaises(UnsupportedError):
            _reason_payload(unknown)

    def test_reproducibility_cases_require_only_their_payload(self) -> None:
        activity = ActivityId(UUID(int=1))
        representation = RepresentationId(UUID(int=2))
        cases = (
            ReproducibilityProducerMissing(),
            ReproducibilityProducerAmbiguous(2),
            ReproducibilityToolMissing(activity),
            ReproducibilityParametersMissing(activity),
            ReproducibilityInputMissing(activity, representation),
        )
        self.assertEqual(
            [case.kind for case in cases], list(ArtifactReproducibilityIssueKind)
        )
        with self.assertRaises(TypeError):
            replace(cases[0], activity_id=activity)
        with self.assertRaises(TypeError):
            replace(cases[1], representation_id=representation)
        for case in cases[2:]:
            with self.subTest(case=case):
                with self.assertRaises(TypeError):
                    replace(case, activity_id=None)
        for count in (0, 1, 2**32):
            with self.assertRaises(ValueError):
                ReproducibilityProducerAmbiguous(count)
        with self.assertRaises(TypeError):
            ReproducibilityProducerAmbiguous(True)
