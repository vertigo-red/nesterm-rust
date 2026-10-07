# nesterm-rust

A native Rust port of [kathoc/nesterm](https://github.com/kathoc/nesterm).
Play NES games in a terminal using **only the 95 printable ASCII characters**.
No blocks, Braille, sixel, or graphical UI. The application and its NES core
are Rust; **Node.js is not needed**.

## Build and install

You need Rust **1.88 or newer** and a native linker:

```sh
cargo install --locked --git https://github.com/vertigo-red/nesterm-rust
nesterm "/path/to/game.nes" --size 64x30 --fps 60
```

Or build from a checkout:

```sh
git clone https://github.com/vertigo-red/nesterm-rust.git
cd nesterm-rust
cargo build --locked --release
./target/release/nesterm "/path/to/game.nes" --size 64x30
```

On Windows the executable is `target\release\nesterm.exe`; use Windows Terminal.
Linux and macOS are the primary interactive targets. CI builds and tests all
three platforms and uploads native executables as workflow artifacts. A built
executable does not require Rust or Cargo. Linux builds use the system's glibc.

If the installed command is not found, add `~/.cargo/bin` to your PATH.
Bring your own `.nes` ROM; ROMs are not downloaded or shipped.

## Display and controls

Use a monospaced font and a dark background. The terminal must be at least the
selected size. Shrinking below that size exits cleanly.

| Grid | Pixels per character | Use |
|---|---|---|
| `40x25` (default) | About 6.4 × 9.6 | Compact terminal |
| `64x30` | 4 × 8 | More recognizable sprites and scenery |

| Key | Action |
|---|---|
| Arrows or WASD | D-pad |
| X / Z | A / B |
| Enter | Start |
| Space | Select |
| Shift | Select in Kitty mode on Unix |
| P | Pause / resume |
| Q or Ctrl-C | Quit |

On Unix, the CLI requests the **Kitty keyboard protocol**, including repeat and
release events. Compatible terminals permit simultaneous held buttons.
Traditional input cannot report releases; each legacy button releases after
**120 ms** without a repeat. Windows uses Crossterm console events with the same
timeout fallback. Held-key behavior depends on your terminal.

The terminal display and raw mode are restored on normal exit, input/output
errors, resize failure, SIGINT, and SIGTERM. The CLI is silent. Audio and
save-file persistence are not implemented, matching the original scope.

## Options

```text
nesterm [options] <rom.nes>
```

| Option | Default | Behavior |
|---|---|---|
| `--size 40x25` / `--size 64x30` | `40x25` | Character grid |
| `--mode shape` / `--mode ramp` | `shape` | Shape matching or brightness ramp |
| `--mono` | Color | Disable per-cell ANSI foreground colors |
| `--fps N` | `30` | Maximum redraw rate, positive and at most 240 |
| `--seconds N` | Unlimited | Stop after a positive number of seconds |
| `--record path.cast` | Off | Record actual ANSI output as asciicast v2 |
| `--help` | | Help without opening a ROM |
| `--version` | | Application version |
| `--` | | End options for a ROM path beginning with `-` |

```sh
nesterm game.nes --size 64x30 --fps 60 --record session.cast --seconds 30
```

Recording creates a **new file**. Existing paths are rejected instead of being
overwritten, including if the path points at the ROM. Events contain the bytes
flushed to stdout, including terminal entry and restoration sequences.

The emulation clock is independent of redraws: approximately **60.0988 Hz for
NTSC** and **50.007 Hz for PAL/Dendy**, selected by the core from ROM metadata.
Output is synchronous with bounded input buffering and at most five catch-up
frames per loop.

## Compatibility and differences from JS

The renderer preserves fractional area sampling, RMS luminance, 4×6/4×8 glyph
masks, local shape fitting, color boost, and the character tie preference.
The ANSI formatter chooses the shorter of changed runs and a full frame.

**tetanes-core 0.17.0** replaces `@nesjs/core 2.7.0`. This changes the CPU/PPU/APU
implementation, palette, supported mappers, and region handling. Matching
renderer output on identical framebuffers does **not** guarantee identical
gameplay or compatibility across the cores.

The pinned core rejects trainer-ROMs. NES 2.0 exponent-encoded PRG/CHR sizes are
explicitly rejected before loading because the core interprets sizes in whole
banks. Unsupported mappers and corrupted ROMs produce errors before entering
the alternate screen. Ordinary iNES and supported bank-sized NES 2.0 files are
validated for a complete PRG/CHR payload; CHR-RAM cartridges are supported.

The port handles fragmented input, application-mode arrows, modern Kitty
alternate-key fields and Ctrl-C, region timing, and ignores gameplay keys while
paused. No JavaScript tooling is needed for normal builds or tests.

## Verification

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
python3 scripts/pty-smoke.py target/release/nesterm  # Linux/macOS
```

Tests cover:

- **72 complete reference frames** from the unmodified upstream: both grids,
  modes, color/mono, motion, blanks, overscan, and seeded noise. Characters must
  match exactly; hashes include every 32-bit cell color.
- Generated NROM execution for **120 frames**, PPU palette changes, controller
  state, CHR-RAM, reload behavior, and ROM validation.
- Kitty input split at every byte boundary, releases, simultaneous buttons,
  legacy deadlines, pause controls, and malformed sequences.
- Full/diff convergence in an independent `vt100` parser at exactly 40×25 and
  64×30, short writes, and cleanup after simulated output errors.
- Real POSIX PTYs: sizes, shrink failure, keyboard quit, SIGTERM, pause/resume,
  restored termios settings, and byte-exact asciicast output.

Optional testing with a user-supplied ROM:

```sh
NESTERM_ROM="/path/to/game.nes" cargo test --locked --test emulator \
  optional_user_rom_integration -- --ignored
```

To regenerate reference fixtures, check out the upstream commit documented in
`docs/porting.md` and run `node scripts/generate-golden.mjs /path/to/upstream`.
Node is needed only for this optional generation step.
See [docs/porting.md](docs/porting.md) for design choices and testing limits.

## License

MIT. The original kathoc copyright and DejaVu glyph notices are preserved.
See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
