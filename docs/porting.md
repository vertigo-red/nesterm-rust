# Porting decisions

Source: `kathoc/nesterm@fd3b83e159fdfcd1497b9ed5dbaa4854d1f17b1a`.
The destination's initial commit and history are preserved.

## NES core alternatives

| Alternative | Benefit | Cost / trade-off |
|---|---|---|
| `tetanes-core` | Native Rust library, multiple mappers, permissive license | Behavior and palette differ from `@nesjs/core`; game compatibility must be checked separately |
| Also port `@nesjs/core` | Closest route to preserving original CPU/PPU behavior | Much larger than the CLI; new bugs could affect the entire emulation model |
| Minimal custom NROM core | Few dependencies, full control | Excludes MMC1/MMC3 and other mapper games; not a general NES replacement |

The implementation pins `tetanes-core = 0.17.0`, disables audio mixing and the
core's filesystem SRAM behavior, uses unfiltered pixels, and initializes RAM
deterministically. Replacing just the frontend while retaining a JavaScript
NES core would not meet the native distribution goal.

Trainer-ROMs are rejected by the core. NES 2.0 exponent sizes are rejected in the
adapter to prevent interpretation as bank counts. Length validation does not
imply mapper support.

## Renderer

The original glyph samples are embedded Rust constants. The 64×30 grid uses
4×8 masks; 40×25 uses 4×6 masks. Fractional overlaps preserve sub-cell movement.

The source's `Float32Array` rounding points are reproduced: masks, sample
weights, and transformed samples are `f32`; accumulation uses `f64`, like
JavaScript's ordinary arithmetic. The independent JS-generated fixture covers
72 frames, including seeded noise. Rust checks every character and a hash of
all colors. Node is absent from ordinary builds and tests.

Cell luminance and color depend on local pixels, not a global histogram. A
0.055 score preference retains a nearly tied glyph. Pixels are not blended
across time, so moved sprites do not leave trailing copies.

## Terminal

Unix input is parsed independently of rendering, with a bounded channel and
1024-byte carry limit. Kitty release events are explicit; legacy buttons get
120 ms deadlines. Pause clears held buttons and ignores gameplay input until
resume. Windows uses Crossterm console events.

Output uses `write_all` and `flush`, followed by recording the identical string.
Short writes and backpressure do not create an asynchronous frame queue.
RAII cleanup attempts an ANSI reset even after a write failure and always tries
to disable raw mode. Signals set a flag rather than exiting in the handler.

The frame clock follows the ROM region and permits at most five catch-up frames.
Size is checked before reading the ROM, after terminal entry, and continuously.
A real PTY test exposed a race when resizing immediately after entry; checking
the size on every loop closes that race.

## Limits of the evidence

Matching reference frames establishes frontend equivalence for identical
pixels. It does not certify every mapper, commercial game, terminal font, or
CPU/PPU timing case. Generated ROMs demonstrate CPU/PPU execution without
shipping commercial games. `NESTERM_ROM` is an explicitly enabled optional test.

PTY testing exercises Unix interaction and restoration. Native CI also tests
and builds macOS and Windows. Redirected-output tests do not certify all
interactive Windows consoles or physical keyboards. Traditional terminals
retain their lack of true key-release reporting.
