"""Coherent views, detached scope validation, and explicit edit disposal."""

import tempfile
import unittest
from pathlib import Path

from postproject import (
    AssetRef,
    CancelledError,
    CancelToken,
    CommittedRevision,
    ConflictError,
    ContentVerification,
    DecisionBase,
    ExternalIdentifier,
    InvalidArgumentError,
    LocatorIdentity,
    MetadataProperty,
    MetadataString,
    Production,
    VerificationMode,
    file_locator,
    fingerprint_file,
)


class ReadSessionTests(unittest.TestCase):
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
