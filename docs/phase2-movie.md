# Phase 2, slice 7: the movies

2026-10-13. `MOV_playMovie` (the decomp's MOV overlay) runs unchanged on `psyq::libcd`'s
streaming calls, `psyq::libpress` and `backend::mdec`. Each of the four movies plays to the frame
count in `MOV_MOVIES` and ends; Start (pad bit 0x800) ends it early (not movie 3, the credits:
the game doesn't let you skip it). Try it without the boot (which stops at `RotMatrixZYX` until
libgte lands): `DW1_MOVIE=0 DW1_START_AT=1000 ./target/debug/dw1` (`DW1_SHOTS=...` writes
screenshots; `DW1_START_AT` holds Start from that VBlank).

## Bitstream version 3

The movies are **STR bitstream version 3**: the DC values are Huffman coded differences
(psx-spx, "CDROM File Video BS Compression DC Values"), where DW2-PC's decoder (and so
`backend::mdec::vlc` until now) did versions 1 and 2. `vlc` does 3 now (`dc_diff_v3`: MPEG-1's
DC size tables, differences times 4 summed per Cr, Cb and Y, wrapped to 10 bits). The AC codes
are the same in every version. Unit tests build streams by hand; the whole of OP1 (3,637
frames) decodes without an error. The differential tests against DW2-PC's C only cover
versions 1 and 2 now (its C rejects 3).

## How MOV drives it

- `MOV_startStream`: `DecDCTReset(0)`, `DecDCToutCallback(MOV_onSliceDecoded)`, `StSetRing(buf,
  24)`, `StSetStream(1, 1, -1, 0, 0)`, then `MOV_startRead`: Setloc, Setmode(0x80), `VSync(3)`,
  `CdRead2(0x1e0)` (Stream|Speed|RT|Size1), retried until it returns 1.
- `MOV_readFrame` polls `StGetNext(&bs, &header)` up to 2000 times; `MOV_decodeFrame` does
  `DecDCTvlc2(bs, codes, table)` and `StFreeRing(bs)`; after a timeout `StGetBackloc(&loc)` and
  `MOV_startRead` again.
- Per frame: `DecDCTin(codes, 3)`, `DecDCTout(slice, 24 * height / 2)`; the callback runs per
  slice (20 for a 320-pixel frame: `LoadImage` of 24x`height` halfwords, the next `DecDCTout`);
  `MOV_waitForFlip`, `VSync(0)`, `PutDispEnv` with `isrgb24 = 1`.
- End: `DecDCToutCallback(0)`, `StUnSetRing`, `CdControlB(CdlPause)`.

## Model (`crates/psyq/src/stream.rs`, `libcd.rs`, `libpress.rs`)

- **Sectors:** the n-th sector since `CdRead2` arrives at `t0 + ceil(n * 225,792 / 564,398)`
  VBlanks [doc: 150 sectors a second at double speed = 225,792 cycles; obs: 564,398 cycles per
  VBlank in Redux], 75 a second without the Speed bit. `t0` is the VBlank of the read command
  plus 4 [obs: below]. Sectors are taken in lazily, when a call looks at the stream.
- **`StGetNext`:** frame found already arrived: returned at once. Otherwise the stream runs
  ahead to the frame's last sector, `libetc::advance_to` runs the VBlanks up to its arrival (host
  callbacks included), and the frame is returned. So the game's 2000-poll limit never fires
  for lack of time. It returns 1 only when no frame is coming (the read left the disc or was
  stopped), and `MOV_decodeFrame` then restarts at `StGetBackloc`, as on hardware.
- **Frames:** video sectors (the `0x0160`/`0x8001` header, not XA audio) are collected into
  frames; a gap or a frame change drops the partial frame. A frame occupies `sectors * 2048` bytes
  of a circular buffer of `StSetRing`'s size until `StFreeRing`; one that doesn't fit is lost
  [guess: the SDK also drops it] and `StGetBackloc` points at it. The frames are held in memory the
  stream owns, not in the game's ring array. `StCdIntrFlag` stays 0 and `StCdInterrupt` does
  nothing.
- **MDEC:** `DecDCTout` decodes its slice, runs the callback, and runs what the callback queued
  (a `DecDCTout` from inside the callback), so the frame is complete when the first call
  returns. `DecDCTin`'s mode bit 0 is 24-bit output, bit 1 the mask bit [doc: DW2-PC's reading,
  and MOV: mode 3 with a 24-bit display]. `DecDCTvlcBuild` does nothing (the decoder has its
  own tables); `DecDCTvlc2` finds the frame's length from the stream, since it isn't passed one.

## Observed in PCSX-Redux (26bfd693), new-game route, OP1

- `CdRead2` at VBlank 1639, first frame available at about 1646: **7 VBlanks**. The native model
  (4 + ceil(n sectors)) gives the same 7 with `START_LATENCY = 4`.
