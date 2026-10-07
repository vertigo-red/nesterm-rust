use crate::{Result, renderer::AsciiFrame};
use std::fmt::Write;

pub const ENTER_SEQUENCE: &str = "\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H\x1b[40m\x1b[97m\x1b[>3u";
pub const EXIT_SEQUENCE: &str = "\x1b[<u\x1b[0m\x1b[?25h\x1b[?1049l";

fn validate(frame: &AsciiFrame, color: bool) -> Result<()> {
    if frame.cols == 0 || frame.rows == 0 {
        return Err("terminal frame dimensions must be positive integers".into());
    }
    let cells = frame
        .cols
        .checked_mul(frame.rows)
        .ok_or("terminal dimensions overflow")?;
    if frame.chars.len() != cells {
        return Err(format!("frame.chars must contain exactly {cells} characters").into());
    }
    if frame.chars.iter().any(|c| !(32..=126).contains(c)) {
        return Err("frame.chars must contain printable ASCII only".into());
    }
    if color && frame.colors.len() != cells {
        return Err(format!("frame.colors must contain exactly {cells} colors").into());
    }
    Ok(())
}

fn changed(frame: &AsciiFrame, previous: &AsciiFrame, i: usize, color: bool) -> bool {
    frame.chars[i] != previous.chars[i]
        || (color && (frame.colors[i] & 0xffffff) != (previous.colors[i] & 0xffffff))
}

fn foreground(out: &mut String, color: u32) {
    let _ = write!(
        out,
        "\x1b[38;2;{};{};{}m",
        (color >> 16) & 255,
        (color >> 8) & 255,
        color & 255
    );
}

fn full(frame: &AsciiFrame, color: bool) -> String {
    let mut output = String::from("\x1b[H");
    let mut active_color = None;
    for row in 0..frame.rows {
        for col in 0..frame.cols {
            let i = row * frame.cols + col;
            if color {
                let next = frame.colors[i] & 0xffffff;
                if active_color != Some(next) {
                    foreground(&mut output, next);
                    active_color = Some(next);
                }
            }
            output.push(frame.chars[i] as char);
        }
        if row + 1 < frame.rows {
            output.push_str("\r\n");
        }
    }
    if color {
        output.push_str("\x1b[39m");
    }
    output
}

/// Choose the shorter of a full frame and changed horizontal runs. No output
/// is produced for identical frames; colors are compared only in color mode.
pub fn format_frame(
    frame: &AsciiFrame,
    color: bool,
    previous: Option<&AsciiFrame>,
) -> Result<String> {
    validate(frame, color)?;
    let Some(previous) = previous.filter(|p| p.cols == frame.cols && p.rows == frame.rows) else {
        return Ok(full(frame, color));
    };
    validate(previous, color)?;
    let mut output = String::new();
    let mut active_color = None;
    for row in 0..frame.rows {
        let mut col = 0;
        while col < frame.cols {
            let i = row * frame.cols + col;
            if !changed(frame, previous, i, color) {
                col += 1;
                continue;
            }
            let _ = write!(output, "\x1b[{};{}H", row + 1, col + 1);
            loop {
                let i = row * frame.cols + col;
                if color {
                    let next = frame.colors[i] & 0xffffff;
                    if active_color != Some(next) {
                        foreground(&mut output, next);
                        active_color = Some(next);
                    }
                }
                output.push(frame.chars[i] as char);
                col += 1;
                if col >= frame.cols || !changed(frame, previous, row * frame.cols + col, color) {
                    break;
                }
            }
        }
    }
    if output.is_empty() {
        return Ok(output);
    }
    if color {
        output.push_str("\x1b[39m");
    }
    let full = full(frame, color);
    Ok(if output.len() < full.len() {
        output
    } else {
        full
    })
}
