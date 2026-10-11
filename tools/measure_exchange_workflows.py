"""Measure identical local workflows and isolated streaming phases on Linux.

Build the two declared benchmark programs with the same compiler/profile. Copy
native_workflows.rs and its fixture verbatim into an isolated released source
tree for --baseline-bin. Output must be a new directory. RSS uses wait4 per child;
the launcher can impose an RSS floor. Results describe this machine and fixture.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
from pathlib import Path


def run(binary: Path, args: list[str], output: Path) -> dict[str, int]:
    with output.open("x") as sink:
        pid = os.posix_spawn(
            str(binary),
            [str(binary), *args],
            os.environ,
            file_actions=[(os.POSIX_SPAWN_DUP2, sink.fileno(), 1)],
        )
        _, status, usage = os.wait4(pid, 0)
    if os.waitstatus_to_exitcode(status) != 0:
        raise RuntimeError(f"benchmark failed: {binary.name} {args[0]}")
    values = {
        key: int(value)
        for key, value in (line.split("\t") for line in output.read_text().splitlines())
    }
    return {**values, "peak_rss_kib": usage.ru_maxrss}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline-bin", "native-bin", "exchange-bin", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=3)
    args = parser.parse_args()
    if args.samples < 1 or platform.system() != "Linux":
        parser.error("positive samples and Linux wait4 RSS units are required")
    root: Path = args.output
    root.mkdir()
    binaries = {
        "released_0_7": args.baseline_bin.resolve(),
        "development": args.native_bin.resolve(),
        "exchange": args.exchange_bin.resolve(),
    }
    results: dict = {
        "compiler": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "platform": platform.platform(),
        "binaries": {
            name: {
                "path": str(binary),
                "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            }
            for name, binary in binaries.items()
        },
        "samples": [],
    }

    def measure(sample: int | str, phase: str, binary: Path, arguments: list[str]) -> None:
        output = root / f"{sample}-{phase}.tsv"
        values = run(binary, arguments, output)
        results["samples"].append({"sample": sample, "phase": phase, **values})
        (root / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        print(sample, phase, values, flush=True)

    for sample in range(args.samples):
        for version in ("released_0_7", "development"):
            directory = root / f"{version}-{sample}"
            directory.mkdir()
            store = directory / "production.pproj"
            for phase in ("create", "read", "append"):
                measure(
                    sample,
                    f"{version}-{phase}",
                    binaries[version],
                    [phase, str(store)],
                )
    for sample in range(args.samples):
        directory = root / f"scale-{sample}"
        directory.mkdir()
        source = directory / "authority.pproj"
        checkpoint = directory / "checkpoint.ppxc"
        mirror = directory / "mirror.pproj"
        changes = directory / "changes.ppxd"
        measure(sample, "create", binaries["development"], ["create", str(source)])
        for phase, input_path, output_path in (
            ("checkpoint", source, checkpoint),
            ("import", checkpoint, mirror),
            ("append", source, None),
            ("changes", source, changes),
            ("apply", mirror, changes),
            ("duplicate", mirror, changes),
        ):
            if output_path is None:
                measure(
                    sample, phase, binaries["development"], [phase, str(input_path)]
                )
            else:
                measure(
                    sample,
                    phase,
                    binaries["exchange"],
                    [
                        "apply" if phase == "duplicate" else phase,
                        str(input_path),
                        str(output_path),
                    ],
                )
    directory = root / "large"
    directory.mkdir()
    source = directory / "authority.pproj"
    checkpoint = directory / "checkpoint.ppxc"
    mirror = directory / "mirror.pproj"
    for phase, input_path, output_path in (
        ("large", source, directory / "unused"),
        ("checkpoint", source, checkpoint),
        ("import", checkpoint, mirror),
        ("verify-large", mirror, directory / "unused"),
    ):
        measure(
            "large",
            phase,
            binaries["exchange"],
            [phase, str(input_path), str(output_path)],
        )
    measure(
        "outcomes",
        "growth",
        binaries["exchange"],
        ["outcomes", str(root / "outcomes.pproj"), str(root / "unused")],
    )


if __name__ == "__main__":
    main()
