"""Create a production and import one opaque media fixture."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from postproject import OriginIdentity, Production


class Cancelled(Exception):
    """Raised to abandon a change before it is committed."""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("production", type=Path)
    parser.add_argument("media", type=Path)
    parser.add_argument(
        "--library",
        type=Path,
        help="native library to load instead of POSTPROJECT_LIBRARY",
    )
    args = parser.parse_args()

    # library_path=None falls back to the POSTPROJECT_LIBRARY variable. The
    # binding never searches the working directory or the loader path.
    with Production.create(
        args.production, "Python quickstart", library_path=args.library
    ) as production:
        # Commit explicitly; an uncommitted context always rolls back.
        with production.transaction(
            origin=OriginIdentity("org.postproject:python-quickstart"),
            message="Import quickstart media",
        ) as transaction:
            asset_id = transaction.import_media(
                args.media, display_name="Quickstart media"
            )
            transaction.commit()

        # An exception escaping the context rolls the transaction back.
        try:
            with production.transaction() as transaction:
                transaction.add_media_root("proxies")
                raise Cancelled
        except Cancelled:
            pass
        assert production.media_roots == ()

        # Threads may share one production; calls on it serialize internally.
        with ThreadPoolExecutor(max_workers=4) as pool:
            counts = list(
                pool.map(lambda _: len(production.representations[asset_id]), range(4))
            )
        print(f"representations: {counts[0]}")

        # A separately opened handle does not wait for calls on the first one,
        # such as a long commit.
        reader = Production.open(args.production, library_path=args.library)
        assert asset_id in reader.assets
        reader.close()

    production.close()  # The context closed it already; this is harmless.


if __name__ == "__main__":
    main()
