"""Python integers cannot wrap into accepted native query limits."""

import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path

from postproject import (
    AssetRef,
    InvalidArgumentError,
    MetadataProperty,
    Production,
    RevisionObserver,
)


class QueryBoundsTests(unittest.TestCase):
    def test_sequence_traversal_and_resolver_bounds_cannot_wrap(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            media = root / "media.mov"
            media.write_bytes(b"query bounds fixture")
            with Production.create(root / "production.pproj") as production:
                with production.transaction() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                representation = production.representations[asset][0].id
                with (
                    production.read_session() as view,
                    production.revision_waiter() as waiter,
                ):
                    for reader in (production, view):
                        for invalid in (-1, 2**64, 2**64 + 1):
                            with self.assertRaises(InvalidArgumentError):
                                reader.changes_since(invalid, 1)
                            with self.assertRaises(InvalidArgumentError):
                                reader.objects_changed_since(invalid, limit=1)
                            with self.assertRaises(InvalidArgumentError):
                                waiter.wait(invalid, timeout=0)
                        for invalid in (-1, 2**32 + 1):
                            with self.assertRaises(InvalidArgumentError):
                                reader.dependencies(
                                    representation,
                                    max_depth=invalid,
                                    max_representations=10,
                                    limit=1,
                                )
                            with self.assertRaises(InvalidArgumentError):
                                reader.dependents(
                                    AssetRef(asset),
                                    max_depth=1,
                                    max_representations=invalid,
                                    limit=1,
                                )
                            with self.assertRaises(InvalidArgumentError):
                                reader.evaluate_artifact(
                                    representation, max_depth=invalid
                                )
                            with self.assertRaises(InvalidArgumentError):
                                reader.resolve(asset, max_depth=invalid)
                        with self.assertRaises(InvalidArgumentError):
                            reader.resolve(asset, max_entries_per_directory=2**64 + 1)
                        with self.assertRaises(TypeError):
                            reader.changes_since(True, 1)
                        with self.assertRaises(TypeError):
                            reader.dependencies(
                                representation,
                                max_depth=True,
                                max_representations=10,
                                limit=1,
                            )
                        self.assertEqual(len(reader.changes_since(0, 1)), 1)
                with self.assertRaises(InvalidArgumentError):
                    RevisionObserver(
                        production, lambda revision, events: None, after_sequence=2**64
                    )

    def test_live_retained_and_waiter_limits_reject_wraparound(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            media = root / "media.mov"
            media.write_bytes(b"bounded query fixture")
            with Production.create(root / "production.pproj") as production:
                with production.transaction() as edit:
                    asset = edit.import_media(media)
                    edit.add_media_root("media")
                    receipt = edit.commit()
                assert receipt.revision is not None
                revision = receipt.revision.id
                head = production.latest_revision
                resource = production.representations[asset][0].resources[0].id
                property = MetadataProperty("urn:test", "scene")
                with (
                    production.read_session() as view,
                    production.revision_waiter() as waiter,
                ):
                    for reader in (production, view):
                        operations: tuple[Callable[[int], object], ...] = (
                            lambda limit, reader=reader: reader.assets_page(
                                limit=limit
                            ),
                            lambda limit, reader=reader: reader.media_roots_page(
                                limit=limit
                            ),
                            lambda limit, reader=reader: reader.jobs(limit=limit),
                            lambda limit, reader=reader: reader.locators_page(
                                resource, limit=limit
                            ),
                            lambda limit, reader=reader: reader.query_metadata(
                                property, limit=limit
                            ),
                            lambda limit, reader=reader: reader.revision_events_page(
                                revision, limit=limit
                            ),
                            lambda limit, reader=reader: reader.changes_since(0, limit),
                        )
                        for operation in (
                            *operations,
                            lambda limit: waiter.wait(0, limit=limit, timeout=0),
                        ):
                            for invalid in (
                                0,
                                -1,
                                1001,
                                2**32 + 1,
                                1 - 2**32,
                                2**64 + 1,
                            ):
                                with self.subTest(
                                    reader=type(reader).__name__, limit=invalid
                                ):
                                    with self.assertRaises(
                                        InvalidArgumentError
                                    ) as failure:
                                        operation(invalid)
                                    self.assertEqual(failure.exception.code, 1)
                            with self.assertRaises(TypeError):
                                operation(True)
                        self.assertEqual(reader.assets_page(limit=1).items[0].id, asset)
                        self.assertEqual(len(reader.assets_page(limit=1000).items), 1)
                with self.assertRaises(TypeError):
                    production.assets_page(limit=1.0)  # ty: ignore[invalid-argument-type]
                with self.assertRaises(TypeError):
                    production.assets_page(limit="1")  # ty: ignore[invalid-argument-type]
                self.assertEqual(production.latest_revision, head)
