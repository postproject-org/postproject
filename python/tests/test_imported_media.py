"""Import returns its staged identities without rediscovering representations."""

import tempfile
import unittest
from fractions import Fraction
from pathlib import Path
from uuid import UUID

from postproject import (
    ImageSequenceSource,
    ImportedMedia,
    NotFoundError,
    Production,
    SequenceNaming,
)


class ImportedMediaTests(unittest.TestCase):
    def test_file_result_is_copied_and_commit_makes_both_identities_durable(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            media = root / "take.mov"
            media.write_bytes(b"same content may have distinct logical identities")
            with Production.create(root / "production.pproj") as production:
                with production.edit() as edit:
                    first = edit.import_media(media)
                    second = edit.import_media(media)
                    self.assertIsInstance(first, ImportedMedia)
                    self.assertIsInstance(first.asset_id, UUID)
                    self.assertIsInstance(first.representation_id, UUID)
                    self.assertNotEqual(first.asset_id, second.asset_id)
                    self.assertNotEqual(
                        first.representation_id, second.representation_id
                    )
                    with self.assertRaises(NotFoundError):
                        production.representation(first.representation_id)
                    edit.commit()
                for imported in (first, second):
                    self.assertEqual(
                        production.representation(imported.representation_id).asset_id,
                        imported.asset_id,
                    )
                with production.edit() as edit:
                    discarded = edit.import_media(media)
                with self.assertRaises(NotFoundError):
                    production.representation(discarded.representation_id)

    def test_compound_result_names_the_compact_original(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for frame in (1, 2):
                (root / f"shot.{frame:04}.exr").write_bytes(f"frame {frame}".encode())
            source = ImageSequenceSource(
                root, SequenceNaming("shot.", ".exr", 4), 1, 2, 1, Fraction(24)
            )
            with Production.create(root / "production.pproj") as production:
                with production.edit() as edit:
                    imported = edit.import_media(source)
                    edit.commit()
                representation = production.representation(imported.representation_id)
                self.assertEqual(representation.asset_id, imported.asset_id)
                self.assertEqual(len(representation.resources), 1)
                sequence = representation.image_sequence
                assert sequence is not None
                self.assertEqual(sequence.end, 2)
