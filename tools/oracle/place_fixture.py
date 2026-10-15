#!/usr/bin/env python3
"""Makes a fixture card whose Digimon World (USA) saves load on a chosen screen and spot.

    python3 tools/oracle/place_fixture.py IN.mcd OUT.mcd SCREEN PREV EXIT X Z [TRIGGER=0|1 ...]
        [--time HH:MM] [--register CUP]

The game saves only in a few rooms, but loading a save is generic (decomp/src/main/main.c,
the continue path after `loadSavegame`): the map is the save's currentScreen, the tamer and the
partner stand at its tamerPosition and partnerPosition, and PREVIOUS_SCREEN / CURRENT_EXIT come
from the save. So a save moved to the spot where a warp into SCREEN through EXIT lands (that
map's MAP_WARPS spawn) starts a recording there, the way debug_fixture.py's flag change opens the
debug room. The partner goes 200 units behind the tamer as a warp puts it (rotation 0x800:
behind is -z; the rotation is the fifth argument's direction, see ROTATION below), and the
tamer's waypoint trail starts empty on the spot's tile, as initializeTamerWaypoints leaves it
after a warp. TRIGGER=0|1 arguments clear or set story flags (SCRIPT_STATE.triggers, bit n of
byte n/8 at SCRIPT_STATE + 0xF5).

--time HH:MM sets the time of day as the game's own advanceToTime (decomp/src/main/clock.c,
the partner waking up) writes it on the same day: HOUR, MINUTE and CURRENT_FRAME = HOUR * 1200 +
MINUTE * 20; lastHandledFrame equals it, as in a save written after the clock's tick
(tickGameClock sets LAST_HANDLED_FRAME = CURRENT_FRAME). Like advanceToTime it doesn't catch up the
partner's care (that is skipHours, which the script's ADVANCE_TO_DATE_AT adds): the care state stays
as saved.

--register CUP registers for today's cup number CUP the way the Arena Lobby's schedule window does
(DGET, decomp/src/dget/dget.c, initTournamentSchedule's cases 3 and 5 with today's column chosen):
PSTAT_TOURNAMENT_ID (pstat 3) = CUP (the schedule entry & 0x3f), triggers 37
(TRIGGER_TOURNAMENT_REGISTERED), 38 and 39 (the noon and 20:00 reminders) set,
PSTAT_TOURNAMENT_DAY (pstat 2) = the save's DAY (no 0x80: today) and PSTAT_TOURNAMENT_DIGIMON
(pstat 4) = the save's partner type. Trigger 40 (TRIGGER_TOURNAMENT_OVERLEVELED) is what
tournamentCheckFair decides for the cup and the partner: give it as 40=0|1. Only pstats 0..34 are
in a save (the payload keeps 0x17C bytes of ScriptState; pstats start at +0x159), so the
conversation's script variables past those (pstat 103, 254) aren't part of it.

Every valid copy of every DW1 save on the card is changed and its checksum redone (the sum of
the first 0x4E4 bytes and of the 0xA00 bytes of battle registrations at 0x500, stored at 0x4FC).
Only reads IN; never overwrites an existing OUT.
"""
import os
import struct
import sys

BLOCK = 0x2000
PAYLOAD = 0xF00
COPIES = (0x200, 0x200 + PAYLOAD)
SUMMED, CHECKSUM_AT, REGS_AT, REGS = 0x4E4, 0x4FC, 0x500, 0xA00
PREFIX = b"BASLUS-01032DMR"
# SavegamePayload (decomp/src/main/main_menu.c) offsets.
TAMER_POS, PARTNER_POS = 0x04, 0x14
SCRIPT_STATE, TRIGGERS = 0x290, 0xF5
TRIGGER_COUNT = 100 * 8  # SCRIPT_STATE.triggers is 100 bytes
PARTNER_TYPE = 0x1B8
CURRENT_FRAME, LAST_HANDLED_FRAME, DAY, HOUR, MINUTE = 0x1CC, 0x1CE, 0x1D2, 0x1D4, 0x1D6
PSTATS = 0x159  # ScriptState.pstats; the save keeps ScriptState's first 0x17C bytes
PSTAT_TOURNAMENT_DAY, PSTAT_TOURNAMENT_ID, PSTAT_TOURNAMENT_DIGIMON = 2, 3, 4
TRIGGER_TOURNAMENT_REGISTERED, TRIGGER_38, TRIGGER_39 = 37, 38, 39
CURRENT_SCREEN, PREVIOUS_SCREEN, CURRENT_EXIT = 0x49A, 0x49B, 0x49C
WAYPOINT_X, WAYPOINT_Y = 0x49E, 0x4BC
PREV_TILE_X, PREV_TILE_Y, WAYPOINT_CURRENT, WAYPOINT_COUNT = 0x4DA, 0x4DB, 0x4DC, 0x4DD
START_TILE_X, START_TILE_Y, WAYPOINT_ACTIVE = 0x4DE, 0x4DF, 0x4E0
# Where the partner stands after a warp, by the spawn rotation (main.c's continue path).
ROTATION = 0x800


def checksum(save):
    return (sum(save[:SUMMED]) + sum(save[REGS_AT:REGS_AT + REGS])) & 0xFFFFFFFF


