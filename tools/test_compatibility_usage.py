"""Unit tests for compatibility-family usage evidence."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "compatibility_usage", ROOT / "tools" / "compatibility_usage.py"
)
assert SPEC is not None and SPEC.loader is not None
USAGE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = USAGE
SPEC.loader.exec_module(USAGE)


class CompatibilityUsageTests(unittest.TestCase):
    """Exercise family validation and whole-family host matching."""

    def test_repository_manifest_names_real_c_operations(self) -> None:
        manifest = USAGE.load_manifest(ROOT / "docs" / "compatibility-families.toml")
        USAGE.validate_c_operations(
            manifest, ROOT / "include" / "postproject" / "postproject.h"
        )

    def test_complete_family_use_requires_every_operation(self) -> None:
        family = USAGE.Family(
            identifier="example",
            introduced_release="0.4.0-alpha.1",
            last_changed_release="0.4.0-alpha.1",
            last_changed_abi=35,
            c_operations=("pp_one", "pp_two"),
            cpp_operations=(),
            python_operations=(),
            projection_only=False,
        )
        manifest = USAGE.Manifest(
            baseline_release="0.4.0-alpha.1",
            candidate_release="0.5.0-alpha.1",
            release_order=("0.4.0-alpha.1", "0.5.0-alpha.1"),
            families=(family,),
        )
        report = USAGE.render_report(
            manifest,
            {
                "host-a": frozenset({"pp_one", "pp_two"}),
                "host-b": frozenset({"pp_one"}),
            },
            {},
        )

        self.assertIn("independent hosts=1", report)
        self.assertIn("| no |", report)

    def test_evidence_lines_are_deduplicated(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.txt"
            path.write_text("# comment\npp_one\n\npp_one\npp_two\n", encoding="utf-8")
            self.assertEqual(USAGE.load_evidence(path), frozenset({"pp_one", "pp_two"}))


if __name__ == "__main__":
    unittest.main()
