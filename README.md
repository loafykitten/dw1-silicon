# dw1-silicon

A native Apple Silicon port of **Digimon World** (PlayStation, 1999; USA release
SLUS-01032) for macOS.

It isn't an emulator. The game's own code, recovered as C by the
[dw_decomp](https://github.com/jype0/dw_decomp) decompilation project, is compiled for arm64,
and the PlayStation's libraries and hardware underneath it are replaced by a Mac layer
written in Rust. The game's rules, RNG, scripts, text, music and saves behave as on the
original; improvements like higher resolution, no texture wobble and gamepad support are
options, each labelled as original or added, with the original picture always available.
The long-term goal is the whole game in Rust, migrated one module at a time and checked
against the original at every step.

**Status:** early. The plan is written and reviewed, and setup is under way; nothing is
playable yet. See [PLAN.md](PLAN.md).

## You need your own copy of the game

This repository contains no game data: no disc image, no assets, no text, no music. The
port builds its data on your machine from your own disc image of Digimon World (USA) and
checks it against a list of known hashes. Please don't open issues asking where to get the
game.

## Credits

This port stands on other people's work:

- **[dw_decomp](https://github.com/jype0/dw_decomp)** by jype0 and contributors (MIT): the
  matching decompilation of Digimon World. Every line of the game's logic this port runs
  comes from it. Without it this project couldn't exist.
- **SydMontague**'s [DW1-Code](https://github.com/SydMontague/DW1-Code) and
  [DW1-SydPatches](https://github.com/SydMontague/DW1-SydPatches): years of
  reverse-engineering behind the decomp's names and structures.
- **[DW2-PC](https://github.com/Wyrelade/DW2-PC)** by Wyrelade (CC0): a native PC port of
  Digimon World 2 from its decompilation. Its design (data packs built from the player's
  disc, a hash manifest, overlays linked into one program, a GTE model checked against an
  emulator) is the template for this one.
- **[PCSX-Redux](https://github.com/grumpycoders/pcsx-redux)** by grumpycoders and
  contributors: the emulator we use as the reference, recording inputs and comparing the
  port's state and pictures against the original game.
- **[psx-spx](https://psx-spx.consoledev.net/)** (nocash and contributors): the PlayStation
  hardware documentation.
- **[splat](https://github.com/ethteck/splat)**, **[spimdisasm](https://github.com/Decompollaborate/spimdisasm)**
  and **[rabbitizer](https://github.com/Decompollaborate/rabbitizer)**: the decompilation
  tooling dw_decomp's build relies on.
- **[mkpsxiso / dumpsxiso](https://github.com/Lameguy64/mkpsxiso)** by Lameguy64 and
  contributors: reading files from the disc image.

## License

The code in this repository is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution you intentionally
submit for inclusion in this work, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.

This covers this repository only. dw_decomp (the `decomp/` submodule) is under its own MIT
license, and code adapted from DW2-PC keeps a note of its origin (CC0).

## Legal

Digimon and Digimon World are trademarks of Bandai Namco and Toei Animation. This is an
unofficial fan project, not affiliated with or endorsed by them. This repository contains
none of the game's assets; the decompiled game code comes from dw_decomp (fetched as a git
submodule), and the game data comes from your own disc.
