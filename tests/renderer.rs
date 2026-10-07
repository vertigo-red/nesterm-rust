use nesterm_rust::renderer::{AsciiRenderer, Mode, PIXELS, ShapeMatcher};

fn pixels(kind: &str) -> Vec<u32> {
    let mut out = vec![0; PIXELS];
    match kind {
        "black" => {}
        "white" => out.fill(0xffffff),
        "blue" => out.fill(0x2070e0),
        "patch" | "shift1" | "shift4" => {
            let shift = if kind == "shift1" {
                1
            } else if kind == "shift4" {
                4
            } else {
                0
            };
            for y in 72..112 {
                for x in 64 + shift..112 + shift {
                    out[y * 256 + x] = if x % 4 < 2 { 0x2070e0 } else { 0xe08020 };
                }
            }
            for y in 91..103 {
                for x in 178 + shift..182 + shift {
                    out[y * 256 + x] = 0xffffff;
                }
            }
        }
        "overscan" => {
            out.fill(0x88eeaa);
            out[210 * 256..].fill(0);
        }
        "noise" => {
            let mut state = 0x5eedu32;
            for p in &mut out {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                *p = state & 0xffffff;
            }
        }
        _ => panic!("unknown fixture"),
    }
    out
}

fn color_hash(colors: &[u32]) -> u32 {
    colors
        .iter()
        .flat_map(|c| c.to_le_bytes())
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

#[test]
fn matches_original_js_renderer_for_72_complete_frames() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/renderer-golden.json")).unwrap();
    for group in fixtures["groups"].as_array().unwrap() {
        let cols = group["cols"].as_u64().unwrap() as usize;
        let rows = group["rows"].as_u64().unwrap() as usize;
        let mode = if group["mode"] == "shape" {
            Mode::Shape
        } else {
            Mode::Ramp
        };
        let color = group["color"].as_bool().unwrap();
        let mut renderer = AsciiRenderer::new(cols, rows, mode, color).unwrap();
        for case in group["frames"].as_array().unwrap() {
            let kind = case["kind"].as_str().unwrap();
            let frame = renderer.render(&pixels(kind)).unwrap();
            assert_eq!(
                std::str::from_utf8(&frame.chars).unwrap(),
                case["chars"].as_str().unwrap(),
                "{cols}x{rows} {mode:?} color={color} {kind}"
            );
            assert_eq!(
                color_hash(&frame.colors) as u64,
                case["colorsHash"].as_u64().unwrap(),
                "color: {cols}x{rows} {mode:?} color={color} {kind}"
            );
            assert!(frame.chars.iter().all(|c| (32..=126).contains(c)));
            assert_eq!(frame.text().lines().count(), rows);
        }
    }
}

#[test]
fn fractional_motion_changes_glyph_before_whole_cell_boundary() {
    let mut renderer = ShapeMatcher::new(40, 25).unwrap();
    let mut outputs = std::collections::HashSet::new();
    for shift in 0..6 {
        let mut frame = vec![0; PIXELS];
        for y in 91..103 {
            for x in 78 + shift..82 + shift {
                frame[y * 256 + x] = 0xffffff;
            }
        }
        outputs.insert(renderer.render(&frame, false).unwrap().chars);
    }
    assert!(outputs.len() >= 3);
}

#[test]
fn stable_frames_reset_and_motion_have_no_trailing_ghosts() {
    let mut renderer = AsciiRenderer::new(64, 30, Mode::Shape, true).unwrap();
    let frame = pixels("patch");
    let first = renderer.render(&frame).unwrap();
    assert_eq!(first, renderer.render(&frame).unwrap());
    renderer.reset();
    assert_eq!(first, renderer.render(&frame).unwrap());
    let blank = renderer.render(&pixels("black")).unwrap();
    assert!(blank.chars.iter().all(|c| *c == b' '));
    assert!(blank.colors.iter().all(|c| *c == 0));
}

#[test]
fn distant_scene_colors_do_not_change_local_glyph_or_color() {
    for (cols, rows) in [(40, 25), (64, 30)] {
        let mut renderer = ShapeMatcher::new(cols, rows).unwrap();
        let mut outputs = Vec::new();
        for background in [0x2070e0, 0xe08020, 0x2070e0, 0xffffff, 0] {
            let mut frame = vec![background; PIXELS];
            for y in 72..112 {
                for x in 64..112 {
                    frame[y * 256 + x] = if x % 4 < 2 { 0x2070e0 } else { 0xe08020 };
                }
            }
            let out = renderer.render(&frame, true).unwrap();
            let cell = (88 * rows / 240) * cols + 84 * cols / 256;
            outputs.push((out.chars[cell], out.colors[cell]));
        }
        assert!(outputs.iter().all(|o| *o == outputs[0]));
    }
}

#[test]
fn an_eight_by_eight_tile_spans_two_cells_in_large_grid() {
    let mut renderer = AsciiRenderer::new(64, 30, Mode::Shape, true).unwrap();
    let mut frame = vec![0; PIXELS];
    for y in 80..88 {
        for x in 80..88 {
            frame[y * 256 + x] = 0xffffff;
        }
    }
    let out = renderer.render(&frame).unwrap();
    let occupied: Vec<_> = out
        .chars
        .iter()
        .enumerate()
        .filter(|(_, c)| **c != b' ')
        .map(|(i, _)| i)
        .collect();
    assert_eq!(occupied, vec![660, 661]);
}

#[test]
fn dimensions_and_framebuffer_are_validated() {
    for (cols, rows) in [(0, 25), (40, 0), (257, 25), (40, 241)] {
        assert!(AsciiRenderer::new(cols, rows, Mode::Shape, true).is_err());
    }
    let mut renderer = AsciiRenderer::new(40, 25, Mode::Shape, true).unwrap();
    assert!(renderer.render(&[0; 3]).is_err());
}
