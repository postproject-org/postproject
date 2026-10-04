#!/usr/bin/env python3
"""Generate ctypes declarations or an exported-symbol list from the C header."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


PRIMITIVES = {
    "char": "ctypes.c_char",
    "int8_t": "ctypes.c_int8",
    "uint8_t": "ctypes.c_uint8",
    "int16_t": "ctypes.c_int16",
    "uint16_t": "ctypes.c_uint16",
    "int32_t": "ctypes.c_int32",
    "uint32_t": "ctypes.c_uint32",
    "int64_t": "ctypes.c_int64",
    "uint64_t": "ctypes.c_uint64",
}


class HeaderError(ValueError):
    """The public header contains a declaration this generator cannot model."""


@dataclass(frozen=True)
class CType:
    base: str
    pointers: int = 0
    array_length: int | None = None


@dataclass(frozen=True)
class Field:
    name: str
    ctype: CType


@dataclass(frozen=True)
class Struct:
    tag: str
    alias: str
    fields: tuple[Field, ...] | None


@dataclass(frozen=True)
class Alias:
    name: str
    ctype: CType


@dataclass(frozen=True)
class Function:
    name: str
    result: CType
    parameters: tuple[CType, ...]


@dataclass(frozen=True)
class Header:
    structs: tuple[Struct, ...]
    aliases: tuple[Alias, ...]
    constants: tuple[tuple[str, int], ...]
    functions: tuple[Function, ...]


def parse_header(text: str) -> Header:
    constants = _parse_constants(text)
    source = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    source = re.sub(r"^[ \t]*#.*$", "", source, flags=re.MULTILINE)
    source = re.sub(r'extern\s+"C"\s*\{', "", source)
    source = re.sub(r"^\s*}\s*$", "", source, flags=re.MULTILINE)

    structs: list[Struct] = []
    aliases: list[Alias] = []
    functions: list[Function] = []
    for declaration in _declarations(source):
        struct_match = re.fullmatch(
            r"typedef\s+struct\s+(\w+)\s*\{(.*)}\s*(\w+)",
            declaration,
            flags=re.DOTALL,
        )
        if struct_match:
            tag, body, alias = struct_match.groups()
            fields = tuple(_parse_field(field) for field in body.split(";") if field.strip())
            structs.append(Struct(tag, alias, fields))
            continue

        opaque_match = re.fullmatch(r"typedef\s+struct\s+(\w+)\s+(\w+)", declaration)
        if opaque_match:
            structs.append(Struct(*opaque_match.groups(), fields=None))
            continue

        alias_match = re.fullmatch(r"typedef\s+(.+?)\s+(\w+)", declaration)
        if alias_match:
            source_type, name = alias_match.groups()
            aliases.append(Alias(name, _parse_type(source_type)))
            continue

        function_match = re.fullmatch(
            r"PP_API\s+(.+?[*\s])(pp_\w+)\s*\((.*)\)",
            declaration,
            flags=re.DOTALL,
        )
        if function_match:
            result, name, parameters = function_match.groups()
            functions.append(
                Function(
                    name,
                    _parse_type(result),
                    _parse_parameters(parameters),
                )
            )
            continue

        raise HeaderError(f"unsupported declaration: {_one_line(declaration)}")

    if not functions:
        raise HeaderError("header declares no PP_API functions")
    _validate_types(structs, aliases, functions)
    return Header(tuple(structs), tuple(aliases), constants, tuple(functions))


def _parse_constants(text: str) -> tuple[tuple[str, int], ...]:
    constants: list[tuple[str, int]] = []
    for match in re.finditer(
        r"^\s*#define\s+(PP_[A-Z0-9_]+)\s+U?INT\d+_C\(([^)]+)\)\s*$",
        text,
        flags=re.MULTILINE,
    ):
        name, value = match.groups()
        try:
            constants.append((name, int(value, 0)))
        except ValueError as error:
            raise HeaderError(f"unsupported integer constant {name}: {value}") from error
    return tuple(constants)


def _declarations(source: str) -> tuple[str, ...]:
    declarations: list[str] = []
    start = 0
    depth = 0
    for index, character in enumerate(source):
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth < 0:
                raise HeaderError("unmatched closing brace")
        elif character == ";" and depth == 0:
            declaration = source[start:index].strip()
            if declaration:
                declarations.append(declaration)
            start = index + 1
    if depth:
        raise HeaderError("unclosed brace")
    remainder = source[start:].strip()
    if remainder:
        raise HeaderError(f"unterminated declaration: {_one_line(remainder)}")
    return tuple(declarations)


def _parse_field(declaration: str) -> Field:
    name, ctype = _parse_declarator(declaration)
    return Field(name, ctype)


def _parse_parameters(parameters: str) -> tuple[CType, ...]:
    if parameters.strip() == "void":
        return ()
    parsed: list[CType] = []
    for parameter in parameters.split(","):
        if "..." in parameter or "(" in parameter or ")" in parameter:
            raise HeaderError(f"unsupported parameter: {_one_line(parameter)}")
        _, ctype = _parse_declarator(parameter)
        parsed.append(ctype)
    return tuple(parsed)


def _parse_declarator(declaration: str) -> tuple[str, CType]:
    match = re.fullmatch(
        r"\s*(.+?[*\s])([A-Za-z_]\w*)\s*(?:\[\s*(\d+)\s*])?\s*",
        declaration,
        flags=re.DOTALL,
    )
    if not match:
        raise HeaderError(f"unsupported declarator: {_one_line(declaration)}")
    source_type, name, array_length = match.groups()
    ctype = _parse_type(source_type)
    if array_length is not None:
        if ctype.pointers:
            raise HeaderError(f"pointer arrays are unsupported: {_one_line(declaration)}")
        ctype = CType(ctype.base, array_length=int(array_length))
    return name, ctype


def _parse_type(source_type: str) -> CType:
    if "[" in source_type or "]" in source_type:
        raise HeaderError(f"unsupported type: {_one_line(source_type)}")
    pointers = source_type.count("*")
    base = source_type.replace("*", " ")
    tokens = [token for token in base.split() if token != "const"]
    if len(tokens) != 1:
        raise HeaderError(f"unsupported type: {_one_line(source_type)}")
    return CType(tokens[0], pointers)


def _validate_types(
    structs: list[Struct], aliases: list[Alias], functions: list[Function]
) -> None:
    known = set(PRIMITIVES) | {"void"}
    known.update(struct.alias for struct in structs)
    known.update(alias.name for alias in aliases)
    types = [alias.ctype for alias in aliases]
    types.extend(field.ctype for struct in structs if struct.fields for field in struct.fields)
    for function in functions:
        types.append(function.result)
        types.extend(function.parameters)
    for ctype in types:
        if ctype.base not in known:
            raise HeaderError(f"unknown C type: {ctype.base}")
        if ctype.base == "void" and ctype.pointers > 1:
            raise HeaderError("only void and void pointers are supported")


def render_python(header: Header, source: str) -> str:
    lines = [
        f'"""Generated from {source}; do not edit manually."""',
        "",
        "from __future__ import annotations",
        "",
        "import ctypes",
        "",
        "",
    ]
    for struct in header.structs:
        lines.extend(
            [
                f"class {_python_name(struct.alias)}(ctypes.Structure):",
                "    pass",
                "",
                "",
            ]
        )
    for alias in header.aliases:
        lines.append(f"{_python_name(alias.name)} = {_render_ctype(alias.ctype)}")
    if header.aliases:
        lines.extend(["", ""])
    for name, value in header.constants:
        lines.append(f"{name} = {value}")
    if header.constants:
        lines.extend(["", ""])
    for struct in header.structs:
        if struct.fields is None:
            continue
        lines.append(f"{_python_name(struct.alias)}._fields_ = [")
        for field in struct.fields:
            lines.append(f'    ("{field.name}", {_render_ctype(field.ctype)}),')
        lines.extend(["]", ""])
    lines.extend(["", "PUBLIC_STRUCTS = {"])
    for struct in header.structs:
        if struct.fields is None:
            continue
        field_names = ", ".join(f'"{field.name}"' for field in struct.fields)
        if len(struct.fields) == 1:
            field_names += ","
        lines.append(
            f'    "{struct.alias}": ({_python_name(struct.alias)}, ({field_names})),'
        )
    lines.extend(
        [
            "}",
            "",
            "",
            "EXPORTED_SYMBOLS = (",
            *(f'    "{function.name}",' for function in sorted(header.functions, key=lambda item: item.name)),
            ")",
            "",
            "",
            "def configure_api(lib: ctypes.CDLL) -> None:",
            '    """Configure every function declared by the public C header."""',
            "",
        ]
    )
    for function in header.functions:
        arguments = ", ".join(_render_ctype(parameter) for parameter in function.parameters)
        lines.append(f"    lib.{function.name}.argtypes = [{arguments}]")
        lines.append(f"    lib.{function.name}.restype = {_render_ctype(function.result)}")
    return "\n".join(lines) + "\n"


def render_symbols(header: Header) -> str:
    return "".join(f"{function.name}\n" for function in sorted(header.functions, key=lambda item: item.name))


def render_layout_c(header: Header) -> str:
    lines = [
        "/* Generated layout probe; do not edit manually. */",
        "#include <stddef.h>",
        "#include <stdio.h>",
        "#include <postproject/postproject.h>",
        "",
        "int main(void) {",
    ]
    for struct in header.structs:
        if struct.fields is None:
            continue
        lines.append(
            f'  printf("{struct.alias}.size=%zu\\n", sizeof({struct.alias}));'
        )
        lines.append(
            f'  printf("{struct.alias}.alignment=%zu\\n", _Alignof({struct.alias}));'
        )
        for field in struct.fields:
            lines.append(
                f'  printf("{struct.alias}.{field.name}=%zu\\n", '
                f"offsetof({struct.alias}, {field.name}));"
            )
    lines.extend(["  return 0;", "}"])
    return "\n".join(lines) + "\n"


def render_layout_rust(header: Header) -> str:
    """Probe the implementation's types using the authoritative header fields."""
    types = [
        "".join(part.title() for part in struct.tag.split("_"))
        for struct in header.structs
        if struct.fields is not None
    ]
    lines = [
        "//! Generated implementation layout probe; do not edit manually.",
        "use postproject::{" + ", ".join(sorted(types)) + ",};",
        "use std::mem::{align_of, offset_of, size_of};",
        "",
        "macro_rules! layout {",
        "    ($ty:ty, $name:literal, $($field:ident),+) => {{",
        '        println!(concat!($name, ".size={}"), size_of::<$ty>());',
        '        println!(concat!($name, ".alignment={}"), align_of::<$ty>());',
        '        $(println!(concat!($name, ".", stringify!($field), "={}"),',
        "                   offset_of!($ty, $field));)+",
        "    }};",
        "}",
        "",
        "// One generated invocation per public struct keeps the probe auditable.",
        "#[allow(clippy::too_many_lines)]",
        "fn main() {",
    ]
    for struct in header.structs:
        if struct.fields is None:
            continue
        rust_type = "".join(part.title() for part in struct.tag.split("_"))
        fields = ", ".join(field.name for field in struct.fields)
        lines.append(f'    layout!({rust_type}, "{struct.alias}", {fields});')
    lines.extend(["}", ""])
    return "\n".join(lines)


