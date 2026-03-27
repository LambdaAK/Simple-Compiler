#!/usr/bin/env python3
"""Build the compiler, compile example.src to output.s, link with clang, run the binary."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "src",
        nargs="?",
        default="example.src",
        type=Path,
        help="Source file (default: example.src)",
    )
    p.add_argument(
        "-o",
        "--out-bin",
        type=Path,
        default=Path("example.run"),
        help="Linked executable name (default: example.run)",
    )
    args = p.parse_args()

    root = Path(__file__).resolve().parent
    src = args.src if args.src.is_absolute() else root / args.src
    out_asm = root / "output.s"
    out_bin = args.out_bin if args.out_bin.is_absolute() else root / args.out_bin

    if not src.is_file():
        print(f"error: no such file: {src}", file=sys.stderr)
        return 1

    subprocess.run(
        ["cargo", "run", "-q", "--", str(src)],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["clang", str(out_asm), "-o", str(out_bin)],
        cwd=root,
        check=True,
    )
    subprocess.run([str(out_bin)], cwd=root, check=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as e:
        raise SystemExit(e.returncode) from e
