"""Render PostProject's Flatpak module and check its offline Cargo sources.

``render`` writes ``postproject-VERSION-flatpak.json`` and
``postproject-VERSION-cargo-sources.json`` for a release source archive, either
published (by SHA-256) or local (by path). For a local archive it also writes
``org.postproject.FlatpakCheck.json``, an application that builds the module and
tests the installed package with ``tests/abi``. ``check`` verifies that the committed
Cargo sources list exactly the crates.io packages in ``Cargo.lock``; regenerate
them with flatpak-builder-tools' ``cargo/flatpak-cargo-generator.py`` when it
fails.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]
PACKAGING = ROOT / "packaging" / "flatpak"
TEMPLATE = PACKAGING / "postproject.json"
CARGO_SOURCES = PACKAGING / "postproject-cargo-sources.json"
CHECK_APP = PACKAGING / "check-app.json"
CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"


def render(
    version: str, sha256: str | None, archive: Path | None, output: Path
) -> Path:
    text = TEMPLATE.read_text(encoding="utf-8").replace("@VERSION@", version)
    module = json.loads(text.replace("@SHA256@", sha256 or ""))
    if archive is not None:
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        module["sources"][0] = {
            "type": "archive",
            "path": str(archive.resolve()),
            "sha256": digest,
        }
    output.mkdir(parents=True, exist_ok=True)
    (output / f"postproject-{version}-cargo-sources.json").write_text(
        CARGO_SOURCES.read_text(encoding="utf-8"), encoding="utf-8"
    )
    path = output / f"postproject-{version}-flatpak.json"
    path.write_text(json.dumps(module, indent=4) + "\n", encoding="utf-8")
    if archive is not None:
        app = json.loads(CHECK_APP.read_text(encoding="utf-8"))
        app["modules"][0] = path.name
        app["modules"][1]["sources"] = [module["sources"][0]]
        path = output / "org.postproject.FlatpakCheck.json"
        path.write_text(json.dumps(app, indent=4) + "\n", encoding="utf-8")
    return path


def crate_sources(sources: list[dict[str, str]]) -> dict[str, str]:
    """Maps each vendored crate directory to its archive checksum."""
    archives = {}
    checksums = {}
    for source in sources:
        destination = source.get("dest", "")
        if source["type"] == "archive":
            archives[destination] = source["sha256"]
        elif source.get("dest-filename") == ".cargo-checksum.json":
            checksums[destination] = json.loads(source["contents"])["package"]
    if archives != checksums:
        raise ValueError("archive and .cargo-checksum.json entries disagree")
    return archives


def locked_crates(lockfile: str) -> dict[str, str]:
    return {
        f"cargo/vendor/{package['name']}-{package['version']}": package["checksum"]
        for package in tomllib.loads(lockfile)["package"]
        if package.get("source") == CRATES_IO
    }


def check(lockfile: str, sources: list[dict[str, str]]) -> list[str]:
    expected = locked_crates(lockfile)
    actual = crate_sources(sources)
    problems = [f"missing {name}" for name in sorted(expected.keys() - actual.keys())]
    problems += [f"unused {name}" for name in sorted(actual.keys() - expected.keys())]
    problems += [
        f"checksum differs for {name}"
        for name in sorted(expected.keys() & actual.keys())
        if expected[name] != actual[name]
    ]
    return problems


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    render_parser = commands.add_parser("render")
    render_parser.add_argument("--version", required=True)
    origin = render_parser.add_mutually_exclusive_group(required=True)
    origin.add_argument("--sha256", help="checksum of the published source archive")
    origin.add_argument("--archive", type=Path, help="local source archive")
    render_parser.add_argument("--output", type=Path, required=True)
    commands.add_parser("check")
    arguments = parser.parse_args()

    if arguments.command == "render":
        print(
            render(
                arguments.version,
                arguments.sha256,
                arguments.archive,
                arguments.output,
            )
        )
        return
    problems = check(
        (ROOT / "Cargo.lock").read_text(encoding="utf-8"),
        json.loads(CARGO_SOURCES.read_text(encoding="utf-8")),
    )
    if problems:
        raise SystemExit(
            f"{CARGO_SOURCES.relative_to(ROOT)} is out of date:\n  "
            + "\n  ".join(problems)
        )


if __name__ == "__main__":
    main()
