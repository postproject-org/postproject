"""Check positive hints and expected wrong-kind errors with the installed ty."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ty", default="ty", help="type-checker executable")
    parser.add_argument("--python", help="installed candidate Python environment")
    parser.add_argument("--source", help="source package, for development checks only")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    command = [
        args.ty,
        "check",
        "--python-version",
        "3.11",
        "--output-format",
        "concise",
    ]
    if args.python:
        command.extend(["--python", args.python])
    if args.source:
        command.extend(["--extra-search-path", str(Path(args.source).resolve())])
    with tempfile.TemporaryDirectory(prefix="postproject-typing-") as directory:
        positive = Path(directory) / "positive.py"
        negative = Path(directory) / "negative.py"
        positive.write_bytes((root / "python/tests/typing_ids.py").read_bytes())
        negative.write_bytes((root / "python/typing/wrong_ids.py.fail").read_bytes())
        good = subprocess.run(
            [*command, str(positive)], capture_output=True, text=True, check=False
        )
        bad = subprocess.run(
            [*command, str(negative)], capture_output=True, text=True, check=False
        )
    if good.returncode:
        raise SystemExit(good.stdout + good.stderr)
    diagnostics = [
        line for line in (bad.stdout + bad.stderr).splitlines() if "error[" in line
    ]
    if (
        bad.returncode != 1
        or len(diagnostics) != 6
        or any("error[invalid-argument-type]" not in line for line in diagnostics)
    ):
        raise SystemExit(
            "wrong-kind fixture did not produce exactly six type errors:\n"
            + bad.stdout
            + bad.stderr
        )
    print("positive identity hints pass; all six wrong-kind calls are rejected")


if __name__ == "__main__":
    main()
