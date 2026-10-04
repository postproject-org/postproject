from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "check_abi_layout", ROOT / "tools" / "check_abi_layout.py"
)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER
SPEC.loader.exec_module(CHECKER)


class CheckAbiLayoutTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        module = CHECKER.load_abi_module(
            ROOT / "python" / "src" / "postproject" / "_abi.py"
        )
        cls.expected = CHECKER.expected_layout(module)

    def test_matching_layout_is_accepted(self) -> None:
        lines = [f"{key}={value}\n" for key, value in self.expected.items()]
        actual = CHECKER.parse_layout(lines)
        CHECKER.verify_layout(self.expected, actual)

    def test_mismatch_is_rejected(self) -> None:
        actual = dict(self.expected)
        key = next(iter(actual))
        actual[key] += 1
        with self.assertRaisesRegex(CHECKER.LayoutError, "layout mismatch"):
            CHECKER.verify_layout(self.expected, actual)

    def test_missing_value_is_rejected(self) -> None:
        actual = dict(self.expected)
        actual.pop(next(iter(actual)))
        with self.assertRaisesRegex(CHECKER.LayoutError, "missing compiler value"):
            CHECKER.verify_layout(self.expected, actual)

    def test_alignment_mismatch_is_rejected(self) -> None:
        actual = dict(self.expected)
        key = next(key for key in actual if key.endswith(".alignment"))
        actual[key] *= 2
        with self.assertRaisesRegex(CHECKER.LayoutError, "layout mismatch"):
            CHECKER.verify_layout(self.expected, actual)

    def test_duplicate_value_is_rejected(self) -> None:
        with self.assertRaisesRegex(CHECKER.LayoutError, "invalid layout line"):
            CHECKER.parse_layout(["pp_uuid_t.size=16\n", "pp_uuid_t.size=16\n"])


if __name__ == "__main__":
    unittest.main()
