use nesterm_rust::{
    ansi::format_frame,
    renderer::{AsciiRenderer, Mode, PIXELS, WIDTH},
    text::TextRenderer,
};

const FONT: &str = include_str!("../fonts/cyrillic-demo.json");

fn word(text: &str, scale: usize, color: u32, background: u32) -> Vec<u32> {
    let font: serde_json::Value = serde_json::from_str(FONT).unwrap();
    let mut pixels = vec![background; PIXELS];
    for (i, c) in text.chars().enumerate() {
        let rows = font["glyphs"][c.to_string()].as_array().unwrap();
        for (y, row) in rows.iter().enumerate() {
            for (x, bit) in row.as_str().unwrap().bytes().enumerate() {
                if bit == b'#' {
                    for yy in 0..scale {
                        for xx in 0..scale {
                            pixels[(48 + y * scale + yy) * WIDTH
                                + 32
                                + i * 8 * scale
                                + x * scale
                                + xx] = color;
                        }
                    }
                }
            }
        }
    }
    pixels
}

#[test]
fn small_cyrillic_word_is_literal_text_in_both_grids_and_terminal_parser() {
    for (cols, rows) in [(40, 25), (64, 30)] {
        for (ink, background) in [(0xff1122ee, 0xff000000), (0xff000000, 0xffffffff)] {
            let pixels = word("Привет", 1, ink, background);
            let mut text = TextRenderer::new(&[], Some(FONT)).unwrap();
            let mut renderer = AsciiRenderer::new(cols, rows, Mode::Shape, true).unwrap();
            let mut frame = renderer.render(&pixels).unwrap();
            let runs = text.apply(&pixels, &mut frame, true).unwrap();
            assert!(runs.iter().any(|r| r.text == "Привет"), "{runs:?}");
            assert!(frame.text().contains("Привет"), "{}", frame.text());
            let mut parser = vt100::Parser::new(rows as u16, cols as u16, 0);
            parser.process(format_frame(&frame, true, None).unwrap().as_bytes());
            assert!(parser.screen().contents().contains("Привет"));
            for (index, &c) in &frame.unicode {
                let cell = parser
                    .screen()
                    .cell((index / cols) as u16, (index % cols) as u16)
                    .unwrap();
                assert_eq!(cell.contents(), c.to_string());
                let rgb = frame.colors[*index];
                assert_eq!(
                    cell.fgcolor(),
                    vt100::Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
                );
            }
        }
    }
}

#[test]
fn large_letters_keep_their_footprint_and_use_ascii_contours() {
    let pixels = word("Привет", 3, 0xffffffff, 0xff000000);
    let mut text = TextRenderer::new(&[], Some(FONT)).unwrap();
    let mut renderer = AsciiRenderer::new(64, 30, Mode::Shape, false).unwrap();
    let mut frame = renderer.render(&pixels).unwrap();
    let runs = text.apply(&pixels, &mut frame, false).unwrap();
    let run = runs.iter().find(|r| r.text == "Привет").unwrap();
    assert_eq!(run.height, 24);
    assert!(frame.unicode.is_empty());
    assert!(!frame.text().contains("Привет"));
    let left = run.bounds.x * 64 / 256;
    let right = ((run.bounds.x + run.bounds.width) * 64).div_ceil(256);
    let top = run.bounds.y * 30 / 240;
    let bottom = ((run.bounds.y + run.bounds.height) * 30).div_ceil(240);
    let contour_rows: Vec<_> = (top..bottom)
        .map(|row| &frame.chars[row * 64 + left..row * 64 + right])
        .collect();
    assert!(contour_rows.len() >= 3);
    assert!(
        contour_rows
            .iter()
            .all(|row| row.iter().all(|&c| b" |_-/#\\".contains(&c)))
    );
    assert!(
        contour_rows
            .iter()
            .filter(|row| row.iter().any(|&c| c != b' '))
            .count()
            >= 3
    );
    assert!(
        contour_rows
            .iter()
            .flat_map(|row| row.iter())
            .any(|&c| b"|_-/\\".contains(&c))
    );
}

