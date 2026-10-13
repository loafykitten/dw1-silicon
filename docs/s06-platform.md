# S0.6: window, input and audio, SDL3 (Rust) vs winit + cpal

Spikes: `tools/spikes/s06/sdl3` and `tools/spikes/s06/winit-cpal` (standalone packages, own
`[workspace]`, not in the main workspace; throwaway). Both open a 960x720 window, make a
`wgpu` 30.0.1 Metal surface from the raw window handle, clear a colour and draw one
triangle at Fifo (vsync), log keyboard events, play a 440 Hz tone at 44100 Hz stereo s16 for
about one second, and time 300 presents. Run on an M2 Max, macOS 27, from a shell. Windows
opened and presented (the program ran 300 frames at the display's rate), but nobody could see
the screen or hear the tone, so "the triangle is drawn" and "the tone is audible" are
unverified; only that the calls succeeded and the callbacks ran. No gamepad was connected:
both report "gamepads found at startup: 0", so hotplug and button events were never
exercised.

## What each needed

| | sdl3 (dynamic) | sdl3 (static) | winit + cpal + gilrs |
|---|---|---|---|
| Crates | sdl3 0.20.0 (sdl3-sys 0.7.2+SDL-3.4.18), wgpu 30.0.1, pollster | same, `build-from-source-static` | winit 0.30.13, cpal 0.18.2, gilrs 0.11.2, wgpu 30.0.1, pollster |
| Crates in the normal dependency tree | 84 | not counted | 124 |
| Clean release build (deps cached in the registry) | 18.5 s | 47.8 s | 18.8 s |
| Release binary | 5.7 MB | 8.4 MB | 6.8 MB |
| Non-system libraries linked | `/opt/homebrew/opt/sdl3/lib/libSDL3.0.dylib` (2.5 MB, 3.4.18) | none | none |

Notes. winit is 0.30.13, not the 0.31 beta that crates.io lists as latest; 0.31 changes the
application API and is still beta. The spike links the system SDL3 with the `use-pkg-config` feature (I did not try the
crate's default features or `link-framework`), and `--features static` switches to the vendored
build, which compiles SDL 3.4.18 from source with the system clang (needs cmake, which is
installed). The sdl3 crate's `raw-window-handle` support on macOS requires
`.metal_view()` on the window builder, which is easy to miss. Both hit a few wgpu 30 API
changes (`InstanceDescriptor::new_without_display_handle`, `queue.present`), which are
wgpu, not the toolkit.

## Measured

Refresh and present intervals, 300 frames after 20 warm-up frames, Fifo present mode. This
machine has two displays: a 2560x1440 at 240 Hz and a 1440x2560 portrait at 75 Hz. A window
without a set position lands on either, which first muddied the comparison, so both spikes
take `S06_HZ=240|75` to place the window on that display. There is no 60 Hz display here, so
DW1's 59.94 Hz case could not be tested.

| Display | sdl3 mean (stddev) | winit mean (stddev) |
|---|---|---|
| 75 Hz, 3 runs each | 13.34 ms (0.12 to 0.16) | 13.34 ms (0.17 to 0.43) |
| 240 Hz, 3 runs each | 4.31 to 4.47 ms (0.8 to 2.8) | 4.24 to 4.35 ms (0.8 to 1.0) |

At 75 Hz both lock exactly to the display. At 240 Hz both average about 4.3 to 4.5 ms
(225 to 235 Hz) against 4.17 ms, with min intervals down to 1 to 2 ms and occasional spikes
to 8 to 48 ms. That points at wgpu's drawable queue (up to 3 drawables, so a few presents
return early) and not at either toolkit; the windowed numbers are the same shape in both.
Pacing for the game will be our own (a fixed 60 Hz, or 59.94, step with its own clock), not
"one present per vsync", so this matters less than it looks. One winit run on the unpinned
window had a 1 s stall (window on the other display or occluded); not reproduced.

Window scale: both reported 1.00 scale (960x720 points = 960x720 pixels) on these external
displays; neither could be tested on a Retina panel. Both expose the HiDPI scale and
pixel size (SDL: `high_pixel_density()`, `pixel_density`, `size_in_pixels`; winit:
`scale_factor`, `inner_size`). SDL3 lists displays with name, bounds and refresh rate
(`displays()`, `get_mode()`). winit lists monitors with `refresh_rate_millihertz()`; for the
window's monitor the value matched the real display.

Audio: both asked for 44100 Hz stereo s16 and got it. SDL3 opens the device at its native
format and converts inside the stream (so 44100 is always accepted, resampled if the device is
at 48000). cpal on CoreAudio accepted 44100 directly on the built-in output (the device's
default config was 48000 f32 in one run and 44100 in another; CoreAudio itself converts
between the app format and the device). Both called back with 512 frames (11.6 ms) by
default. cpal reports the supported buffer range as 14 to 4096 frames and can ask for a fixed
size; SDL3 takes a sample-frames hint. Actual output latency (buffer plus device) was not
measured; that needs a loopback, which this session cannot do.

## Comparison

Gamepads. SDL3's gamepad API normalises DualShock 4, DualSense, Xbox, Switch Pro and 8BitDo
pads to one layout (south/east/west/north, shoulders, triggers, sticks, d-pad, guide) through
its maintained mapping database, and exposes hotplug events, rumble (including trigger
rumble), LEDs and gyro on pads that have them; on macOS it uses GameController.framework plus
IOKit HID. That is the part that matters most for rebinding, since the layout is stable and
the names (`Button::South`) are positional. `gilrs` also ships an SDL-format mapping
database and has hotplug events and a button/axis event model, but its mapping data lags
SDL's, and its force-feedback support is, as far as I know, Linux and Windows only (not
verified on macOS here), so rumble on a Mac needs something else. Neither was tested with
real hardware. For DW1 (digital pad, optional sticks, no rumble in the original) both are
adequate; SDL3 is the safer for odd 8BitDo or Bluetooth pads.

