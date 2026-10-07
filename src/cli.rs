use crate::{Result, renderer::Mode};
use std::{ffi::OsString, path::PathBuf};

pub const HELP: &str = "Usage: nesterm [options] <rom.nes>\n\n\
Play NES games as ASCII art with readable text in your terminal.\n\n\
Options:\n\
  --mono             disable ANSI foreground colors\n\
  --mode <mode>      shape or ramp (default: shape)\n\
  --size <grid>      40x25 or 64x30 (default: 40x25)\n\
  --text <mode>      auto or off (default: auto)\n\
  --text-font <path> additional bitmap font mapping (JSON)\n\
  --fps <number>     maximum redraw rate, up to 240 (default: 30)\n\
  --seconds <number> stop after this many seconds\n\
  --record <path>    record actual ANSI output as asciicast v2\n\
  -h, --help         show help\n\
  -V, --version      show version\n\n\
Keys: arrows/WASD = D-pad; X/Z = A/B; Enter = Start; Space = Select;\n\
      Shift = Select (Kitty); P = pause; Q/Ctrl-C = quit.\n\
Kitty terminals report key releases. Legacy keys release after 120 ms\n\
without a repeat. Emulation runs at the ROM's NTSC/PAL/Dendy frame rate.\n\
Bring your own ROM. Audio and save-file persistence are not implemented.\n";

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub color: bool,
    pub mode: Mode,
    pub cols: usize,
    pub rows: usize,
    pub fps: f64,
    pub seconds: Option<f64>,
    pub record: Option<PathBuf>,
    pub rom: Option<PathBuf>,
    pub help: bool,
    pub version: bool,
    pub text: bool,
    pub text_font: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            color: true,
            mode: Mode::Shape,
            cols: 40,
            rows: 25,
            fps: 30.0,
            seconds: None,
            record: None,
            rom: None,
            help: false,
            version: false,
            text: true,
            text_font: None,
        }
    }
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut out = Self::default();
        let mut args = args.into_iter();
        let mut positional_only = false;
        while let Some(argument) = args.next() {
            let text = argument.to_str();
            if positional_only || text.is_none_or(|s| !s.starts_with('-') || s == "-") {
                if out.rom.replace(PathBuf::from(argument)).is_some() {
                    return Err("exactly one ROM path is required".into());
                }
                continue;
            }
            let argument = text.unwrap();
            match argument {
                "--" => positional_only = true,
                "-h" | "--help" => out.help = true,
                "-V" | "--version" => out.version = true,
                "--mono" => out.color = false,
                "--record" => out.record = Some(value(&mut args, argument)?.into()),
                "--text-font" => out.text_font = Some(value(&mut args, argument)?.into()),
                "--text" => {
                    out.text = match value(&mut args, argument)?.to_str() {
                        Some("auto") => true,
                        Some("off") => false,
                        _ => return Err("--text must be auto or off".into()),
                    }
                }
                "--mode" => {
                    out.mode = match value(&mut args, argument)?.to_str() {
                        Some("shape") => Mode::Shape,
                        Some("ramp") => Mode::Ramp,
                        _ => return Err("--mode must be ramp or shape".into()),
                    }
                }
                "--size" => {
                    (out.cols, out.rows) = match value(&mut args, argument)?.to_str() {
                        Some("40x25") => (40, 25),
                        Some("64x30") => (64, 30),
                        _ => return Err("--size must be 40x25 or 64x30".into()),
                    };
                }
                "--fps" => {
                    out.fps = positive(value(&mut args, argument)?, argument)?;
                    if out.fps > 240.0 {
                        return Err("--fps must not exceed 240".into());
                    }
                }
                "--seconds" => out.seconds = Some(positive(value(&mut args, argument)?, argument)?),
                _ => return Err(format!("unknown option: {argument}").into()),
            }
        }
        if !out.help && !out.version && out.rom.is_none() {
            return Err("a ROM path is required".into());
        }
        if !out.text && out.text_font.is_some() {
            return Err("--text-font requires --text auto".into());
        }
        Ok(out)
    }
}

fn value(args: &mut impl Iterator<Item = OsString>, option: &str) -> Result<OsString> {
    let next = args
        .next()
        .ok_or_else(|| format!("{option} requires a value"))?;
    if next.to_str().is_some_and(|s| s.starts_with("--")) {
        return Err(format!("{option} requires a value").into());
    }
    Ok(next)
}

fn positive(value: OsString, option: &str) -> Result<f64> {
    let number = value.to_str().and_then(|s| s.parse::<f64>().ok());
    match number {
        Some(n) if n.is_finite() && n > 0.0 => Ok(n),
        _ => Err(format!("{option} must be a positive number").into()),
    }
}
