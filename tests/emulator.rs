mod common;
use nesterm_rust::{
    emulator::{Emulator, validate_rom},
    input::{BUTTONS, Gamepad},
    renderer::{AsciiRenderer, Mode},
};
use std::collections::HashSet;

#[test]
fn generated_nrom_runs_120_varying_renderable_frames() {
    let mut emulator = Emulator::new();
    emulator.load(&common::nrom()).unwrap();
    for button in BUTTONS {
        emulator.set_button(button, true);
        assert!(emulator.button_pressed(button));
        emulator.set_button(button, false);
        assert!(!emulator.button_pressed(button));
    }
    let mut hashes = HashSet::new();
    for _ in 0..120 {
        let frame = emulator.frame().unwrap();
        hashes.insert(
            frame
                .iter()
                .step_by(61)
                .fold(0u32, |h, &v| h.wrapping_mul(31).wrapping_add(v)),
        );
    }
    assert_eq!(emulator.frames(), 120);
    assert!(hashes.len() > 2, "got {} frame hashes", hashes.len());
    assert!(emulator.pixels().iter().any(|p| p & 0xffffff != 0));
    let out = AsciiRenderer::new(40, 25, Mode::Shape, true)
        .unwrap()
        .render(emulator.pixels())
        .unwrap();
    assert!(out.chars.iter().any(|c| *c != b' '));
    assert!(out.chars.iter().all(|c| (32..=126).contains(c)));
}

#[test]
fn loading_releases_buttons_and_resets_frame_count() {
    let mut e = Emulator::new();
    e.load(&common::nrom()).unwrap();
    for b in BUTTONS {
        e.set_button(b, true);
    }
    e.frame().unwrap();
    e.load(&common::nrom()).unwrap();
    assert_eq!(e.frames(), 0);
    assert!(BUTTONS.iter().all(|b| !e.button_pressed(*b)));
}

#[test]
fn malformed_and_truncated_roms_never_run_core() {
    let mut e = Emulator::new();
    assert!(e.frame().unwrap_err().to_string().contains("ROM"));
    assert!(e.load(&[0; 16]).unwrap_err().to_string().contains("iNES"));
    let full = common::nrom();
    for size in [0, 3, 15, 16, 500, 16 + 16384, full.len() - 1] {
        assert!(validate_rom(&full[..size]).is_err(), "size={size}");
    }
    assert!(validate_rom(&full).is_ok());
    let mut no_prg = full.clone();
    no_prg[4] = 0;
    assert!(validate_rom(&no_prg).is_err());
}

#[test]
fn nes2_extended_sizes_and_trainers_are_validated() {
    let mut rom = common::nrom();
    rom[6] |= 4;
    assert!(validate_rom(&rom).is_err());
    rom.splice(16..16, vec![0; 512]);
    assert!(validate_rom(&rom).is_ok());
    let mut rom = common::nrom();
    rom[7] = 8;
    rom[9] = 1;
    assert!(validate_rom(&rom).is_err());
    // NES 2.0 exponent: 2^14 * (2*0+1) bytes = the 16 KiB PRG.
    rom[4] = 14 << 2;
    rom[9] = 15;
    assert!(validate_rom(&rom).is_ok());
    rom[4] = 255;
    assert!(validate_rom(&rom).is_err());
}

#[test]
fn chr_ram_cartridge_runs_without_chr_rom() {
    let mut rom = common::nrom();
    rom[5] = 0;
    rom.truncate(16 + 16384);
    let mut e = Emulator::new();
    e.load(&rom).unwrap();
    for _ in 0..3 {
        e.frame().unwrap();
    }
    assert_eq!(e.frames(), 3);
}

#[test]
#[ignore = "requires NESTERM_ROM pointing to a user-supplied ROM"]
fn optional_user_rom_integration() {
    let Some(path) = std::env::var_os("NESTERM_ROM") else {
        return;
    };
    let mut e = Emulator::new();
    e.load(&std::fs::read(path).unwrap()).unwrap();
    let mut hashes = HashSet::new();
    for _ in 0..1200 {
        let f = e.frame().unwrap();
        hashes.insert(
            f.iter()
                .step_by(61)
                .fold(0u32, |h, &v| h.wrapping_mul(31).wrapping_add(v)),
        );
    }
    assert_eq!(e.frames(), 1200);
    assert!(hashes.len() > 2);
}
