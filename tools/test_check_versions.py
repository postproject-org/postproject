"""Release-documentation regressions for package and Python version checks."""

import tempfile
import unittest
from pathlib import Path

from tools.check_versions import documented_package_errors


class DocumentedPackagesTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.guide = self.root / "docs/src/integrators/installing-a-release.md"
        self.guide.parent.mkdir(parents=True)

    def errors(self, version="0.7.0-alpha.1", python_version="0.7.0a1"):
        return documented_package_errors(self.root, version, python_version)

    def test_current_archives_wheels_and_directories(self):
        self.guide.write_text(
            "postproject-0.7.0-alpha.1-linux-x86_64.tar.gz.sha256\n"
            "postproject-0.7.0-alpha.1-windows-x86_64.zip\n"
            "postproject-0.7.0-alpha.1-flatpak.json\n"
            "postproject-0.7.0-alpha.1/\n"
            "postproject-0.7.0a1-py3-none-any.whl\n"
            "postproject-0.7.0a1-py3-none-manylinux_2_28_x86_64.whl.sha256\n"
            "postproject-0.7.0a1-py3-none-macosx_11_0_arm64.whl\n"
            "postproject-0.7.0a1-py3-none-win_amd64.whl\n"
        )
        self.assertEqual(self.errors(), [])

    def test_old_wheel_reports_the_guide_and_line(self):
        self.guide.write_text("Install:\npostproject-0.5.0a1-py3-none-any.whl\n")
        self.assertEqual(
            self.errors(),
            [
                (
                    "docs/src/integrators/installing-a-release.md:2: "
                    "postproject-0.5.0a1-py3-none-any.whl must use version 0.7.0a1"
                )
            ],
        )

    def test_future_release_rejects_previous_examples(self):
        self.guide.write_text(
            "postproject-0.7.0-alpha.1-source.tar.gz\n"
            "postproject-0.7.0a1-py3-none-any.whl\n"
        )
        errors = self.errors("0.8.0-alpha.1", "0.8.0a1")
        self.assertEqual(len(errors), 2)
        self.assertIn("must use version 0.8.0-alpha.1", errors[0])
        self.assertIn("must use version 0.8.0a1", errors[1])

    def test_historical_reports_and_version_history_remain_valid(self):
        (self.root / "docs/release-0.5-report.md").write_text(
            "postproject-0.5.0a1-py3-none-any.whl"
        )
        self.guide.write_text("Each release since `0.4.0-alpha.1` has packages.\n")
        self.assertEqual(self.errors(), [])

    def test_readme_is_checked(self):
        (self.root / "README.md").write_text("postproject-0.5.0-alpha.1/")
        self.assertTrue(self.errors()[0].startswith("README.md:1:"))

    def test_wheel_requires_python_version_spelling(self):
        self.guide.write_text("postproject-0.7.0-alpha.1-py3-none-any.whl")
        self.assertIn("must use version 0.7.0a1", self.errors()[0])

    def test_beta_rc_and_stable_release_filenames(self):
        for package, python in [
            ("0.8.0-beta.2", "0.8.0b2"),
            ("0.8.0-rc.3", "0.8.0rc3"),
            ("1.0.0", "1.0.0"),
        ]:
            with self.subTest(package=package):
                self.guide.write_text(
                    f"postproject-{package}-source.tar.gz\n"
                    f"postproject-{python}-py3-none-any.whl\n"
                )
                self.assertEqual(self.errors(package, python), [])


if __name__ == "__main__":
    unittest.main()
