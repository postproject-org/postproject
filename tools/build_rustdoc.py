#!/usr/bin/env python3
"""Build the Rust reference with the site's local font and adaptive palette."""

import os
import shlex
import shutil
import subprocess
from pathlib import Path


def build(root: Path) -> None:
    """Generate themed documentation and place assets beside rustdoc's theme.css."""
    assets = root / "docs" / "_static"
    env = os.environ.copy()
    encoded = env.get("CARGO_ENCODED_RUSTDOCFLAGS")
    flags = encoded.split("\x1f") if encoded else shlex.split(env.get("RUSTDOCFLAGS", ""))
    flags.extend(["--extend-css", str(assets / "rustdoc.css")])
    env["CARGO_ENCODED_RUSTDOCFLAGS"] = "\x1f".join(flags)
    target = root / "target"
    subprocess.run(
        [
            "cargo", "doc", "--workspace", "--all-features", "--no-deps",
            "--locked", "--target-dir", str(target),
        ],
        cwd=root,
        env=env,
        check=True,
    )
    output = target / "doc"
    shutil.copyfile(assets / "theme.css", output / "postproject-theme.css")
    shutil.copytree(assets / "fonts", output / "fonts", dirs_exist_ok=True)


if __name__ == "__main__":
    build(Path(__file__).resolve().parents[1])
