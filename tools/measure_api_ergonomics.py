"""Measure schema-27 Python read/staging paths in a selected installed wheel.

Run each phase in a new process. Use the published 0.8 wheel for historical
phases and the candidate for scans/native inputs. Python allocation peaks do
not include Rust/SQLite allocations; RSS includes interpreter/loader overhead.
"""

from __future__ import annotations

import argparse
import gc
import json
import resource
import time
import tracemalloc
from pathlib import Path

import postproject as pp


def prepare(root: Path) -> None:
    root.mkdir()
    media = root / "media.bin"
    media.write_bytes(b"benchmark fixture\n" * 64)
    with pp.Production.create(root / "production.pproj", "Read workload") as production:
        for batch in range(50):
            with production.edit() as edit:
                for item in range(100):
                    asset = edit.import_media(
                        media, display_name=f"{batch:02}-{item:02}"
                    )
                edit.commit()
        for index in range(1000):
            with production.edit() as edit:
                edit.add_metadata(pp.AssetRef(asset), ("benchmark", "history"), index)
                edit.commit()


def measure(root: Path, phase: str) -> int:
    with pp.Production.open(root / "production.pproj") as production:
        if phase == "assets-eager":
            # Historical 0.8 API only; never used by candidate integrations.
            # This branch exercises the removed, published 0.8 collection.
            return len(list(getattr(production, "assets")))
        with production.read_session() as view:
            if phase == "assets-pages":
                count, cursor = 0, None
                while True:
                    page = view.assets_page(limit=100, cursor=cursor)
                    count += len(page.items)
                    if page.next_cursor is None:
                        return count
                    cursor = page.next_cursor
            if phase.startswith("assets-"):
                size = int(phase.removeprefix("assets-"))
                return sum(1 for _ in view.assets(page_size=size))
            if phase.startswith("revisions-"):
                if phase == "revisions-scan":
                    return sum(1 for _ in view.revisions(page_size=100))
                after = count = 0
                while True:
                    page = view.changes_since(after, 100)
                    count += len(page)
                    if not page:
                        return count
                    after = page[-1].sequence
            asset = view.assets_page(limit=1).items[0].id
        if phase.startswith("metadata-"):
            factory = (
                production.edit
                if hasattr(view, "assets")
                else getattr(production, "transaction")
            )
            with factory() as edit:
                for index in range(1000):
                    if phase == "metadata-native":
                        key, value = ("benchmark", "input"), index
                    else:
                        key = pp.MetadataProperty("benchmark", "input")
                        value = pp.MetadataI64(index)
                    edit.add_metadata(pp.AssetRef(asset), key, value)
                # Measure staging only: context exit explicitly rolls back.
            return 1000
        if phase == "resolution":
            for _ in range(100):
                production.resolve(asset, search_directories=[root], max_depth=2)
            return 100
    raise ValueError(phase)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root", type=Path)
    parser.add_argument(
        "phase",
        choices=(
            "prepare",
            "assets-eager",
            "assets-pages",
            "assets-10",
            "assets-100",
            "assets-1000",
            "revisions-pages",
            "revisions-scan",
            "metadata-native",
            "metadata-explicit",
            "resolution",
        ),
    )
    args = parser.parse_args()
    if args.phase == "prepare":
        prepare(args.root)
        return
    gc.collect()
    tracemalloc.start()
    started = time.perf_counter()
    error = None
    try:
        count = measure(args.root, args.phase)
    except pp.UnsupportedError as failure:
        count = None
        error = str(failure)
    elapsed = time.perf_counter() - started
    _, peak = tracemalloc.get_traced_memory()
    print(
        json.dumps(
            {
                "phase": args.phase,
                "error": error,
                "items": count,
                "seconds": elapsed,
                "python_peak_bytes": peak,
                "peak_rss_kib": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
            }
        )
    )


if __name__ == "__main__":
    main()
