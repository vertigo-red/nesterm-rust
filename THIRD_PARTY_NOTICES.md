# Third-party notices

This is a Rust port of [kathoc/nesterm](https://github.com/kathoc/nesterm), based on
commit `fd3b83e159fdfcd1497b9ed5dbaa4854d1f17b1a`. Its renderer algorithms,
terminal behavior, and original generated NROM test program are adapted under
MIT, copyright (c) 2026 kathoc. The complete notice is in [LICENSE](LICENSE).

`src/glyphs.rs` contains the original numeric samples from `src/glyphs.mjs`,
converted to Rust arrays. They were sampled from DejaVu Sans Mono. The font
itself is not bundled. Full notices are in [LICENSES-glyphs.txt](LICENSES-glyphs.txt).

`src/text_font.rs` contains the printable Basic Latin rows from Daniel Hepper's
[font8x8_basic.h](https://github.com/dhepper/font8x8/blob/master/font8x8_basic.h),
Git blob `125cf165c93f0bcf954d570aa6e3c84d9b6bacf4`, with bit order reversed
for the renderer. The source declares these VGA bitmaps **Public Domain** and
credits Daniel Hepper, Marcel Sondaar, and International Business Machines.
The declaration is reproduced in [licenses/font8x8-Public-Domain.txt](licenses/font8x8-Public-Domain.txt).
`fonts/cyrillic-demo.json` is an original example supplied under this project's MIT license.

| Runtime dependency | License | Purpose |
|---|---|---|
| [tetanes-core 0.17.0](https://github.com/lukexor/tetanes) | MIT OR Apache-2.0 | Rust NES emulation |
| [crossterm](https://github.com/crossterm-rs/crossterm) | MIT | Raw mode, size, Windows input |
| [ctrlc](https://github.com/Detegr/rust-ctrlc) | MIT OR Apache-2.0 | SIGINT/SIGTERM shutdown |
| [serde_json](https://github.com/serde-rs/json) | MIT OR Apache-2.0 | Asciicast serialization |
| [unicode-width 0.1.14](https://github.com/unicode-rs/unicode-width) | MIT OR Apache-2.0 | Single-column Unicode validation |

`vt100` (MIT) is development-only and independently interprets ANSI output.
`Cargo.lock` records resolved versions and transitive dependencies. Cargo
downloads dependencies under their respective license terms. The TetaNES MIT
notice is also included in `licenses/tetanes-core-MIT.txt` because its published
crate omits the repository's license file.

Release archives include notices for the target's normal and build dependency
graph under `licenses/dependencies/`, with an `INDEX.txt` listing exact versions
and declared licenses. The complete, unmodified `option-ext` crate source is
provided there under MPL-2.0. Rust standard-library notices are copied from the
build toolchain into `licenses/Rust-COPYRIGHT-library.html`. Static Linux builds
also include the musl copyright notice.

The macOS dependencies `block2 0.6.2`, `dispatch2 0.3.1`, `objc2 0.6.5`, and
`objc2-encode 4.1.0` omit their workspace's license files from the published
crates. Their shared [MIT notice](https://github.com/madsmtm/objc2/blob/main/LICENSE-MIT.txt)
is preserved in `licenses/objc2-workspace-MIT.txt`; releases include it under
each dependency's directory as well.

No Node.js runtime, JavaScript NES core, GUI, or commercial ROM is bundled.
