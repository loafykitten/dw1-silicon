# PCSX-Redux #2322 check

The oracle is pinned to PCSX-Redux 26bfd693 because PR #2283 (merged as 54c357f7) makes
an MVMVA one instruction after `mtc2 VZ0` use the old VZ0. That breaks Sony's `MulMatrix`
and with it every DW1 character model. Reported upstream as
grumpycoders/pcsx-redux#2322 (2026-10-13). This is the standalone test from that issue.
Use it to decide when the pin can move.

```
python3 mkexe.py            # writes gte-vz0-delay.exe (a tiny PS-X EXE, no game data)
pcsx-redux -cli -portable -interpreter -softgpu -bios scph1001.bin \
    -exe gte-vz0-delay.exe -dofile gte-vz0-delay.lua -run
```

It prints which VZ0 the MVMVA used after N = 0..3 nops (0x100 is old, 0x200 is new):

| N | 26bfd693 | 5811e223 (after #2283) |
|---|---|---|
| 0 | 0x200 | 0x100 |
| 1 | 0x200 | **0x100** |
| 2 | 0x200 | 0x200 |
| 3 | 0x200 | 0x200 |

N = 1 is `MulMatrix`'s case and must print 0x200. Before moving the pin, also re-run
`oracle/capture-libgs.lua` and `crates/psyq/tests/libgs_capture.rs`, and certify a replay
(`docs/s08-oracle.md`). `tools/oracle/stalevz.py` tells whether wrong bone matrices in a
capture are this bug.
