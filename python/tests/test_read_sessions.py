"""Coherent views, detached scope validation, and explicit edit disposal."""

import tempfile
import unittest
from pathlib import Path

from postproject import (
    ActivityEdge,
    ActivitySpec,
    ArtifactKnowledgeState,
    AssetRef,
    CancelledError,
    CancelToken,
    CommittedRevision,
    ConflictError,
    ContentVerification,
    DecisionBase,
    Dependency,
    ExternalIdentifier,
    InvalidArgumentError,
    JobRequest,
    JobState,
    LocatorIdentity,
    MetadataProperty,
    MetadataString,
    NotFoundError,
    Production,
    RepresentationKind,
    RepresentationRef,
    ToolIdentity,
    VerificationMode,
    file_locator,
    fingerprint_file,
)


class ReadSessionTests(unittest.TestCase):
    def test_revision_head_and_pages_end_at_the_retained_view(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "journal.pproj") as production:
                with production.read_session() as empty:
                    self.assertIsNone(empty.latest_revision)
                    with empty.edit() as edit:
                        edit.add_media_root("first")
                        first = edit.commit()
                    self.assertIsNone(empty.latest_revision)
                    self.assertEqual(empty.changes_since(0, 10), ())
                with production.read_session() as retained:
                    latest = retained.latest_revision
                    assert latest is not None and first.revision is not None
                    self.assertEqual(latest.id, first.revision.id)
                    self.assertEqual(latest.sequence, first.revision.sequence)
                    self.assertEqual(retained.changes_since(0, 1), (latest,))
                    with retained.edit() as edit:
                        edit.add_media_root("second")
                        second = edit.commit()
                    self.assertEqual(retained.latest_revision, latest)
                    self.assertEqual(retained.changes_since(0, 10), (latest,))
                    self.assertEqual(retained.changes_since(latest.sequence, 10), ())
                    with production.read_session() as fresh:
                        head = fresh.latest_revision
                        assert head is not None and second.revision is not None
                        self.assertEqual(head.id, second.revision.id)
                        self.assertEqual(
                            fresh.changes_since(latest.sequence, 1), (head,)
                        )
                    with self.assertRaises(InvalidArgumentError):
                        retained.changes_since(0, 0)
                self.assertEqual(latest.id, first.revision.id)
                for operation in (
                    lambda: retained.latest_revision,
                    lambda: retained.changes_since(0, 1),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_provenance_and_staleness_retain_intervening_activity_facts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            media.write_bytes(b"provenance view fixture")
            with Production.create(Path(directory) / "provenance.pproj") as production:
                with production.read_session() as empty, empty.edit() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                with production.read_session() as before:
                    original = before.representations_page(asset, limit=1).items[0]
                    tool = ToolIdentity("Example", "1", "https://example.com/tool")
                    with before.edit() as edit:
                        proxy = edit.add_representation(
                            asset, RepresentationKind.PROXY, media
                        )
                        activity = edit.create_activity(
                            ActivitySpec(
                                "example:render",
                                tool=tool,
                                inputs=(ActivityEdge(original.id),),
                                outputs=(ActivityEdge(proxy),),
                            )
                        )
                        inspected = edit.add_representation(
                            asset, RepresentationKind.OPTIMIZED, media
                        )
                        edit.create_activity(
                            ActivitySpec(
                                "example:inspection",
                                inputs=(ActivityEdge(original.id),),
                                outputs=(ActivityEdge(inspected),),
                            )
                        )
                        edit.commit()
                    self.assertEqual(
                        before.activities_consuming_page(original.id, limit=1).items, ()
                    )
                    self.assertEqual(
                        before.provenance_descendants_page(
                            original.id, max_depth=64, max_representations=100, limit=10
                        ).items,
                        (),
                    )
                    self.assertEqual(
                        before.outputs_by_activity_kind(
                            "example:render", limit=10
                        ).items,
                        (),
                    )
                    self.assertEqual(before.outputs_by_tool(tool, limit=10).items, ())
                with production.read_session() as retained:
                    ancestors = retained.provenance_ancestors_page(
                        proxy, max_depth=64, max_representations=100, limit=10
                    )
                    descendants = retained.provenance_descendants_page(
                        original.id, max_depth=64, max_representations=100, limit=10
                    )
                    self.assertEqual(
                        tuple(
                            (item.representation_id, item.depth)
                            for item in ancestors.items
                        ),
                        ((original.id, 1),),
                    )
                    self.assertEqual(
                        {
                            (item.representation_id, item.depth)
                            for item in descendants.items
                        },
                        {(proxy, 1), (inspected, 1)},
                    )
                    self.assertEqual(
                        retained.stale_artifacts(
                            max_depth=64, max_representations=100, limit=10
                        ).items,
                        (),
                    )
                    self.assertEqual(
                        retained.outputs_by_activity_kind(
                            "example:render", limit=10
                        ).items,
                        (proxy,),
                    )
                    self.assertEqual(
                        retained.outputs_by_tool(tool, limit=10).items, (proxy,)
                    )
                    self.assertEqual(
                        retained.outputs_by_tool(
                            ToolIdentity("Example", "2", tool.uri), limit=10
                        ).items,
                        (),
                    )
                    producing = retained.activities_producing_page(proxy, limit=10)
                    self.assertEqual({item.id for item in producing.items}, {activity})
                    consuming = retained.activities_consuming_page(original.id, limit=1)
                    self.assertIsNotNone(consuming.next_cursor)
                    assert consuming.next_cursor is not None
                    continuation = retained.activities_consuming_page(
                        original.id, limit=1, cursor=consuming.next_cursor
                    )
                    self.assertEqual(len(continuation.items), 1)
                    self.assertNotEqual(consuming.items[0].id, continuation.items[0].id)
                    with self.assertRaises(InvalidArgumentError):
                        retained.activities_producing_page(
                            proxy, limit=1, cursor=consuming.next_cursor
                        )
                    media.write_bytes(b"changed original content")
                    with retained.edit() as edit:
                        edit.observe_resource_content(original.resources[0].id, media)
                        edit.commit()
                    self.assertEqual(
                        retained.stale_artifacts(
                            max_depth=64,
                            max_representations=100,
                            limit=10,
                            source=original.id,
                        ).items,
                        (),
                    )
                    self.assertEqual(
                        retained.activities_producing_page(proxy, limit=10), producing
                    )
                    self.assertEqual(
                        retained.activities_consuming_page(original.id, limit=1),
                        consuming,
                    )
                    with production.read_session() as fresh:
                        with self.assertRaises(InvalidArgumentError):
                            fresh.activities_consuming_page(
                                original.id, limit=1, cursor=consuming.next_cursor
                            )
                        self.assertEqual(
                            set(
                                fresh.stale_artifacts(
                                    max_depth=64,
                                    max_representations=100,
                                    limit=10,
                                    source=original.id,
                                ).items
                            ),
                            {proxy, inspected},
                        )
                    with self.assertRaises(InvalidArgumentError):
                        retained.provenance_ancestors_page(
                            proxy, max_depth=64, max_representations=100, limit=0
                        )
                self.assertEqual(ancestors.items[0].representation_id, original.id)
                for operation in (
                    lambda: retained.activities_producing_page(proxy, limit=1),
                    lambda: retained.activities_consuming_page(original.id, limit=1),
                    lambda: retained.outputs_by_activity_kind(
                        "example:render", limit=1
                    ),
                    lambda: retained.outputs_by_tool(tool, limit=1),
                    lambda: retained.provenance_ancestors_page(
                        proxy, max_depth=64, max_representations=100, limit=1
                    ),
                    lambda: retained.provenance_descendants_page(
                        original.id, max_depth=64, max_representations=100, limit=1
                    ),
                    lambda: retained.stale_artifacts(
                        max_depth=64, max_representations=100, limit=1
                    ),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_object_filters_retain_root_locator_and_journal_facts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            moved = Path(directory) / "moved.mov"
            media.write_bytes(b"filtered view fixture")
            moved.write_bytes(media.read_bytes())
            with Production.create(Path(directory) / "filters.pproj") as production:
                with production.read_session() as empty, empty.edit() as edit:
                    asset = edit.import_media(media)
                    edit.add_media_root("rushes")
                    edit.commit()
                with production.read_session() as retained:
                    representation = retained.representations_page(
                        asset, limit=1
                    ).items[0]
                    resource = representation.resources[0]
                    head = retained.decision_base.revision
                    assert head is not None
                    self.assertEqual(
                        retained.representations_under_media_root(
                            "rushes", limit=1
                        ).items,
                        (),
                    )
                    self.assertEqual(retained.unresolved_media(limit=1).items, ())
                    self.assertEqual(
                        retained.objects_changed_since(head.sequence, limit=10).items,
                        (),
                    )
                    with retained.edit() as edit:
                        edit.confirm_locator(
                            resource.id, file_locator(moved), media_root="rushes"
                        )
                        edit.commit()
                    self.assertEqual(
                        retained.representations_under_media_root(
                            "rushes", limit=1
                        ).items,
                        (),
                    )
                    self.assertEqual(
                        retained.objects_changed_since(head.sequence, limit=10).items,
                        (),
                    )
                    with production.read_session() as located:
                        rooted = located.representations_under_media_root(
                            "rushes", limit=1
                        )
                        self.assertEqual(
                            tuple(item.id for item in rooted.items),
                            (representation.id,),
                        )
                        changed = located.objects_changed_since(head.sequence, limit=10)
                        self.assertTrue(changed.items)
                        with located.edit() as edit:
                            for locator in located.locators_page(
                                resource.id, limit=10
                            ).items:
                                edit.retire_locator(locator.locator.id)
                            edit.commit()
                        self.assertEqual(located.unresolved_media(limit=1).items, ())
                        self.assertEqual(
                            located.representations_under_media_root("rushes", limit=1),
                            rooted,
                        )
                        with production.read_session() as fresh:
                            self.assertEqual(
                                fresh.unresolved_media(limit=1).items,
                                (representation.id,),
                            )
                    with self.assertRaises(NotFoundError):
                        retained.representations_under_media_root("missing", limit=1)
                    with self.assertRaises(InvalidArgumentError):
                        retained.objects_changed_since(0, limit=0)
                for operation in (
                    lambda: retained.representations_under_media_root(
                        "rushes", limit=1
                    ),
                    lambda: retained.unresolved_media(limit=1),
                    lambda: retained.objects_changed_since(0, limit=1),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_dependency_reads_retain_replaced_knowledge(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            media.write_bytes(b"dependency view fixture")
            with Production.create(
                Path(directory) / "dependencies.pproj"
            ) as production:
                with production.read_session() as empty, empty.edit() as edit:
                    asset = edit.import_media(media)
                    proxy = edit.add_representation(
                        asset, RepresentationKind.PROXY, media
                    )
                    edit.commit()
                with production.read_session() as before:
                    self.assertIsNone(before.dependency_set(proxy))
                    dependency = Dependency("example:source", AssetRef(asset), "camera")
                    with before.edit() as edit:
                        edit.record_dependency_set(proxy, (dependency,))
                        edit.commit()
                    self.assertIsNone(before.dependency_set(proxy))
                    self.assertEqual(
                        before.dependencies(
                            proxy, max_depth=1, max_representations=100, limit=1
                        ).items,
                        (),
                    )
                with production.read_session() as retained:
                    recorded = retained.dependency_set(proxy)
                    self.assertIsNotNone(recorded)
                    assert recorded is not None
                    self.assertEqual(recorded.dependencies, (dependency,))
                    forward = retained.dependencies(
                        proxy, max_depth=1, max_representations=100, limit=1
                    )
                    reverse = retained.dependents(
                        AssetRef(asset), max_depth=1, max_representations=100, limit=1
                    )
                    self.assertEqual(
                        tuple(item.target for item in forward.items), (AssetRef(asset),)
                    )
                    self.assertEqual(
                        tuple(item.target for item in reverse.items),
                        (RepresentationRef(proxy),),
                    )
                    with retained.edit() as edit:
                        edit.record_dependency_set(proxy, ())
                        edit.commit()
                    self.assertEqual(retained.dependency_set(proxy), recorded)
                    self.assertEqual(
                        retained.dependencies(
                            proxy, max_depth=1, max_representations=100, limit=1
                        ),
                        forward,
                    )
                    self.assertEqual(
                        retained.dependents(
                            AssetRef(asset),
                            max_depth=1,
                            max_representations=100,
                            limit=1,
                        ),
                        reverse,
                    )
                    with production.read_session() as fresh:
                        empty_set = fresh.dependency_set(proxy)
                        self.assertIsNotNone(empty_set)
                        assert empty_set is not None
                        self.assertEqual(empty_set.dependencies, ())
                        self.assertEqual(
                            fresh.dependents(
                                AssetRef(asset),
                                max_depth=1,
                                max_representations=100,
                                limit=1,
                            ).items,
                            (),
                        )
                    with self.assertRaises(InvalidArgumentError):
                        retained.dependencies(
                            proxy, max_depth=1, max_representations=100, limit=0
                        )
                self.assertEqual(recorded.dependencies, (dependency,))
                for operation in (
                    lambda: retained.dependency_set(proxy),
                    lambda: retained.dependencies(
                        proxy, max_depth=1, max_representations=100, limit=1
                    ),
                    lambda: retained.dependents(
                        AssetRef(asset), max_depth=1, max_representations=100, limit=1
                    ),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_artifact_reports_retain_their_provenance_view(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            media.write_bytes(b"artifact view fixture")
            with Production.create(Path(directory) / "artifacts.pproj") as production:
                with production.read_session() as empty, empty.edit() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                with production.read_session() as view:
                    representation = view.representations_page(asset, limit=1).items[0]
                    original = view.evaluate_artifact(representation.id)
                    report = view.artifact_reproducibility(representation.id)
                    self.assertEqual(
                        original.state, ArtifactKnowledgeState.INDETERMINATE
                    )
                    self.assertIsNone(report.producing_activity_id)
                    with view.edit() as edit:
                        activity = edit.create_activity(
                            ActivitySpec(
                                "example:observed",
                                outputs=(ActivityEdge(representation.id),),
                            )
                        )
                        edit.commit()
                    self.assertEqual(
                        view.evaluate_artifact(representation.id), original
                    )
                    self.assertEqual(
                        view.artifact_reproducibility(representation.id), report
                    )
                    with production.read_session() as fresh:
                        self.assertEqual(
                            fresh.evaluate_artifact(representation.id).state,
                            ArtifactKnowledgeState.CURRENT,
                        )
                        self.assertEqual(
                            fresh.artifact_reproducibility(
                                representation.id
                            ).producing_activity_id,
                            activity,
                        )
                    with self.assertRaises(InvalidArgumentError):
                        view.evaluate_artifact(representation.id, max_representations=0)
                for operation in (
                    lambda: view.evaluate_artifact(representation.id),
                    lambda: view.artifact_reproducibility(representation.id),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_job_reads_remain_pinned_across_request_and_cancellation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            media.write_bytes(b"job view fixture")
            with Production.create(Path(directory) / "jobs.pproj") as production:
                with production.read_session() as empty, empty.edit() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                with production.read_session() as before:
                    source = before.representations_page(asset, limit=1).items[0]
                    with before.edit() as edit:
                        job = edit.request_job(
                            JobRequest(
                                "example:proxy",
                                (source.id,),
                                asset,
                                RepresentationKind.PROXY,
                            )
                        )
                        edit.commit()
                    self.assertEqual(before.jobs(limit=1).items, ())
                    with self.assertRaises(NotFoundError):
                        before.job(job)
                with production.read_session() as requested:
                    original = requested.job(job)
                    with requested.edit() as edit:
                        edit.cancel_job(job)
                        edit.commit()
                    self.assertEqual(requested.job(job), original)
                    self.assertEqual(
                        requested.jobs(
                            limit=1, state=JobState.REQUESTED, kind="example:proxy"
                        ).items,
                        (original,),
                    )
                    self.assertEqual(production.job(job).state, JobState.CANCELLED)
                    with self.assertRaises(InvalidArgumentError):
                        requested.jobs(limit=0)
                for operation in (
                    lambda: requested.job(job),
                    lambda: requested.jobs(limit=1),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_decision_tokens_are_canonical_bounded_and_validate_on_edit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "tokens.pproj") as production:
                with production.read_session() as empty:
                    empty_base = empty.decision_base
                    self.assertEqual(
                        DecisionBase.from_token(empty_base.to_token()), empty_base
                    )
                    with empty.edit() as edit:
                        edit.add_media_root("rushes")
                        edit.commit()
                with production.read_session() as view:
                    base = view.decision_base
                token = base.to_token()
                self.assertEqual(DecisionBase.from_token(token), base)
                for invalid in (
                    token + ":extra",
                    token + "\0",
                    "x" * 129,
                    token.replace("ppdb1", "ppdb2"),
                    token.rsplit(":", 1)[0] + ":01",
                ):
                    with self.assertRaises((InvalidArgumentError, ValueError)):
                        DecisionBase.from_token(invalid)
                assert base.revision is not None
                with self.assertRaises(ValueError):
                    DecisionBase(
                        base.production_id, CommittedRevision(base.revision.id, 0)
                    ).to_token()
                with Production.create(Path(directory) / "other.pproj") as other:
                    with self.assertRaises(InvalidArgumentError):
                        other.edit(DecisionBase.from_token(token))

    def test_resolution_pins_database_facts_but_reads_current_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            media.write_bytes(b"original content")
            with Production.create(Path(directory) / "resolve.pproj") as production:
                with production.read_session() as empty, empty.edit() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                with production.read_session() as view:
                    representation = view.representations_page(asset, limit=1).items[0]
                    resource = view.resources_page(representation.id, limit=1).items[0]
                    original = view.resolve(
                        asset, verification=VerificationMode.CONTENT
                    )
                    base = view.decision_base
                    self.assertEqual(
                        view.verify_resource(resource, media),
                        ContentVerification.MATCHES,
                    )
                    media.write_bytes(b"changed content")
                    with view.edit() as edit:
                        edit.observe_resource_content(resource, media)
                        receipt = edit.commit()
                    self.assertEqual(view.decision_base, base)
                    self.assertEqual(
                        view.verify_resource(resource, media),
                        ContentVerification.DIFFERS,
                    )
                    self.assertNotEqual(
                        view.resolve(asset, verification=VerificationMode.CONTENT),
                        original,
                    )
                    with production.read_session() as fresh:
                        self.assertEqual(
                            fresh.verify_resource(resource, media),
                            ContentVerification.MATCHES,
                        )
                        self.assertEqual(fresh.decision_base.revision, receipt.revision)
                    token = CancelToken()
                    token.cancel()
                    with self.assertRaises(CancelledError):
                        view.resolve(asset, cancel_token=token)
                    latest = production.latest_revision
                    assert latest is not None and receipt.revision is not None
                    self.assertEqual(latest.id, receipt.revision.id)
                for operation in (
                    lambda: view.verify_resource(resource, media),
                    lambda: view.resolve(asset),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_metadata_pages_and_exact_values_remain_in_the_pinned_view(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            media.write_bytes(b"metadata fixture")
            title = MetadataProperty("https://example.com/editorial", "title")
            with Production.create(Path(directory) / "metadata.pproj") as production:
                with production.read_session() as empty, empty.edit() as edit:
                    target = AssetRef(edit.import_media(media))
                    for value in ("Camera", "Alternate"):
                        edit.add_metadata(target, title, MetadataString(value))
                    edit.commit()
                with production.read_session() as view:
                    original = view.metadata(target)
                    page = view.query_metadata(title, limit=1)
                    assert page.next_cursor is not None
                    with view.edit() as edit:
                        edit.remove_metadata_property(target, title)
                        edit.add_metadata(target, title, MetadataString("Changed"))
                        edit.commit()
                    remainder = view.query_metadata(
                        title, limit=1, cursor=page.next_cursor
                    )
                    self.assertEqual(page.items + remainder.items, original)
                    self.assertEqual(view.metadata_by_property(title), original)
                    exact = view.query_metadata(
                        title, limit=1, value=MetadataString("Camera")
                    )
                    self.assertEqual(exact.items[0].value, MetadataString("Camera"))
                    self.assertEqual(
                        view.query_metadata(
                            title, limit=1, value=MetadataString("Changed")
                        ).items,
                        (),
                    )
                    with production.read_session() as refreshed:
                        self.assertEqual(
                            refreshed.metadata(target)[0].value,
                            MetadataString("Changed"),
                        )
                        with self.assertRaises(InvalidArgumentError):
                            refreshed.query_metadata(
                                title, limit=1, cursor=page.next_cursor
                            )
                self.assertEqual(original[0].target, target)
                for operation in (
                    lambda: view.metadata(target),
                    lambda: view.metadata_by_property(title),
                    lambda: view.query_metadata(title, limit=1),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_resource_and_locator_pages_retain_old_storage_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            moved = Path(directory) / "moved.mov"
            media.write_bytes(b"storage evidence fixture")
            moved.write_bytes(media.read_bytes())
            with Production.create(Path(directory) / "storage.pproj") as production:
                with production.read_session() as empty, empty.edit() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                with production.read_session() as view:
                    representation = view.representations_page(asset, limit=1).items[0]
                    resources = view.resources_page(representation.id, limit=1)
                    self.assertEqual(len(resources.items), 1)
                    resource = resources.items[0]
                    locators = view.locators_page(resource, limit=1)
                    self.assertEqual(locators.items[0].locator.uri, file_locator(media))
                    with view.edit() as edit:
                        edit.retire_locator(locators.items[0].locator.id)
                        edit.confirm_locator(resource, file_locator(moved))
                        edit.commit()
                    self.assertEqual(view.locators_page(resource, limit=1), locators)
                    self.assertEqual(
                        view.resources_page(representation.id, limit=1), resources
                    )
                    self.assertEqual(
                        view.representations_using_resource(resource, limit=1).items,
                        (representation,),
                    )
                    self.assertEqual(
                        production.locators_page(resource, limit=1)
                        .items[0]
                        .locator.uri,
                        file_locator(moved),
                    )
                    for operation in (
                        lambda: view.resources_page(representation.id, limit=0),
                        lambda: view.locators_page(resource, limit=0),
                        lambda: view.representations_using_resource(resource, limit=0),
                    ):
                        with self.assertRaises(InvalidArgumentError):
                            operation()
                self.assertEqual(locators.items[0].locator.uri, file_locator(media))
                for operation in (
                    lambda: view.resources_page(representation.id, limit=1),
                    lambda: view.locators_page(resource, limit=1),
                    lambda: view.representations_using_resource(resource, limit=1),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_roots_identifiers_and_known_media_use_the_same_pinned_view(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            media.write_bytes(b"coherent lookup fixture")
            with Production.create(Path(directory) / "lookup.pproj") as production:
                with production.transaction() as transaction:
                    asset = transaction.import_media(media)
                target = AssetRef(asset)
                with production.read_session() as view:
                    locator = LocatorIdentity(file_locator(media))
                    fingerprint = fingerprint_file(media)
                    matches = view.find_known_media_by_locator(locator, limit=1)
                    self.assertEqual(matches.items[0].asset_id, asset)
                    with view.edit() as edit:
                        edit.add_media_root("rushes")
                        edit.add_external_identifier(
                            target,
                            ExternalIdentifier("https://example.com/id", "camera"),
                        )
                        edit.commit()
                    self.assertEqual(view.media_roots, ())
                    self.assertEqual(view.external_identifiers(target), ())
                    self.assertEqual(
                        view.find_by_external_identifier(
                            "https://example.com/id", "camera", None
                        ),
                        (),
                    )
                    self.assertEqual(
                        view.find_known_media_by_fingerprint(
                            fingerprint, limit=1
                        ).items,
                        matches.items,
                    )
                    self.assertEqual(len(production.media_roots), 1)
                    self.assertEqual(
                        production.objects_by_external_identifier[
                            "https://example.com/id", "camera"
                        ],
                        (target,),
                    )
                self.assertEqual(matches.items[0].asset_id, asset)
                for operation in (
                    lambda: view.media_roots,
                    lambda: view.external_identifiers(target),
                    lambda: view.find_by_external_identifier(
                        "https://example.com/id", "camera", None
                    ),
                    lambda: view.find_known_media_by_locator(locator, limit=1),
                    lambda: view.find_known_media_by_fingerprint(fingerprint, limit=1),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

    def test_empty_view_remains_pinned_and_stale_edit_conflicts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "production.pproj") as production:
                with production.read_session() as empty:
                    base = empty.decision_base
                    self.assertIsNone(base.revision)
                    with empty.edit() as edit:
                        root = edit.add_media_root("rushes")
                        receipt = edit.commit()
                    self.assertEqual(empty.decision_base, base)
                    self.assertEqual(empty.assets_page(limit=1).items, ())
                    with production.edit(base) as stale:
                        stale.set_media_root_enabled(root, False)
                        with self.assertRaises(ConflictError) as failure:
                            stale.commit()
                        assert failure.exception.conflict is not None
                        self.assertIsNone(failure.exception.conflict.base_revision_id)
                with self.assertRaises(RuntimeError):
                    empty.assets_page(limit=1)
                empty.close()
                with production.read_session() as refreshed:
                    self.assertEqual(refreshed.decision_base.revision, receipt.revision)

    def test_detached_bases_validate_scope_and_integer_bounds(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with (
                Production.create(Path(directory) / "first.pproj") as first,
                Production.create(Path(directory) / "second.pproj") as second,
            ):
                with first.read_session() as view:
                    base = view.decision_base
                with self.assertRaises(InvalidArgumentError):
                    second.edit(base)
                with first.edit(base) as discarded:
                    discarded.add_media_root("discarded")
                self.assertEqual(first.media_roots, ())
                with first.edit(base) as edit:
                    edit.add_media_root("saved")
                    receipt = edit.commit()
                assert receipt.revision is not None
                for sequence in (0, -1, 2**64, True):
                    invalid = DecisionBase(
                        first.id, CommittedRevision(receipt.revision.id, sequence)
                    )
                    with self.assertRaises((InvalidArgumentError, ValueError)):
                        first.edit(invalid)
                with first.edit(base) as additive:
                    additive.add_media_root("independent")
                    self.assertIsNotNone(additive.commit().revision)
