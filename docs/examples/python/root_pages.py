"""Bounded root reads through live and retained readers."""

import sys
from pathlib import Path

from postproject import Production


# [root-pages]
def root_pages(path: Path) -> None:
    with Production.create(path) as production:
        with production.transaction() as transaction:
            transaction.add_media_root("first", priority=-1)
            transaction.add_media_root("second")
            transaction.commit()
        live = production.media_roots_page(limit=1)
        assert len(live.items) == 1 and live.next_cursor is not None
        with production.read_session() as view:
            first = view.media_roots_page(limit=1)
            last = view.media_roots_page(limit=1, cursor=first.next_cursor)
            assert first.items[0].name == "first"
            assert last.items[0].name == "second" and last.next_cursor is None


# [/root-pages]

if __name__ == "__main__":
    root_pages(Path(sys.argv[1]) / "roots.pproj")
