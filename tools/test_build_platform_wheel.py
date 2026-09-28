from __future__ import annotations

import base64
import hashlib
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import build_platform_wheel

DIST_INFO = "postproject-0.4.0a1.dist-info"


def neutral_wheel(directory: Path) -> Path:
    path = directory / "postproject-0.4.0a1-py3-none-any.whl"
    with zipfile.ZipFile(path, "w") as wheel:
        wheel.writestr("postproject/__init__.py", "")
        wheel.writestr(
            f"{DIST_INFO}/WHEEL",
            "Wheel-Version: 1.0\nGenerator: test\nRoot-Is-Purelib: true\n"
            "Tag: py3-none-any\n",
        )
        wheel.writestr(f"{DIST_INFO}/RECORD", "stale\n")
    return path


class BuildPlatformWheelTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.library = self.root / "anything.so"
        self.library.write_bytes(b"native library")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def build(self, tag: str) -> zipfile.ZipFile:
        wheel = build_platform_wheel.build(
            neutral_wheel(self.root), self.library, tag, self.root / "out"
        )
        self.assertEqual(wheel.name, f"postproject-0.4.0a1-py3-none-{tag}.whl")
        return zipfile.ZipFile(wheel)

    def test_adds_the_library_under_the_platform_name(self) -> None:
        for tag, name in (
            ("manylinux_2_28_x86_64", "libpostproject.so"),
            ("macosx_11_0_arm64", "libpostproject.dylib"),
            ("win_amd64", "postproject.dll"),
        ):
            with self.subTest(tag=tag), self.build(tag) as wheel:
                self.assertEqual(
                    wheel.read(f"postproject/_lib/{name}"), b"native library"
                )

    def test_tags_the_wheel_for_its_platform(self) -> None:
        with self.build("win_amd64") as wheel:
            metadata = wheel.read(f"{DIST_INFO}/WHEEL").decode()
        self.assertIn("Root-Is-Purelib: false\n", metadata)
        self.assertIn("Tag: py3-none-win_amd64\n", metadata)
        self.assertNotIn("py3-none-any", metadata)

    def test_records_every_file_with_its_digest(self) -> None:
        with self.build("manylinux_2_28_x86_64") as wheel:
            record = wheel.read(f"{DIST_INFO}/RECORD").decode().splitlines()
            names = {line.split(",")[0] for line in record}
            self.assertEqual(names, set(wheel.namelist()))
            for line in record:
                name, digest, size = line.split(",")
                if name.endswith("RECORD"):
                    self.assertEqual((digest, size), ("", ""))
                    continue
                data = wheel.read(name)
                expected = base64.urlsafe_b64encode(hashlib.sha256(data).digest())
                self.assertEqual(digest, "sha256=" + expected.rstrip(b"=").decode())
                self.assertEqual(int(size), len(data))

    def test_rejects_a_platform_wheel_as_input(self) -> None:
        wheel = self.root / "postproject-0.4.0a1-py3-none-win_amd64.whl"
        wheel.write_bytes(b"")
        with self.assertRaises(ValueError):
            build_platform_wheel.build(wheel, self.library, "win_amd64", self.root)


if __name__ == "__main__":
    unittest.main()
