from __future__ import annotations

import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import flatpak_module

LOCKFILE = """
version = 4

[[package]]
name = "blake3"
version = "1.8.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "aaaa"

[[package]]
name = "postproject-core"
version = "0.4.0"
"""


def crate(name: str, checksum: str) -> list[dict[str, str]]:
    destination = f"cargo/vendor/{name}"
    return [
        {"type": "archive", "sha256": checksum, "dest": destination},
        {
            "type": "inline",
            "contents": json.dumps({"package": checksum, "files": {}}),
            "dest": destination,
            "dest-filename": ".cargo-checksum.json",
        },
    ]


class CheckTest(unittest.TestCase):
    def test_committed_sources_match_the_lockfile(self) -> None:
        problems = flatpak_module.check(
            (flatpak_module.ROOT / "Cargo.lock").read_text(encoding="utf-8"),
            json.loads(flatpak_module.CARGO_SOURCES.read_text(encoding="utf-8")),
        )
        self.assertEqual(problems, [])

    def test_reports_missing_unused_and_changed_crates(self) -> None:
        self.assertEqual(
            flatpak_module.check(LOCKFILE, crate("blake3-1.8.2", "aaaa")), []
        )
        self.assertEqual(
            flatpak_module.check(LOCKFILE, crate("blake3-1.8.2", "bbbb")),
            ["checksum differs for cargo/vendor/blake3-1.8.2"],
        )
        self.assertEqual(
            flatpak_module.check(LOCKFILE, crate("blake3-1.8.1", "aaaa")),
            ["missing cargo/vendor/blake3-1.8.2", "unused cargo/vendor/blake3-1.8.1"],
        )


class RenderTest(unittest.TestCase):
    def test_published_archive(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = flatpak_module.render("1.2.3", "abcd", None, Path(directory))
            module = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(path.name, "postproject-1.2.3-flatpak.json")
            self.assertEqual(
                module["sources"],
                [
                    {
                        "type": "archive",
                        "url": "https://github.com/postproject-org/postproject/releases/download/v1.2.3/postproject-1.2.3-source.tar.gz",
                        "sha256": "abcd",
                    },
                    "postproject-1.2.3-cargo-sources.json",
                ],
            )
            self.assertTrue(
                (Path(directory) / "postproject-1.2.3-cargo-sources.json").is_file()
            )

    def test_local_archive(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "source.tar.gz"
            archive.write_bytes(b"archive")
            path = flatpak_module.render("1.2.3", None, archive, Path(directory))
            self.assertEqual(path.name, "org.postproject.FlatpakCheck.json")
            app = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(app["modules"][0], "postproject-1.2.3-flatpak.json")
            module = json.loads(
                (Path(directory) / app["modules"][0]).read_text(encoding="utf-8")
            )
            source = module["sources"][0]
            self.assertEqual(source["path"], str(archive.resolve()))
            self.assertEqual(source["sha256"], hashlib.sha256(b"archive").hexdigest())
            self.assertEqual(app["modules"][1]["sources"], [source])


if __name__ == "__main__":
    unittest.main()
