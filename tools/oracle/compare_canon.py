#!/usr/bin/env python3
"""Compare canonical game-state checkpoints (docs/canonical-state.md) of a native run with the
emulator's, pairing them by checkpoint number and name (ckpt_NNNN_<why>), and print the
field-level differences from the .txt files. Exit 0 only if at least one pair was found, every
pair is identical, and (unless --allow-unpaired) no checkpoint on either side is left unpaired.

    tools/oracle/compare_canon.py NATIVE_DIR ORACLE_DIR [--allow-unpaired] [--also-bin]

The native run usually stops early (an unimplemented SDK call), so it has fewer checkpoints
than the oracle; pass --allow-unpaired to compare just the ones both have. --also-bin checks
that each pair's .bin files are byte-identical too (always checked now). Every record must be
well formed: the .txt has the schema's header line and exactly its fields and counts (from
docs/canonical-state.md), the .bin a valid DW1CANON header; anything else counts as a difference.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

NAME = re.compile(r"^ckpt_(\d{4})_(.+)\.txt$")
SIZES = {"u8": 1, "s8": 1, "u16": 2, "s16": 2, "u32": 4, "s32": 4}
DOC = Path(__file__).resolve().parents[2] / "docs" / "canonical-state.md"
VERSION = 3  # the schema docs/canonical-state.md describes


def schema() -> list[tuple[str, int, int]]:
    """(name, element size, count) of every field, from the layout table in docs/canonical-state.md."""
    rows = []
    for line in DOC.read_text().splitlines():
        m = re.match(r"^\| 0x[0-9A-F]{4} \| \d+ \| `([^`]+)` \| (\w+)(?:\[(\d+)\])? \|", line)
        if m:
            rows.append((m.group(1), SIZES[m.group(2)], int(m.group(3) or 1)))
    if not rows:
        raise SystemExit(f"{DOC}: no layout table found")
    return rows


FIELDS = schema()
BODY_LEN = sum(size * count for _, size, count in FIELDS)


def check_bin(path: Path) -> str | None:
    """Why the .bin is not a valid record, or None."""
    data = path.read_bytes()
    if len(data) < 16 or data[:8] != b"DW1CANON":
        return "bad or missing DW1CANON header"
    version = int.from_bytes(data[8:12], "little")
    body = int.from_bytes(data[12:16], "little")
    if version != VERSION:
        return f"schema version {version}, expected {VERSION}"
    if body != BODY_LEN or len(data) != 16 + body:
        return f"body length {body}, file {len(data)} bytes, expected body {BODY_LEN}"
    return None


def checkpoints(directory: Path) -> dict[str, Path]:
    found = {}
    for path in sorted(directory.iterdir()):
        m = NAME.match(path.name)
        if m:
            found[f"{m.group(1)}_{m.group(2)}"] = path
    return found


def parse(path: Path) -> dict[str, str]:
    """The .txt's fields; raises ValueError unless it has exactly the schema's fields."""
    lines = path.read_text().splitlines()
    if not lines or lines[0] != f"# dw1 canonical state schema v{VERSION}":
        raise ValueError("missing or wrong schema header line")
    fields = {}
    for line in lines[1:]:
        if not line.strip():
            continue
        name, sep, value = line.partition(" = ")
        if not sep:
            raise ValueError(f"malformed line: {line!r}")
        if name in fields:
            raise ValueError(f"duplicate field {name}")
        fields[name] = value
    want = [n for n, _, _ in FIELDS]
    if list(fields) != want:
        missing = [n for n in want if n not in fields]
        extra = [n for n in fields if n not in want]
        raise ValueError(f"fields differ from the schema: missing {missing[:3]}, extra {extra[:3]}"
                         if missing or extra else "fields out of order")
    for name, _, count in FIELDS:
        if len(fields[name].split()) != count:
            raise ValueError(f"{name}: {len(fields[name].split())} values, expected {count}")
    return fields


def diff_pair(native: Path, oracle: Path, also_bin: bool) -> list[str]:
    out = []
    for side, path in (("native", native), ("oracle", oracle)):
        try:
            parse(path)
        except ValueError as e:
            out.append(f"  invalid {side} {path.name}: {e}")
        bin_path = path.with_suffix(".bin")
        if not bin_path.exists():
            out.append(f"  {side} {bin_path.name} is missing")
        elif (why := check_bin(bin_path)) is not None:
            out.append(f"  invalid {side} {bin_path.name}: {why}")
    if out:
        return out
    a, b = parse(native), parse(oracle)
    for name in a.keys() | b.keys():
        if name not in a:
            out.append(f"  {name}: only in the oracle: {b[name]}")
        elif name not in b:
            out.append(f"  {name}: only in the native run: {a[name]}")
        elif a[name] != b[name]:
            va, vb = a[name].split(), b[name].split()
            if len(va) > 1 and len(va) == len(vb):
                idx = [i for i in range(len(va)) if va[i] != vb[i]]
                shown = ", ".join(f"[{i}] native {va[i]} oracle {vb[i]}" for i in idx[:8])
                more = f" (+{len(idx) - 8} more)" if len(idx) > 8 else ""
                out.append(f"  {name}: {len(idx)} of {len(va)} differ: {shown}{more}")
            else:
                out.append(f"  {name}: native {a[name]} oracle {b[name]}")
    if not out:
        if native.with_suffix(".bin").read_bytes() != oracle.with_suffix(".bin").read_bytes():
            out.append("  .txt identical but .bin files differ")
    order = {n: i for i, n in enumerate(list(b) + list(a))}
    return sorted(out, key=lambda s: order.get(s.split(":")[0].strip(), 0))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("native", type=Path)
    ap.add_argument("oracle", type=Path)
    ap.add_argument("--allow-unpaired", action="store_true")
    ap.add_argument("--also-bin", action="store_true", help="(default now; kept for compatibility)")
    args = ap.parse_args()
    nat, ora = checkpoints(args.native), checkpoints(args.oracle)
    paired = sorted(nat.keys() & ora.keys())
    bad = 0
    for key in paired:
        diffs = diff_pair(nat[key], ora[key], args.also_bin)
        if diffs:
            bad += 1
            print(f"ckpt_{key}: {len(diffs)} field(s) differ")
            print("\n".join(diffs))
        else:
            print(f"ckpt_{key}: identical")
    only_native = sorted(nat.keys() - ora.keys())
    only_oracle = sorted(ora.keys() - nat.keys())
    for key in only_native:
        print(f"ckpt_{key}: only in the native run")
    for key in only_oracle:
        print(f"ckpt_{key}: only in the oracle")
    unpaired = bool(only_native or only_oracle) and not args.allow_unpaired
    print(f"{len(paired)} paired, {len(paired) - bad} identical, {bad} differ, "
          f"{len(only_native)} only native, {len(only_oracle)} only oracle")
    return 0 if paired and not bad and not unpaired else 1


if __name__ == "__main__":
    sys.exit(main())
