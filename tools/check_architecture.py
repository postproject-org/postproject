"""Enforce production dependency direction and the unsafe-code boundary."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]
ALLOWED = {
    "postproject-core": set(),
    "postproject-protocol": {"postproject-core"},
    "postproject-storage-sqlite": {"postproject-core", "postproject-protocol"},
    "postproject-media": {"postproject-core"},
    "postproject-ffi-macros": set(),
    "postproject-ffi": {
        "postproject-core",
        "postproject-media",
        "postproject-storage-sqlite",
        "postproject-ffi-macros",
    },
    "postproject-cli": {
        "postproject-core",
        "postproject-media",
        "postproject-protocol",
        "postproject-storage-sqlite",
    },
    "postproject-doc-examples": set(),
}
CORE_LIBRARIES = {"thiserror", "url", "uuid"}


def dependency_violations(packages: list[dict]) -> list[str]:
    """Check normal/build edges, including optional and target-specific ones.

    Cargo reports original package names even when a dependency is renamed.
    Test-only edges are excluded: storage tests legitimately use media services.
    """
    problems = []
    names = {package["name"] for package in packages}
    for package in packages:
        name = package["name"]
        if name not in ALLOWED:
            problems.append(f"unreviewed workspace crate: {name}")
            continue
        for dependency in package["dependencies"]:
            if dependency["kind"] == "dev":
                continue
            target = dependency["name"]
            if target in names and target not in ALLOWED[name]:
                problems.append(f"forbidden production dependency: {name} -> {target}")
            if name == "postproject-core" and target not in CORE_LIBRARIES:
                problems.append(f"unreviewed core dependency: {target}")
    return sorted(set(problems))


def lint_violations(workspace: dict, manifests: dict[str, dict]) -> list[str]:
    """Keep the compiler's unsafe prohibition active outside the C adapter."""
    problems = []
    if workspace["workspace"]["lints"]["rust"].get("unsafe_code") != "forbid":
        problems.append("workspace unsafe_code lint must be forbid")
    for name, manifest in manifests.items():
        if name != "postproject-ffi" and manifest.get("lints") != {"workspace": True}:
            problems.append(f"{name} must inherit workspace lints")
    return problems


def main() -> int:
    metadata = json.loads(
        subprocess.check_output(
            [
                "cargo",
                "metadata",
                "--no-deps",
                "--format-version",
                "1",
                "--all-features",
                "--locked",
                "--offline",
            ],
            cwd=ROOT,
            text=True,
        )
    )
    packages = metadata["packages"]
    manifests = {
        package["name"]: tomllib.loads(Path(package["manifest_path"]).read_text())
        for package in packages
    }
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
    problems = dependency_violations(packages) + lint_violations(workspace, manifests)
    for problem in problems:
        print(problem, file=sys.stderr)
    if not problems:
        print("dependency direction, core libraries and unsafe boundary checked")
    return bool(problems)


if __name__ == "__main__":
    raise SystemExit(main())
