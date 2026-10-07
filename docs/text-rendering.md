# Hybrid text rendering

`--text auto` runs after the normal shape/ramp renderer. Small recognized
labels become terminal characters; large recognized labels keep their pixel
footprint and become ASCII outlines. `--text off` skips the entire text stage.

## Recognition and confidence

The built-in reference font is the public-domain 8×8 VGA bitmap font. At startup
the renderer looks for ordered `A..Z` and `0..9` sequences in CHR-ROM using that
reference. Discovery is approximate; subsequent frame matching is pixel-exact.
No external process, network service, OCR model, or dictionary is used.

Known layouts can provide explicit tile labels. The verified European SMB3
profile in `src/text/profiles.rs` activates only for 131072 CHR bytes with FNV-1a
fingerprint `f921ce5b9b1d4cd2`. This is a layout identifier, not a security hash.
Its menu font, ordered fonts, and composite word/logo templates are decoded
from the user's ROM. No commercial glyph bitmap or ROM payload is in the source.

Matching supports palette changes, disconnected strokes, and uniform integer
scales. Whole-tile matches additionally check the exact color layout. Component
matching checks every foreground pixel against the chosen template. Adjacent
matches establish a font grid and a text run. Unknown ink between or immediately
beside alphabetic matches prevents a truncated word from being presented as a
complete label. Digit/letter lookalikes (`O/0`, `I/1/l`) need surrounding context;
other conflicting labels are skipped. Verified complete glyphs can stand alone;
heuristically learned glyphs need a longer unambiguous run.

The stage has bounded font and component sizes. Excessively fragmented frames
are left as graphics. The ordinary renderer still handles all unrecognized
regions, including decorative fonts and backgrounds.

## Custom font JSON

Use the actual unscaled bitmap font from your game:

```json
{
  "glyphs": {
    "П": [
      "........",
      ".######.",
      ".##..##.",
      ".##..##.",
      ".##..##.",
      ".##..##.",
      ".##..##.",
      "........"
    ]
  }
}
```

`#` is foreground and `.` is background. Include the normal padding around a
glyph so its slot origin and baseline can be inferred. Each glyph has 1–32 rows
and 1–32 columns, with equal row lengths. Glyphs belonging to the same line
should have a consistent slot height. There must be foreground pixels.

Keys are one printable terminal-column character. Cyrillic, accented Latin,
and other narrow Unicode characters are allowed. Controls, combining marks,
spaces, and wide characters are rejected. There are at most 512 glyphs, and
the JSON file must be no larger than 1 MiB. Custom labels override matching
reference-font shapes. Unsupported shapes remain graphics; supplying Cyrillic
labels does not translate Latin game text.

```sh
nesterm game.nes --size 64x30 --text-font fonts/my-game.json
```

`fonts/cyrillic-demo.json` is an original demonstration font for `Привет`, used
by the synthetic tests. Replace it with your ROM's font mapping for actual use.
Release archives include the example and this document.

## Terminal output

A small label must fit both vertically (one row) and horizontally within its
source region. It is placed at that region's left edge on the middle row;
superseded image cells are cleared. A large label instead uses the boundary
pixels of its actual foreground. Horizontal, vertical and diagonal boundaries
select `_`, `-`, `|`, `/`, or `\`; dense junctions use `#`.

Unicode is stored separately from ordinary ASCII cells. ANSI output validates
single-column width and emits UTF-8. Full frames and differences compare the
visible character, including replacement or removal of a Unicode cell.
Asciicast recording preserves the same UTF-8 output bytes. Use a terminal font
with the desired alphabet; the fixed grid still limits contour readability.
