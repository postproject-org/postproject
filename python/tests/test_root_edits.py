"""Root decisions and additive recovery through the public binding."""

import tempfile
import unittest
from pathlib import Path

from postproject import ConflictError, ConflictKeyKind, InvalidArgumentError, Production


class RootEditTests(unittest.TestCase):
    def test_unbased_rejection_preserves_creation_and_stale_removal_rolls_back(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "roots.pproj") as production:
                with production.transaction() as additive:
                    root = additive.add_media_root("rushes")
                    for enabled in (True, False):
                        with self.assertRaises(InvalidArgumentError):
                            additive.set_media_root_enabled(root, enabled)
                    with self.assertRaises(InvalidArgumentError):
                        additive.remove_media_root(root)
                    additive.commit()
                self.assertTrue(production.media_roots[0].enabled)
                with production.read_session() as old:
                    with old.edit() as winner:
                        winner.set_media_root_enabled(root, False)
                        receipt = winner.commit()
                    with old.edit() as stale:
                        stale.remove_media_root(root)
                        with self.assertRaises(ConflictError) as rejected:
                            stale.commit()
                    conflict = rejected.exception.conflict
                    assert conflict is not None and receipt.revision is not None
                    self.assertEqual(conflict.key.kind, ConflictKeyKind.MEDIA_ROOT)
                    self.assertEqual(conflict.key.target, root)
                    self.assertEqual(
                        conflict.superseding_revision_id, receipt.revision.id
                    )
                self.assertFalse(production.media_roots[0].enabled)
                head = production.latest_revision
                assert head is not None and receipt.revision is not None
                self.assertEqual(head.id, receipt.revision.id)
                with production.read_session() as fresh, fresh.edit() as removal:
                    removal.remove_media_root(root)
                    removal.commit()
                self.assertEqual(production.media_roots, ())
