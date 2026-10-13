"""Canonical lookup preserves candidates, continuation and coherent ownership."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from uuid import uuid4

from postproject import (
    InvalidArgumentError,
    IoError,
    LocatorIdentity,
    Production,
    file_locator,
)


class KnownMediaInputTests(unittest.TestCase):
    def test_paths_and_explicit_evidence_preserve_all_logical_candidates(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            media = root / "take % café.bin"
            media.write_bytes(b"matching content, distinct identities")
            with Production.create(root / "production.pproj") as production:
                with production.edit() as edit:
                    first = edit.import_media(media).asset_id
                    edit.commit()
                locator = LocatorIdentity(file_locator(media))
                fingerprint = (
                    production.representations[first][0].resources[0].fingerprints[0]
                )
                with production.read_session() as view:
                    before = view.decision_base
                    with production.edit() as edit:
                        second = edit.import_media(media).asset_id
                        edit.commit()
                    # Path conversion uses this handle's loaded native library.
                    with patch(
                        "postproject._production.NativeLibrary",
                        side_effect=AssertionError,
                    ):
                        retained = view.find_known_media(media, limit=10)
                        current = production.find_known_media(str(media), limit=1)
                    self.assertEqual([v.asset_id for v in retained.items], [first])
                    self.assertEqual(view.decision_base, before)
                    self.assertIsNotNone(current.next_cursor)
                    remaining = production.find_known_media(
                        media, limit=1, cursor=current.next_cursor
                    )
                    self.assertEqual(
                        {v.asset_id for v in current.items + remaining.items},
                        {first, second},
                    )
                    self.assertIsNone(remaining.next_cursor)
                    expected = production.find_known_media(locator, limit=10)
                    self.assertEqual(
                        expected, production.find_known_media(media, limit=10)
                    )
                    self.assertEqual(
                        expected, production.find_known_media(fingerprint, limit=10)
                    )
                    unknown = root / "unknown"
                    unknown.write_bytes(b"not imported")
                    self.assertEqual(
                        production.find_known_media(unknown, limit=10).items, ()
                    )
                    with self.assertRaises(IoError):
                        production.find_known_media(root / "absent", limit=10)
                    with self.assertRaises(InvalidArgumentError):
                        view.find_known_media(
                            media, limit=1, cursor=current.next_cursor
                        )
                self.assertEqual(retained.items[0].asset_id, first)
                with self.assertRaises(InvalidArgumentError):
                    view.find_known_media(media, limit=1)
                self.assertFalse(hasattr(production, "find_known_media_by_locator"))
                self.assertFalse(hasattr(production, "find_known_media_by_fingerprint"))

    def test_invalid_inputs_leave_the_production_unchanged(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "production.pproj") as production:
                with production.read_session() as view:
                    for reader in (production, view):
                        for source in (None, True, uuid4(), b"path", ["path"]):
                            with (
                                self.subTest(source=source),
                                self.assertRaises(TypeError),
                            ):
                                reader.find_known_media(source, limit=10)  # ty: ignore[invalid-argument-type]
                        for limit in (0, -1, 1001):
                            with self.assertRaises(ValueError):
                                reader.find_known_media(
                                    LocatorIdentity("file:///unused"), limit=limit
                                )
                self.assertIsNone(production.latest_revision)
