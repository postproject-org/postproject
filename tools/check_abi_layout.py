#!/usr/bin/env python3
"""Compare C-compiler struct layout output with generated ctypes declarations."""

from __future__ import annotations

import argparse
import ctypes
import importlib.util
import sys
from pathlib import Path
from types import ModuleType


class LayoutError(ValueError):
    """Compiler layout output and ctypes declarations disagree."""


def load_abi_module(path: Path) -> ModuleType:
    spec = importlib.util.spec_from_file_location("_postproject_generated_abi", path)
    if spec is None or spec.loader is None:
        raise LayoutError(f"cannot import generated ABI module: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def expected_layout(module: ModuleType) -> dict[str, int]:
    try:
        public_structs = module.PUBLIC_STRUCTS
    except AttributeError as error:
        raise LayoutError("generated ABI module has no PUBLIC_STRUCTS table") from error
    expected: dict[str, int] = {}
    for c_name, (structure, fields) in public_structs.items():
        expected[f"{c_name}.size"] = ctypes.sizeof(structure)
        expected[f"{c_name}.alignment"] = ctypes.alignment(structure)
        for field in fields:
            expected[f"{c_name}.{field}"] = getattr(structure, field).offset
    return expected


def parse_layout(lines: list[str]) -> dict[str, int]:
    layout: dict[str, int] = {}
    for line_number, raw_line in enumerate(lines, start=1):
        line = raw_line.strip()
        if not line:
            continue
        key, separator, raw_value = line.partition("=")
        if not separator or not key or key in layout:
            raise LayoutError(f"invalid layout line {line_number}: {line!r}")
        try:
            layout[key] = int(raw_value)
        except ValueError as error:
            raise LayoutError(
                f"invalid layout value on line {line_number}: {raw_value!r}"
            ) from error
    return layout


def verify_layout(expected: dict[str, int], actual: dict[str, int]) -> None:
    problems: list[str] = []
    for key in sorted(expected.keys() - actual.keys()):
        problems.append(f"missing compiler value: {key}")
    for key in sorted(actual.keys() - expected.keys()):
        problems.append(f"unexpected compiler value: {key}")
    for key in sorted(expected.keys() & actual.keys()):
        if expected[key] != actual[key]:
            problems.append(
                f"layout mismatch for {key}: ctypes={expected[key]}, compiler={actual[key]}"
            )
    if problems:
        raise LayoutError("\n".join(problems))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("abi_module", type=Path)
    parser.add_argument(
        "layout",
        nargs="?",
        type=argparse.FileType("r", encoding="utf-8"),
        default=sys.stdin,
    )
    arguments = parser.parse_args()
    try:
        module = load_abi_module(arguments.abi_module)
        actual = parse_layout(arguments.layout.readlines())
        verify_layout(expected_layout(module), actual)
    except (OSError, LayoutError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
