"""Verify release versions agree across packages and generated documentation."""

from __future__ import annotations

import re
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]
SEMVER = re.compile(
    r"^(?P<base>\d+\.\d+\.\d+)(?:-(?P<stage>alpha|beta|rc)\.(?P<number>\d+))?$"
)
PEP440_STAGE = {"alpha": "a", "beta": "b", "rc": "rc"}
DOCUMENTED_PACKAGE = re.compile(
    r"\bpostproject-(?P<version>\d+\.\d+\.\d+"
    r"(?:-(?:alpha|beta|rc)\.\d+|(?:a|b|rc)\d+)?)"
    r"(?P<suffix>[-\w.]*)"
)


def documented_package_errors(
    root: Path, package_version: str, python_version: str
) -> list[str]:
    """Check current guides; historical release reports retain their versions."""
    paths = [
        root / "README.md",
        root / "docs/index.md",
        root / "docs/releasing.md",
        *sorted((root / "docs/src").rglob("*.md")),
    ]
    errors = []
    for path in paths:
        if not path.is_file():
            continue
        for line, text in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for match in DOCUMENTED_PACKAGE.finditer(text):
                expected = (
                    python_version if ".whl" in match["suffix"] else package_version
                )
                if match["version"] != expected:
                    errors.append(
                        f"{path.relative_to(root)}:{line}: {match[0]} "
                        f"must use version {expected}"
                    )
    return errors


def main() -> None:
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    rust_version = workspace["workspace"]["package"]["version"]
    match = SEMVER.fullmatch(rust_version)
    if match is None:
        raise SystemExit(f"unsupported workspace version: {rust_version}")

    expected_python = match["base"]
    if match["stage"] is not None:
        expected_python += PEP440_STAGE[match["stage"]] + match["number"]
    python = tomllib.loads(
        (ROOT / "python" / "pyproject.toml").read_text(encoding="utf-8")
    )
    python_version = python["project"]["version"]
    if python_version != expected_python:
        raise SystemExit(
            f"Python version {python_version} does not match {rust_version}"
        )

    cmake = (ROOT / "CMakeLists.txt").read_text(encoding="utf-8")
    project_version = _capture(cmake, r"project\(PostProject VERSION ([^ )]+)")
    package_version = _capture(cmake, r'set\(POSTPROJECT_PACKAGE_VERSION "([^"]+)"\)')
    if project_version != match["base"]:
        raise SystemExit(
            f"CMake project version {project_version} does not match {match['base']}"
        )
    if package_version != rust_version:
        raise SystemExit(
            f"CMake package version {package_version} does not match {rust_version}"
        )
    doxygen = (ROOT / "Doxyfile").read_text(encoding="utf-8")
    documentation_version = _capture(doxygen, r"(?m)^PROJECT_NUMBER\s*=\s*(\S+)")
    if documentation_version != rust_version:
        raise SystemExit(
            f"Doxygen version {documentation_version} does not match {rust_version}"
        )
    sphinx = (ROOT / "docs" / "conf.py").read_text(encoding="utf-8")
    if _capture(sphinx, r'(?m)^release = "([^"]+)"') != rust_version:
        raise SystemExit("Sphinx release does not match the workspace version")
    if _capture(sphinx, r'(?m)^version = "([^"]+)"') != ".".join(
        match["base"].split(".")[:2]
    ):
        raise SystemExit("Sphinx series does not match the workspace version")
    ffi = (ROOT / "crates/postproject-ffi/src/lib.rs").read_text(encoding="utf-8")
    abi = _capture(ffi, r"pub const ABI_VERSION: u32 = (\d+);")
    if _capture(sphinx, r'html_title = f"PostProject \{release\} · ABI (\d+)"') != abi:
        raise SystemExit("Sphinx title does not match the C ABI version")
    errors = documented_package_errors(ROOT, rust_version, python_version)
    if errors:
        raise SystemExit("Outdated documented packages:\n" + "\n".join(errors))
    print(f"release versions agree: {rust_version} / {python_version}")


def _capture(text: str, pattern: str) -> str:
    match = re.search(pattern, text)
    if match is None:
        raise SystemExit(f"version declaration not found: {pattern}")
    return match.group(1)


if __name__ == "__main__":
    main()