- Frame times: the emulator's `PutDispEnv` calls came at 1651, 1653, 1655, 1657, 1659, 1661,
  1662, 1665, 1667 ... The model, run on the real sector layout (frame boundaries and the XA
  sectors between them), gives the same sequence, including its 3-VBlank gaps, with a constant
  offset of 2 VBlanks. The offset is the CPU time of `DecDCTvlc2` and the rest of the loop, which
  the native run doesn't take (the game's code takes no time); movie frames come that much sooner
  after their sectors than on hardware. Nothing depends on it until the XA audio is played
  (Phase 6): the audio then has to be started with the same offset.
- `MOV_playMovie` passes `PutDrawEnv` an uninitialised `DRAWENV` (clip, offset and flags from
  stack garbage in the emulator, texture window zero). Natively that can be a texture window
  (trap) or a background fill, so the decomp fork zeroes it under `DW1_NATIVE`
  (`src/mov/mov.c`, a commit on top of the `dw1-native` branch).
- `DecDCTout` is called 20 times per frame with 0x840 words (24 x 176 / 2): the first from the
  main loop, 19 from the callback, none after the last slice; `MOVIE_WIDTH` x `MOVIE_HEIGHT` is
  320 x 176 for OP1 (the picture is letterboxed in a 320 x 240 display).
- OP1 reaches frame 3,637 (`MOV_MOVIES[0].frameCount`) about 5,640 VBlanks after the read
  starts; the file is longer than that.

## Pictures against PCSX-Redux (2026-10-13)

Movie 1 (the new-game movie, which the route plays in full) from Redux at VBlanks 5750-6590,
every 30, against `DW1_MOVIE=1` natively, each Redux frame matched to the closest native frame:
every one pairs with the native frame 5,690 VBlanks earlier (the boot before the movie, which
natively takes no time), so frames come out in the same order and cadence. The pixels are close
but not identical: about 50,000 of 76,800 differ in 24-bit colour, by at most 3-4 levels of 255
in a channel (about 10,000 at 15-bit depth). That is the IDCT and colour conversion rounding:
`backend::mdec` (from DW2-PC) and Redux's MDEC model round differently, and neither is known to
be the hardware's. Not chased now: it can't be seen, and matching Redux's rounding wouldn't
make it hardware-exact. Revisit with hardware captures if they ever exist. (Measured again
stage by stage in 2026-10-15's pass below, which fixed the colour conversion.)

### Measured stage by stage (2026-10-15, Phase 9 sound stream)

`oracle/capture-mdec.lua` (on a certified replay) records, for every 25th frame, the run-length
codes `DecDCTin` is given and every slice MOV uploads with `LoadImage` (the MDEC's output);
natively `DW1_MDEC_LOG` logs the same, and `tools/oracle/compare_mdec.py` pairs frames by their
codes (`tools/sound-check.sh` runs it on `new-game`, the new-game movie, and `ending`, the credits
and the movies around them):

- **Codes: identical.** All 295 sampled frames (130 of the new-game movie, 165 of the credits and
  the title and continue movies) have exactly the codes the SDK's `DecDCTvlc2` made on the
  emulator, so the CPU half of the decoding (bitstream version 3, `backend::mdec::vlc`) is exact.
- **Colour conversion: fixed.** The MDEC's YCbCr to RGB rounded its products down (DW2-PC's
  `>> 10`), which made every channel lean half a level against the emulator (R and B -0.5, G
  +0.5 on average). psx-spx gives real factors with no fixed point ("unknown"); rounding to the
  nearest takes the lean to under 0.02 of a level and the bytes that differ from 49% to 36%.
  And psx-spx's 9-bit limit ("Y AND 1FFh" before saturating, stated for monochrome, "probably"
  for colour): three pixels of the credits whose blue sum was -264 came out 0 natively and 255
  on the emulator; the sums are now cut to 9 bits first and those match.
- **IDCT: within its rounding.** What still differs is the IDCT: Redux computes it with a fast
  (AAN) factorisation of its own, the port with psx-spx's matrix form, and psx-spx says the
  hardware's rounding "isn't perfect" in its own code. On the new-game movie 7.8 million of 22
  million channel bytes differ, 95% by one level, at most 4 (5 in two corner pixels of the
  credits), with no lean (mean signed difference under 0.02 per channel). Matching Redux here
  would mean copying its factorisation and wouldn't be the hardware's either, so the check
  allows 6 levels and no lean of more than 0.1.

DW1 shows its movies in 24-bit mode, so no dithering is involved.

## Tests

`psyq/src/stream.rs` (assembly, arrival times, ring, loss and backloc), `libpress.rs` (slices,
the callback chain without recursion), `libetc.rs` (`advance_to`), `backend/src/mdec.rs` (version
3), `psyq/tests/movie_synthetic.rs` (the C entry points end to end on a synthetic disc, with the
clock), and `psyq/tests/movie_disc.rs` (gated on the disc: all of OP1 through the libraries, a
few frames written as PNG under `captures/movie/`).
