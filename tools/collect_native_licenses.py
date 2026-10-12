"""Collect dependency notices for a locked native build, outside the consumer.

Run after Cargo has fetched the build dependencies. The inventory includes
normal and build dependencies of the FFI library and CLI for the given target;
it is deliberately conservative about code eliminated by the linker.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def cargo(*arguments: str) -> str:
    return subprocess.check_output(["cargo", *arguments], cwd=ROOT, text=True)


def collect(output: Path, target: str) -> None:
    metadata = json.loads(
        cargo("metadata", "--locked", "--offline", "--format-version", "1")
    )
    tree = cargo(
        "tree",
        "--locked",
        "--offline",
        "--target",
        target,
        "-p",
        "postproject-ffi",
        "-p",
        "postproject-cli",
        "--edges",
        "normal,build",
        "--prefix",
        "none",
        "--format",
        "{p}",
    )
    names = set(re.findall(r"(?m)^(\S+) v(\S+)", tree))
    output.mkdir(parents=True, exist_ok=False)
    inventory = []
    for package in sorted(
        metadata["packages"], key=lambda item: (item["name"], item["version"])
    ):
        if (package["name"], package["version"]) not in names:
            continue
        source = Path(package["manifest_path"]).parent
        if package["source"] is None:
            source = ROOT
        notices = sorted(
            path
            for path in source.iterdir()
            if path.is_file()
            and path.name.lower().startswith(
                ("license", "copying", "notice", "unlicense")
            )
        )
        if not notices:
            raise RuntimeError(
                f"missing notices: {package['name']} {package['version']}"
            )
        destination = output / f"{package['name']}-{package['version']}"
        destination.mkdir()
        for notice in notices:
            shutil.copyfile(notice, destination / notice.name)
        inventory.append(
            {
                "name": package["name"],
                "version": package["version"],
                "license": package["license"],
                "repository": package["repository"],
                "notices": [f"{destination.name}/{notice.name}" for notice in notices],
            }
        )
    (output / "inventory.json").write_text(
        json.dumps({"target": target, "dependencies": inventory}, indent=2) + "\n",
        encoding="utf-8",
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("output", type=Path, help="new notices directory")
    parser.add_argument(
        "--target", help="Rust target triple; defaults to the compiler host"
    )
    args = parser.parse_args()
    target = args.target
    if target is None:
        version = subprocess.check_output(["rustc", "-vV"], text=True)
        target = next(
            line.removeprefix("host: ")
            for line in version.splitlines()
            if line.startswith("host: ")
        )
    collect(args.output, target)


if __name__ == "__main__":
    main()
