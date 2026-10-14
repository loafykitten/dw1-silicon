#!/usr/bin/env python3
"""Compare canonical game-state checkpoints (docs/canonical-state.md) of a native run with the
emulator's, pairing them by checkpoint number and name (ckpt_NNNN_<why>), and print the
field-level differences from the .txt files. Exit 0 only if at least one pair was found, every
pair is identical, and (unless --allow-unpaired) no checkpoint on either side is left unpaired.

    tools/oracle/compare_canon.py NATIVE_DIR ORACLE_DIR [--allow-unpaired] [--also-bin]

The native run usually stops early (an unimplemented SDK call), so it has fewer checkpoints
than the oracle; pass --allow-unpaired to compare just the ones both have. --also-bin checks
that each pair's .bin files are byte-identical too.
"""
import argparse
import re
import sys
from pathlib import Path

NAME = re.compile(r"^ckpt_(\d{4})_(.+)\.txt$")


def checkpoints(directory: Path) -> dict[str, Path]:
    found = {}
    for path in sorted(directory.iterdir()):
        m = NAME.match(path.name)
        if m:
            found[f"{m.group(1)}_{m.group(2)}"] = path
    return found


def parse(path: Path) -> dict[str, str]:
    fields = {}
    for line in path.read_text().splitlines():
        if line.startswith("#") or not line.strip():
            continue
        name, sep, value = line.partition(" = ")
        if not sep:
            raise SystemExit(f"{path}: malformed line: {line!r}")
        fields[name] = value
    return fields


def diff_pair(native: Path, oracle: Path, also_bin: bool) -> list[str]:
    a, b = parse(native), parse(oracle)
    out = []
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
    if also_bin and not out:
        if native.with_suffix(".bin").read_bytes() != oracle.with_suffix(".bin").read_bytes():
            out.append("  .txt identical but .bin files differ")
    order = {n: i for i, n in enumerate(list(b) + list(a))}
    return sorted(out, key=lambda s: order.get(s.split(":")[0].strip(), 0))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("native", type=Path)
    ap.add_argument("oracle", type=Path)
    ap.add_argument("--allow-unpaired", action="store_true")
    ap.add_argument("--also-bin", action="store_true")
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
