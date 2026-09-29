"""Build a compatibility-family usage matrix from explicit ABI traces."""

from __future__ import annotations

import argparse
import re
from dataclasses import dataclass
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]
EXPORTED_FUNCTION = re.compile(r"\b(pp_[a-z0-9_]+)\s*\(")


@dataclass(frozen=True)
class Family:
    """One all-or-nothing compatibility evidence family."""

    identifier: str
    introduced_release: str
    last_changed_release: str
    last_changed_abi: int
    c_operations: tuple[str, ...]
    cpp_operations: tuple[str, ...]
    python_operations: tuple[str, ...]
    projection_only: bool


@dataclass(frozen=True)
class Manifest:
    """Parsed compatibility evidence policy and operation families."""

    baseline_release: str
    candidate_release: str
    release_order: tuple[str, ...]
    families: tuple[Family, ...]


def load_manifest(path: Path) -> Manifest:
    """Read and validate a compatibility-family manifest."""
    document = tomllib.loads(path.read_text(encoding="utf-8"))
    if document.get("format_version") != 1:
        raise ValueError("unsupported compatibility-family manifest version")
    release_order = tuple(document["release_order"])
    if len(release_order) != len(set(release_order)):
        raise ValueError("release_order contains duplicates")

    families = tuple(
        Family(
            identifier=item["id"],
            introduced_release=item["introduced_release"],
            last_changed_release=item["last_changed_release"],
            last_changed_abi=item["last_changed_abi"],
            c_operations=tuple(item["c_operations"]),
            cpp_operations=tuple(item["cpp_operations"]),
            python_operations=tuple(item["python_operations"]),
            projection_only=item.get("projection_only", False),
        )
        for item in document["families"]
    )
    identifiers = [family.identifier for family in families]
    if len(identifiers) != len(set(identifiers)):
        raise ValueError("family identifiers must be unique")
    for family in families:
        if family.introduced_release not in release_order:
            raise ValueError(f"unknown introduced release for {family.identifier}")
        if family.last_changed_release not in release_order:
            raise ValueError(f"unknown last-change release for {family.identifier}")
        if family.projection_only != (not family.c_operations):
            raise ValueError(
                f"{family.identifier}: projection_only must match an empty C operation set"
            )

    return Manifest(
        baseline_release=document["baseline_release"],
        candidate_release=document["candidate_release"],
        release_order=release_order,
        families=families,
    )


def load_evidence(path: Path) -> frozenset[str]:
    """Read newline-delimited operation evidence, ignoring comments and blanks."""
    return frozenset(
        line
        for raw_line in path.read_text(encoding="utf-8").splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )


def exported_functions(path: Path) -> frozenset[str]:
    """Return exported function names declared by the authoritative C header."""
    return frozenset(EXPORTED_FUNCTION.findall(path.read_text(encoding="utf-8")))


def validate_c_operations(manifest: Manifest, header: Path) -> None:
    """Reject misspelled or removed ABI operations in the manifest."""
    exports = exported_functions(header)
    missing = sorted(
        operation
        for family in manifest.families
        for operation in family.c_operations
        if operation not in exports
    )
    if missing:
        raise ValueError(f"manifest names unknown C operations: {', '.join(missing)}")


