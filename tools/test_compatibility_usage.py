"""Unit tests for compatibility-family usage evidence."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from dataclasses import replace
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

    def test_resolution_family_covers_search_and_candidate_inspection(self) -> None:
        manifest = USAGE.load_manifest(ROOT / "docs" / "compatibility-families.toml")
        resolution = next(
            family for family in manifest.families if family.identifier == "resolution"
        )

        self.assertIn(
            "pp_resolution_options_add_search_directory", resolution.c_operations
        )
        self.assertIn("pp_resolution_set_get_candidate", resolution.c_operations)
        self.assertIn(
            "pp_resolution_set_get_candidate_evidence", resolution.c_operations
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

    def test_projection_only_family_requires_all_helpers_per_host(self) -> None:
        manifest = USAGE.load_manifest(ROOT / "docs" / "compatibility-families.toml")
        family = next(
            item
            for item in manifest.families
            if item.identifier == "cpp-result-propagation"
        )
        manifest = replace(manifest, families=(family,))
        for count in range(1, 4):
            with self.subTest(helper_count=count):
                operations = frozenset(family.cpp_operations[:count])
                report = USAGE.render_report(
                    manifest, {}, {"host-a": operations, "host-b": operations}
                )
                self.assertIn(f"independent hosts={2 if count == 3 else 0}", report)

    def test_projection_review_is_language_specific(self) -> None:
        manifest = USAGE.load_manifest(ROOT / "docs" / "compatibility-families.toml")
        family = replace(manifest.families[0], c_operations=(), projection_only=True)
        report = USAGE.render_report(
            replace(manifest, families=(family,)),
            {},
            {
                "cpp-host": frozenset(family.cpp_operations),
                "python-host": frozenset(family.python_operations),
            },
        )
        self.assertIn("independent hosts=2", report)

    def test_leaf_evidence_does_not_qualify_its_dependency_closure(self) -> None:
        manifest = USAGE.load_manifest(ROOT / "docs" / "compatibility-families.toml")
        leaves = ("external-identifiers", "resolution")
        operations = frozenset(
            operation
            for family in manifest.families
            if family.identifier in leaves
            for operation in family.c_operations
        )
        report = USAGE.render_report(
            manifest, {"host-a": operations, "host-b": operations}, {}, leaves
        )
        self.assertIn(
            "Required closure: `external-identifiers`, `production-lifecycle`, "
            "`resolution`, `transaction-lifecycle`",
            report,
        )
        self.assertIn(
            "Ineligible required families: `production-lifecycle`, `transaction-lifecycle`",
            report,
        )
        self.assertIn("| ABI | ABI | yes | no |", report)

    def test_dependency_closure_is_transitive_and_sorted(self) -> None:
        manifest = USAGE.load_manifest(ROOT / "docs" / "compatibility-families.toml")
        self.assertEqual(
            USAGE.dependency_closure(manifest, ("external-identifiers", "resolution")),
            USAGE.dependency_closure(manifest, ("resolution", "external-identifiers")),
        )
        with self.assertRaisesRegex(ValueError, "unknown family"):
            USAGE.dependency_closure(manifest, ("unknown",))

    def test_manifest_rejects_unknown_dependencies_and_cycles(self) -> None:
        source = (ROOT / "docs" / "compatibility-families.toml").read_text()
        for dependency, diagnostic in (
            ("unknown", "unknown family"),
            ("external-identifiers", "cycle"),
        ):
            with (
                self.subTest(dependency=dependency),
                tempfile.TemporaryDirectory() as directory,
            ):
                path = Path(directory) / "families.toml"
                path.write_text(
                    source.replace(
                        'id = "production-lifecycle"',
                        f'id = "production-lifecycle"\ndependencies = ["{dependency}"]',
                    )
                )
                with self.assertRaisesRegex(ValueError, diagnostic):
                    USAGE.load_manifest(path)


if __name__ == "__main__":
    unittest.main()
