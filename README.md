# nesterm-rust

A native Rust port of [kathoc/nesterm](https://github.com/kathoc/nesterm).
Play NES games as ASCII art, with **readable text for recognized fonts**.
Graphics use the 95 printable ASCII characters; recognized labels can also use
single-column Unicode, including Cyrillic. No blocks, Braille, sixel, or graphical UI. The application and its NES core
are Rust; **Node.js is not needed**.

## Download and run

Download a ready-to-run archive from [Releases](https://github.com/vertigo-red/nesterm-rust/releases/latest).
No Rust, Cargo, or Node.js installation is needed.

| System | File suffix |
|---|---|
| Linux x86-64 | `x86_64-unknown-linux-musl.tar.gz` (static; no glibc dependency) |
| macOS Apple Silicon | `aarch64-apple-darwin.tar.gz` |
| macOS Intel | `x86_64-apple-darwin.tar.gz` |
| Windows x86-64 | `x86_64-pc-windows-msvc.zip` |

Extract the archive and open a terminal in its directory:

```sh
./nesterm "/path/to/game.nes" --size 64x30 --fps 60
```

On Windows use Windows Terminal and `./nesterm.exe "C:/path/to/game.nes"`.
macOS binaries target macOS 13 or newer and are tested on macOS 15. They are
not Apple-signed or notarized, so the first launch may require approval in
System Settings → Privacy & Security. Each release includes `SHA256SUMS`
for verifying downloads, full license notices, and no game ROMs.

## Build from source

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
three platforms and uploads native executables as workflow artifacts. Normal
Linux source builds use the system's glibc; release archives use static musl.

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
| `--text auto` / `--text off` | `auto` | Readable small text and ASCII contours for recognized large text |
| `--text-font path.json` | Off | Add exact bitmap-to-character mappings, including Cyrillic |
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

## Readable text

Hybrid rendering is enabled by default. A confidently matched label is printed
as ordinary characters when its font fits within one terminal row and its text
fits within the original horizontal area. Larger recognized text is drawn as
ASCII contours using `|`, `_`, `-`, `/`, `\\`, and `#`, preserving its size and
position. An unknown or partly obscured word stays in the usual graphics mode.
Contour detail is limited by the selected grid; prefer `64x30`.

```powershell
.\nesterm.exe ".\Super Mario Bros. 3 (Europe).nes" --size 64x30 --text auto
```

The European Mario 3 ROM supplied for development was verified: the player
menu, copyright digits, `Nintendo`, `WORLD`, score and timer are readable; the
large `SUPER`, `MARIO BROS.` and `3` use contours. The profile is selected by an
exact CHR fingerprint. Its source contains tile locations and labels, and reads
the font graphics from your ROM at startup. Other revisions and ROM hacks may
have different font layouts.

This is exact font matching rather than a general OCR model. The renderer also
tries to find ordered Latin alphabets and digit sets in CHR-ROM; their discovery
is heuristic, and coverage varies by game. Arbitrary stylized fonts, CHR-RAM
fonts, non-integer scaling, and partially hidden letters need additional font
mappings or remain graphics. No words are translated or completed from a dictionary.

For a game using a Cyrillic bitmap font, provide its actual glyph shapes with
`--text-font my-font.json`. The included `fonts/cyrillic-demo.json` demonstrates
`Привет`; it is a format example, not a font map for every Russian ROM. See
[docs/text-rendering.md](docs/text-rendering.md) for the format and matching rules.
Use `--text off` to select the original v0.1 ASCII renderer.

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
- Literal Cyrillic text, scaled ASCII contours, unknown-letter fallback, safe
  Unicode cells, font validation, and a user-enabled Mario 3 text/profile test.
- Real POSIX PTYs: sizes, shrink failure, keyboard quit, SIGTERM, pause/resume,
  restored termios settings, and byte-exact asciicast output.

Optional testing with a user-supplied ROM:

```sh
NESTERM_ROM="/path/to/game.nes" cargo test --locked --test emulator \
  optional_user_rom_integration -- --ignored
```

To verify the supported European Mario 3 profile with your own ROM:

```sh
NESTERM_ROM="/path/to/Super Mario Bros. 3 (Europe).nes" cargo test --locked --release \
  --test text optional_mario_text_and_logo_integration -- --ignored
```

To regenerate reference fixtures, check out the upstream commit documented in
`docs/porting.md` and run `node scripts/generate-golden.mjs /path/to/upstream`.
Node is needed only for this optional generation step.
See [docs/porting.md](docs/porting.md) for design choices and testing limits.

## License

MIT. The original kathoc copyright and DejaVu glyph notices are preserved.
See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
