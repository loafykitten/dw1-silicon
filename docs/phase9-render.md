# Phase 9: the model-rendering paths the game still lacked

2026-10-15, stream "render" (worktree `dw1-silicon-wt/render`, branch `p9-render`). The overlay
audit's finding 9 (`docs/phase9-overlay-audit.md`): `GsSortObject4J` stopped on most of the
primitive kinds outside the characters' models, on the arenas' divide attribute and on the
champion's hidden objects, and `SetDrawLoad` was a trapping stub. All of it is now implemented
clean-room and matches the real SDK on the emulator.

## What the disc's models use

Every TMD on the disc, the `.TMD` files and the TMDs inside other files (character `.MMD`s,
`EFEDAT.EFE`, the overlays), by primitive kind: 345 TMDs, 3,554 objects, 82,359 primitives
(the table's counts, 82,411, also include five TMD-like blocks inside other files whose primitive
lists don't all check out). `tests/libgs_disc_models.rs` finds the 345 the same way, sorts every
object natively from four views and must not stop.

| flag, mode | kind | primitives | handler |
|---|---|---|---|
| 0, 0x34/0x36 | lit Gouraud textured triangle | 31,799 | `GsTMDfastTG3L` (had) |
| 0, 0x3C/0x3E | lit Gouraud textured quad | 41,039 | `GsTMDfastTG4L` (had) |
| 1, 0x2F | unlit flat textured quad, semi | 3,903 | `GsTMDfastTNF4` (had: 0x2F only) |
| 0, 0x2C/0x2E | lit flat textured quad | 2,494 | `GsTMDfastTF4L` |
| 1, 0x2D | unlit flat textured quad | 964 | `GsTMDfastTNF4` |
| 0, 0x24/0x26 | lit flat textured triangle | 851 | `GsTMDfastTF3L` |
| 1, 0x25/0x27 | unlit flat textured triangle | 753 | `GsTMDfastTNF3` |
| 1, 0x3D/0x3F | unlit Gouraud textured quad | 410 | `GsTMDfastTNG4` |
| 0, 0x28 | lit flat quad | 131 | `GsTMDfastF4L` |
| 1, 0x35 | unlit Gouraud textured triangle | 47 | `GsTMDfastTNG3` |
| 0, 0x38 | lit Gouraud quad | 19 | `GsTMDfastG4L` (KARRING's stones) |
| 0, 0x20 | lit flat triangle | 1 | `GsTMDfastF3L` (an EFE model) |

No primitive is two-sided, gradated, a line or a sprite; the raw-texture bit (mode bit 0) is
set on every unlit primitive and clear on every lit one. The game's objects have attribute 0,
0x200 (`GsDIV1`: object 0 of `STDDAT\*_TOGI.TMD`, the arena floor, in STD once the bracket is
finished and always in VS) or 0x80000000 (display off: the champion's models at first). Under
0x200 the floors' kinds go to `GsTMDdivTNF3`, `TNF4`, `TNG3` and `TNG4`.

Handlers the game installs that no disc model reaches stay trapping stubs: `GsTMDfastG3L`,
`F4NL`, `NF4`, `TF3NL`, `TF4NL`, `TG3NL`, `TG4NL` and the lit dividing ones (`GsTMDdivTF3NL`,
`TF4L`, `TF4NL`, `TG3NL`, `TG4NL`).

## Method

Black-box calls of the real SDK through `oracle/call-sdk.lua`, as for `GsSortSprite`: each
request builds a synthetic object (a `GsDOBJ2`, its record, vertices, normals and primitives
linked as `GsLinkObject4` leaves them), an ordering table cleared as `GsClearOt` does with a
guard area after it, a filled packet area and a patterned scratchpad, presets all 64 GTE
registers and calls `GsSortObject4J(obj, ot, shift, 0x1F800000)`; it reads back the packets, the
table and guard, the work base, the scratchpad and the GTE.

`call-sdk.lua` gained two things for this: `in`/`out` addresses in the scratchpad, and
`DW1_RESTORE=1`, which puts RAM and the scratchpad back before each call (some inputs make the
real code scribble over memory and every later call then faulted).

The requests (`tools/oracle/tmd_requests.py`), runs under `captures/oracle-runs/`:

| run | set | requests | compared |
|---|---|---|---|
| `p9-render-s4j-fast` | every kind, mixed objects, random matrices, lights, positions (behind the eye, off screen, far), shifts, table sizes, `GsOT` offsets | 3,000 | 2,668 (332 wrote outside the table) |
| `p9-render-s4j-div` | `GsDIV1` on the four unlit textured kinds | 1,600 | 1,489 (111 outside the table) |
| `p9-render-s4j-bounds` | the dividing handlers' screen-edge and near tests at their edges, other screen sizes | 600 | 600 |
| `p9-render-s4j-bits` | each kind with and without the semi-transparency bit | 44 | 42 |
| `p9-render-s4j-attr` | display off with other attribute bits | 32 | 32 |
| `p9-render-drawload` | `SetDrawLoad` rectangles (`tools/oracle/drawload_requests.py`) | 160 | 72 (88 over 12 words) |
| `p9-render-explore` | a few hand-made objects, to read the output while implementing | 19 | (not a test) |

`tests/libgs_tmd_oracle.rs` and `tests/libgpu_drawload_oracle.rs` replay them natively and
compare every word read back (links translated). A request whose call wrote outside the
ordering table is not compared: natively that stops the program. The captured game calls
(`tests/libgs_capture.rs`, 528 calls) still match.

## What the SDK does [obs]

`GsSortObject4J`:
- display off (bit 31): returns at once, writing nothing (not even its mode globals);
- writes libgs's `GsLMODE`, `GsLIOFF`, `GsNDIV`, `GsTON` from the attribute; the dividing
  handlers read the divide level from `GsNDIV` (natively `sort4j::NDIV`; the others have no
  reader);
- divide levels past 1, the light modes, the semi-transparency bits: the game never sets them
  and the real code runs away on some kinds, so natively they stop.

The non-dividing handlers (`libgs/tmd.rs`), per primitive:
- RTPT on vertices 0-2; a FLAG error (bit 31) drops it; NCLIP 0 or less drops it; a quad's
  fourth vertex through RTPS, a FLAG error drops it; AVSZ3 / AVSZ4;
- the entry is `(OTZ - GsOT.offset) >> shift`, unchecked (the SDK writes outside the table);
- lit kinds: RGBC is the colour word as stored (untextured: its high byte, the mode on the disc,
  becomes the command) or the mode over 0x808080 (textured), then NCCS on the first normal
  (flat) or NCCT and NCCS (Gouraud); unlit kinds copy their colour words, the first with the
  mode without its raw-texture bit as the command, the others as stored;
- packets as before; the texture words as stored;
- some words go into the packet before the tests, so a dropped primitive leaves them past the
  work base: the texture words (FT3 lit and unlit, GT3 unlit) or the first colour word (FT4
  unlit) before the projection test; the first three XYs (every quad) and the texture words (FT4)
  after the back-face test.

The dividing handlers (`libgs/divide.rs`) work in the scratchpad (libgte's `DIVPOLYGON` shape:
level, screen width and height, CLUT and texture page, colour, the entry's address, then one
24-byte record per vertex and the original records' addresses):
- the originals' positions and the record addresses first, then RTPT and NCLIP (0 or less drops
  it, nothing else written); FLAG errors don't drop it;
- then the header and the originals' texture words, colours, screen XYs and depths; then a
  polygon wholly outside the screen (every vertex further than half libgs's screen width or
  height, `HWD0`/`VWD0` = `GsInitGraph`'s 320 x 240, from the GTE's screen offset) or wholly
  nearer than `H >> 1` is dropped;
- level 1: the edge midpoints (position, texture coordinates and colour each half their ends'
  sum, arithmetic shift) and a quad's centre (a quarter of the four corners' sums), projected
  with RTPS; only the bytes that carry a value are written (the rest of a record keeps what the
  scratchpad held); the midpoints' depths are kept by the Gouraud quad handler only, and by the
  flat quad handler for two of its five;
- four pieces in a fixed order, all at the whole polygon's entry: a triangle's (v1 m12 m01),
  (m01 m12 m02), (v0 m01 m02), (v2 m02 m12); a quad's (v0 m01 m02 c), (v1 m13 m01 c),
  (v2 m02 m23 c), (v3 m23 m13 c); texture words: the first with the CLUT, the second with the
  texture page, the rest with nothing; Gouraud colours: the first vertex's record gets the
  command in its high byte, the others go out as their records stand (whatever the scratchpad
  held in a midpoint's high byte).

Natively the scratchpad is the game's own (`dw1_scratchpad`), so the pieces and the work area
come out as the PS1's.

`SetDrawLoad(p, r)` (libgpu): GP0(A0h), then x and y, then width and height (halfwords as
given), the pixel words left to the caller (`(w * h + 1) / 2`), GP0(01h) after them; the tag's
length byte counts all of it. Over 12 words the SDK writes a one-word packet with a null link;
the game's rectangles are 16 x 1, so natively that stops.

## What now renders natively

Not yet seen in a native run of those scenes (no recording reaches them yet; the fishcurl and
arena streams are making them): STD and VS arenas (the floors divided), the STD/VS draw, win/lose
and champion scenes (`DRAW.TMD`, `WIN_LOSE.TMD`, `CHAMP.TMD`, `1P2PWIN.TMD`, the champion's
hidden-then-shown models), Seadramon (`SEA.TMD`), the curling stones (`KARRING.TMD`), EFE effects
with flat or untextured models, the `ETCNA` and `DOOR` models, and the Digimon whose `.MMD`s use
flat textured or untextured kinds (TOYA, CLEA, TONO, BRIK, HAGU, HYOG). STD/VS's EFE sub-opcode
0x50 (`SetDrawLoad`) works for the CLUT rows it loads. Every disc model sorts natively without a
stop (`tests/libgs_disc_models.rs`).

## Still open

- What the real SDK does with an OTZ outside the table (it writes there): natively a stop.
- `SetDrawLoad` over 12 words, the light-off and fog columns, divide levels past 1, two-sided
  and gradated primitives: unobserved in the game, stops.
- Seen only on synthetic objects: a scene check against the emulator's frames when the other
  streams' recordings reach the arenas, Seadramon and curling.
