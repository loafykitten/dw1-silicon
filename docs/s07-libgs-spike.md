# S0.7: libgs/TMD spike

2026-10-13. The question: can a clean Rust libgs (no Sony code: PLAN.md ground rules) turn
a DW1 model into the same GPU packets the real SDK does? DW1 draws its 3D models through
`GsSortObject4J` (42 calls) and 27 TMD handlers installed in `GsFCALL4`
(`decomp/src/main/graphics.c:60`); DW2-PC has none of this (`docs/survey.md`).

## Method: same inputs, same packets

Compare at the libgs boundary instead of comparing whole frames:

1. **Get to a frame with the partner on screen** in PCSX-Redux
   (`oracle/route-new-game.lua`).
2. **Capture one `GsSortObject4J` call** at that frame. Run to the frame without
   breakpoints (they cost ~20x), then set an Exec breakpoint on `GsSortObject4J`
   (0x80099D88) and one on its return address. At the call, dump the inputs:
   - the arguments (`GsDOBJ2 *objp`, `GsOT *otp`, `shift`, scratch pointer);
   - the `GsDOBJ2` (attribute, `coord2`, `tmd`, id) and the TMD object it points to (header,
     vertices, normals, primitive packets; their counts are in the object header);
   - the coordinate chain (`GsCOORDINATE2`, following `super`), `GsWSMATRIX` (0x80136F84),
     the light matrices, and the GTE control registers as the game left them (the light
     and local-screen matrices set by `GsSetLightMatrix`/`GsSetLsMatrix` just before);
   - `GsOUT_PACKET_P` (0x80136FE4, where packets get written) and the OT.

   At the return, dump the outputs: the packets written from the old to the new
   `GsOUT_PACKET_P`, and the OT entries that changed. Save input + output as one fixture
   file under `captures/` (gitignored: it holds model data from the disc).
3. **Rust libgs, clean:** implement the subset `GsSortObject4J` needs for that model, from
   psx-spx's TMD description ("CDROM File Video 3D Graphics TMD/PMD/TOD/HMD/RSD") and the
   API's documented behaviour, on `backend::gte` (needs the GTE commands the handlers use:
   at least RTPT, NCLIP, the lighting ones) and writing GP0 packets.
4. **Compare** the Rust packets with the captured ones word for word (addresses
   translated), then render both through `backend::gpu` and compare pixels.

Done when one real DW1 model, captured at a real frame, produces identical packets. Then
the remaining handlers (which primitive types DW1's models use) and the other libgs entry
points (`GsSortSprite`, `GsSortBoxFill`, the OT functions) are a matter of coverage, not
risk.

## Known facts to start from

| Symbol | US address | Notes |
|---|---|---|
| `GsSortObject4J` | 0x80099D88 | `(GsDOBJ2 *objp, GsOT *otp, int shift, u_long *scratch)` |
| `GsLinkObject4` | 0x80099BF8 | `(u_long tmd_base, GsDOBJ2 *objp, int n)` |
| `GsMapModelingData` | 0x80098158 | relocates TMD offsets in place (once per model) |
| `GsSetLsMatrix`, `GsSetLightMatrix` | 0x80097DD8, 0x80097E04 | set GTE state before sorting |
| `GsWSMATRIX` | 0x80136F84 | 0x20 bytes |
| `GsOUT_PACKET_P` | 0x80136FE4 | packet write pointer |
| `GsFCALL4` | 0x80137000 | 0x130 bytes: the TMD handler table |

Layouts (from the API headers): `GsDOBJ2` is `{u32 attribute; GsCOORDINATE2 *coord2;
u32 *tmd; u32 id}`; `GsOT` is `{u32 length; GsOT_TAG *org; u32 offset; u32 point;
GsOT_TAG *tag}`; an OT tag is a 24-bit link plus an 8-bit word count.

## Result (2026-10-13): one frame's models match exactly

**Capture.** `oracle/capture-libgs.lua` follows the new-game route to `first_partner_dialogue`
(VBlank 14000: the tamer, Agumon, Jijimon and the Tokomon around them). Only there does it turn
Redux's debugger on (the debugger slows every instruction). It then records the whole machine
(registers, GTE, scratchpad, 2 MiB RAM) at the entry and the return of every `GsSortObject4J`
call: 48 calls in one frame, plus VRAM.

On the first try every character drew as a streak. The cause was a Redux bug
(`docs/s08-oracle.md`), so the oracle is now pinned to 26bfd693.

**Clean Rust libgs.** `crates/psyq/src/libgs.rs`, `sort_object4j`. Each rule is tagged with its
source: psx-spx (`[doc]`) or the captures (`[obs]`). No SDK code was read; the implementer was a
Sonnet subagent with a clean-room brief. The test is `crates/psyq/tests/libgs_capture.rs`
(ignored without the captures). It loads each call's entry state, runs the Rust sort and
requires that:
- every word libgs outputs matches: packets, OT links, `GsOUT_PACKET_P`;
- Rust writes nothing else, the stack included.

**48 of 48 calls match, 7,246 packet words.** Drawing the packets through `backend::gpu` on the
captured VRAM gives the same pixels as the captured packets.

What the SDK does, as observed:
- **TMD primitives:** stored in runs. A header with input size 0 starts a run, and its low 16 bits
  are the run's count. Lit textured polygons carry no colour word: texture words come first,
  then (normal, vertex) index pairs.
- **Transform and culling:** RTPT on vertices 0-2, then NCLIP. A result of 0 or less is culled; a
  quad is judged on its first triangle. A quad's fourth vertex goes through RTPS and AVSZ4, a
  triangle through AVSZ3.
- **OT insertion:** `OTZ >> shift`, at the head of the list.
- **Lighting:** RGBC = code | 0x808080 through NCCT, plus NCCS for a fourth vertex.
- **Packets:** per vertex, colour (with the command byte), XY, then the texture word unchanged.

`backend::gte` gained NCT, NCCS, NCCT, AVSZ3 and AVSZ4 (from psx-spx, hand-worked unit tests).

**Not covered yet** (coverage, not risk):
- the other primitive kinds DW1's models and effects use (only lit GT3/GT4 appear here);
- object attributes other than 0;
- OTZ past the table: the Rust stops with an error, and the SDK's behaviour is unobserved;
- near-plane and divide-overflow vertices;
- `GsOT.offset`/`point`;
- the other libgs entry points.

Each needs a capture of a scene that uses it.

## Update (2026-10-14): native

The spike's model became the native `GsSortObject4J` (`crates/psyq/src/libgs/sort4j.rs`), with
the lit Gouraud textured handlers split out as `GsTMDfastTG3L`/`TG4L` behind the `GsFCALL4`
table, and `tests/libgs_capture.rs` runs these 48 calls and 180 more (new-game scene to name
entry) on native memory. See `docs/phase2-plan.md`, slice 6e.

## Update (2026-10-15): coverage

The "not covered yet" list above is done (`docs/phase9-render.md`): every primitive kind the
disc's models use, attribute `GsDIV1` and display off, `GsOT.offset`; OTZ past the table still
stops (the SDK writes outside it). Near-plane and overflowing vertices: the non-dividing handlers
drop a polygon whose projection has a FLAG error, the dividing ones test the screen and H/2.

