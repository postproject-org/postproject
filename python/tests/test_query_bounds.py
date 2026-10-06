"""Python integers cannot wrap into accepted native query limits."""

import tempfile
import unittest
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path

from postproject import (
    ActivityEdge,
    ActivitySpec,
    AssetRef,
    Fingerprint,
    ImageSequenceSource,
    InvalidArgumentError,
    MetadataDecimal,
    MetadataI64,
    MetadataProperty,
    MetadataRational,
    MetadataTimestamp,
    MetadataU64,
    Production,
    ProductionRef,
    RevisionObserver,
    SequenceNaming,
)


class QueryBoundsTests(unittest.TestCase):
    def test_activity_timestamps_and_boolean_priorities_reject_before_staging(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            media = root / "media.mov"
            media.write_bytes(b"activity timestamp fixture")
            with Production.create(root / "production.pproj") as production:
                with production.transaction() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                representation = production.representations[asset][0].id
                head = production.latest_revision
                spec = ActivitySpec(
                    "org.example:render", (ActivityEdge(representation),)
                )
                with production.transaction() as edit:
                    for timestamp in (-(2**63) - 1, 2**64 + 1):
                        with self.assertRaises(InvalidArgumentError):
                            edit.create_activity(
                                replace(spec, started_at_unix_micros=timestamp)
                            )
                        with self.assertRaises(InvalidArgumentError):
                            edit.create_activity(
                                replace(spec, finished_at_unix_micros=timestamp)
                            )
                    with self.assertRaises(TypeError):
                        edit.create_activity(replace(spec, started_at_unix_micros=True))
                    with self.assertRaises(TypeError):
                        edit.add_media_root("media", priority=True)
                    self.assertIsNone(edit.commit().revision)
                self.assertEqual(production.latest_revision, head)
                self.assertEqual(production.activities, ())
                with production.transaction() as edit:
                    activity = edit.create_activity(
                        replace(spec, started_at_unix_micros=-(2**63))
                    )
                    edit.commit()
                self.assertEqual(production.activities[0].id, activity)
                self.assertEqual(
                    production.activities[0].started_at_unix_micros, -(2**63)
                )

    def test_metadata_integer_values_cannot_wrap(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "production.pproj") as production:
                target = ProductionRef(production.id)
                property = MetadataProperty("urn:test", "numbers")
                invalid_values = (
                    MetadataI64(2**63),
                    MetadataI64(-(2**63) - 1),
                    MetadataU64(-1),
                    MetadataU64(2**64 + 1),
                    MetadataTimestamp(2**64 + 1),
                    MetadataDecimal(1, 2**32),
                    MetadataRational(2**64 + 1, 1),
                    MetadataRational(1, 2**64 + 1),
                )
                with production.transaction() as edit:
                    for value in invalid_values:
                        with self.subTest(value=value):
                            with self.assertRaises(InvalidArgumentError):
                                edit.add_metadata(target, property, value)
                    for value in (MetadataI64(True), MetadataU64(True)):
                        with self.assertRaises(TypeError):
                            edit.add_metadata(target, property, value)
                    self.assertIsNone(edit.commit().revision)
                self.assertEqual(production.metadata[target], ())
                self.assertIsNone(production.latest_revision)
                valid_values = (
                    MetadataI64(-(2**63)),
                    MetadataI64(2**63 - 1),
                    MetadataU64(2**64 - 1),
                    MetadataTimestamp(-(2**63)),
                    MetadataTimestamp(2**63 - 1),
                    MetadataRational(-(2**63), 2**64 - 1),
                )
                with production.transaction() as edit:
                    for value in valid_values:
                        edit.add_metadata(target, property, value)
                    edit.commit()
                self.assertEqual(
                    tuple(item.value for item in production.metadata[target]),
                    valid_values,
                )

    def test_sequence_integers_reject_truncation_before_import(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "frame0001.exr").write_bytes(b"frame one")
            source = ImageSequenceSource(
                root, SequenceNaming("frame", ".exr", 4), 1, 2, 1, 24, 1, (2,)
            )
            invalid_sources = (
                replace(source, start=2**64 + 1),
                replace(source, end=2**64 + 2),
                replace(source, step=2**32 + 1),
                replace(source, rate_numerator=2**32 + 24),
                replace(source, rate_denominator=2**32 + 1),
                replace(source, missing_frames=(2**64 + 2,)),
                replace(source, naming=SequenceNaming("frame", ".exr", 260)),
            )
            with Production.create(root / "production.pproj") as production:
                with production.transaction() as edit:
                    for invalid in invalid_sources:
                        with self.subTest(source=invalid):
                            with self.assertRaises(InvalidArgumentError):
                                edit.import_media(invalid)
                    with self.assertRaises(TypeError):
                        edit.import_media(replace(source, step=True))
                    self.assertIsNone(edit.commit().revision)
                self.assertEqual(production.assets_page(limit=1).items, ())
                self.assertIsNone(production.latest_revision)
                with production.transaction() as edit:
                    asset = edit.import_media(source)
                    edit.commit()
                sequence = production.representations[asset][0].image_sequence
                assert sequence is not None
                self.assertEqual(sequence.missing_frames, (2,))

    def test_fingerprint_versions_reject_wrapping_in_reads_and_edits(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            media = root / "media.mov"
            media.write_bytes(b"fingerprint integer fixture")
            with Production.create(root / "production.pproj") as production:
                with production.transaction() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                representation = production.representations[asset][0]
                resource = representation.resources[0]
                fingerprint = resource.fingerprints[0]
                head = production.latest_revision
                with production.read_session() as view:
                    with view.edit() as edit:
                        for version in (-1, 2**16 + fingerprint.version):
                            invalid = replace(fingerprint, version=version)
                            for reader in (production, view):
                                with self.assertRaises(InvalidArgumentError):
                                    reader.find_known_media_by_fingerprint(
                                        invalid, limit=1
                                    )
                            with self.assertRaises(InvalidArgumentError):
                                edit.record_resource_fingerprint(resource.id, invalid)
                            with self.assertRaises(InvalidArgumentError):
                                edit.record_representation_fingerprint(
                                    representation.id, invalid
                                )
                        with self.assertRaises(TypeError):
                            edit.record_resource_fingerprint(
                                resource.id, replace(fingerprint, version=True)
                            )
                        self.assertIsNone(edit.commit().revision)
                self.assertEqual(production.latest_revision, head)
                maximum = Fingerprint("test-content", 2**16 - 1, b"max version")
                with production.read_session() as view, view.edit() as edit:
                    edit.record_resource_fingerprint(resource.id, maximum)
                    edit.commit()
                matches = production.find_known_media_by_fingerprint(maximum, limit=1)
                self.assertEqual(matches.items[0].resource_id, resource.id)

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
