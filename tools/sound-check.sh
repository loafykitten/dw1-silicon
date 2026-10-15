#!/bin/sh
# Runs the replays that have an emulator sound or movie capture natively and compares them with the
# emulator's (docs/phase6-plan.md): the SPU register traffic (tools/oracle/compare_spu.py, from an
# oracle/capture-spu.lua log), the movies' XA audio (tools/oracle/compare_xa.py, from an
# oracle/capture-xa.lua run) and the movies' MDEC codes and pictures (tools/oracle/compare_mdec.py,
# from an oracle/capture-mdec.lua run). Exits non-zero if any differs.
#
#   tools/sound-check.sh [NAME...]     NAME: battle-fixture battle-twice training-six new-game ending
#
# Needs the local, gitignored captures (captures/oracle-runs/) and fixture cards; missing ones are
# skipped. Native output goes to captures/native-runs/sound-<name>/.
set -u
cd "$(dirname "$0")/.." || exit 1
cargo build --release -p dw1 -q || exit 1
bin=target/release/dw1
# The checks themselves first: each must fail on a deliberately bad synthetic log.
if ! python3 tools/oracle/test_sound_checks.py > /dev/null 2>&1; then
    echo "tools/oracle/test_sound_checks.py fails: the checks can't be trusted"
    exit 1
fi

# name  SPU capture dir  XA capture dir  MDEC capture dir  fixture card (- for none / fresh cards)
table='battle-fixture p6-spu-battle-full - - ember-save2-prefight
battle-twice p6-spu-battle-twice - - ember-save2-prefight
training-six p6-spu-training-six - - ember-save2-prefight
new-game p6-spu-new-game p9-sound-xa-newgame p9-sound-mdec-newgame -
ending - p9-sound-xa-ending p9-sound-mdec-ending p9-ending-mt-infinity'

want="$*"
fail=0
while read -r name spu xa mdec card; do
    if [ -n "$want" ] && ! echo " $want " | grep -q " $name "; then continue; fi
    spulog=captures/oracle-runs/$spu/spu-writes.txt
    xadir=captures/oracle-runs/$xa
    [ "$spu" != - ] && [ ! -f "$spulog" ] && spu=-
    mdecdir=captures/oracle-runs/$mdec
    [ "$xa" != - ] && [ ! -f "$xadir/xa-log.txt" ] && xa=-
    [ "$mdec" != - ] && [ ! -f "$mdecdir/mdec-log.txt" ] && mdec=-
    if [ "$spu" = - ] && [ "$xa" = - ] && [ "$mdec" = - ]; then
        echo "$name: SKIP (no capture)"
        continue
    fi
    out=captures/native-runs/sound-$name
    rm -rf "$out" && mkdir -p "$out"
    set -- env DW1_REPLAY="oracle/replays/$name.txt" DW1_OUT="$out" DW1_CHECKPOINTS=0
    [ "$spu" != - ] && set -- "$@" DW1_SPU_LOG="$out/spu-writes.txt"
    [ "$xa" != - ] && set -- "$@" DW1_XA_LOG="$out/xa"
    if [ "$mdec" != - ]; then
        # Pixels only for the frames the emulator sampled (by their codes' hash).
        python3 tools/oracle/compare_mdec.py --hashes "$mdecdir" > "$out/mdec-hashes.txt"
        set -- "$@" DW1_MDEC_LOG="$out/mdec" DW1_MDEC_LOG_PIXELS="$out/mdec-hashes.txt"
    fi
    if [ "$card" != - ]; then
        cardfile=captures/fixtures/$card.mcd
        if [ ! -f "$cardfile" ]; then echo "$name: SKIP (no card $cardfile)"; continue; fi
        set -- "$@" DW1_CARD1="$cardfile"
    fi
    "$@" "$bin" > "$out/log.txt" 2>&1
    line="$name:"
    if [ "$spu" != - ]; then
        # The capture's last VBlank (it stops a VBlank before the replay's end).
        end=$(tail -1 "$spulog" | cut -d' ' -f1)
        if python3 tools/oracle/compare_spu.py "$spulog" "$out/spu-writes.txt" \
            "oracle/replays/$name.txt" --oracle-end "$end" --show 10 > "$out/compare.txt" 2>&1; then
            line="$line ok; $(grep -E '^(sound effects|music):' "$out/compare.txt" | tr '\n' ' ')"
        else
            line="$line DIFFERS (see $out/compare.txt)"
            fail=1
        fi
    fi
    if [ "$xa" != - ]; then
        if python3 tools/oracle/compare_xa.py "$xadir" "$out/xa" "oracle/replays/$name.txt" \
            > "$out/compare-xa.txt" 2>&1; then
            line="$line xa ok; $(grep -E '^xa:' "$out/compare-xa.txt")"
        else
            line="$line XA DIFFERS (see $out/compare-xa.txt)"
            fail=1
        fi
    fi
    if [ "$mdec" != - ]; then
        if python3 tools/oracle/compare_mdec.py "$mdecdir" "$out/mdec" > "$out/compare-mdec.txt" 2>&1; then
            line="$line mdec ok; $(grep -E '^mdec:' "$out/compare-mdec.txt")"
        else
            line="$line MDEC DIFFERS (see $out/compare-mdec.txt)"
            fail=1
        fi
    fi
    echo "$line"
done <<EOF2
$table
EOF2
exit $fail
