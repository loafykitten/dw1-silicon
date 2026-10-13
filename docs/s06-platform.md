# S0.6: window, input and audio, SDL3 (Rust) vs winit + cpal

Spikes: `tools/spikes/s06/sdl3` and `tools/spikes/s06/winit-cpal` (standalone packages, own
`[workspace]`, not in the main workspace; throwaway). Both open a 960x720 window, make a
`wgpu` 30.0.1 Metal surface from the raw window handle, clear a colour and draw one
triangle at Fifo (vsync), log keyboard events, play a 440 Hz tone at 44100 Hz stereo s16 for
about one second, and time 300 presents. Run on an M2 Max, macOS 27, from a shell. The
programs ran to completion with no errors, but nobody could see the screen or hear the
tone, so "the triangle is drawn" and "the tone is audible" are unverified. No gamepad was
connected: both report "gamepads found at startup: 0" (no enumeration errors), so hotplug,
button events and rumble were never exercised.

## What each needed

| | sdl3 (dynamic) | sdl3 (static) | winit + cpal + gilrs |
|---|---|---|---|
| Crates | sdl3 0.20.0 (sdl3-sys 0.7.2+SDL-3.4.18), wgpu 30.0.1, pollster | same, `build-from-source-static` | winit 0.30.13, cpal 0.18.2, gilrs 0.11.2, wgpu 30.0.1, pollster |
| Unique packages in the normal tree | 60 | 60 | 93 |
| Clean release build (deps cached in the registry) | 18.5 s | 47.8 s | 18.8 s |
| Release binary | 5.7 MB | 8.4 MB | 6.8 MB |
| Non-system libraries linked | `/opt/homebrew/opt/sdl3/lib/libSDL3.0.dylib` (2.5 MB, 3.4.18) | none | none |

Package counts: `cargo tree --prefix none -e normal | sed 's/ (\*)//' | sort -u`, minus the
spike package itself (name plus version, so two versions of one crate count twice; the
static feature adds build-time only crates, which `-e normal` skips). So winit + cpal + gilrs
is 33 more packages than SDL3, not 40.

