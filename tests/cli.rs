mod common;
use nesterm_rust::{
    ansi::{ENTER_SEQUENCE, EXIT_SEQUENCE},
    cli::Options,
    renderer::Mode,
};
use std::{ffi::OsString, fs, process::Command};

fn parse(args: &[&str]) -> nesterm_rust::Result<Options> {
    Options::parse(args.iter().map(OsString::from))
}

#[test]
fn options_match_upstream_defaults_and_accept_both_sizes() {
    let defaults = parse(&["game.nes"]).unwrap();
    assert_eq!((defaults.cols, defaults.rows, defaults.fps), (40, 25, 30.0));
    assert!(defaults.color);
    let options = parse(&[
        "--mono",
        "--mode",
        "ramp",
        "--size",
        "64x30",
        "--fps",
        "24",
        "--seconds",
        "1.5",
        "--record",
        "out.cast",
        "game.nes",
    ])
    .unwrap();
    assert_eq!(
        (options.cols, options.rows, options.fps, options.seconds),
        (64, 30, 24.0, Some(1.5))
    );
    assert!(!options.color);
    assert_eq!(options.mode, Mode::Ramp);
    assert_eq!(options.record.unwrap().to_str(), Some("out.cast"));
    assert_eq!(
        parse(&["--", "--game.nes"]).unwrap().rom.unwrap().to_str(),
        Some("--game.nes")
    );
    assert!(parse(&["--help"]).unwrap().help);
    assert!(parse(&["--version"]).unwrap().version);
}

#[test]
fn rejects_missing_ambiguous_and_invalid_arguments() {
    for args in [
        vec![],
        vec!["a", "b"],
        vec!["--mode", "pixels", "a"],
        vec!["--size", "80x45", "a"],
        vec!["--fps", "241", "a"],
        vec!["--fps", "0", "a"],
        vec!["--fps", "-1", "a"],
        vec!["--fps", "NaN", "a"],
        vec!["--seconds", "inf", "a"],
        vec!["--seconds", "0", "a"],
        vec!["--record"],
        vec!["--fps", "--mono", "a"],
        vec!["--unknown", "a"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
}

#[cfg(unix)]
#[test]
fn unix_paths_need_not_be_utf8() {
    use std::os::unix::ffi::OsStringExt;
    let path = OsString::from_vec(b"game-\xff.nes".to_vec());
    let out = Options::parse([path.clone()]).unwrap();
    assert_eq!(out.rom.unwrap().as_os_str(), path);
}

#[test]
fn finite_cli_session_cast_is_exactly_the_stdout_stream() {
    let temp = common::TempDir::new();
    let rom = temp.0.join("fixture.nes");
    let cast = temp.0.join("out.cast");
    fs::write(&rom, common::nrom()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nesterm"))
        .arg(&rom)
        .args([
            "--size",
            "64x30",
            "--mode",
            "ramp",
            "--seconds",
            "0.12",
            "--record",
        ])
        .arg(&cast)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(ENTER_SEQUENCE));
    assert!(stdout.ends_with(EXIT_SEQUENCE));
    let data = fs::read_to_string(&cast).unwrap();
    let lines: Vec<serde_json::Value> = data
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0]["version"], 2);
    assert_eq!(lines[0]["width"], 64);
    assert_eq!(lines[0]["height"], 30);
    let recorded: String = lines[1..].iter().map(|v| v[2].as_str().unwrap()).collect();
    assert_eq!(recorded, stdout);
    assert!(
        lines[1..]
            .windows(2)
            .all(|v| v[0][0].as_f64().unwrap() <= v[1][0].as_f64().unwrap())
    );
    assert!(lines.len() > 3);
}

#[test]
fn recording_never_overwrites_the_rom_or_an_existing_file() {
    let temp = common::TempDir::new();
    let path = temp.0.join("fixture.nes");
    let rom = common::nrom();
    fs::write(&path, &rom).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nesterm"))
        .arg(&path)
        .args(["--record"])
        .arg(&path)
        .args(["--seconds", "0.01"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read(&path).unwrap(), rom);
    assert!(output.stdout.is_empty());
}

#[test]
fn bad_rom_exits_before_entering_terminal() {
    let temp = common::TempDir::new();
    let path = temp.0.join("bad.nes");
    fs::write(&path, [0; 16]).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nesterm"))
        .arg(path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("iNES"));
}
