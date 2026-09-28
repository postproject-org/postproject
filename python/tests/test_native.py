from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from postproject import _native


class LibraryLocationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.bundled = self.root / "_lib" / "libpostproject.so"
        self.explicit = self.root / "explicit.so"
        self.variable = self.root / "variable.so"
        for path in (self.bundled, self.explicit, self.variable):
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(b"")
        patch = mock.patch.object(_native, "BUNDLED_LIBRARY", self.bundled)
        patch.start()
        self.addCleanup(patch.stop)
        environment = mock.patch.dict(os.environ)
        environment.start()
        self.addCleanup(environment.stop)
        os.environ.pop(_native.LIBRARY_ENVIRONMENT_VARIABLE, None)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_an_explicit_path_wins(self) -> None:
        os.environ[_native.LIBRARY_ENVIRONMENT_VARIABLE] = str(self.variable)
        self.assertEqual(_native._library_path(self.explicit), self.explicit.resolve())

    def test_the_variable_overrides_the_bundled_library(self) -> None:
        os.environ[_native.LIBRARY_ENVIRONMENT_VARIABLE] = str(self.variable)
        self.assertEqual(_native._library_path(None), self.variable.resolve())

    def test_the_bundled_library_is_the_last_choice(self) -> None:
        self.assertEqual(_native._library_path(None), self.bundled.resolve())

    def test_without_any_library_loading_fails(self) -> None:
        self.bundled.unlink()
        with self.assertRaisesRegex(RuntimeError, "platform wheel"):
            _native._library_path(None)


if __name__ == "__main__":
    unittest.main()
