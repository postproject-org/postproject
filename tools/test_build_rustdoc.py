"""Checks for the self-contained Rust reference build."""

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.build_rustdoc import build


class BuildTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="rustdoc test ")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        assets = self.root / "docs" / "_static"
        (assets / "fonts").mkdir(parents=True)
        (assets / "theme.css").write_text("/* shared palette */", encoding="utf-8")
        (assets / "fonts" / "InterVariable.woff2").write_bytes(b"font")
        (assets / "fonts" / "LICENSE.txt").write_text("license", encoding="utf-8")
        (self.root / "target" / "doc").mkdir(parents=True)

    def test_preserves_flags_and_copies_local_assets(self):
        with (
            patch.dict(os.environ, {"RUSTDOCFLAGS": "-D warnings"}, clear=True),
            patch("tools.build_rustdoc.subprocess.run") as run,
        ):
            build(self.root)
        flags = run.call_args.kwargs["env"]["CARGO_ENCODED_RUSTDOCFLAGS"].split("\x1f")
        self.assertEqual(flags[:3], ["-D", "warnings", "--extend-css"])
        self.assertEqual(flags[3], str(self.root / "docs" / "rustdoc.css"))
        self.assertTrue(run.call_args.kwargs["check"])
        output = self.root / "target" / "doc"
        self.assertEqual(
            (output / "postproject-theme.css").read_text(), "/* shared palette */"
        )
        self.assertEqual(
            (output / "fonts" / "InterVariable.woff2").read_bytes(), b"font"
        )
        self.assertEqual((output / "fonts" / "LICENSE.txt").read_text(), "license")

    def test_encoded_flags_take_precedence(self):
        env = {
            "RUSTDOCFLAGS": "ignored",
            "CARGO_ENCODED_RUSTDOCFLAGS": "-D\x1fwarnings",
        }
        with (
            patch.dict(os.environ, env, clear=True),
            patch("tools.build_rustdoc.subprocess.run") as run,
        ):
            build(self.root)
        flags = run.call_args.kwargs["env"]["CARGO_ENCODED_RUSTDOCFLAGS"].split("\x1f")
        self.assertEqual(flags[:2], ["-D", "warnings"])

    def test_empty_encoded_flags_still_take_precedence(self):
        env = {"RUSTDOCFLAGS": "ignored", "CARGO_ENCODED_RUSTDOCFLAGS": ""}
        with (
            patch.dict(os.environ, env, clear=True),
            patch("tools.build_rustdoc.subprocess.run") as run,
        ):
            build(self.root)
        flags = run.call_args.kwargs["env"]["CARGO_ENCODED_RUSTDOCFLAGS"].split("\x1f")
        self.assertEqual(flags[0], "--extend-css")


if __name__ == "__main__":
    unittest.main()
