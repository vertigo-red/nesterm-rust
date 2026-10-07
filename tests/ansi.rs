use nesterm_rust::{ansi::format_frame, renderer::AsciiFrame};

fn solid(cols: usize, rows: usize) -> AsciiFrame {
    AsciiFrame {
        cols,
        rows,
        chars: vec![b'.'; cols * rows],
        colors: vec![0x123456; cols * rows],
        unicode: Default::default(),
    }
}

fn assert_screen(parser: &vt100::Parser, frame: &AsciiFrame, color: bool) {
    for row in 0..frame.rows {
        for col in 0..frame.cols {
            let cell = parser.screen().cell(row as u16, col as u16).unwrap();
            let i = row * frame.cols + col;
            assert_eq!(
                cell.contents(),
                frame.character(i).to_string(),
                "cell={row},{col}"
            );
            if color {
                let c = frame.colors[i];
                assert_eq!(
                    cell.fgcolor(),
                    vt100::Color::Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
                );
            }
        }
    }
}

#[test]
fn full_and_diff_converge_in_independent_vt100_parser_at_exact_size() {
    for (cols, rows) in [(40, 25), (64, 30)] {
        for color in [true, false] {
            let first = solid(cols, rows);
            let mut next = first.clone();
            next.chars[0] = b'@';
            next.colors[0] = 0xff0000;
            next.chars[cols + 8] = b'X';
            next.chars[cols + 9] = b'Y';
            next.colors[cols + 9] = 0x00ff00;
            let last = cols * rows - 1;
            next.chars[last] = b'#';
            next.colors[last] = 0x0000ff;
            let mut parser = vt100::Parser::new(rows as u16, cols as u16, 0);
            parser.process(format_frame(&first, color, None).unwrap().as_bytes());
            assert_screen(&parser, &first, color);
            let diff = format_frame(&next, color, Some(&first)).unwrap();
            assert!(diff.len() < format_frame(&next, color, None).unwrap().len());
            parser.process(diff.as_bytes());
            assert_screen(&parser, &next, color);
            assert!(format_frame(&next, color, Some(&next)).unwrap().is_empty());
        }
    }
}

#[test]
fn scattered_changes_choose_the_shorter_full_frame() {
    let first = solid(40, 25);
    let mut next = first.clone();
    for i in (0..next.chars.len()).step_by(2) {
        next.chars[i] = b'X';
    }
    assert_eq!(
        format_frame(&next, false, Some(&first)).unwrap(),
        format_frame(&next, false, None).unwrap()
    );
}

#[test]
fn color_only_changes_and_dimension_changes_are_handled() {
    let first = solid(40, 25);
    let mut next = first.clone();
    next.colors[100] = 0xff00ff;
    assert!(!format_frame(&next, true, Some(&first)).unwrap().is_empty());
    assert!(format_frame(&next, false, Some(&first)).unwrap().is_empty());
    let large = solid(64, 30);
    assert_eq!(
        format_frame(&large, true, Some(&first)).unwrap(),
        format_frame(&large, true, None).unwrap()
    );
}

#[test]
fn malicious_cells_and_inconsistent_lengths_are_rejected() {
    let mut frame = solid(40, 25);
    frame.chars[12] = 27;
    assert!(format_frame(&frame, true, None).is_err());
    frame = solid(40, 25);
    frame.chars.pop();
    assert!(format_frame(&frame, false, None).is_err());
    frame = solid(40, 25);
    frame.colors.pop();
    assert!(format_frame(&frame, true, None).is_err());
    assert!(format_frame(&frame, false, None).is_ok());
    frame.cols = 0;
    assert!(format_frame(&frame, false, None).is_err());
}

#[test]
fn unicode_changes_removals_and_last_column_converge_without_ghosts() {
    let first = solid(40, 25);
    let mut next = first.clone();
    for (i, c) in "Привет".chars().enumerate() {
        next.put_character(39 + i, c);
    }
    next.put_character(999, 'Я');
    let mut parser = vt100::Parser::new(25, 40, 0);
    parser.process(format_frame(&first, true, None).unwrap().as_bytes());
    parser.process(format_frame(&next, true, Some(&first)).unwrap().as_bytes());
    assert_screen(&parser, &next, true);
    let mut changed = next.clone();
    changed.put_character(39, 'Р');
    changed.put_character(40, ' ');
    changed.put_character(999, '!');
    parser.process(
        format_frame(&changed, true, Some(&next))
            .unwrap()
            .as_bytes(),
    );
    assert_screen(&parser, &changed, true);
    parser.process(
        format_frame(&first, true, Some(&changed))
            .unwrap()
            .as_bytes(),
    );
    assert_screen(&parser, &first, true);
}

#[test]
fn unicode_cells_reject_controls_wide_and_combining_characters() {
    for c in ['\u{001b}', '\u{0085}', '\u{0301}', '界', '😀'] {
        let mut frame = solid(40, 25);
        frame.unicode.insert(5, c);
        assert!(format_frame(&frame, false, None).is_err(), "{c:?}");
    }
    let mut frame = solid(40, 25);
    frame.unicode.insert(1000, 'П');
    assert!(format_frame(&frame, false, None).is_err());
}
