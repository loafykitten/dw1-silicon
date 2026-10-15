#!/usr/bin/env python3
"""Makes a fixture card whose Digimon World (USA) saves open the game's debug room.

    python3 tools/oracle/debug_fixture.py [--keep TRIGGER]... [--clear TRIGGER]... [--item ID]...
        IN.mcd OUT.mcd

The debug room (TCRF, "Digimon World": the area with Jijimon's house, with debug Jijimon and
Tokomon actors: Mr. Warp, time skips, Mr. Digivolution, a shop) appears when four story-flag
bytes are 0: RAM 0x801BDFD2, 0x801BDFD3, 0x801BDFD4 and 0x801BDFE6 (TCRF's GameShark codes).
They are inside SCRIPT_STATE (0x801BDED8, decomp/config/us/symbols.txt), whose first 0x17C bytes
the save keeps as SavegamePayload.scriptState at payload offset 0x290 (found from the payload's
playerName at 0x467 minus Inventory's 91 bytes and scriptState's 0x17C). So the bytes are payload
0x38A, 0x38B, 0x38C and 0x39E. Every valid copy of every save on the card is changed and its
checksum redone (crates/dw1/src/saves.rs: the sum of the first 0x4E4 bytes and of the 0xA00
bytes of battle registrations at 0x500, stored at 0x4FC).

The flag bytes are whole bytes of the trigger bitfield (ScriptState.triggers, at SCRIPT_STATE +
0xF5: triggers 40-63 and 200-207), so they also hold flags the debug room doesn't need, among them
the fishing rods (TRIGGER_HAVE_OLD_FISHROD 45, TRIGGER_HAVE_AMAZING_ROD 46, decomp
include/dw/trigger.h). `--keep N` leaves trigger N as the input had it (the debug room still
appears with 45 and 46 set: measured 2026-10-15 on Master99's 2002 save). `--clear N` also
clears trigger N anywhere in the bitfield (e.g. 257, Penguinmon recruited, so that he is back in
front of the Curling Cave with his free first match: map script 206, section 6). `--item ID`
adds one of item ID as the game's giveItem would (the payload's Inventory at 0x40C: 30 item types,
0xFF for empty, 30 counts, 30 name-texture indices, then the size in use), e.g.
115, the Blue Flute that the lake's tile 81 asks for (map script 7, section 81: it calls the Lake
Guardian, who carries you to Beetle Land).

The card stays a fixture for the oracle: a recording starts from it on both sides, so nothing
is poked into RAM during a run. Only reads IN; never overwrites an existing OUT.
"""
import os
import struct
import sys

BLOCK = 0x2000
PAYLOAD = 0xF00
COPIES = (0x200, 0x200 + PAYLOAD)
SUMMED, CHECKSUM_AT, REGS_AT, REGS = 0x4E4, 0x4FC, 0x500, 0xA00
SCRIPT_STATE_AT = 0x290
DEBUG_FLAGS = [0x1BDFD2, 0x1BDFD3, 0x1BDFD4, 0x1BDFE6]
SCRIPT_STATE_RAM = 0x1BDED8
TRIGGERS_AT = 0xF5  # ScriptState.triggers: smth[212], cards[33]
TRIGGER_COUNT = 100 * 8  # ScriptState.triggers is 100 bytes
ITEM_TYPES = 128  # ITEM_PARA / ITEM_FUNCTIONS entries (decomp/include/dw/item.h)
INVENTORY_AT = 0x40C  # SavegamePayload: Inventory, 91 bytes before playerName (0x467)
PREFIX = b"BASLUS-01032DMR"


def checksum(save):
    return (sum(save[:SUMMED]) + sum(save[REGS_AT:REGS_AT + REGS])) & 0xFFFFFFFF


def valid(save):
    return checksum(save) == struct.unpack_from("<I", save, CHECKSUM_AT)[0]


def give_item(save, item, src):
    """One of `item`, as the game's giveItem (decomp/src/main/item.c) adds it: within the saved
    Inventory.size, onto the item's slot if it has one (up to 99), else the first empty slot, which
    also takes the lowest name-texture index (Inventory.names) no slot uses."""
    types, amounts, names = INVENTORY_AT, INVENTORY_AT + 30, INVENTORY_AT + 60
    size = save[INVENTORY_AT + 90]
    if size > 30:
        sys.exit(f"{src}: inventory size {size} past its 30 slots")
    for i in range(size):
        if save[types + i] == item:
            save[amounts + i] = min(save[amounts + i] + 1, 99)
            return
    for i in range(size):
        if save[types + i] == 0xFF:
            save[types + i], save[amounts + i] = item, 1
            used = {save[names + j] for j in range(size)}
            save[names + i] = next(j for j in range(size) if j not in used)
            return
    sys.exit(f"{src}: the inventory ({size} slots) is full; no room for item {item}")


def main():
    args, keep, clear, items = sys.argv[1:], [], [], []
    while args and args[0] in ("--keep", "--clear", "--item"):
        if len(args) < 2 or not args[1].isdigit():
            sys.exit(__doc__)
        n = int(args[1])
        if args[0] == "--item" and not 0 <= n < ITEM_TYPES:
            sys.exit(f"--item {n}: item types are 0..{ITEM_TYPES - 1}")
        if args[0] != "--item" and not 0 <= n < TRIGGER_COUNT:
            sys.exit(f"{args[0]} {n}: triggers are 0..{TRIGGER_COUNT - 1}")
        {"--keep": keep, "--clear": clear, "--item": items}[args[0]].append(n)
        args = args[2:]
    if len(args) != 2:
        sys.exit(__doc__)
    src, dst = args
    if os.path.exists(dst):
        sys.exit(f"{dst} exists; not overwriting it")
    card = bytearray(open(src, "rb").read())
    if len(card) != 128 * 1024 or card[:2] != b"MC":
        sys.exit(f"{src}: not a raw 128 KB memory card")
    offsets = [SCRIPT_STATE_AT + a - SCRIPT_STATE_RAM for a in DEBUG_FLAGS]
    changed = 0
    for frame in range(1, 16):
        entry = card[frame * 128:(frame + 1) * 128]
        if entry[0] != 0x51 or not entry[10:10 + len(PREFIX)] == PREFIX:
            continue
        base = frame * BLOCK
        for copy in COPIES:
            at = base + copy
            save = card[at:at + PAYLOAD]
            if not valid(save):
                continue
            before = [save[o] for o in offsets]
            for o in offsets:
                kept = 0
                for t in keep:
                    if SCRIPT_STATE_AT + TRIGGERS_AT + t // 8 == o:
                        kept |= save[o] & (1 << (t % 8))
                save[o] = kept
            for t in clear:
                save[SCRIPT_STATE_AT + TRIGGERS_AT + t // 8] &= ~(1 << (t % 8)) & 0xFF
            for item in items:
                give_item(save, item, src)
            struct.pack_into("<I", save, CHECKSUM_AT, checksum(save))
            card[at:at + PAYLOAD] = save
            changed += 1
            name = entry[10:30].split(b"\0")[0].decode()
            print(f"{name} copy at {copy:#x}: flags {[hex(b) for b in before]} -> {[hex(save[o]) for o in offsets]}")
    if not changed:
        sys.exit(f"{src}: no Digimon World (USA) save with a good checksum")
    with open(dst, "wb") as f:
        f.write(card)
    print(f"wrote {dst} ({changed} save copies)")


if __name__ == "__main__":
    main()
