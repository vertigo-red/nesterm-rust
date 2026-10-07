#![allow(dead_code)] // Shared by separate integration-test crates using different helpers.
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct TempDir(pub PathBuf);
impl TempDir {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nesterm-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Original NROM test program from kathoc/nesterm (MIT). It sets up a patterned
/// nametable and updates a palette entry in NMI every frame. No external ROM.
pub fn nrom() -> Vec<u8> {
    struct Assembler {
        code: Vec<u8>,
        labels: HashMap<&'static str, usize>,
        branches: Vec<(usize, &'static str)>,
    }
    impl Assembler {
        fn emit(&mut self, bytes: &[u8]) {
            self.code.extend_from_slice(bytes);
        }
        fn mark(&mut self, label: &'static str) {
            self.labels.insert(label, self.code.len());
        }
        fn branch(&mut self, opcode: u8, label: &'static str) {
            self.emit(&[opcode, 0]);
            self.branches.push((self.code.len() - 1, label));
        }
    }
    let mut a = Assembler {
        code: Vec::new(),
        labels: HashMap::new(),
        branches: Vec::new(),
    };
    a.emit(&[
        0x78, 0xd8, 0xa2, 0x40, 0x8e, 0x17, 0x40, 0xa2, 0xff, 0x9a, 0xe8, 0x8e, 0x00, 0x20, 0x8e,
        0x01, 0x20,
    ]);
    a.mark("vblank");
    a.emit(&[0x2c, 0x02, 0x20]);
    a.branch(0x10, "vblank");
    a.emit(&[
        0xa9, 0x3f, 0x8d, 0x06, 0x20, 0xa9, 0x00, 0x8d, 0x06, 0x20, 0xa2, 0x00,
    ]);
    let palette_load = a.code.len();
    a.mark("palette");
    a.emit(&[0xbd, 0, 0, 0x8d, 0x07, 0x20, 0xe8, 0xe0, 0x20]);
    a.branch(0xd0, "palette");
    a.emit(&[
        0xa9, 0x20, 0x8d, 0x06, 0x20, 0xa9, 0x00, 0x8d, 0x06, 0x20, 0xa9, 0x01, 0xa2, 0x00, 0xa0,
        0x04,
    ]);
    a.mark("nametable");
    a.emit(&[0x8d, 0x07, 0x20, 0xe8]);
    a.branch(0xd0, "nametable");
    a.emit(&[0x88]);
    a.branch(0xd0, "nametable");
    a.emit(&[
        0xa9, 0x00, 0x8d, 0x05, 0x20, 0x8d, 0x05, 0x20, 0xa9, 0x80, 0x8d, 0x00, 0x20, 0xa9, 0x0a,
        0x8d, 0x01, 0x20,
    ]);
    a.mark("main");
    a.emit(&[0x4c, 0, 0]);
    let main = 0x8000 + a.labels["main"];
    let tail = a.code.len();
    a.code[tail - 2] = main as u8;
    a.code[tail - 1] = (main >> 8) as u8;
    a.mark("nmi");
    a.emit(&[
        0x48, 0x8a, 0x48, 0xa9, 0x3f, 0x8d, 0x06, 0x20, 0xa9, 0x01, 0x8d, 0x06, 0x20, 0xe6, 0x00,
        0xa5, 0x00, 0x29, 0x0f, 0x09, 0x10, 0x8d, 0x07, 0x20, 0xa9, 0x80, 0x8d, 0x00, 0x20, 0xa9,
        0x00, 0x8d, 0x05, 0x20, 0x8d, 0x05, 0x20, 0x68, 0xaa, 0x68, 0x40,
    ]);
    a.mark("paletteData");
    for _ in 0..2 {
        a.emit(&[
            0x0f, 0x21, 0x11, 0x01, 0x0f, 0x27, 0x17, 0x07, 0x0f, 0x2a, 0x1a, 0x0a, 0x0f, 0x30,
            0x20, 0x10,
        ]);
    }
    let palette = 0x8000 + a.labels["paletteData"];
    a.code[palette_load + 1] = palette as u8;
    a.code[palette_load + 2] = (palette >> 8) as u8;
    for (operand, label) in &a.branches {
        let delta = a.labels[label] as isize - (*operand + 1) as isize;
        assert!((-128..=127).contains(&delta));
        a.code[*operand] = delta as u8;
    }
    let mut prg = vec![0xea; 16384];
    prg[..a.code.len()].copy_from_slice(&a.code);
    let nmi = 0x8000 + a.labels["nmi"];
    prg[0x3ffa..].copy_from_slice(&[nmi as u8, (nmi >> 8) as u8, 0x00, 0x80, 0x00, 0x80]);
    let mut rom = vec![0x4e, 0x45, 0x53, 0x1a, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    rom.extend_from_slice(&prg);
    let mut chr = vec![0; 8192];
    for row in 0..8 {
        chr[16 + row] = if row % 2 == 1 { 0xaa } else { 0x55 };
    }
    rom.extend(chr);
    rom
}
