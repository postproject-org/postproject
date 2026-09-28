from __future__ import annotations

import gc
import os
import tempfile
import unittest
import weakref
from pathlib import Path
from uuid import UUID

from postproject import (
    ActivityCreatedEvent,
    ActivityEdge,
    ActivityInputAddedEvent,
    ActivityOutputAddedEvent,
    ActivitySpec,
    AgentIdentity,
    ArtifactEdgeKind,
    ArtifactKnowledgeState,
    ArtifactReasonKind,
    AssetId,
    AssetImportedEvent,
    AvailabilityIssueKind,
    CancelledError,
    CancelToken,
    ContentObservationOutcome,
    ContentStructureKind,
    ContentVerification,
    Dependency,
    DependencySetRecordedEvent,
    DependencySetStatus,
    EvidenceKind,
    ExternalIdentifier,
    ExternalIdentifierAddedEvent,
    ExternalIdentifierRemovedEvent,
    FileResourceInput,
    FileSource,
    Fingerprint,
    HostObjectBinding,
    ImageSequenceSource,
    InvalidArgumentError,
    JobCancelledEvent,
    JobClaimedEvent,
    JobClaimReleasedEvent,
    JobClaimRenewedEvent,
    JobFailedEvent,
    JobId,
    JobRequest,
    JobRequestedEvent,
    JobState,
    JobSucceededEvent,
    LocatorAddedEvent,
    LocatorAvailability,
    LocatorMatch,
    LocatorRetiredEvent,
    MediaRootEnabledChangedEvent,
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
    NativeLibrary,
    NotFoundError,
    OrderedPartsSource,
    OriginIdentity,
    PackageSource,
    Production,
    ProvenanceMatch,
    RepresentationAddedEvent,
    RepresentationAvailability,
    RepresentationFingerprintObservedEvent,
    RepresentationId,
    RepresentationKind,
    RepresentationResourceAddedEvent,
    ResourceAddedEvent,
    ResourceFingerprintObservedEvent,
    ResourceId,
    ResourceResolutionState,
    RevisionContext,
    RevisionId,
    SequenceNaming,
    ToolIdentity,
    VerificationMode,
    file_locator,
    fingerprint_file,
    locator_file_path,
)

LIBRARY_PATH = os.environ.get("POSTPROJECT_LIBRARY")


class ProductionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if LIBRARY_PATH is None:
            raise RuntimeError("POSTPROJECT_LIBRARY must name the native test library")

    def test_a_library_is_loaded_once_per_path(self) -> None:
        assert LIBRARY_PATH is not None
        first = NativeLibrary(LIBRARY_PATH)
        self.assertIs(NativeLibrary(LIBRARY_PATH), first)
        self.assertIs(NativeLibrary(Path(LIBRARY_PATH).resolve()), first)

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.production_path = self.root / "production.pproj"
        self.media_path = self.root / "A001.mov"
        self.media_path.write_bytes(b"Python binding media fixture")
        self.second_media_path = self.root / "A002.mov"
        self.second_media_path.write_bytes(b"Second Python binding media fixture")

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def test_create_commit_reopen_and_identity(self) -> None:
        with Production.create(
            self.production_path,
            "Documentary",
            library_path=LIBRARY_PATH,
        ) as production:
            production_id = production.id
            with production.transaction(
                origin=OriginIdentity(
                    "python-test", "1.0", "https://example.com/python-test"
                ),
                message="Import original",
            ) as transaction:
                asset_id = transaction.import_media(self.media_path, "Camera A")
            self.assertIn(asset_id, production.assets)
            assets = tuple(production.assets)
            self.assertEqual(len(assets), 1)
            self.assertEqual(assets[0].id, asset_id)
            self.assertGreater(assets[0].created_at_unix_micros, 0)
            self.assertEqual(assets[0].display_name, "Camera A")
            self.assertIsNone(assets[0].import_source)

        with Production.open(
            self.production_path, library_path=LIBRARY_PATH
        ) as reopened:
            self.assertEqual(reopened.id, production_id)
            self.assertIn(asset_id, reopened.assets)

    def test_point_reads_return_one_object_or_raise_not_found(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path, "Camera A")
            representation = production.representations[asset_id][0]
            resource_id = representation.resources[0].id

            self.assertEqual(production.asset(asset_id).display_name, "Camera A")
            self.assertEqual(
                production.representation(representation.id), representation
            )
            users = production.representations_using_resource(resource_id, limit=10)
            self.assertEqual(users.items, (representation,))
            self.assertIsNone(users.next_cursor)

            absent = UUID(int=1)
            with self.assertRaises(NotFoundError):
                production.asset(AssetId(absent))
            with self.assertRaises(NotFoundError):
                production.representation(RepresentationId(absent))
            with self.assertRaises(NotFoundError):
                production.representations_using_resource(ResourceId(absent), limit=1)
            with self.assertRaises(NotFoundError):
                production.job(JobId(absent))

    def test_job_requests_roundtrip_as_typed_values(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)
            source_id = production.representations[asset_id][0].id
            with production.transaction() as transaction:
                job_id = transaction.request_job(
                    JobRequest(
                        "org.postproject:generate-proxy",
                        (source_id,),
                        asset_id,
                        RepresentationKind.PROXY,
                    )
                )

            jobs_page = production.jobs(limit=1000)
            jobs = jobs_page.items
            self.assertIsNone(jobs_page.next_cursor)
            self.assertEqual(len(jobs), 1)
            self.assertEqual(jobs[0].id, job_id)
            self.assertEqual(jobs[0].kind, "org.postproject:generate-proxy")
            self.assertEqual(jobs[0].inputs, (source_id,))
            self.assertEqual(jobs[0].output_asset_id, asset_id)
            self.assertEqual(
                jobs[0].output_representation_kind, RepresentationKind.PROXY
            )
            self.assertIsNone(jobs[0].target_root)
            self.assertEqual(jobs[0].state, JobState.REQUESTED)
            self.assertIsNone(jobs[0].claim)
            self.assertIsNone(jobs[0].completion)
            self.assertIsNone(jobs[0].failure_diagnostic)
            self.assertEqual(production.job(job_id), jobs[0])

            worker = ToolIdentity("Python worker", "1.0")
            agent = AgentIdentity(
                "operator", ExternalIdentifier("com.example.worker", "worker-1")
            )
            with production.transaction() as transaction:
                claim_id = transaction.claim_job(job_id, worker, agent, 10, 20)
            claimed = production.jobs(limit=1000).items[0]
            self.assertEqual(claimed.state, JobState.CLAIMED)
            self.assertIsNotNone(claimed.claim)
            assert claimed.claim is not None
            self.assertEqual(claimed.claim.id, claim_id)
            self.assertEqual(claimed.claim.tool, worker)
            self.assertEqual(claimed.claim.agent, agent)
            self.assertEqual(claimed.claim.expires_at_unix_micros, 20)

            with production.transaction() as transaction:
                transaction.renew_job_claim(job_id, claim_id, 11, 30)
            renewed = production.jobs(limit=1000).items[0]
            self.assertIsNotNone(renewed.claim)
            assert renewed.claim is not None
            self.assertEqual(renewed.claim.expires_at_unix_micros, 30)

            with production.transaction() as transaction:
                transaction.release_job_claim(job_id, claim_id)
            self.assertEqual(
                production.jobs(limit=1000).items[0].state, JobState.REQUESTED
            )

            with production.transaction() as transaction:
                second_claim_id = transaction.claim_job(job_id, worker, None, 31, 40)
            with production.transaction() as transaction:
                transaction.fail_job(job_id, second_claim_id, 32, "encoder exited")

            with production.transaction() as transaction:
                cancelled_job_id = transaction.request_job(
                    JobRequest(
                        "org.postproject:generate-thumbnail",
                        (source_id,),
                        asset_id,
                        RepresentationKind.DERIVED,
                    )
                )
            with production.transaction() as transaction:
                transaction.cancel_job(cancelled_job_id)

            final_jobs = {job.id: job for job in production.jobs(limit=1000).items}
            self.assertEqual(final_jobs[job_id].state, JobState.FAILED)
            self.assertIsNone(final_jobs[job_id].claim)
            self.assertEqual(final_jobs[job_id].failure_diagnostic, "encoder exited")
            self.assertEqual(final_jobs[cancelled_job_id].state, JobState.CANCELLED)

            with production.transaction() as transaction:
                completed_job_id = transaction.request_job(
                    JobRequest(
                        "org.postproject:generate-proxy",
                        (source_id,),
                        asset_id,
                        RepresentationKind.PROXY,
                    )
                )
            with production.transaction() as transaction:
                completion_claim_id = transaction.claim_job(
                    completed_job_id, worker, None, 41, 50
                )
            with production.transaction() as transaction:
                completed_representation_id = transaction.add_representation(
                    asset_id, RepresentationKind.PROXY, self.second_media_path
                )
                completion_activity_id = transaction.create_activity(
                    ActivitySpec(
                        kind="org.postproject:transcode",
                        outputs=(
                            ActivityEdge(
                                completed_representation_id,
                                "org.postproject:output.proxy",
                            ),
                        ),
                        inputs=(
                            ActivityEdge(
                                source_id, "org.postproject:input.primary-video"
                            ),
                        ),
                        tool=worker,
                    )
                )
                transaction.complete_job(
                    completed_job_id,
                    completion_claim_id,
                    42,
                    completed_representation_id,
                    completion_activity_id,
                )

            completed_jobs = {job.id: job for job in production.jobs(limit=1000).items}
            completed_job = completed_jobs[completed_job_id]
            self.assertEqual(completed_job.state, JobState.SUCCEEDED)
            self.assertIsNotNone(completed_job.completion)
            assert completed_job.completion is not None
            self.assertEqual(
                completed_job.completion.representation_id,
                completed_representation_id,
            )
            self.assertEqual(
                completed_job.completion.activity_id, completion_activity_id
            )
            producing = production.activities_producing[completed_representation_id]
            self.assertEqual(len(producing), 1)
            self.assertIsNotNone(producing[0].outputs[0].snapshot)

            job_event_types = {
                type(event.payload)
                for revision in production.changes_since(0, 1000)
                for event in production.revision_events[revision.id]
                if hasattr(event.payload, "job_id")
            }
            self.assertEqual(
                job_event_types,
                {
                    JobRequestedEvent,
                    JobClaimedEvent,
                    JobClaimRenewedEvent,
                    JobClaimReleasedEvent,
                    JobFailedEvent,
                    JobCancelledEvent,
                    JobSucceededEvent,
                },
            )

            first_page = production.jobs(limit=1)
            self.assertIsNotNone(first_page.next_cursor)
            paged_jobs = list(first_page.items)
            cursor = first_page.next_cursor
            while cursor is not None:
                page = production.jobs(limit=1, cursor=cursor)
                paged_jobs.extend(page.items)
                cursor = page.next_cursor
            self.assertEqual(
                {job.id for job in paged_jobs},
                {job_id, cancelled_job_id, completed_job_id},
            )
            failed_page = production.jobs(limit=10, state=JobState.FAILED)
            self.assertEqual(tuple(job.id for job in failed_page.items), (job_id,))

    def test_representations_are_typed_keyed_and_copied(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)
            representations = production.representations[asset_id]
            self.assertIsNone(production.dependency_set(representations[0].id))
            self.assertEqual(
                production.dependents(
                    asset_id, max_depth=1, max_representations=1000, limit=1000
                ).items,
                (),
            )

        self.assertEqual(len(representations), 1)
        representation = representations[0]
        self.assertEqual(representation.asset_id, asset_id)
        self.assertEqual(representation.kind, RepresentationKind.ORIGINAL)
        self.assertEqual(
            representation.structure_kind, ContentStructureKind.SINGLE_RESOURCE
        )
        self.assertIsNone(representation.image_sequence)
        self.assertEqual(len(representation.fingerprints), 1)
        self.assertEqual(
            representation.fingerprints[0].algorithm, "pp-blake3-representation"
        )
        self.assertEqual(representation.fingerprints[0].version, 2)
        self.assertTrue(representation.fingerprints[0].value)
        self.assertEqual(len(representation.members), 1)
        self.assertTrue(representation.members[0].required)
        self.assertIsNone(representation.members[0].role)

        self.assertEqual(len(representation.resources), 1)
        resource = representation.resources[0]
        self.assertEqual(resource.id, representation.members[0].resource_id)
        self.assertEqual(resource.file_size, len(self.media_path.read_bytes()))
        self.assertIsNotNone(resource.modified_at_unix_micros)
        self.assertEqual(len(resource.fingerprints), 1)
        self.assertEqual(resource.fingerprints[0].version, 1)
        self.assertTrue(resource.fingerprints[0].value)
        self.assertEqual(len(resource.locators), 1)
        self.assertEqual(resource.locators[0].availability, LocatorAvailability.ONLINE)
        self.assertIsNotNone(resource.locators[0].last_seen_unix_micros)

    def test_fingerprint_observations_are_explicit_and_idempotent(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)
            representation = production.representations[asset_id][0]
            resource_id = representation.resources[0].id
            resource_fingerprint = Fingerprint("python-test", 1, b"resource")
            representation_fingerprint = Fingerprint(
                "python-test-tree", 1, b"representation"
            )
            with production.transaction() as transaction:
                transaction.record_resource_fingerprint(
                    resource_id, resource_fingerprint
                )
                transaction.record_representation_fingerprint(
                    representation.id, representation_fingerprint
                )

            observed = production.representations[asset_id][0]
            self.assertIn(resource_fingerprint, observed.resources[0].fingerprints)
            self.assertIn(representation_fingerprint, observed.fingerprints)
            revision = production.latest_revision
            assert revision is not None
            observation_events = production.revision_events[revision.id]
            self.assertIsInstance(
                observation_events[0].payload, ResourceFingerprintObservedEvent
            )
            self.assertIsInstance(
                observation_events[1].payload,
                RepresentationFingerprintObservedEvent,
            )

            with production.transaction() as transaction:
                transaction.record_resource_fingerprint(
                    resource_id, resource_fingerprint
                )
            self.assertEqual(production.latest_revision, revision)

    def test_content_is_computed_verified_and_observed(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)
            representation = production.representations[asset_id][0]
            resource = representation.resources[0]

            (locator,) = resource.locators
            self.assertEqual(
                file_locator(self.media_path, library_path=LIBRARY_PATH), locator.uri
            )
            self.assertEqual(
                locator_file_path(locator.uri, library_path=LIBRARY_PATH),
                Path(self.media_path).resolve(),
            )

            computed = fingerprint_file(self.media_path, library_path=LIBRARY_PATH)
            self.assertEqual(resource.fingerprints, (computed,))
            self.assertIs(
                production.verify_resource(resource.id, self.media_path),
                ContentVerification.MATCHES,
            )

            Path(self.media_path).write_bytes(b"replaced content")
            self.assertIs(
                production.verify_resource(resource.id, self.media_path),
                ContentVerification.DIFFERS,
            )
            with production.transaction() as transaction:
                self.assertIs(
                    transaction.observe_resource_content(resource.id, self.media_path),
                    ContentObservationOutcome.CHANGED,
                )
            latest = production.latest_revision

            observed = production.representations[asset_id][0]
            self.assertEqual(
                observed.resources[0].fingerprints,
                (fingerprint_file(self.media_path, library_path=LIBRARY_PATH),),
            )
            self.assertNotEqual(observed.fingerprints, representation.fingerprints)
            self.assertIs(
                production.verify_resource(resource.id, self.media_path),
                ContentVerification.MATCHES,
            )
            # Observing unchanged content records nothing.
            with production.transaction() as transaction:
                self.assertIs(
                    transaction.observe_resource_content(resource.id, self.media_path),
                    ContentObservationOutcome.UNCHANGED,
                )
            self.assertEqual(production.latest_revision, latest)

    def test_media_roots_and_locators_have_a_complete_lifecycle(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)
                root_id = transaction.add_media_root("media", "Media", 4)

            roots = production.media_roots
            self.assertEqual(len(roots), 1)
            self.assertEqual(roots[0].id, root_id)
            self.assertEqual(roots[0].name, "media")
            self.assertEqual(roots[0].label, "Media")
            self.assertIsNone(roots[0].legacy_uri)
            self.assertEqual(roots[0].priority, 4)
            self.assertTrue(roots[0].enabled)
            locator_id = (
                production.representations[asset_id][0].resources[0].locators[0].id
            )

            with production.transaction() as transaction:
                transaction.set_media_root_enabled(root_id, False)
                transaction.retire_locator(locator_id)

            self.assertFalse(production.media_roots[0].enabled)
            self.assertEqual(
                production.representations[asset_id][0].resources[0].locators, ()
            )
            revision = production.latest_revision
            self.assertIsNotNone(revision)
            assert revision is not None
            events = production.revision_events[revision.id]
            self.assertIsInstance(events[0].payload, MediaRootEnabledChangedEvent)
            self.assertIsInstance(events[1].payload, LocatorRetiredEvent)

            with production.transaction() as transaction:
                transaction.remove_media_root(root_id)

            self.assertEqual(production.media_roots, ())
            revision = production.latest_revision
            self.assertIsNotNone(revision)
            assert revision is not None
            events = production.revision_events[revision.id]
            self.assertEqual(len(events), 1)
            self.assertIsInstance(events[0].payload, MediaRootRemovedEvent)

    def test_additional_and_compound_representations_roundtrip(self) -> None:
        sequence_frame = self.root / "frame0001.exr"
        sequence_frame.write_bytes(b"sequence frame")
        sidecar = self.root / "clip.xml"
        sidecar.write_text("<metadata />")
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)

            with production.transaction() as transaction:
                proxy_id = transaction.add_representation(
                    asset_id,
                    RepresentationKind.PROXY,
                    FileSource(self.second_media_path),
                )
                sequence_id = transaction.add_representation(
                    asset_id,
                    RepresentationKind.DERIVED,
                    ImageSequenceSource(
                        self.root,
                        SequenceNaming("frame", ".exr", 4),
                        1,
                        1,
                        1,
                        24_000,
                        1_001,
                    ),
                )
                ordered_id = transaction.add_representation(
                    asset_id,
                    RepresentationKind.OPTIMIZED,
                    OrderedPartsSource(
                        (
                            FileResourceInput(
                                str(self.media_path),
                                "org.postproject:essence.first",
                            ),
                            FileResourceInput(
                                str(self.second_media_path),
                                "org.postproject:essence.second",
                            ),
                        )
                    ),
                )
                package_id = transaction.add_representation(
                    asset_id,
                    RepresentationKind.DERIVED,
                    PackageSource(
                        (
                            FileResourceInput(
                                str(self.media_path), "org.postproject:essence"
                            ),
                            FileResourceInput(
                                str(sidecar), "org.postproject:sidecar", False
                            ),
                        )
                    ),
                )
                with self.assertRaises(InvalidArgumentError):
                    transaction.add_representation(
                        asset_id,
                        RepresentationKind.DERIVED,
                        OrderedPartsSource(
                            (
                                FileResourceInput(
                                    str(sidecar), "org.postproject:sidecar", False
                                ),
                            )
                        ),
                    )

            representations = {
                representation.id: representation
                for representation in production.representations[asset_id]
            }
            self.assertEqual(len(representations), 5)
            self.assertEqual(
                representations[proxy_id].structure_kind,
                ContentStructureKind.SINGLE_RESOURCE,
            )
            sequence = representations[sequence_id]
            self.assertEqual(
                sequence.structure_kind, ContentStructureKind.IMAGE_SEQUENCE
            )
            assert sequence.image_sequence is not None
            self.assertEqual(sequence.image_sequence.rate_numerator, 24_000)
            self.assertEqual(sequence.image_sequence.rate_denominator, 1_001)
            self.assertEqual(
                representations[ordered_id].structure_kind,
                ContentStructureKind.ORDERED_PARTS,
            )
            package = representations[package_id]
            self.assertEqual(package.structure_kind, ContentStructureKind.PACKAGE)
            self.assertEqual(
                tuple(member.required for member in package.members), (True, False)
            )

    def test_image_sequence_imports_as_the_only_original(self) -> None:
        for frame in range(1, 4):
            (self.root / f"strip_{frame:04}.png").write_bytes(b"frame %d" % frame)
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(
                    ImageSequenceSource(
                        self.root, SequenceNaming("strip_", ".png", 4), 1, 3, 1, 24, 1
                    ),
                    "Image strip",
                )

            representations = production.representations[asset_id]
            self.assertEqual(len(representations), 1)
            original = representations[0]
            self.assertEqual(original.kind, RepresentationKind.ORIGINAL)
            self.assertEqual(
                original.structure_kind, ContentStructureKind.IMAGE_SEQUENCE
            )
            assert original.image_sequence is not None
            (locator,) = original.resources[0].locators
            self.assertEqual(
                locator.sequence_naming, SequenceNaming("strip_", ".png", 4)
            )
            self.assertEqual(
                (original.image_sequence.start, original.image_sequence.end), (1, 3)
            )

    def _write_frames(
        self, directory: Path, prefix: str, frames: range, content: bytes = b"frame"
    ) -> None:
        directory.mkdir(parents=True, exist_ok=True)
        for frame in frames:
            (directory / f"{prefix}{frame:04}.png").write_bytes(content + b"%d" % frame)

    def test_renamed_sequence_resolves_by_content_under_its_new_naming(self) -> None:
        plates = self.root / "shots" / "plates"
        graded = self.root / "shots" / "graded"
        self._write_frames(plates, "shot_", range(1, 4))
        original = SequenceNaming("shot_", ".png", 4)
        renamed = SequenceNaming("shot-graded_", ".png", 4)
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(
                    ImageSequenceSource(plates, original, 1, 3, 1, 24, 1), "Shot"
                )
            graded.mkdir()
            for frame in range(1, 4):
                (plates / original.filename(frame)).rename(
                    graded / renamed.filename(frame)
                )
            # Same names with other content, and an incomplete copy, are no
            # candidates.
            self._write_frames(
                self.root / "shots" / "other", "shot-graded_", range(1, 4), b"other"
            )
            self._write_frames(
                self.root / "shots" / "partial", "shot-graded_", range(1, 3)
            )

            (resolution,) = production.resolve(
                asset_id, search_directories=[self.root / "shots"]
            )
            (resource,) = resolution.resources
            self.assertEqual(resource.state, ResourceResolutionState.RESOLVED_PROBABLE)
            (candidate,) = resource.candidates
            self.assertEqual(candidate.uri, file_locator(graded))
            self.assertEqual(candidate.sequence_naming, renamed)
            kinds = {evidence.kind for evidence in candidate.evidence}
            self.assertIn(EvidenceKind.PARTIAL_FINGERPRINT_MATCH, kinds)
            self.assertNotIn(EvidenceKind.FILE_NAME_MATCH, kinds)

            with production.transaction() as transaction:
                transaction.confirm_locator(
                    resource.resource_id,
                    candidate.uri,
                    media_root=candidate.media_root,
                    sequence_naming=candidate.sequence_naming,
                )
            locators = production.locators_page(resource.resource_id, limit=10).items
            self.assertEqual(
                {match.locator.sequence_naming for match in locators},
                {original, renamed},
            )
            self.assertEqual(
                production.verify_resource(resource.resource_id, graded),
                ContentVerification.MATCHES,
            )

            # An identical renamed copy is ambiguous once the confirmed
            # directory is gone.
            self._write_frames(
                self.root / "shots" / "copy", "shot-graded_", range(1, 4)
            )
            for frame in range(1, 4):
                (graded / renamed.filename(frame)).rename(
                    plates / renamed.filename(frame)
                )
            (ambiguous,) = production.resolve(
                asset_id, search_directories=[self.root / "shots"]
            )
            self.assertEqual(
                ambiguous.resources[0].state, ResourceResolutionState.AMBIGUOUS
            )
            self.assertEqual(len(ambiguous.resources[0].candidates), 2)

            with self.assertRaises(InvalidArgumentError):
                with production.transaction() as transaction:
                    transaction.confirm_locator(resource.resource_id, candidate.uri)

    def test_dependency_sets_roundtrip_replace_and_support_reverse_queries(
        self,
    ) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)
            original = production.representations[asset_id][0]
            with production.transaction() as transaction:
                proxy_id = transaction.add_representation(
                    asset_id, RepresentationKind.PROXY, self.second_media_path
                )
            proxy = next(
                value
                for value in production.representations[asset_id]
                if value.id == proxy_id
            )
            dependency = Dependency(
                kind="org.postproject:reference.character",
                target=asset_id,
                authored_reference="../Characters/Lead A.blend#Rig",
                source_resource_id=proxy.resources[0].id,
                resolved_representation_id=original.id,
            )

            with production.transaction() as transaction:
                transaction.record_dependency_set(proxy_id, (dependency,))

            revision = production.latest_revision
            assert revision is not None
            events = production.revision_events[revision.id]
            self.assertEqual(len(events), 1)
            self.assertEqual(events[0].payload, DependencySetRecordedEvent(proxy_id))
            recorded = production.dependency_set(proxy_id)
            self.assertIsNotNone(recorded)
            assert recorded is not None
            self.assertEqual(recorded.source_representation_id, proxy_id)
            self.assertGreater(recorded.recorded_at_revision, 0)
            self.assertEqual(recorded.status, DependencySetStatus.CURRENT)
            self.assertEqual(recorded.dependencies, (dependency,))
            dependents = production.dependents(
                asset_id, max_depth=1, max_representations=1000, limit=1000
            )
            self.assertEqual(
                tuple(match.target for match in dependents.items), (proxy_id,)
            )
            self.assertEqual(tuple(match.depth for match in dependents.items), (1,))
            dependencies = production.dependencies(
                proxy_id, max_depth=2, max_representations=1000, limit=1000
            )
            self.assertEqual(
                tuple(match.target for match in dependencies.items), (asset_id,)
            )

            with production.transaction() as transaction:
                transaction.record_dependency_set(proxy_id, ())

            empty = production.dependency_set(proxy_id)
            self.assertIsNotNone(empty)
            assert empty is not None
            self.assertEqual(empty.dependencies, ())
            self.assertEqual(
                production.dependents(
                    asset_id, max_depth=1, max_representations=1000, limit=1000
                ).items,
                (),
            )

    def test_host_bindings_are_keyed_and_round_trip_through_native_abi(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)

            encoded = production.host_bindings[asset_id]
            self.assertTrue(encoded.startswith("https://postproject.org/ref/v1/"))
            self.assertEqual(
                production.host_bindings.parse(encoded),
                HostObjectBinding(production.id, asset_id),
            )
            with self.assertRaises(InvalidArgumentError):
                production.host_bindings.parse("postproject:v1:obsolete")

    def test_context_exception_and_explicit_rollback_discard_imports(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with self.assertRaisesRegex(RuntimeError, "abort"):
                with production.transaction() as transaction:
                    exception_asset = transaction.import_media(self.media_path)
                    raise RuntimeError("abort")
            self.assertNotIn(exception_asset, production.assets)

            with production.transaction() as transaction:
                explicit_asset = transaction.import_media(self.media_path)
                transaction.rollback()
            self.assertNotIn(explicit_asset, production.assets)

    def test_close_is_idempotent_and_closed_handles_are_rejected(self) -> None:
        production = Production.create(self.production_path, library_path=LIBRARY_PATH)
        transaction = production.transaction()
        transaction.close()
        transaction.close()
        with self.assertRaisesRegex(RuntimeError, "transaction is closed"):
            transaction.set_revision_context(RevisionContext(message="closed"))

        production.close()
        production.close()
        with self.assertRaisesRegex(RuntimeError, "production is closed"):
            _ = production.id

    def test_finalizer_releases_an_open_transaction(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            transaction = production.transaction()
            reference = weakref.ref(transaction)
            del transaction
            gc.collect()
            self.assertIsNone(reference())

            with production.transaction() as replacement:
                replacement.rollback()

    def test_embedded_nul_is_rejected_before_native_call(self) -> None:
        with self.assertRaisesRegex(ValueError, "NUL"):
            Production.open("invalid\0path.pproj", library_path=LIBRARY_PATH)

    def test_revision_summaries_are_copied_and_paginated(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            self.assertIsNone(production.latest_revision)
            with production.transaction(
                origin=OriginIdentity("python-test", "1.0"),
                message="First import",
            ) as transaction:
                transaction.import_media(self.media_path)

            first = production.latest_revision
            self.assertIsNotNone(first)
            assert first is not None
            self.assertEqual(first.sequence, 1)
            self.assertEqual(first.message, "First import")
            self.assertEqual(first.origin, OriginIdentity("python-test", "1.0"))

            with production.transaction() as transaction:
                transaction.import_media(self.media_path, "Second")

            page = production.changes_since(0, 1)
            self.assertEqual(page, (first,))
            second_page = production.changes_since(page[-1].sequence, 1)
            self.assertEqual(len(second_page), 1)
            self.assertEqual(second_page[0].sequence, 2)
            self.assertIsNone(second_page[0].origin)
            self.assertIsNone(second_page[0].message)

            with self.assertRaises(InvalidArgumentError):
                production.changes_since(0, 0)

    def test_revision_events_are_typed_ordered_and_copied(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)

            revision = production.latest_revision
            assert revision is not None
            events = production.revision_events[revision.id]

            self.assertEqual([event.position for event in events], list(range(5)))
            imported, representation, resource, membership, locator = (
                event.payload for event in events
            )
            assert isinstance(imported, AssetImportedEvent)
            assert isinstance(representation, RepresentationAddedEvent)
            assert isinstance(resource, ResourceAddedEvent)
            assert isinstance(membership, RepresentationResourceAddedEvent)
            assert isinstance(locator, LocatorAddedEvent)
            self.assertEqual(imported.asset_id, asset_id)
            self.assertEqual(representation.asset_id, asset_id)
            self.assertEqual(
                membership.representation_id, representation.representation_id
            )
            self.assertEqual(membership.resource_id, resource.resource_id)
            self.assertEqual(membership.structural_position, 0)
            self.assertEqual(locator.resource_id, resource.resource_id)

            missing = RevisionId(UUID("00000000-0000-0000-0000-000000000001"))
            with self.assertRaises(NotFoundError):
                _ = production.revision_events[missing]

    def test_external_identifiers_roundtrip_lookup_and_remove(self) -> None:
        camera_id = ExternalIdentifier("com.example.camera", "A001-C023", "primary")
        umid = ExternalIdentifier("urn:smpte:umid", "060A2B340101010501010D4313000000")
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)

            with production.transaction() as transaction:
                transaction.add_external_identifier(asset_id, camera_id)
                transaction.add_external_identifier(asset_id, umid)

            self.assertEqual(
                set(production.external_identifiers[asset_id]), {camera_id, umid}
            )
            self.assertEqual(
                production.objects_by_external_identifier[
                    camera_id.scheme, camera_id.value
                ],
                (asset_id,),
            )
            self.assertEqual(
                production.objects_by_external_identifier[
                    camera_id.scheme, camera_id.value, "primary"
                ],
                (asset_id,),
            )
            self.assertEqual(
                production.objects_by_external_identifier[
                    camera_id.scheme, camera_id.value, "secondary"
                ],
                (),
            )
            revision = production.latest_revision
            assert revision is not None
            added = production.revision_events[revision.id]
            self.assertTrue(
                all(
                    isinstance(event.payload, ExternalIdentifierAddedEvent)
                    for event in added
                )
            )

            with production.transaction() as transaction:
                transaction.remove_external_identifier(asset_id, camera_id)

            self.assertEqual(production.external_identifiers[asset_id], (umid,))
            self.assertEqual(
                production.objects_by_external_identifier[
                    camera_id.scheme, camera_id.value
                ],
                (),
            )
            revision = production.latest_revision
            assert revision is not None
            removed = production.revision_events[revision.id]
            self.assertEqual(len(removed), 1)
            payload = removed[0].payload
            assert isinstance(payload, ExternalIdentifierRemovedEvent)
            self.assertEqual(payload.target, asset_id)
            self.assertEqual(payload.identifier, camera_id)

    def test_resolution_results_are_typed_and_keyed_by_asset(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)

            resolutions = production.resolutions[asset_id]
            self.assertEqual(len(resolutions), 1)
            representation = resolutions[0]
            self.assertEqual(
                representation.availability, RepresentationAvailability.ONLINE
            )
            self.assertEqual(representation.issues, ())
            self.assertEqual(len(representation.resources), 1)
            resource = representation.resources[0]
            self.assertEqual(
                resource.state,
                ResourceResolutionState.ONLINE_AT_KNOWN_LOCATOR,
            )
            self.assertEqual(resource.evidence, ())
            self.assertEqual(len(resource.candidates), 1)
            candidate = resource.candidates[0]
            self.assertEqual(candidate.uri, self.media_path.resolve().as_uri())
            self.assertEqual(candidate.confidence_basis_points, 10_000)
            self.assertEqual(
                tuple(item.kind for item in candidate.evidence),
                (EvidenceKind.KNOWN_LOCATOR_AVAILABLE,),
            )

    def test_search_directories_batches_and_cancellation(self) -> None:
        nearby = self.root / "nearby"
        nearby.mkdir()
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)
            moved = nearby / self.media_path.name
            self.media_path.rename(moved)

            (resolution,) = production.resolve(
                [asset_id],
                search_directories=[nearby],
                verification=VerificationMode.CONTENT,
                max_entries_per_directory=1_000,
            )
            self.assertEqual(resolution.asset_id, asset_id)
            (candidate,) = resolution.resources[0].candidates
            self.assertEqual(candidate.uri, moved.resolve().as_uri())
            self.assertIsNone(candidate.media_root)

            token = CancelToken(library_path=LIBRARY_PATH)
            token.cancel()
            with self.assertRaises(CancelledError):
                production.resolve(
                    asset_id, search_directories=[nearby], cancel_token=token
                )

    def test_ambiguous_resolution_requires_explicit_confirmation(self) -> None:
        candidates = self.root / "candidates"
        candidates.mkdir()
        (candidates / "a.mov").write_bytes(b"Python binding media fixture")
        (candidates / "b.mov").write_bytes(b"Python binding media fixture")

        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)
                transaction.add_media_root("relocated", "Relocated")
            self.media_path.unlink()

            resolution = production.resolve(asset_id, {"relocated": candidates})[0]
            self.assertEqual(
                resolution.availability, RepresentationAvailability.AMBIGUOUS
            )
            self.assertEqual(len(resolution.resources), 1)
            resource = resolution.resources[0]
            self.assertEqual(resource.state, ResourceResolutionState.AMBIGUOUS)
            self.assertEqual(len(resource.candidates), 2)
            self.assertEqual(
                tuple(candidate.uri for candidate in resource.candidates),
                tuple(
                    path.resolve().as_uri()
                    for path in (candidates / "a.mov", candidates / "b.mov")
                ),
            )
            self.assertEqual(len(resolution.issues), 1)
            self.assertEqual(
                resolution.issues[0].kind,
                AvailabilityIssueKind.AMBIGUOUS_RESOURCE,
            )

            with production.transaction() as transaction:
                transaction.confirm_locator(
                    resource.resource_id, resource.candidates[0].uri
                )

            confirmed = production.resolutions[asset_id][0]
            self.assertEqual(confirmed.availability, RepresentationAvailability.ONLINE)
            self.assertEqual(
                confirmed.resources[0].state,
                ResourceResolutionState.ONLINE_AT_KNOWN_LOCATOR,
            )

    def test_external_identifier_nul_is_rejected_before_native_call(self) -> None:
        with (
            Production.create(
                self.production_path, library_path=LIBRARY_PATH
            ) as production,
            self.assertRaisesRegex(ValueError, "NUL"),
        ):
            with production.transaction() as transaction:
                transaction.add_external_identifier(
                    production.id, ExternalIdentifier("invalid\0scheme", "value")
                )

    def test_text_metadata_is_typed_repeatable_searchable_and_removable(self) -> None:
        title = MetadataProperty("https://example.com/metadata", "title")
        plain = MetadataString("Interview")
        localized = MetadataLanguageString("Gespräch", "de")
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)

            with production.transaction() as transaction:
                transaction.add_metadata(asset_id, title, plain)
                transaction.add_metadata(asset_id, title, localized)

            expected = (
                MetadataAssertion(asset_id, title, plain),
                MetadataAssertion(asset_id, title, localized),
            )
            self.assertEqual(production.metadata[asset_id], expected)
            self.assertEqual(production.metadata_by_property[title], expected)
            revision = production.latest_revision
            assert revision is not None
            added = production.revision_events[revision.id]
            self.assertTrue(
                all(
                    isinstance(event.payload, MetadataAddedOrReplacedEvent)
                    for event in added
                )
            )

            with production.transaction() as transaction:
                transaction.remove_metadata_property(asset_id, title)

            self.assertEqual(production.metadata[asset_id], ())
            self.assertEqual(production.metadata_by_property[title], ())
            revision = production.latest_revision
            assert revision is not None
            removed = production.revision_events[revision.id]
            self.assertEqual(len(removed), 1)
            payload = removed[0].payload
            assert isinstance(payload, MetadataRemovedEvent)
            self.assertEqual(payload.target, asset_id)
            self.assertEqual(payload.property, title)

    def test_recursive_typed_metadata_write_roundtrips(self) -> None:
        property = MetadataProperty("https://example.com/metadata", "technical")
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                asset_id = transaction.import_media(self.media_path)

            value = MetadataStruct(
                (
                    MetadataStructField("signed", MetadataI64(-42)),
                    MetadataStructField("unsigned", MetadataU64(42)),
                    MetadataStructField("decimal", MetadataDecimal(-12345, 2)),
                    MetadataStructField("enabled", MetadataBool(True)),
                    MetadataStructField(
                        "captured", MetadataTimestamp(1_700_000_000_123_456)
                    ),
                    MetadataStructField(
                        "source", MetadataUri("https://example.com/source")
                    ),
                    MetadataStructField("payload", MetadataBytes(b"\x00\xff")),
                    MetadataStructField("rate", MetadataRational(24_000, 1_001)),
                    MetadataStructField("asset", MetadataReference(asset_id)),
                    MetadataStructField(
                        "labels",
                        MetadataList(
                            (
                                MetadataString("interview"),
                                MetadataLanguageString("Gespräch", "de"),
                            )
                        ),
                    ),
                )
            )
            with production.transaction() as transaction:
                transaction.add_metadata(asset_id, property, value)

            self.assertEqual(
                production.metadata[asset_id],
                (MetadataAssertion(asset_id, property, value),),
            )

    def test_provenance_activity_roundtrips_and_supports_graph_queries(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                transaction.import_media(self.media_path)
            revision = production.latest_revision
            assert revision is not None
            source_event = next(
                event.payload
                for event in production.revision_events[revision.id]
                if isinstance(event.payload, RepresentationAddedEvent)
            )

            with production.transaction() as transaction:
                transaction.import_media(self.second_media_path)
            revision = production.latest_revision
            assert revision is not None
            output_event = next(
                event.payload
                for event in production.revision_events[revision.id]
                if isinstance(event.payload, RepresentationAddedEvent)
            )

            input_edge = ActivityEdge(
                source_event.representation_id, "org.postproject:primary"
            )
            output_edge = ActivityEdge(
                output_event.representation_id, "org.postproject:proxy"
            )
            spec = ActivitySpec(
                "org.postproject:transcode",
                outputs=(output_edge,),
                inputs=(input_edge,),
                started_at_unix_micros=100,
                finished_at_unix_micros=200,
                tool=ToolIdentity("FFmpeg", "8.0", "https://ffmpeg.org/"),
                agent=AgentIdentity(
                    "automation",
                    ExternalIdentifier("com.example.worker", "worker-1"),
                ),
            )
            with production.transaction() as transaction:
                activity_id = transaction.create_activity(spec)

            expected = production.activities[0]
            self.assertEqual(expected.id, activity_id)
            self.assertEqual(expected.kind, spec.kind)
            self.assertEqual(
                expected.inputs[0].representation_id, input_edge.representation_id
            )
            self.assertEqual(
                expected.outputs[0].representation_id, output_edge.representation_id
            )
            input_snapshot = expected.inputs[0].snapshot
            output_snapshot = expected.outputs[0].snapshot
            assert input_snapshot is not None
            assert output_snapshot is not None
            self.assertEqual(input_snapshot.revision_sequence, 3)
            self.assertEqual(output_snapshot.revision_sequence, 3)
            self.assertEqual(
                input_snapshot.fingerprints[0].observed_revision_sequence, 1
            )
            self.assertEqual(
                output_snapshot.fingerprints[0].observed_revision_sequence, 2
            )
            self.assertEqual(production.activities, (expected,))
            self.assertEqual(
                production.activities_consuming[source_event.representation_id],
                (expected,),
            )
            self.assertEqual(
                production.activities_producing[output_event.representation_id],
                (expected,),
            )
            self.assertEqual(
                production.provenance_ancestors[output_event.representation_id],
                (source_event.representation_id,),
            )
            self.assertEqual(
                production.provenance_descendants[source_event.representation_id],
                (output_event.representation_id,),
            )

            revision = production.latest_revision
            assert revision is not None
            payloads = tuple(
                event.payload for event in production.revision_events[revision.id]
            )
            self.assertEqual(len(payloads), 3)
            self.assertIsInstance(payloads[0], ActivityCreatedEvent)
            self.assertIsInstance(payloads[1], ActivityInputAddedEvent)
            self.assertIsInstance(payloads[2], ActivityOutputAddedEvent)

    def test_artifact_knowledge_is_explainable_and_reproducible(self) -> None:
        parameter = MetadataProperty("org.postproject.parameters", "profile")
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                source_asset = transaction.import_media(self.media_path)
                output_asset = transaction.import_media(self.second_media_path)
            source = production.representations[source_asset][0]
            output = production.representations[output_asset][0]

            with production.transaction() as transaction:
                activity_id = transaction.create_activity(
                    ActivitySpec(
                        "org.postproject:transcode",
                        inputs=(ActivityEdge(source.id),),
                        outputs=(ActivityEdge(output.id),),
                        tool=ToolIdentity("FFmpeg", "8.0", "https://ffmpeg.org/"),
                    )
                )
                transaction.add_metadata(
                    activity_id, parameter, MetadataString("editorial-proxy")
                )

            current = production.evaluate_artifact(output.id)
            self.assertEqual(current.representation_id, output.id)
            self.assertEqual(current.state, ArtifactKnowledgeState.CURRENT)
            self.assertEqual(current.reasons, ())
            self.assertFalse(current.truncated)
            self.assertEqual(current.visited_representations, 1)

            reproducibility = production.artifact_reproducibility(output.id)
            self.assertTrue(reproducibility.reproducible)
            self.assertEqual(reproducibility.producing_activity_id, activity_id)
            self.assertEqual(reproducibility.activity_kind, "org.postproject:transcode")
            self.assertEqual(reproducibility.issues, ())

            source_fingerprint = source.fingerprints[0]
            with production.transaction() as transaction:
                transaction.record_representation_fingerprint(
                    source.id,
                    Fingerprint(
                        source_fingerprint.algorithm,
                        source_fingerprint.version,
                        b"changed-python-fingerprint",
                    ),
                )

            stale = production.evaluate_artifact(output.id)
            self.assertEqual(stale.state, ArtifactKnowledgeState.STALE)
            self.assertEqual(len(stale.reasons), 1)
            reason = stale.reasons[0]
            self.assertEqual(reason.kind, ArtifactReasonKind.FINGERPRINT_CHANGED)
            self.assertEqual(reason.activity_id, activity_id)
            self.assertEqual(reason.representation_id, source.id)
            self.assertEqual(reason.edge_kind, ArtifactEdgeKind.INPUT)
            self.assertEqual(reason.current_value, b"changed-python-fingerprint")

    def test_media_structure_queries_are_paginated(self) -> None:
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                first_asset = transaction.import_media(self.media_path)
                second_asset = transaction.import_media(self.second_media_path)
                transaction.add_media_root("media", "Media", 1)
            imported = production.latest_revision
            assert imported is not None

            first_page = production.assets_page(limit=1)
            self.assertEqual(len(first_page.items), 1)
            self.assertIsNotNone(first_page.next_cursor)
            self.assertFalse(first_page.traversal_truncated)
            paged_assets = list(first_page.items)
            cursor = first_page.next_cursor
            while cursor is not None:
                page = production.assets_page(limit=1, cursor=cursor)
                paged_assets.extend(page.items)
                cursor = page.next_cursor
            self.assertEqual(tuple(paged_assets), tuple(production.assets))
            self.assertEqual(
                {asset.id for asset in paged_assets}, {first_asset, second_asset}
            )

            representations = production.representations_page(first_asset, limit=1000)
            self.assertIsNone(representations.next_cursor)
            self.assertEqual(
                representations.items, production.representations[first_asset]
            )
            representation = representations.items[0]
            resource = representation.resources[0]
            locator = resource.locators[0]

            resources = production.resources_page(representation.id, limit=1000)
            self.assertEqual(resources.items, (resource.id,))
            self.assertIsNone(resources.next_cursor)

            locators = production.locators_page(resource.id, limit=1000)
            self.assertEqual(
                locators.items, (LocatorMatch(resource.id, locator, None),)
            )
            self.assertIsNone(locators.next_cursor)
            self.assertEqual(production.unresolved_media(limit=1000).items, ())
            self.assertEqual(
                production.representations_under_media_root("media", limit=1000).items,
                (),
            )

            with production.transaction() as transaction:
                transaction.retire_locator(locator.id)
            self.assertEqual(
                production.unresolved_media(limit=1000).items, (representation.id,)
            )
            self.assertEqual(
                production.locators_page(resource.id, limit=1000).items, ()
            )

            with production.transaction() as transaction:
                transaction.confirm_locator(
                    resource.id, locator.uri, media_root="media"
                )
            self.assertEqual(production.unresolved_media(limit=1000).items, ())
            (confirmed,) = production.locators_page(resource.id, limit=1000).items
            self.assertEqual(confirmed.resource_id, resource.id)
            self.assertEqual(confirmed.locator.uri, locator.uri)
            self.assertEqual(confirmed.media_root, "media")
            under_root = production.representations_under_media_root(
                "media", limit=1000
            )
            self.assertEqual(
                tuple(value.id for value in under_root.items), (representation.id,)
            )
            with self.assertRaises(NotFoundError):
                production.representations_under_media_root("archive", limit=1000)

            changed = production.objects_changed_since(imported.sequence, limit=1000)
            self.assertIsNone(changed.next_cursor)
            self.assertIn(resource.id, changed.items)
            self.assertNotIn(second_asset, changed.items)
            everything = production.objects_changed_since(0, limit=1000).items
            self.assertIn(first_asset, everything)
            self.assertIn(second_asset, everything)
            first_change = production.objects_changed_since(0, limit=1)
            assert first_change.next_cursor is not None
            rest = production.objects_changed_since(
                0, limit=1000, cursor=first_change.next_cursor
            )
            self.assertEqual(first_change.items + rest.items, everything)

            with self.assertRaises(InvalidArgumentError):
                production.assets_page(limit=0)
            with self.assertRaises(InvalidArgumentError):
                production.assets_page(limit=1, cursor="not-a-cursor")
            with self.assertRaises(InvalidArgumentError):
                production.representations_under_media_root("", limit=1)
            with self.assertRaises(ValueError):
                with production.transaction() as transaction:
                    transaction.confirm_locator(
                        resource.id, "file:///nul", media_root="bad\0root"
                    )

    def test_metadata_queries_page_and_filter_exact_scalars(self) -> None:
        scene = MetadataProperty("https://example.com/metadata", "scene")
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                first_asset = transaction.import_media(self.media_path)
                second_asset = transaction.import_media(self.second_media_path)
            with production.transaction() as transaction:
                transaction.add_metadata(first_asset, scene, MetadataString("12A"))
                transaction.add_metadata(second_asset, scene, MetadataString("14"))
                transaction.add_metadata(second_asset, scene, MetadataU64(12))

            everything = production.query_metadata(scene, limit=1000)
            self.assertIsNone(everything.next_cursor)
            self.assertEqual(
                set(everything.items), set(production.metadata_by_property[scene])
            )
            self.assertEqual(len(everything.items), 3)

            first_page = production.query_metadata(scene, limit=2)
            assert first_page.next_cursor is not None
            second_page = production.query_metadata(
                scene, limit=2, cursor=first_page.next_cursor
            )
            self.assertIsNone(second_page.next_cursor)
            self.assertEqual(first_page.items + second_page.items, everything.items)

            exact = production.query_metadata(
                scene, limit=1000, value=MetadataString("12A")
            )
            self.assertEqual(
                exact.items,
                (MetadataAssertion(first_asset, scene, MetadataString("12A")),),
            )
            typed = production.query_metadata(scene, limit=1000, value=MetadataU64(12))
            self.assertEqual(
                typed.items, (MetadataAssertion(second_asset, scene, MetadataU64(12)),)
            )
            self.assertEqual(
                production.query_metadata(
                    scene, limit=1000, value=MetadataString("99")
                ).items,
                (),
            )
            with self.assertRaises(InvalidArgumentError):
                production.query_metadata(
                    scene, limit=1000, value=MetadataList((MetadataString("12A"),))
                )
            with self.assertRaises(InvalidArgumentError):
                production.query_metadata(scene, limit=0)

    def test_provenance_queries_are_bounded_and_paginated(self) -> None:
        tool = ToolIdentity("FFmpeg", "8.0", "https://ffmpeg.org/")
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                source_asset = transaction.import_media(self.media_path)
                output_asset = transaction.import_media(self.second_media_path)
            source = production.representations[source_asset][0]
            output = production.representations[output_asset][0]
            with production.transaction() as transaction:
                transaction.create_activity(
                    ActivitySpec(
                        "org.postproject:transcode",
                        inputs=(ActivityEdge(source.id),),
                        outputs=(ActivityEdge(output.id),),
                        tool=tool,
                    )
                )
            activity = production.activities[0]

            producing = production.activities_producing_page(output.id, limit=1000)
            self.assertEqual(producing.items, (activity,))
            self.assertIsNone(producing.next_cursor)
            consuming = production.activities_consuming_page(source.id, limit=1000)
            self.assertEqual(consuming.items, (activity,))
            self.assertEqual(
                production.activities_producing_page(source.id, limit=1000).items, ()
            )

            self.assertEqual(
                production.outputs_by_activity_kind(
                    "org.postproject:transcode", limit=1000
                ).items,
                (output.id,),
            )
            self.assertEqual(
                production.outputs_by_activity_kind(
                    "org.postproject:conform", limit=1000
                ).items,
                (),
            )
            self.assertEqual(
                production.outputs_by_tool(tool, limit=1000).items, (output.id,)
            )
            self.assertEqual(
                production.outputs_by_tool(
                    ToolIdentity("FFmpeg", "7.1", "https://ffmpeg.org/"), limit=1000
                ).items,
                (),
            )

            ancestors = production.provenance_ancestors_page(
                output.id, max_depth=4, max_representations=1000, limit=1000
            )
            self.assertEqual(ancestors.items, (ProvenanceMatch(source.id, 1),))
            self.assertIsNone(ancestors.next_cursor)
            self.assertFalse(ancestors.traversal_truncated)
            descendants = production.provenance_descendants_page(
                source.id, max_depth=4, max_representations=1000, limit=1000
            )
            self.assertEqual(descendants.items, (ProvenanceMatch(output.id, 1),))
            with self.assertRaises(InvalidArgumentError):
                production.provenance_ancestors_page(
                    output.id, max_depth=0, max_representations=1000, limit=1000
                )

            self.assertEqual(
                production.stale_artifacts(
                    max_depth=64, max_representations=1000, limit=1000
                ).items,
                (),
            )
            source_fingerprint = source.fingerprints[0]
            with production.transaction() as transaction:
                transaction.record_representation_fingerprint(
                    source.id,
                    Fingerprint(
                        source_fingerprint.algorithm,
                        source_fingerprint.version,
                        b"changed-python-fingerprint",
                    ),
                )
            stale = production.stale_artifacts(
                max_depth=64, max_representations=1000, limit=1000
            )
            self.assertEqual(stale.items, (output.id,))
            self.assertIsNone(stale.next_cursor)
            self.assertFalse(stale.traversal_truncated)
            self.assertEqual(
                production.stale_artifacts(
                    max_depth=64,
                    max_representations=1000,
                    limit=1000,
                    source=source.id,
                ).items,
                (output.id,),
            )
            self.assertEqual(
                production.stale_artifacts(
                    max_depth=64,
                    max_representations=1000,
                    limit=1000,
                    source=output.id,
                ).items,
                (),
            )

    def test_regeneration_plans_copy_provenance_without_enqueuing(self) -> None:
        parameter = MetadataProperty("org.postproject.parameters", "profile")
        with Production.create(
            self.production_path, library_path=LIBRARY_PATH
        ) as production:
            with production.transaction() as transaction:
                source_asset = transaction.import_media(self.media_path)
                output_asset = transaction.import_media(self.second_media_path)
            source = production.representations[source_asset][0]
            output = production.representations[output_asset][0]
            with production.transaction() as transaction:
                activity_id = transaction.create_activity(
                    ActivitySpec(
                        "org.postproject:transcode",
                        inputs=(ActivityEdge(source.id),),
                        outputs=(ActivityEdge(output.id),),
                    )
                )
                transaction.add_metadata(
                    activity_id, parameter, MetadataString("editorial-proxy")
                )
            revision = production.latest_revision

            plans = production.plan_regeneration((output.id, output.id))

            self.assertEqual(len(plans), 1)
            plan = plans[0]
            self.assertEqual(plan.artifact_representation_id, output.id)
            self.assertEqual(plan.job.kind, "org.postproject:transcode")
            self.assertEqual(plan.job.inputs, (source.id,))
            self.assertEqual(plan.job.output_asset_id, output_asset)
            self.assertEqual(
                plan.job.output_representation_kind, RepresentationKind.ORIGINAL
            )
            self.assertEqual(plan.job.state, JobState.REQUESTED)
            self.assertEqual(
                plan.parameters,
                (
                    MetadataAssertion(
                        plan.job.id, parameter, MetadataString("editorial-proxy")
                    ),
                ),
            )
            self.assertEqual(production.jobs(limit=1000).items, ())
            self.assertEqual(production.latest_revision, revision)


if __name__ == "__main__":
    unittest.main()
