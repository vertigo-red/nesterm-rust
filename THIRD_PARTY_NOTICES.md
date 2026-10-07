# Third-party notices

This is a Rust port of [kathoc/nesterm](https://github.com/kathoc/nesterm), based on
commit `fd3b83e159fdfcd1497b9ed5dbaa4854d1f17b1a`. Its renderer algorithms,
terminal behavior, and original generated NROM test program are adapted under
MIT, copyright (c) 2026 kathoc. The complete notice is in [LICENSE](LICENSE).

`src/glyphs.rs` contains the original numeric samples from `src/glyphs.mjs`,
converted to Rust arrays. They were sampled from DejaVu Sans Mono. The font
itself is not bundled. Full notices are in [LICENSES-glyphs.txt](LICENSES-glyphs.txt).

| Runtime dependency | License | Purpose |
|---|---|---|
| [tetanes-core 0.17.0](https://github.com/lukexor/tetanes) | MIT OR Apache-2.0 | Rust NES emulation |
| [crossterm](https://github.com/crossterm-rs/crossterm) | MIT | Raw mode, size, Windows input |
| [ctrlc](https://github.com/Detegr/rust-ctrlc) | MIT OR Apache-2.0 | SIGINT/SIGTERM shutdown |
| [serde_json](https://github.com/serde-rs/json) | MIT OR Apache-2.0 | Asciicast serialization |

`vt100` (MIT) is development-only and independently interprets ANSI output.
`Cargo.lock` records resolved versions and transitive dependencies. Cargo
downloads dependencies under their respective license terms. The TetaNES MIT
notice is also included in `licenses/tetanes-core-MIT.txt` because its published
crate omits the repository's license file.

No Node.js runtime, JavaScript NES core, GUI, or commercial ROM is bundled.
