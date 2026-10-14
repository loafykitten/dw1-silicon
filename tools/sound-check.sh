#!/bin/sh
# Runs the replays that have an emulator SPU capture natively with DW1_SPU_LOG and compares the
# sound (tools/oracle/compare_spu.py; docs/phase6-plan.md). Exits non-zero if any differs.
#
#   tools/sound-check.sh [NAME...]     NAME: battle-fixture battle-twice training-six new-game
#
# Needs the local, gitignored captures (captures/oracle-runs/p6-spu-*, from
# oracle/capture-spu.lua on the replay's route) and fixture cards; missing ones are skipped.
# Native output goes to captures/native-runs/sound-<name>/.
set -u
cd "$(dirname "$0")/.." || exit 1
cargo build --release -p dw1 -q || exit 1
bin=target/release/dw1

# name  capture dir  fixture card (- for fresh cards)
table='battle-fixture p6-spu-battle-full ember-save2-prefight
battle-twice p6-spu-battle-twice ember-save2-prefight
training-six p6-spu-training-six ember-save2-prefight
new-game p6-spu-new-game -'

want="$*"
fail=0
while read -r name capture card; do
    if [ -n "$want" ] && ! echo " $want " | grep -q " $name "; then continue; fi
    log=captures/oracle-runs/$capture/spu-writes.txt
    if [ ! -f "$log" ]; then echo "$name: SKIP (no capture $log)"; continue; fi
    out=captures/native-runs/sound-$name
    rm -rf "$out" && mkdir -p "$out"
    if [ "$card" = - ]; then
        DW1_REPLAY=oracle/replays/$name.txt DW1_OUT=$out DW1_SPU_LOG=$out/spu-writes.txt \
            "$bin" > "$out/log.txt" 2>&1
    else
        cardfile=captures/fixtures/$card.mcd
        if [ ! -f "$cardfile" ]; then echo "$name: SKIP (no card $cardfile)"; continue; fi
        DW1_REPLAY=oracle/replays/$name.txt DW1_CARD1=$cardfile DW1_OUT=$out \
            DW1_SPU_LOG=$out/spu-writes.txt "$bin" > "$out/log.txt" 2>&1
    fi
    # The capture's last VBlank (it stops a VBlank before the replay's end).
    end=$(tail -1 "$log" | cut -d' ' -f1)
    if python3 tools/oracle/compare_spu.py "$log" "$out/spu-writes.txt" "oracle/replays/$name.txt" \
        --oracle-end "$end" --show 10 > "$out/compare.txt" 2>&1; then
        echo "$name: ok; $(grep -E '^(sound effects|music):' "$out/compare.txt" | tr '\n' ' ')"
    else
        echo "$name: DIFFERS (see $out/compare.txt)"
        fail=1
    fi
done <<EOF2
$table
EOF2
exit $fail
