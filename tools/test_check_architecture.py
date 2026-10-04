"""Mutations that must be rejected by the architecture gate."""

import unittest

from tools.check_architecture import dependency_violations, lint_violations


def package(name, *dependencies):
    return {"name": name, "dependencies": list(dependencies)}


def dependency(name, kind=None, **options):
    return {"name": name, "kind": kind, **options}


class ArchitectureTests(unittest.TestCase):
    def test_optional_renamed_target_dependency_cannot_reverse_direction(self):
        packages = [
            package(
                "postproject-media",
                dependency(
                    "postproject-storage-sqlite",
                    rename="backend",
                    optional=True,
                    target="cfg(windows)",
                ),
            ),
            package("postproject-storage-sqlite"),
        ]
        self.assertEqual(
            dependency_violations(packages),
            [
                "forbidden production dependency: postproject-media -> postproject-storage-sqlite"
            ],
        )

    def test_core_cannot_acquire_sqlite_or_host_framework(self):
        for name in ("rusqlite", "pyo3", "qt_core"):
            with self.subTest(name=name):
                self.assertEqual(
                    dependency_violations(
                        [package("postproject-core", dependency(name))]
                    ),
                    [f"unreviewed core dependency: {name}"],
                )

    def test_build_dependency_cannot_reach_adapter(self):
        packages = [
            package("postproject-ffi-macros", dependency("postproject-ffi", "build")),
            package("postproject-ffi"),
        ]
        self.assertTrue(dependency_violations(packages))

    def test_storage_tests_can_use_media(self):
        packages = [
            package(
                "postproject-storage-sqlite", dependency("postproject-media", "dev")
            ),
            package("postproject-media"),
        ]
        self.assertEqual(dependency_violations(packages), [])

    def test_new_crate_requires_architecture_review(self):
        self.assertEqual(
            dependency_violations([package("postproject-server")]),
            ["unreviewed workspace crate: postproject-server"],
        )

    def test_non_ffi_crate_cannot_relax_unsafe_prohibition(self):
        workspace = {"workspace": {"lints": {"rust": {"unsafe_code": "forbid"}}}}
        self.assertEqual(
            lint_violations(
                workspace,
                {"postproject-core": {"lints": {"rust": {"unsafe_code": "allow"}}}},
            ),
            ["postproject-core must inherit workspace lints"],
        )


if __name__ == "__main__":
    unittest.main()
