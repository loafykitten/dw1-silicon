#!/bin/sh
# Runs every certified replay natively and compares its canonical-state checkpoints with the
# emulator's (tools/oracle/compare_canon.py). Exits non-zero if any replay fails or differs.
#
#   tools/replay-check.sh [--debug] [NAME...]     NAME: new-game file-city-walk care-fixture
#                                                 battle-fixture battle-twice training-fixture
#                                                 training-bonus training-six keeper-fixture
#                                                 register-save debug-evo debug-death
#                                                 battle-mode arena-cup tour-panorama
#                                                 tour-jungle tour-drill tour-savanna
#                                                 tour-mansion tour-canyon
#
# Needs the local, gitignored oracle dumps (captures/oracle-runs/) and fixture cards
# (captures/fixtures/); a replay whose dumps or card are missing is reported and skipped.
# Native output goes to captures/native-runs/check-<name>/ (`DW1_CHECK_PREFIX` replaces `check-`,
# so parallel worktrees sharing captures/ don't overwrite each other's runs).
set -u
cd "$(dirname "$0")/.." || exit 1
# The oracle dumps have the default checkpoints; a caller's interval would unpair them.
unset DW1_CHECKPOINT_EVERY

profile=release
if [ "${1:-}" = "--debug" ]; then profile=debug; shift; fi
if [ "$profile" = release ]; then
    cargo build --release -p dw1 -q || exit 1
else
    cargo build -p dw1 -q || exit 1
fi
bin=target/$profile/dw1

# name  oracle dump dir  fixture card (- for fresh cards)  [second card, for two-card replays]
# (the v3-* dumps are fresh Redux replays with the schema v3 canonical state; p5-*-replay1 are
# the battle recordings' certification runs)
table='new-game v3-new-game -
file-city-walk v3-file-city-walk -
care-fixture v3-care-fixture ember-day2-gabumon
battle-fixture p5-battle-replay1 ember-save2-prefight
battle-twice p5-twice-replay1 ember-save2-prefight
training-fixture p5-trn-replay1 ember-save2-prefight
training-bonus p5-bonus-replay1 ember-save2-prefight
training-six p5-six-replay1 ember-save2-prefight
keeper-fixture p5-keeper-replay1 ember-day2-gabumon
register-save p7-register-save ember-day2-gabumon
debug-evo p9-debug-evo-record debug-day2-gabumon
debug-death p9-debug-death-record debug-day2-gabumon
battle-mode p9-arena-vs-record p9-arena-vs-p1 p9-arena-vs-p2
arena-cup p9-arena-std-record p9-arena-master99
tour-panorama p9-tour-panorama-record debug-day2-gabumon
tour-jungle p9-tour-jungle-record debug-day2-gabumon
tour-drill p9-tour-drill-record debug-day2-gabumon
tour-savanna p9-tour-savanna-record debug-day2-gabumon
tour-mansion p9-tour-mansion-record debug-day2-gabumon
tour-canyon p9-tour-canyon-record debug-day2-gabumon'

want="$*"
echo "$table" | while read -r name oracle card card2; do
    if [ -n "$want" ] && ! echo " $want " | grep -q " $name "; then continue; fi
    odir=captures/oracle-runs/$oracle
    if [ ! -d "$odir" ]; then echo "$name: SKIP (no oracle dumps in $odir)"; continue; fi
    out=captures/native-runs/${DW1_CHECK_PREFIX:-check-}$name
    rm -rf "$out" && mkdir -p "$out"
    if [ "$card" = - ]; then
        DW1_REPLAY=oracle/replays/$name.txt DW1_OUT=$out "$bin" > "$out/log.txt" 2>&1
    else
        cardfile=captures/fixtures/$card.mcd
        if [ ! -f "$cardfile" ]; then echo "$name: SKIP (no card $cardfile)"; continue; fi
        if [ -n "$card2" ]; then
            card2file=captures/fixtures/$card2.mcd
            if [ ! -f "$card2file" ]; then echo "$name: SKIP (no card $card2file)"; continue; fi
            DW1_REPLAY=oracle/replays/$name.txt DW1_CARD1=$cardfile DW1_CARD2=$card2file DW1_OUT=$out "$bin" > "$out/log.txt" 2>&1
        else
            DW1_REPLAY=oracle/replays/$name.txt DW1_CARD1=$cardfile DW1_OUT=$out "$bin" > "$out/log.txt" 2>&1
        fi
    fi
    rc=$?
    # Every checkpoint must pair: a certified replay runs to its end on both sides.
    compared=$(python3 tools/oracle/compare_canon.py "$out" "$odir" 2>&1)
    crc=$?
    summary=$(echo "$compared" | tail -1)
    if [ $rc -ne 0 ]; then
        echo "$name: FAIL (exit $rc; $out/log.txt) $summary"
        exit 1
    fi
    # compare_canon.py exits non-zero on any difference, an unpaired checkpoint, an invalid
    # record or no pairs at all.
    if [ $crc -ne 0 ]; then
        echo "$name: FAIL (comparison exit $crc) $summary"
        exit 1
    fi
    echo "$name: exit 0; $summary"
done
status=$?
exit $status
