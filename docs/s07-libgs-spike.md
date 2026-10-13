# S0.7: libgs/TMD spike (plan)

2026-10-13. The question: can a clean Rust libgs (no Sony code: PLAN.md ground rules) turn
a DW1 model into the same GPU packets the real SDK does? DW1 draws its 3D models through
`GsSortObject4J` (42 calls) and 27 TMD handlers installed in `GsFCALL4`
(`decomp/src/main/graphics.c:60`); DW2-PC has none of this (`docs/survey.md`).

## Method: same inputs, same packets

Compare at the libgs boundary instead of comparing whole frames:

1. **Get to a frame with the partner on screen** in PCSX-Redux
   (`oracle/route-new-game.lua`, in progress).
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
