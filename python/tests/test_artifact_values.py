"""Artifact observations retain only the payload of their actual case."""

import unittest
from dataclasses import replace
from uuid import UUID

from postproject import (
    ActivityId,
    ArtifactReproducibilityIssueKind,
    RepresentationId,
    ReproducibilityInputMissing,
    ReproducibilityParametersMissing,
    ReproducibilityProducerAmbiguous,
    ReproducibilityProducerMissing,
    ReproducibilityToolMissing,
)


class ArtifactValueTests(unittest.TestCase):
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