def render_report(
    manifest: Manifest,
    traces: dict[str, frozenset[str]],
    projections: dict[str, frozenset[str]],
) -> str:
    """Render the deterministic family-to-host evidence report."""
    release_position = {
        release: position for position, release in enumerate(manifest.release_order)
    }
    baseline_position = release_position[manifest.baseline_release]
    hosts = sorted(set(traces) | set(projections))
    lines = [
        "# Compatibility-family usage evidence",
        "",
        (
            f"Baseline: `{manifest.baseline_release}`. Candidate: "
            f"`{manifest.candidate_release}`."
        ),
        "",
        "| Family | Last change | ABI | "
        + " | ".join(hosts)
        + " | Mechanically eligible |",
        "|---|---|---:|" + "---|" * len(hosts) + "---|",
    ]

    details: list[str] = []
    for family in manifest.families:
        used_by: list[str] = []
        host_cells: list[str] = []
        required_projection = set(family.cpp_operations) | set(family.python_operations)
        for host in hosts:
            abi_used = bool(family.c_operations) and set(family.c_operations) <= set(
                traces.get(host, frozenset())
            )
            projection_used = bool(required_projection) and bool(
                required_projection & set(projections.get(host, frozenset()))
            )
            used = projection_used if family.projection_only else abi_used
            if used:
                used_by.append(host)
            evidence = []
            if abi_used:
                evidence.append("ABI")
            if projection_used:
                evidence.append("projection")
            host_cells.append(" + ".join(evidence) if evidence else "—")

        existed = release_position[family.introduced_release] <= baseline_position
        unchanged = release_position[family.last_changed_release] <= baseline_position
        eligible = existed and unchanged and len(used_by) >= 2
        lines.append(
            f"| `{family.identifier}` | `{family.last_changed_release}` | "
            f"{family.last_changed_abi} | "
            + " | ".join(host_cells)
            + f" | {'yes' if eligible else 'no'} |"
        )

        details.extend(
            [
                "",
                f"## {family.identifier}",
                "",
                "C ABI: "
                + (
                    ", ".join(f"`{item}`" for item in family.c_operations)
                    if family.c_operations
                    else "projection-only"
                ),
                "",
                "C++ projection: "
                + (
                    ", ".join(f"`{item}`" for item in family.cpp_operations)
                    if family.cpp_operations
                    else "none"
                ),
                "",
                "Python projection: "
                + (
                    ", ".join(f"`{item}`" for item in family.python_operations)
                    if family.python_operations
                    else "none"
                ),
                "",
                "Hosts with complete mechanical use evidence: "
                + (", ".join(f"`{host}`" for host in used_by) or "none"),
                "",
                (
                    "Eligibility checks: "
                    f"existed at baseline={'yes' if existed else 'no'}, "
                    f"unchanged after baseline={'yes' if unchanged else 'no'}, "
                    f"independent hosts={len(used_by)}."
                ),
            ]
        )

    return "\n".join(lines + details) + "\n"


def named_paths(values: list[str]) -> dict[str, Path]:
    """Parse repeatable HOST=PATH command-line values."""
    parsed: dict[str, Path] = {}
    for value in values:
        name, separator, path = value.partition("=")
        if not separator or not name or not path:
            raise ValueError(f"expected HOST=PATH, received {value!r}")
        if name in parsed:
            raise ValueError(f"duplicate evidence for host {name!r}")
        parsed[name] = Path(path)
    return parsed


def main() -> None:
    """Run the compatibility usage report command."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=ROOT / "docs" / "compatibility-families.toml",
    )
    parser.add_argument(
        "--header",
        type=Path,
        default=ROOT / "include" / "postproject" / "postproject.h",
    )
    parser.add_argument("--trace", action="append", default=[], metavar="HOST=PATH")
    parser.add_argument(
        "--projection", action="append", default=[], metavar="HOST=PATH"
    )
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()

    try:
        manifest = load_manifest(arguments.manifest)
        validate_c_operations(manifest, arguments.header)
        traces = {
            host: load_evidence(path)
            for host, path in named_paths(arguments.trace).items()
        }
        projections = {
            host: load_evidence(path)
            for host, path in named_paths(arguments.projection).items()
        }
    except (OSError, KeyError, TypeError, ValueError, tomllib.TOMLDecodeError) as error:
        raise SystemExit(str(error)) from error

    report = render_report(manifest, traces, projections)
    if arguments.output is None:
        print(report, end="")
    else:
        arguments.output.write_text(report, encoding="utf-8")


if __name__ == "__main__":
    main()
