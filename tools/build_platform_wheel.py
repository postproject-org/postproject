"""Build a platform wheel carrying the native library (ADR 0039).

The platform-neutral wheel built from ``python/`` is copied with the native
library added as ``postproject/_lib/<name>``, where the binding finds it when
no explicit path is given. ``WHEEL`` and ``RECORD`` are rewritten, and the file
is named with the platform tag:

    python tools/build_platform_wheel.py WHEEL LIBRARY PLATFORM_TAG --output-dir DIR

``PLATFORM_TAG`` is a Python platform compatibility tag such as
``manylinux_2_28_x86_64``, ``macosx_11_0_arm64``, or ``win_amd64``. The library
is stored under the name the binding expects on that platform, whatever its
file name.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import zipfile
from pathlib import Path

LIBRARY_NAMES = {
    "manylinux": "libpostproject.so",
    "linux": "libpostproject.so",
    "macosx": "libpostproject.dylib",
    "win": "postproject.dll",
}


def library_name(platform_tag: str) -> str:
    for prefix, name in LIBRARY_NAMES.items():
        if platform_tag.startswith(prefix):
            return name
    raise ValueError(f"unsupported platform tag: {platform_tag}")


def record_entry(name: str, data: bytes) -> str:
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=")
    return f"{name},sha256={digest.decode()},{len(data)}"


def build(wheel: Path, library: Path, platform_tag: str, output_dir: Path) -> Path:
    distribution, version, python_tag, abi_tag, neutral = wheel.stem.split("-")
    if (python_tag, abi_tag, neutral) != ("py3", "none", "any"):
        raise ValueError(f"not a platform-neutral wheel: {wheel.name}")
    dist_info = f"{distribution}-{version}.dist-info"
    target = output_dir / f"{distribution}-{version}-py3-none-{platform_tag}.whl"
    output_dir.mkdir(parents=True, exist_ok=True)
    records = []
    with (
        zipfile.ZipFile(wheel) as source,
        zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as out,
    ):
        for info in source.infolist():
            if info.filename == f"{dist_info}/RECORD":
                continue
            data = source.read(info)
            if info.filename == f"{dist_info}/WHEEL":
                data = rewrite_wheel_metadata(data.decode(), platform_tag).encode()
            out.writestr(info, data)
            records.append(record_entry(info.filename, data))
        name = f"postproject/_lib/{library_name(platform_tag)}"
        data = library.read_bytes()
        entry = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
        entry.external_attr = 0o755 << 16
        entry.compress_type = zipfile.ZIP_DEFLATED
        out.writestr(entry, data)
        records.append(record_entry(name, data))
        records.append(f"{dist_info}/RECORD,,")
        out.writestr(f"{dist_info}/RECORD", "\n".join(records) + "\n")
    return target


def rewrite_wheel_metadata(text: str, platform_tag: str) -> str:
    lines = []
    for line in text.splitlines():
        if line.startswith("Root-Is-Purelib:"):
            line = "Root-Is-Purelib: false"
        elif line.startswith("Tag:"):
            line = f"Tag: py3-none-{platform_tag}"
        lines.append(line)
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("wheel", type=Path)
    parser.add_argument("library", type=Path)
    parser.add_argument("platform_tag")
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    print(build(args.wheel, args.library, args.platform_tag, args.output_dir))


if __name__ == "__main__":
    main()
