"""Bounded scans retain one explicitly owned view and surface failures."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from postproject import InvalidArgumentError, Production


class AssetIterationTests(unittest.TestCase):
    def test_pages_are_lazy_bounded_and_pinned_across_writer_commits(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "production.pproj"
            media = Path(directory) / "clip.mov"
            media.write_bytes(b"fixture")
            with Production.create(path) as production:
                with production.edit() as edit:
                    expected = [edit.import_media(media) for _ in range(5)]
                    edit.commit()
                with production.read_session() as view:
                    with patch.object(
                        view, "assets_page", wraps=view.assets_page
                    ) as page:
                        assets = view.assets(page_size=2)
                        page.assert_not_called()
                        first = next(assets)
                        self.assertIn(first.id, expected)
                        self.assertEqual(page.call_count, 1)
                        with Production.open(path) as writer:
                            with writer.edit() as edit:
                                late = edit.import_media(media)
                                edit.commit()
                        rest = list(assets)
                        self.assertEqual(
                            {first.id, *(a.id for a in rest)}, set(expected)
                        )
                        self.assertNotIn(late, {a.id for a in rest})
                        self.assertEqual(page.call_count, 3)
                        self.assertTrue(
                            all(
                                call.kwargs["limit"] == 2
                                for call in page.call_args_list
                            )
                        )
                        self.assertEqual(list(assets), [])
                    self.assertEqual(len(list(view.assets(page_size=1000))), 5)
                with production.read_session() as fresh:
                    self.assertEqual(len(list(fresh.assets())), 6)

    def test_close_rejects_first_advance_and_buffered_values(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "production.pproj"
            media = Path(directory) / "clip.mov"
            media.write_bytes(b"fixture")
            with Production.create(path) as production:
                with production.edit() as edit:
                    edit.import_media(media, "first")
                    edit.import_media(media, "second")
                    edit.commit()
                with production.read_session() as view:
                    untouched = view.assets()
                    buffered = view.assets(page_size=2)
                    copied = next(buffered)
                for iterator in (untouched, buffered):
                    with self.assertRaisesRegex(InvalidArgumentError, "closed"):
                        next(iterator)
                self.assertIn(copied.display_name, ("first", "second"))
                with self.assertRaisesRegex(InvalidArgumentError, "closed"):
                    view.assets()

    def test_invalid_sizes_fail_before_iteration_and_empty_scans_exhaust(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "production.pproj") as production:
                with production.read_session() as view:
                    for size in (0, -1, 1001, 2**32):
                        with self.assertRaises(InvalidArgumentError):
                            view.assets(page_size=size)
                    for size in (True, 1.5, "2"):
                        with self.assertRaises(TypeError):
                            view.assets(page_size=size)  # ty: ignore[invalid-argument-type]
                    self.assertEqual(list(view.assets(page_size=1)), [])


if __name__ == "__main__":
    unittest.main()