def _render_ctype(ctype: CType) -> str:
    if ctype.base == "void":
        expression = "None" if ctype.pointers == 0 else "ctypes.c_void_p"
        pointers = max(0, ctype.pointers - 1)
    elif ctype.base == "char" and ctype.pointers:
        expression = "ctypes.c_char_p"
        pointers = ctype.pointers - 1
    else:
        expression = PRIMITIVES.get(ctype.base, _python_name(ctype.base))
        pointers = ctype.pointers
    for _ in range(pointers):
        expression = f"ctypes.POINTER({expression})"
    if ctype.array_length is not None:
        expression = f"{expression} * {ctype.array_length}"
    return expression


def _python_name(c_name: str) -> str:
    name = c_name.removeprefix("pp_").removesuffix("_t")
    return "".join(part.capitalize() for part in name.split("_"))


def _one_line(value: str) -> str:
    return " ".join(value.split())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("header", type=Path)
    output = parser.add_mutually_exclusive_group()
    output.add_argument("--symbols", action="store_true")
    output.add_argument("--layout-c", action="store_true")
    output.add_argument("--layout-rust", action="store_true")
    arguments = parser.parse_args()
    try:
        header = parse_header(arguments.header.read_text(encoding="utf-8"))
    except (OSError, HeaderError) as error:
        parser.error(str(error))
    if arguments.symbols:
        rendered = render_symbols(header)
    elif arguments.layout_c:
        rendered = render_layout_c(header)
    elif arguments.layout_rust:
        rendered = render_layout_rust(header)
    else:
        rendered = render_python(header, str(arguments.header))
    sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
