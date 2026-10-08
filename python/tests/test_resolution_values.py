"""Resolution cases own their evidence and enforce candidate cardinality."""

import unittest
from dataclasses import replace
from uuid import UUID

from postproject import (
    EvidenceKind,
    InternalError,
    ResolutionCandidate,
    ResolutionEvidence,
    ResourceAmbiguous,
    ResourceId,
    ResourceOffline,
    ResourceOnlineAtKnownLocator,
    ResourceResolution,
    ResourceResolutionFailure,
    ResourceResolutionState,
    ResourceResolvedExact,
    ResourceResolvedProbable,
    UnsupportedError,
)
from postproject._production import _resolution_outcome, _resource_resolution_state


class ResolutionValueTests(unittest.TestCase):
    def test_cases_have_only_their_applicable_candidates(self) -> None:
        candidate = ResolutionCandidate(
            "file:///example.mov",
            10000,
            None,
            None,
            (ResolutionEvidence(EvidenceKind.FULL_HASH_MATCH),),
        )
        outcomes = (
            ResourceOnlineAtKnownLocator(candidate),
            ResourceResolvedExact(candidate),
            ResourceResolvedProbable(candidate),
            ResourceOffline(),
            ResourceAmbiguous((candidate, candidate)),
            ResourceResolutionFailure(),
        )
        for outcome, state, count in zip(
            outcomes, ResourceResolutionState, (1, 1, 1, 0, 2, 0), strict=True
        ):
            result = ResourceResolution(ResourceId(UUID(int=1)), outcome, ())
            self.assertIs(result.state, state)
            self.assertEqual(len(result.candidates), count)
            decoded = _resolution_outcome(state, result.candidates)
            self.assertEqual(decoded, outcome)
            for wrong_count in (0, 1, 2):
                if wrong_count == count or (count == 2 and wrong_count >= 2):
                    continue
                with self.assertRaises(InternalError):
                    _resolution_outcome(state, (candidate,) * wrong_count)
        with self.assertRaises(TypeError):
            ResourceResolvedExact(None)  # ty: ignore[invalid-argument-type]
        with self.assertRaises(TypeError):
            replace(ResourceOffline(), candidate=candidate)
        with self.assertRaises(ValueError):
            ResourceAmbiguous((candidate,))
        with self.assertRaises(UnsupportedError):
            _resource_resolution_state(999)

    def test_owned_collections_and_confidence_bounds(self) -> None:
        evidence = [ResolutionEvidence(EvidenceKind.FILE_SIZE_MATCH, "observed")]
        candidate = replace(
            ResolutionCandidate("file:///a.mov", 0, None, None, (evidence[0],)),
            evidence=evidence,
        )
        evidence.clear()
        self.assertEqual(candidate.evidence[0].detail, "observed")
        candidates = [candidate, candidate]
        outcome = replace(
            ResourceAmbiguous((candidate, candidate)), candidates=candidates
        )
        candidates.clear()
        self.assertEqual(len(outcome.candidates), 2)
        for confidence in (-1, 10001):
            with self.assertRaises(ValueError):
                replace(candidate, confidence_basis_points=confidence)
        with self.assertRaises(TypeError):
            replace(candidate, confidence_basis_points=True)
        with self.assertRaises(ValueError):
            replace(candidate, evidence=())
        with self.assertRaises(TypeError):
            replace(candidate.evidence[0], kind="foreign")
        with self.assertRaises(ValueError):
            replace(candidate, evidence=(candidate.evidence[0],) * 100001)
        with self.assertRaises(ValueError):
            ResourceAmbiguous((candidate,) * 100001)
