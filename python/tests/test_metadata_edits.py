"""Mergeable appends and protected destructive metadata decisions."""

import tempfile
import unittest
from pathlib import Path

from postproject import (
    ConflictError,
    ConflictKeyKind,
    InvalidArgumentError,
    MetadataProperty,
    MetadataString,
    Production,
    ProductionRef,
)


class MetadataEditTests(unittest.TestCase):
    def test_unbased_removal_rejects_without_staging_or_closing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "metadata.pproj") as production:
                target = ProductionRef(production.id)
                property = MetadataProperty("com.example.editor", "keywords")
                with production.transaction() as transaction:
                    transaction.add_metadata(target, property, MetadataString("first"))
                    transaction.commit()
                with production.transaction() as transaction:
                    with self.assertRaises(InvalidArgumentError):
                        transaction.remove_metadata_property(target, property)
                    transaction.add_metadata(target, property, MetadataString("second"))
                    receipt = transaction.commit()
                self.assertIsNotNone(receipt.revision)
                self.assertEqual(
                    [assertion.value for assertion in production.metadata[target]],
                    [MetadataString("first"), MetadataString("second")],
                )

    def test_stale_removal_conflicts_after_mergeable_appends(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "metadata.pproj") as production:
                target = ProductionRef(production.id)
                property = MetadataProperty("com.example.editor", "keywords")
                with production.read_session() as empty:
                    for value in ("first", "second"):
                        with empty.edit() as append:
                            append.add_metadata(target, property, MetadataString(value))
                            append.commit()
                    with empty.edit() as removal:
                        removal.remove_metadata_property(target, property)
                        with self.assertRaises(ConflictError) as rejected:
                            removal.commit()
                    conflict = rejected.exception.conflict
                    assert conflict is not None
                    self.assertEqual(
                        conflict.key.kind, ConflictKeyKind.METADATA_PROPERTY
                    )
                    self.assertEqual(conflict.key.target, target)
                    self.assertIsNone(conflict.base_revision_id)
                self.assertEqual(len(production.metadata[target]), 2)
                with production.read_session() as fresh, fresh.edit() as removal:
                    removal.remove_metadata_property(target, property)
                    self.assertIsNotNone(removal.commit().revision)
                self.assertEqual(production.metadata[target], ())
