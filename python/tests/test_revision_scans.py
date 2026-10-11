"""Revision scans are bounded, finite, pinned and owned by the caller's view."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from postproject import InvalidArgumentError, Production


class RevisionScanTests(unittest.TestCase):
    def test_page_boundaries_and_concurrent_writer_preserve_the_view(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "production.pproj"
            with Production.create(path) as production:
                for index in range(5):
                    with production.edit() as edit:
                        edit.add_media_root(str(index))
                        edit.commit()
                with production.read_session() as view:
                    with patch.object(
                        view, "changes_since", wraps=view.changes_since
                    ) as pages:
                        revisions = view.revisions(after=1, page_size=2)
                        pages.assert_not_called()
                        first = next(revisions)
                        with Production.open(path) as writer, writer.edit() as edit:
                            edit.add_media_root("late")
                            edit.commit()
                        copied = [first, *revisions]
                        self.assertEqual([r.sequence for r in copied], [2, 3, 4, 5])
                        self.assertEqual(
                            [call.args for call in pages.call_args_list],
                            [(1, 2), (3, 2), (5, 2)],
                        )
                        view.close()
                        self.assertEqual(list(revisions), [])
                with production.read_session() as fresh:
                    self.assertEqual(
                        [r.sequence for r in fresh.revisions()], list(range(1, 7))
                    )
                self.assertEqual(copied[-1].sequence, 5)

    def test_validation_lifetime_and_page_failures_are_visible(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "production.pproj") as production:
                with production.edit() as edit:
                    edit.add_media_root("first")
                    edit.commit()
                with production.edit() as edit:
                    edit.add_media_root("second")
                    edit.commit()
                with production.read_session() as view:
                    for size in (0, 1001):
                        with self.assertRaises(InvalidArgumentError):
                            view.revisions(page_size=size)
                    for after in (-1, 2**64):
                        with self.assertRaises(ValueError):
                            view.revisions(after=after)
                    for kwargs in ({"after": True}, {"page_size": True}):
                        with self.assertRaises(TypeError):
                            view.revisions(**kwargs)
                    failure = InvalidArgumentError(1, "page failed")
                    with patch.object(view, "changes_since", side_effect=failure):
                        with self.assertRaises(InvalidArgumentError) as caught:
                            next(view.revisions())
                        self.assertIs(caught.exception, failure)
                    untouched = view.revisions()
                    buffered = view.revisions(page_size=2)
                    next(buffered)
                for revisions in (untouched, buffered):
                    with self.assertRaisesRegex(InvalidArgumentError, "closed"):
                        next(revisions)
                with self.assertRaisesRegex(InvalidArgumentError, "closed"):
                    view.revisions()