Audio. SDL3 gives a pull callback on an audio stream (it also supports push with
`put_data`) and does format and rate conversion for us. cpal gives only a raw callback at the
device's chosen format, so any rate conversion or format change is ours; for 44.1 kHz SPU
output this mostly works because CoreAudio converts, but a 48 kHz device would mean we either
resample or accept CoreAudio's conversion. A ring buffer fed by the SPU thread, drained in the
callback, is the model for either; steady latency comes from picking a fixed callback size
(SDL hint, cpal `BufferSize::Fixed`).

Window. Fullscreen (borderless and exclusive display modes), resize, HiDPI and a Metal view
are available in both. SDL3 also gives display enumeration with real modes, relative mouse,
window flags, a text-input API, and one event loop we poll; winit 0.30 is callback-driven
(`ApplicationHandler`) and 0.31 changes it again, so winit pulls the game loop into its shape
and the churn between 0.30 and 0.31 is a real maintenance cost. Both give a `CAMetalLayer`
for wgpu through raw-window-handle; neither controls present pacing beyond what wgpu does.

Bundling a signed `.app`. Static SDL3: nothing to bundle; the binary links only system
frameworks (8.4 MB, a minute from clean), and the whole `.app` is the executable and
resources, ad-hoc signed with one `codesign`. Dynamic SDL3: copy `libSDL3.0.dylib` from
Homebrew (it links only system libraries; its install name is the absolute Homebrew path) to
`Contents/Frameworks`, `install_name_tool -change` the executable's reference to
`@rpath/libSDL3.0.dylib` (or `-id` the dylib), add `-add_rpath @executable_path/../Frameworks`,
then sign the dylib and the app (inside-out, with the hardened runtime if we ever notarize).
That works but is an extra step that can drift from the Homebrew version; the static build
removes it. winit + cpal + gilrs link only system frameworks already, so it needs nothing
extra.

## Recommendation

Use SDL3 through the `sdl3` crate with `build-from-source-static`: it costs about 30 extra
seconds on a clean build and 2.7 MB of binary, links no Homebrew path, and makes the `.app`
a single signed executable. It gives the strongest answer to the game's real needs: a
positional, database-backed gamepad layer with hotplug and rumble for the later rebinding UI,
a pull audio stream that accepts 44.1 kHz regardless of the device, and display and
fullscreen control, all behind one polled event loop that fits a fixed-step game. Window
creation and wgpu/Metal presenting behaved identically in the two spikes, and the measured
present pacing was the same, so winit has no advantage there while costing about 40 more
crates, a split across three crates for window, audio and gamepads, and a beta API
transition. Caveats to carry forward: the sdl3 crate is a young, smaller project than winit
(pin 0.20.0, expect to read its source), `.metal_view()` is required for the surface, and
present pacing at 59.94 Hz and real gamepads and audio latency still need checking on
hardware once Ember is at the screen.
