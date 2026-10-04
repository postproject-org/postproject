from __future__ import annotations

import importlib.util
import re
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "generate_abi", ROOT / "tools" / "generate_abi.py"
)
assert SPEC is not None and SPEC.loader is not None
GENERATOR = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GENERATOR
SPEC.loader.exec_module(GENERATOR)


class GenerateAbiTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.header_path = ROOT / "include" / "postproject" / "postproject.h"
        cls.header = GENERATOR.parse_header(cls.header_path.read_text(encoding="utf-8"))

    def test_symbol_output_matches_allowlist(self) -> None:
        expected = (ROOT / "tests" / "abi" / "expected-symbols.txt").read_text(
            encoding="utf-8"
        )
        self.assertEqual(GENERATOR.render_symbols(self.header), expected)

    def test_python_output_matches_committed_module(self) -> None:
        first = GENERATOR.render_python(self.header, str(self.header_path.relative_to(ROOT)))
        second = GENERATOR.render_python(self.header, str(self.header_path.relative_to(ROOT)))
        self.assertEqual(first, second)
        compile(first, "_abi.py", "exec")
        committed = (ROOT / "python" / "src" / "postproject" / "_abi.py").read_text(
            encoding="utf-8"
        )
        self.assertEqual(first, committed)

    def test_unsupported_declaration_fails_loudly(self) -> None:
        source = """
        typedef uint32_t pp_value_t;
        typedef union pp_choice { uint32_t value; } pp_choice_t;
        PP_API void pp_use_choice(pp_choice_t value);
        """
        with self.assertRaisesRegex(GENERATOR.HeaderError, "unsupported"):
            GENERATOR.parse_header(source)

    def test_layout_probe_covers_every_public_struct_field(self) -> None:
        probe = GENERATOR.render_layout_c(self.header)
        for struct in self.header.structs:
            if struct.fields is None:
                continue
            self.assertIn(f"sizeof({struct.alias})", probe)
            self.assertIn(f"_Alignof({struct.alias})", probe)
            for field in struct.fields:
                self.assertIn(f"offsetof({struct.alias}, {field.name})", probe)

    def test_rust_layout_probe_covers_same_fields_and_skips_opaque_handles(self) -> None:
        source = """
        typedef struct pp_uuid { uint8_t bytes[16]; } pp_uuid_t;
        typedef struct pp_production pp_production_t;
        PP_API void pp_use_uuid(pp_uuid_t value);
        """
        probe = GENERATOR.render_layout_rust(GENERATOR.parse_header(source))
        self.assertIn('layout!(PpUuid, "pp_uuid_t", bytes);', probe)
        self.assertNotIn("PpProduction", probe)

    def test_committed_rust_probe_matches_header(self) -> None:
        probe = GENERATOR.render_layout_rust(self.header)
        committed = (ROOT / "crates/postproject-ffi/examples/abi_layout.rs").read_text()
        # rustfmt wraps long macro invocations; compare the generated tokens.
        self.assertEqual(re.sub(r"\s+", "", probe), re.sub(r"\s+", "", committed))


if __name__ == "__main__":
    unittest.main()
