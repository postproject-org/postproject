#!/usr/bin/env python3
"""Compile every C++ program against the installed-style header with warnings as errors.

Consumers compile the header-only wrapper with their own compiler, standard, and
optimization level, so a warning it raises lands in every host's build log. Each
program is compiled, not linked, so no native library is needed.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WARNINGS = ["-Wall", "-Wextra", "-Wpedantic", "-Werror"]
STANDARDS = ["c++17", "c++20"]
# Optimizers report possibly uninitialized values only once functions are inlined.
OPTIMIZATION = "-O2"


def programs() -> list[tuple[Path, list[str]]]:
    """Return each C++ program with the extra flags it needs."""
    sources = sorted((ROOT / "docs" / "examples" / "cpp").glob("*.cpp"))
    sources += sorted((ROOT / "examples" / "cpp").glob("*.cpp"))
    sources += sorted((ROOT / "tests" / "abi").glob("cpp_*.cpp"))
    return [
        (source, ["-fno-exceptions"] if source.name == "cpp_no_exceptions.cpp" else [])
        for source in sources
    ]


def compile_program(
    compiler: str, standard: str, source: Path, extra: list[str]
) -> str | None:
    """Compile one program and return the diagnostics of a failure."""
    command = [
        compiler,
        f"-std={standard}",
        OPTIMIZATION,
        *WARNINGS,
        *extra,
        f"-I{ROOT / 'include'}",
        "-c",
        str(source),
        "-o",
        os.devnull,
    ]
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if result.returncode == 0:
        return None
    return f"{compiler} -std={standard} {source.relative_to(ROOT)}:\n{result.stderr}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "compilers", nargs="+", help="C++ compilers to check, such as g++ and clang++"
    )
    args = parser.parse_args()

    jobs = [
        (compiler, standard, source, extra)
        for compiler in args.compilers
        for standard in STANDARDS
        for source, extra in programs()
    ]
    with ThreadPoolExecutor(max_workers=os.cpu_count()) as pool:
        failures = [
            failure
            for failure in pool.map(lambda job: compile_program(*job), jobs)
            if failure
        ]
    for failure in failures:
        print(failure, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