def model_tile(x, z):
    """getModelTile (decomp/src/main/map_collision.c), C division."""
    tx = int(x / 100) + 50 - (1 if x < 0 else 0)
    tz = 50 - int(z / 100) - (1 if z > 0 else 0)
    return tx, tz


def partner_spot(x, z, rot):
    if rot <= 0x200 or rot > 0xE00:
        return x, z + 200
    if rot <= 0x600:
        return x + 200, z
    if rot <= 0xA00:
        return x, z - 200
    return x - 200, z


def main():
    args = sys.argv[1:]
    time = cup = None
    while '--time' in args or '--register' in args:
        for opt in ('--time', '--register'):
            if opt in args:
                i = args.index(opt)
                if i + 1 >= len(args):
                    sys.exit(f"{opt} needs a value")
                value = args[i + 1]
                del args[i:i + 2]
                if opt == '--time':
                    try:
                        hour, minute = (int(v) for v in value.split(':'))
                    except ValueError:
                        sys.exit(f"--time {value}: HH:MM")
                    if not (0 <= hour < 24 and 0 <= minute < 60):
                        sys.exit(f"--time {value}: HH 0..23, MM 0..59")
                    time = (hour, minute)
                else:
                    cup = int(value, 0)
                    if not 0 <= cup < 0x40:
                        sys.exit(f"--register {value}: a cup number is 0..63 (entry & 0x3f)")
    if len(args) < 7:
        sys.exit(__doc__)
    src, dst = args[0], args[1]
    screen, prev, exit_, x, z = (int(a, 0) for a in args[2:7])
    flags = []
    for arg in args[7:]:
        n, v = arg.split('=')
        n, v = int(n, 0), int(v)
        if not 0 <= n < TRIGGER_COUNT or v not in (0, 1):
            sys.exit(f"{arg}: a trigger is 0..{TRIGGER_COUNT - 1}=0|1")
        flags.append((n, v))
    if os.path.exists(dst):
        sys.exit(f"{dst} exists; not overwriting it")
    card = bytearray(open(src, "rb").read())
    if len(card) != 128 * 1024 or card[:2] != b"MC":
        sys.exit(f"{src}: not a raw 128 KB memory card")
    px, pz = partner_spot(x, z, ROTATION)
    tx, tz = model_tile(x, z)
    changed = 0
    for frame in range(1, 16):
        entry = card[frame * 128:(frame + 1) * 128]
        if entry[0] != 0x51 or not entry[10:10 + len(PREFIX)] == PREFIX:
            continue
        for copy in COPIES:
            at = frame * BLOCK + copy
            save = card[at:at + PAYLOAD]
            if checksum(save) != struct.unpack_from("<I", save, CHECKSUM_AT)[0]:
                continue
            struct.pack_into("<4i", save, TAMER_POS, x, 0, z, 0)
            struct.pack_into("<4i", save, PARTNER_POS, px, 0, pz, 0)
            save[CURRENT_SCREEN], save[PREVIOUS_SCREEN], save[CURRENT_EXIT] = screen, prev, exit_
            save[WAYPOINT_X:WAYPOINT_X + 30] = bytes(30)
            save[WAYPOINT_Y:WAYPOINT_Y + 30] = bytes(30)
            save[PREV_TILE_X], save[PREV_TILE_Y] = tx, tz
            save[START_TILE_X], save[START_TILE_Y] = tx, tz
            save[WAYPOINT_CURRENT] = save[WAYPOINT_COUNT] = save[WAYPOINT_ACTIVE] = 0
            if time:
                hour, minute = time
                ticks = hour * 1200 + minute * 20
                struct.pack_into("<HH", save, CURRENT_FRAME, ticks, ticks)
                struct.pack_into("<hh", save, HOUR, hour, minute)
            if cup is not None:
                pstats = SCRIPT_STATE + PSTATS
                day = struct.unpack_from("<h", save, DAY)[0]
                if not 0 <= day < 0x80:
                    sys.exit(f"{src}: day {day} doesn't fit PSTAT_TOURNAMENT_DAY")
                save[pstats + PSTAT_TOURNAMENT_DAY] = day
                save[pstats + PSTAT_TOURNAMENT_ID] = cup
                save[pstats + PSTAT_TOURNAMENT_DIGIMON] = struct.unpack_from("<i", save, PARTNER_TYPE)[0] & 0xFF
                for n in (TRIGGER_TOURNAMENT_REGISTERED, TRIGGER_38, TRIGGER_39):
                    save[SCRIPT_STATE + TRIGGERS + n // 8] |= 1 << (n % 8)
            for n, v in flags:
                byte = SCRIPT_STATE + TRIGGERS + n // 8
                if v:
                    save[byte] |= 1 << (n % 8)
                else:
                    save[byte] &= ~(1 << (n % 8)) & 0xFF
            struct.pack_into("<I", save, CHECKSUM_AT, checksum(save))
            card[at:at + PAYLOAD] = save
            changed += 1
    if not changed:
        sys.exit(f"{src}: no valid DW1 save")
    open(dst, "wb").write(card)
    extra = ''
    if time:
        extra += f", time {time[0]:02}:{time[1]:02}"
    if cup is not None:
        extra += f", registered for today's cup {cup}"
    print(f"{dst}: {changed} save copies moved to screen {screen} ({x}, {z}), tile ({tx}, {tz}){extra}")


if __name__ == "__main__":
    main()
