use crate::{
    Result,
    input::{Button, Gamepad},
    renderer::PIXELS,
};
use std::{io::Cursor, time::Duration};
use tetanes_core::{
    common::NesRegion,
    control_deck::{Config, ControlDeck, HeadlessMode},
    input::{JoypadBtn, Player},
    memory::RamState,
    video::VideoFilter,
};

/// Checks the entire PRG/CHR payload before the core can access a truncated ROM.
/// NES 2.0 extended sizes, exponent encoding, and iNES trainers are included.
pub fn validate_rom(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 16 || &bytes[..4] != b"NES\x1a" {
        return Err("Select a valid iNES ROM file.".into());
    }
    let nes2 = bytes[7] & 0x0c == 8;
    let prg = rom_size(bytes[4], if nes2 { bytes[9] & 15 } else { 0 }, 16384)?;
    let chr = rom_size(bytes[5], if nes2 { bytes[9] >> 4 } else { 0 }, 8192)?;
    if prg == 0 {
        return Err("The ROM has no PRG data.".into());
    }
    let trainer = if bytes[6] & 4 != 0 { 512 } else { 0 };
    let required = 16usize
        .checked_add(trainer)
        .and_then(|n| n.checked_add(prg))
        .and_then(|n| n.checked_add(chr))
        .ok_or("ROM size overflows this platform")?;
    if bytes.len() < required {
        return Err("The ROM file is truncated.".into());
    }
    Ok(())
}

fn rom_size(low: u8, high: u8, unit: usize) -> Result<usize> {
    if high == 15 {
        1usize
            .checked_shl((low >> 2) as u32)
            .and_then(|n| n.checked_mul(((low & 3) * 2 + 1) as usize))
            .ok_or_else(|| "ROM size overflows this platform".into())
    } else {
        Ok((((high as usize) << 8) | low as usize) * unit)
    }
}

pub struct Emulator {
    deck: ControlDeck,
    buffer: Vec<u32>,
    frames: u64,
    loaded: bool,
}

impl Default for Emulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Emulator {
    pub fn new() -> Self {
        let config = Config::default()
            .with_filter(VideoFilter::Pixellate)
            .with_ram_state(RamState::AllZeros)
            .with_headless_mode(HeadlessMode::NO_AUDIO)
            .with_sram_dir(None);
        Self {
            deck: ControlDeck::with_config(config),
            buffer: vec![0; PIXELS],
            frames: 0,
            loaded: false,
        }
    }

    pub fn load(&mut self, bytes: &[u8]) -> Result<()> {
        validate_rom(bytes)?;
        // The pinned core interprets NES 2.0 sizes only in whole banks.
        // Reject exponent encoding explicitly before it can be misinterpreted
        // as a much larger allocation. Validation and compatibility differ.
        if bytes[7] & 0x0c == 8 && (bytes[9] & 15 == 15 || bytes[9] >> 4 == 15) {
            return Err(
                "NES 2.0 exponent-encoded ROM sizes are not supported by tetanes-core 0.17.0."
                    .into(),
            );
        }
        self.loaded = false;
        self.deck.load_rom("nesterm", &mut Cursor::new(bytes))?;
        self.frames = 0;
        self.buffer.fill(0);
        self.loaded = true;
        self.release_all();
        Ok(())
    }

    pub fn frame(&mut self) -> Result<&[u32]> {
        if !self.loaded {
            return Err("Load a ROM before running a frame.".into());
        }
        let _ = self.deck.clock_frame()?;
        self.deck.clear_audio_samples();
        for (pixel, rgba) in self
            .buffer
            .iter_mut()
            .zip(self.deck.frame_buffer().as_chunks::<4>().0)
        {
            *pixel = u32::from_le_bytes([rgba[0], rgba[1], rgba[2], 255]);
        }
        self.frames += 1;
        Ok(&self.buffer)
    }

    pub fn pixels(&self) -> &[u32] {
        &self.buffer
    }
    pub fn frames(&self) -> u64 {
        self.frames
    }
    pub fn region(&self) -> NesRegion {
        self.deck.region()
    }

    pub fn frame_period(&self) -> Duration {
        // One PPU frame per clock_frame at normal speed; NTSC is approximately
        // 60.0988 Hz and PAL/Dendy approximately 50.007 Hz, independent of --fps.
        let fps = match self.region() {
            NesRegion::Pal | NesRegion::Dendy => 50.0070,
            _ => 60.0988,
        };
        Duration::from_secs_f64(1.0 / fps)
    }

    pub fn button_pressed(&self, button: Button) -> bool {
        self.deck
            .joypad(Player::One)
            .button(core_button(button).into())
    }
}

fn core_button(button: Button) -> JoypadBtn {
    match button {
        Button::A => JoypadBtn::A,
        Button::B => JoypadBtn::B,
        Button::Select => JoypadBtn::Select,
        Button::Start => JoypadBtn::Start,
        Button::Up => JoypadBtn::Up,
        Button::Down => JoypadBtn::Down,
        Button::Left => JoypadBtn::Left,
        Button::Right => JoypadBtn::Right,
    }
}

impl Gamepad for Emulator {
    fn set_button(&mut self, button: Button, pressed: bool) {
        self.deck
            .joypad_mut(Player::One)
            .set_button(core_button(button), pressed);
    }
}
