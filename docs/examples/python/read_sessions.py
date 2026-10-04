"""Pinned views and explicit-commit edits through the installed binding."""

import sys
from pathlib import Path

from postproject import Production


# [coherent-reads]
def exercise(path: Path, media: Path) -> None:
    with Production.create(path) as production:
        with production.read_session() as empty:
            with empty.edit() as edit:
                asset = edit.import_media(media)
                receipt = edit.commit()
            assert receipt.revision is not None and receipt.revision.sequence == 1
            assert empty.assets_page(limit=10).items == ()
        with production.read_session() as view:
            copied = view.asset(asset)
            page = view.representations_page(asset, limit=10)
            assert len(page.items) == 1
            representation = view.representation(page.items[0].id)
            base = view.decision_base
        with production.edit(base) as later:
            assert later.commit().revision is None
        # Exiting an uncommitted edit rolls back, even without an exception.
        with production.edit(base) as discarded:
            discarded.add_media_root("discarded")
        assert production.media_roots == ()
        assert copied.id == asset and representation.asset_id == asset
        production.close()
        view.close()  # close is idempotent


# [/coherent-reads]

if __name__ == "__main__":
    directory = Path(sys.argv[1])
    exercise(directory / "views.pproj", directory / "rushes/A001.mov")