#[test]
fn an_unknown_letter_cannot_be_silently_replaced_with_a_space() {
    let mut pixels = word("Привет", 1, 0xffffffff, 0xff000000);
    // Replace the third glyph with an unsupported solid shape.
    for y in 48..56 {
        for x in 48..56 {
            pixels[y * WIDTH + x] = 0xff000000;
        }
    }
    for y in 49..55 {
        for x in 49..54 {
            pixels[y * WIDTH + x] = 0xffffffff;
        }
    }
    let mut text = TextRenderer::new(&[], Some(FONT)).unwrap();
    let runs = text.recognize(&pixels).unwrap();
    assert!(
        !runs
            .iter()
            .any(|r| r.text.contains("Пр вет") || r.text == "Привет"),
        "{runs:?}"
    );
}

#[test]
fn a_trusted_short_label_can_be_one_or_two_characters() {
    for label in ["П", "Пи"] {
        let pixels = word(label, 1, 0xffffffff, 0xff000000);
        let mut text = TextRenderer::new(&[], Some(FONT)).unwrap();
        let runs = text.recognize(&pixels).unwrap();
        assert!(runs.iter().any(|r| r.text == label), "{runs:?}");
    }
}

#[test]
fn empty_frames_noise_and_invalid_destinations_are_conservative() {
    let mut text = TextRenderer::new(&[], Some(FONT)).unwrap();
    assert!(text.recognize(&vec![0; PIXELS]).unwrap().is_empty());
    let noise: Vec<u32> = (0..PIXELS)
        .map(|i| (i as u32).wrapping_mul(0x9e3779b9))
        .collect();
    assert!(text.recognize(&noise).unwrap().is_empty());
    assert!(text.recognize(&[]).is_err());
    let mut renderer = AsciiRenderer::new(64, 30, Mode::Ramp, false).unwrap();
    let mut frame = renderer.render(&vec![0; PIXELS]).unwrap();
    frame.cols = usize::MAX;
    assert!(text.apply(&vec![0; PIXELS], &mut frame, false).is_err());
}

#[test]
fn unsafe_or_malformed_font_mappings_are_rejected() {
    for json in [
        r###"{"glyphs": {}}"###,
        r###"{"glyphs": {"ab": ["#"]}}"###,
        r###"{"glyphs": {"\u001b": ["#"]}}"###,
        r###"{"glyphs": {"界": ["#"]}}"###,
        r###"{"glyphs": {"\u0301": ["#"]}}"###,
        r###"{"glyphs": {" ": ["#"]}}"###,
        r###"{"glyphs": {"a": ["##", "#"]}}"###,
        r###"{"glyphs": {"a": ["xx"]}}"###,
        r###"{"glyphs": {"a": [".."]}}"###,
        "{",
    ] {
        assert!(TextRenderer::new(&[], Some(json)).is_err(), "{json}");
    }
    assert!(TextRenderer::new(&[], Some(&" ".repeat(1024 * 1024 + 1))).is_err());
}

#[test]
#[ignore = "requires NESTERM_ROM pointing to the user's SMB3 European ROM"]
fn optional_mario_text_and_logo_integration() {
    use nesterm_rust::{
        emulator::Emulator,
        input::{Button, Gamepad},
    };
    let rom = std::fs::read(std::env::var_os("NESTERM_ROM").expect("set NESTERM_ROM")).unwrap();
    let mut emulator = Emulator::new();
    emulator.load(&rom).unwrap();
    let mut text = TextRenderer::new(&rom, None).unwrap();
    for frame in 1..=500 {
        if frame == 360 {
            emulator.set_button(Button::Start, true);
        }
        if frame == 365 {
            emulator.set_button(Button::Start, false);
        }
        emulator.frame().unwrap();
    }
    let mut renderer = AsciiRenderer::new(64, 30, Mode::Shape, true).unwrap();
    let mut frame = renderer.render(emulator.pixels()).unwrap();
    let runs = text.apply(emulator.pixels(), &mut frame, true).unwrap();
    for expected in [
        "1 PLAYER GAME",
        "2 PLAYER GAME",
        "SUPER",
        "MARIO BROS.",
        "3",
        "Nintendo",
    ] {
        assert!(
            runs.iter().any(|r| r.text == expected),
            "missing {expected}: {runs:?}"
        );
    }
    assert!(frame.text().contains("1 PLAYER GAME"));
    assert!(frame.text().contains("2 PLAYER GAME"));
    assert!(!frame.text().contains("SUPER"));
    assert!(frame.text().contains("Nintendo"));

    for _ in 501..=650 {
        emulator.frame().unwrap();
    }
    let runs = text.recognize(emulator.pixels()).unwrap();
    assert!(!runs.iter().any(|r| r.text == "YER GAME"), "{runs:?}");
}