winit is 0.30.13, not the 0.31 beta crates.io lists as latest; 0.31 changes the application
API. The sdl3 spike links the system SDL3 with the `use-pkg-config` feature (I did not try
the crate's default features or `link-framework`); `--features static` switches to the
vendored build, which compiles SDL 3.4.18 from source with the system clang (needs cmake,
installed). `.metal_view()` is used in the sdl3 spike; I removed it once and the spike
still created the surface and presented, so it is not required on this path. wgpu 30 API
changes (`InstanceDescriptor::new_without_display_handle`, `queue.present`) hit both.

## Measured

"CPU-side present-return interval": the time between successive returns from
`queue.present` on frames that acquired a surface texture. It is not photon timing and not
proof of what the display showed. Acquisitions that failed are not counted as presents and
are reported as skipped (0 to 4 per 300-frame run). The machine has two displays, 2560x1440
at 240 Hz and 1440x2560 at 75 Hz, so the spikes take `S06_HZ=240|75` to place the window on
one. No 60 Hz display exists here, so DW1's 59.94 Hz case is untested.

| Display | sdl3 mean (stddev) | winit mean (stddev) |
|---|---|---|
| 75 Hz | 13.33 ms (0.26) | 13.34 ms (0.30) |
| 240 Hz | 4.93 ms (1.50) | 4.53 ms (1.22) |

These are the final runs after the fixes (one run each; the earlier three runs per cell,
which counted skipped frames, gave 13.34 ms +-0.12 to 0.43 at 75 Hz and 4.2 to 4.5 ms with
stddev 0.8 to 2.8 at 240 Hz). At 75 Hz both settle on the display rate. At 240 Hz both are
irregular and average above 4.17 ms (200 to 235 Hz) with min intervals of 0.4 to 2 ms. I did
not find the cause; one guess is the drawable queue in wgpu or CAMetalLayer, which would be
the same for both toolkits. Windowed behaviour looked the same in both, so present timing
does not separate them. The game will need its own fixed-step clock regardless.

Window scale: both reported 1.00 (960x720 points = 960x720 pixels) on these external
displays; Retina is untested. Both expose scale and pixel size, and display refresh rate
(SDL `displays()`/`get_mode()`, winit `refresh_rate_millihertz()`); both matched the real
display.

Audio, SDL3. The stream asks for 44100 Hz stereo s16 and SDL converts inside the stream if
the device differs. The device itself reported 44100 Hz, 2 ch, s16, with a 1024-frame
(23.2 ms) buffer (`SDL_GetAudioDeviceFormat`). The callback's `requested` argument counts
i16 samples, so 2048 requested samples are 1024 stereo frames. A stream request size is not
a device callback size; do not read the callback pattern as device latency. Output latency
beyond the device buffer was not measured (needs a loopback).

Audio, cpal. 44100 Hz s16 stereo was accepted and the callback got 512 frames (11.6 ms).
cpal 0.18.2 on CoreAudio does not resample: it asks the device for the requested rate.
The built-in output lists 44100, 48000, 88200 and 96000 Hz. Opening a 44100 Hz stream left
the device at 44100 Hz (its default config had read 48000 Hz on one earlier run); opening a
48000 Hz stream moved the device to 48000 Hz and left it there. So there is no evidence of
conversion either way, and a request outside the device's list should fail rather than be
converted. cpal's supported buffer range is 14 to 4096 frames.

## Comparison

Gamepads. SDL3 maps pads onto one positional layout (south/east/west/north, shoulders,
triggers, sticks, d-pad) with hotplug events and rumble (queried per pad through
`SDL_PROP_GAMEPAD_CAP_RUMBLE_BOOLEAN`; no pad here, so no result), and ships 173
built-in macOS mappings. gilrs 0.11.2 bundles its own copy of SDL_GameControllerDB with 315
`platform:Mac OS X` lines, reads pads through IOKit HID, has hotplug events, and its macOS
force-feedback device is an empty function in gilrs-core 0.6.8 (`ff.rs`
`set_ff_state` does nothing), so gilrs cannot rumble on a Mac. I did not compare mapping
quality for DualShock, DualSense, Xbox or 8BitDo pads, and the counts do not show that
either database is better; SDL also recognises many pads through its own HID drivers
rather than the mapping list. My expectation, untested, is that SDL3 handles odd Bluetooth
pads more reliably because it is the reference consumer of that database. For DW1 (digital
pad, optional sticks, no rumble in the original) both are adequate on paper.

Audio model. SDL3 offers a pull callback and a push `put_data` on a stream, with format and
rate conversion included. cpal gives a raw pull callback. It sets the device to the
requested rate when the device supports it and doesn't resample, so an unsupported rate means
resampling ourselves; CoreAudio still converts between the callback's sample format and the
hardware's. For 44.1 kHz SPU output a ring buffer drained by the callback
works with either. SDL's buffer-size hint (`SDL_HINT_AUDIO_DEVICE_SAMPLE_FRAMES`) and cpal's
`BufferSize::Fixed` are requests, not guarantees; steady latency has to be checked on
hardware after choosing one. Note that cpal changes the device's nominal sample rate
system-wide, which other apps can notice.

Window. Fullscreen, resize, HiDPI and a Metal layer are available in both. SDL3 adds display
enumeration with modes and one polled event loop; winit 0.30 is callback driven
(`ApplicationHandler`), and 0.31 changes it again. Neither controls present pacing beyond
what wgpu does. In the winit spike the `gfx` field is declared before `window` so the
raw-handle surface drops first.

Bundling a signed `.app`. Static SDL3: nothing to bundle; only system frameworks are
linked, so the `.app` is the executable plus resources with one ad-hoc `codesign`. Dynamic
SDL3: copy `libSDL3.0.dylib` (links only system libraries; install name is the absolute
Homebrew path) into `Contents/Frameworks`; on the executable run `install_name_tool
-change /opt/homebrew/opt/sdl3/lib/libSDL3.0.dylib @rpath/libSDL3.0.dylib` and
`-add_rpath @executable_path/../Frameworks`; optionally `-id @rpath/libSDL3.0.dylib` on the
dylib; then sign the dylib and the app. winit + cpal + gilrs links only system frameworks,
so it needs nothing extra.

## Recommendation (provisional)

Use SDL3 through the `sdl3` crate with `build-from-source-static`, subject to the hardware
checks below. It costs about 30 s more on a clean build and 2.7 MB of binary, links no
Homebrew path, and makes the `.app` a single signed executable. In the spikes, window,
wgpu/Metal and present timing behaved the same as winit's, so the decision rests on other
things: SDL3 is one library with a rumble-capable gamepad API and an audio stream that
converts rates, against three libraries, 33 more packages, a gilrs macOS backend with no
rumble, and a winit API in transition. The sdl3 crate is younger than winit (pin 0.20.0).
Still to verify on hardware with Ember: real gamepads (DualSense, Xbox, 8BitDo) on both
stacks, output audio latency and callback steadiness, and pacing at 59.94 Hz. If gamepads
or latency turn out equal, the static-linking and single-library advantages are what remain.
