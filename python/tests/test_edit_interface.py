"""One edit type preserves additive, retained and detached write semantics."""

import tempfile
import unittest
from pathlib import Path

import postproject
from postproject import Edit, InvalidArgumentError, OriginIdentity, Production


class EditInterfaceTests(unittest.TestCase):
    def test_context_options_and_receipts_for_all_three_forms(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "production.pproj") as production:
                with production.edit(origin="importer", message="Add root") as edit:
                    self.assertIsInstance(edit, Edit)
                    root = edit.add_media_root("rushes")
                    first = edit.commit().revision
                self.assertIsNotNone(first)
                with production.read_session() as view:
                    with view.edit(
                        origin=OriginIdentity("host", "1"), message="Disable"
                    ) as edit:
                        edit.set_media_root_enabled(root, False)
                        second = edit.commit().revision
                with production.read_session() as view:
                    base = view.decision_base
                with production.edit(base, origin="host", message="Enable") as edit:
                    edit.set_media_root_enabled(root, True)
                    third = edit.commit().revision
                revisions = production.changes_since(0, 100)
                self.assertEqual(
                    [r.message for r in revisions], ["Add root", "Disable", "Enable"]
                )
                self.assertEqual(
                    [r.id for r in revisions],
                    [r.id for r in (first, second, third) if r],
                )
                self.assertEqual(revisions[1].origin, OriginIdentity("host", "1"))
                self.assertFalse(hasattr(production, "transaction"))
                self.assertFalse(hasattr(postproject, "Transaction"))

    def test_invalid_context_releases_the_write_guard(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "production.pproj") as production:
                with self.assertRaises(TypeError):
                    production.edit(origin=123)  # ty: ignore[invalid-argument-type]
                with production.read_session() as view:
                    with self.assertRaises(ValueError):
                        view.edit(message="invalid\0message")
                with production.edit() as edit:
                    edit.add_media_root("rolled-back")
                self.assertEqual(production.media_roots, ())
                with production.edit() as edit:
                    edit.add_media_root("retained")
                    edit.commit()
                with self.assertRaises(InvalidArgumentError):
                    edit.commit()


if __name__ == "__main__":
    unittest.main()
