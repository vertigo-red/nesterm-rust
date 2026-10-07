//! Exact bitmap text matching. Font graphics are read from the user's ROM.
use crate::{
    Result,
    renderer::{AsciiFrame, HEIGHT, MONO_COLOR, PIXELS, WIDTH},
    text_font::BASIC,
};
use std::collections::{BTreeSet, HashMap, HashSet};
use unicode_width::UnicodeWidthChar;

mod profiles;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Clone)]
struct Glyph {
    character: char,
    ink: Bounds,
    slot_height: usize,
    slot_width: usize,
    rank: u8,
    bitmap: Vec<bool>,
}

struct TileGlyph {
    character: char,
    foreground: Vec<usize>,
    rank: u8,
}

struct WordGlyph {
    text: &'static str,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    ink: Bounds,
    foreground: u8,
}

#[derive(Clone, Debug)]
struct Letter {
    ink: Bounds,
    top: i32,
    left: i32,
    width: usize,
    height: usize,
    choices: Vec<char>,
    color: u32,
    colors: Vec<u32>,
    complete: bool,
    rank: u8,
}

#[derive(Clone, Debug)]
pub struct TextRun {
    pub bounds: Bounds,
    pub text: String,
    pub height: usize,
    letters: Vec<Letter>,
}

pub struct TextRenderer {
    glyphs: HashMap<u64, Vec<Glyph>>,
    tiles: HashMap<u128, Vec<TileGlyph>>,
    words: Vec<WordGlyph>,
    custom_font: bool,
    visited: Vec<bool>,
    owners: Vec<usize>,
    queue: Vec<usize>,
}

fn tile_key<T: Copy + Eq>(pixels: &[T]) -> Option<(u128, Vec<T>)> {
    let mut colors = Vec::new();
    let mut key = 0u128;
    for (i, &color) in pixels.iter().enumerate() {
        let index = match colors.iter().position(|&c| c == color) {
            Some(index) => index,
            None => {
                if colors.len() == 4 {
                    return None;
                }
                colors.push(color);
                colors.len() - 1
            }
        };
        key |= (index as u128) << (i * 2);
    }
    (colors.len() >= 2).then_some((key, colors))
}

fn tile_pixels(tile: &[u8]) -> Vec<u8> {
    (0..64)
        .map(|p| ((tile[p / 8] >> (7 - p % 8)) & 1) | (((tile[p / 8 + 8] >> (7 - p % 8)) & 1) << 1))
        .collect()
}

fn normalized(bitmap: &[bool], width: usize, height: usize) -> Option<(u64, Bounds)> {
    let (mut x0, mut y0, mut x1, mut y1) = (width, height, 0, 0);
    for (index, &ink) in bitmap.iter().enumerate() {
        if ink {
            let (x, y) = (index % width, index / width);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let bounds = Bounds {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    };
    let mut key = 0;
    for y in 0..8 {
        for x in 0..8 {
            if bitmap[(y0 + y * bounds.height / 8) * width + x0 + x * bounds.width / 8] {
                key |= 1 << (y * 8 + x);
            }
        }
    }
    Some((key, bounds))
}

fn basic_bitmap(character: u8) -> Vec<bool> {
    BASIC[(character - 32) as usize]
        .iter()
        .flat_map(|&row| (0..8).map(move |x| row & (128 >> x) != 0))
        .collect()
}

impl TextRenderer {
    pub fn new(rom: &[u8], font_json: Option<&str>) -> Result<Self> {
        let mut out = Self {
            glyphs: HashMap::new(),
            tiles: HashMap::new(),
            words: Vec::new(),
            custom_font: font_json.is_some(),
            visited: vec![false; PIXELS],
            owners: vec![usize::MAX; PIXELS],
            queue: Vec::with_capacity(PIXELS),
        };
        for character in 33..=126 {
            out.add(character as char, &basic_bitmap(character), 8, 8, 1);
        }
        out.learn_rom(rom);
        if let Some(json) = font_json {
            out.add_font(json)?;
        }
        Ok(out)
    }

    fn add(&mut self, character: char, bitmap: &[bool], width: usize, height: usize, rank: u8) {
        if let Some((key, ink)) = normalized(bitmap, width, height) {
            let entries = self.glyphs.entry(key).or_default();
            if !entries.iter().any(|g| {
                g.character == character
                    && g.ink == ink
                    && g.slot_height == height
                    && g.rank == rank
            }) {
                entries.push(Glyph {
                    character,
                    ink,
                    slot_height: height,
                    slot_width: width,
                    rank,
                    bitmap: bitmap.to_vec(),
                });
            }
        }
    }

    fn add_tile(&mut self, character: char, pixels: &[u8], subset: u8, rank: u8) {
        let bitmap: Vec<bool> = pixels.iter().map(|&c| subset & (1 << c) != 0).collect();
        self.add(character, &bitmap, 8, 8, rank);
        if let Some((key, colors)) = tile_key(pixels) {
            let foreground = colors
                .iter()
                .enumerate()
                .filter(|(_, c)| subset & (1 << **c) != 0)
                .map(|(i, _)| i)
                .collect();
            let entries = self.tiles.entry(key).or_default();
            if !entries
                .iter()
                .any(|g| g.character == character && g.rank == rank)
            {
                entries.push(TileGlyph {
                    character,
                    foreground,
                    rank,
                });
            }
        }
    }

    fn add_word(
        &mut self,
        chr: &[u8],
        text: &'static str,
        columns: usize,
        tiles: &[usize],
        foreground: u8,
    ) {
        let width = columns * 8;
        let height = tiles.len() / columns * 8;
        let mut pixels = vec![0; width * height];
        for (i, &tile) in tiles.iter().enumerate() {
            let values = tile_pixels(&chr[tile * 16..(tile + 1) * 16]);
            for y in 0..8 {
                let offset = (i / columns * 8 + y) * width + i % columns * 8;
                pixels[offset..offset + 8].copy_from_slice(&values[y * 8..y * 8 + 8]);
            }
        }
        let bitmap: Vec<bool> = pixels.iter().map(|&c| foreground & (1 << c) != 0).collect();
        if let Some((_, ink)) = normalized(&bitmap, width, height) {
            self.words.push(WordGlyph {
                text,
                width,
                height,
                pixels,
                ink,
                foreground,
            });
        }
    }

    /// A user-supplied mapping is authoritative. Only single-cell printable
    /// characters are accepted; control sequences, combining and wide glyphs are rejected.
    fn add_font(&mut self, json: &str) -> Result<()> {
        if json.len() > 1024 * 1024 {
            return Err("text font is larger than 1 MiB".into());
        }
        let value: serde_json::Value = serde_json::from_str(json)?;
        let glyphs = value["glyphs"]
            .as_object()
            .ok_or("font needs a glyphs object")?;
        if glyphs.is_empty() || glyphs.len() > 512 {
            return Err("font must contain 1..512 glyphs".into());
        }
        for (label, rows) in glyphs {
            let mut chars = label.chars();
            let c = chars.next().ok_or("empty font character")?;
            if chars.next().is_some() || c.is_control() || c.width() != Some(1) || c == ' ' {
                return Err("font keys must be one printable single-column character".into());
            }
            let rows = rows.as_array().ok_or("glyph rows must be an array")?;
            if rows.is_empty() || rows.len() > 32 {
                return Err("glyph height must be 1..32".into());
            }
            let width = rows[0].as_str().ok_or("glyph row must be a string")?.len();
            if width == 0 || width > 32 {
                return Err("glyph width must be 1..32".into());
            }
            let mut bitmap = Vec::new();
            for row in rows {
                let row = row.as_str().ok_or("glyph row must be a string")?;
                if row.len() != width || !row.bytes().all(|v| v == b'.' || v == b'#') {
                    return Err("glyph rows need equal width and only '.' / '#'".into());
                }
                bitmap.extend(row.bytes().map(|v| v == b'#'));
            }
            if normalized(&bitmap, width, rows.len()).is_none() {
                return Err("glyph must contain ink".into());
            }
            self.add(c, &bitmap, width, rows.len(), 5);
        }
        Ok(())
    }

    fn learn_rom(&mut self, rom: &[u8]) {
        if rom.len() < 16 || &rom[..4] != b"NES\x1a" {
            return;
        }
        let nes2 = rom[7] & 12 == 8;
        let prg_high = if nes2 { rom[9] & 15 } else { 0 };
        let chr_high = if nes2 { rom[9] >> 4 } else { 0 };
        if prg_high == 15 || chr_high == 15 {
            return;
        }
        let start = 16
            + usize::from(rom[6] & 4 != 0) * 512
            + (((prg_high as usize) << 8) | rom[4] as usize) * 16384;
        let size = (((chr_high as usize) << 8) | rom[5] as usize) * 8192;
        let Some(chr) = rom.get(start..start + size) else {
            return;
        };
        // Bound startup work for very large NES 2.0 cartridges.
        let chr = &chr[..chr.len().min(1024 * 1024)];
        let tiles: Vec<Vec<(u64, u8)>> = chr
            .as_chunks::<16>()
            .0
            .iter()
            .map(|tile| {
                let colors = tile_pixels(tile);
                (1..15)
                    .filter_map(|subset| {
                        let bitmap: Vec<bool> =
                            colors.iter().map(|&c| subset & (1 << c) != 0).collect();
                        let count = bitmap.iter().filter(|&&p| p).count();
                        if !(8..=48).contains(&count) {
                            return None;
                        }
                        let (key, _) = normalized(&bitmap, 8, 8)?;
                        Some((key, subset))
                    })
                    .collect()
            })
            .collect();
        for alphabet in [
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZ".as_slice(),
            b"0123456789".as_slice(),
        ] {
            let references: Vec<u64> = alphabet
                .iter()
                .map(|&c| normalized(&basic_bitmap(c), 8, 8).unwrap().0)
                .collect();
            for base in 0..tiles.len().saturating_sub(alphabet.len() - 1) {
                let errors: Vec<u32> = references
                    .iter()
                    .enumerate()
                    .map(|(j, key)| {
                        tiles[base + j]
                            .iter()
                            .map(|v| (v.0 ^ key).count_ones())
                            .min()
                            .unwrap_or(64)
                    })
                    .collect();
                if errors.iter().sum::<u32>() >= 16 * alphabet.len() as u32 {
                    continue;
                }
                if alphabet.len() == 26
                    && (errors[0] > 14 || errors[8] > 8 || errors[19] > 12 || errors[20] > 12)
                {
                    continue;
                }
                for (j, &c) in alphabet.iter().enumerate() {
                    if let Some(&(_, subset)) = tiles[base + j]
                        .iter()
                        .min_by_key(|v| (v.0 ^ references[j]).count_ones())
                    {
                        self.add_tile(
                            c as char,
                            &tile_pixels(&chr[(base + j) * 16..(base + j + 1) * 16]),
                            subset,
                            2,
                        );
                    }
                }
            }
        }
        self.learn_profile(chr);
    }

    fn match_component(&self, pixels: &[u32], ink: Bounds, color: u32) -> Option<Letter> {
        if ink.width > 96 || ink.height > 96 {
            return None;
        }
        let bitmap: Vec<bool> = (ink.y..ink.y + ink.height)
            .flat_map(|y| (ink.x..ink.x + ink.width).map(move |x| pixels[y * WIDTH + x] == color))
            .collect();
        let (key, _) = normalized(&bitmap, ink.width, ink.height)?;
        let templates = self.glyphs.get(&key)?;
        let mut matches = Vec::new();
        for glyph in templates {
            if !ink.height.is_multiple_of(glyph.ink.height) {
                continue;
            }
            let scale = ink.height / glyph.ink.height;
            if !(1..=8).contains(&scale) || ink.width != glyph.ink.width * scale {
                continue;
            }
            if !(0..ink.height).all(|y| {
                (0..ink.width).all(|x| {
                    bitmap[y * ink.width + x]
                        == glyph.bitmap
                            [(glyph.ink.y + y / scale) * glyph.slot_width + glyph.ink.x + x / scale]
                })
            }) {
                continue;
            }
            matches.push((glyph, scale));
        }
        let rank = matches.iter().map(|v| v.0.rank).max()?;
        matches.retain(|v| v.0.rank == rank);
        let (first, scale) = matches[0];
        let top = ink.y as i32 - (first.ink.y * scale) as i32;
        let height = first.slot_height * scale;
        // Conflicting labels are resolved only for letter/digit lookalikes in a run.
        let mut choices: Vec<char> = matches
            .iter()
            .filter(|(g, s)| {
                g.slot_height * s == height && ink.y as i32 - (g.ink.y * s) as i32 == top
            })
            .map(|v| v.0.character)
            .collect();
        choices.sort_unstable();
        choices.dedup();
        Some(Letter {
            ink,
            top,
            left: ink.x as i32 - (first.ink.x * scale) as i32,
            width: first.slot_width * scale,
            height,
            choices,
            color,
            colors: vec![color],
            complete: false,
            rank,
        })
    }

    fn match_slot(
        &self,
        pixels: &[u32],
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> Option<Letter> {
        if !self.custom_font && width == height && width.is_multiple_of(8) {
            let scale = width / 8;
            let sample: Vec<u32> = (0..8)
                .flat_map(|yy| {
                    (0..8).map(move |xx| pixels[(y + yy * scale) * WIDTH + x + xx * scale])
                })
                .collect();
            if let Some((key, palette)) = tile_key(&sample)
                && let Some(templates) = self.tiles.get(&key)
                && (0..height).all(|yy| {
                    (0..width).all(|xx| {
                        pixels[(y + yy) * WIDTH + x + xx] == sample[yy / scale * 8 + xx / scale]
                    })
                })
            {
                let rank = templates.iter().map(|g| g.rank).max()?;
                let matches: Vec<_> = templates.iter().filter(|g| g.rank == rank).collect();
                let selected: Vec<u32> =
                    matches[0].foreground.iter().map(|&i| palette[i]).collect();
                let bitmap: Vec<bool> = (0..height)
                    .flat_map(|yy| (0..width).map(move |xx| pixels[(y + yy) * WIDTH + x + xx]))
                    .map(|c| selected.contains(&c))
                    .collect();
                let (_, ink) = normalized(&bitmap, width, height)?;
                let mut choices: Vec<char> = matches.iter().map(|g| g.character).collect();
                choices.sort_unstable();
                choices.dedup();
                return Some(Letter {
                    ink: Bounds {
                        x: x + ink.x,
                        y: y + ink.y,
                        width: ink.width,
                        height: ink.height,
                    },
                    left: x as i32,
                    top: y as i32,
                    width,
                    height,
                    choices,
                    color: brightest(&selected),
                    colors: selected,
                    complete: true,
                    rank,
                });
            }
        }
        let mut colors: Vec<u32> = (y..y + height)
            .flat_map(|yy| (x..x + width).map(move |xx| pixels[yy * WIDTH + xx]))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        colors.sort_unstable();
        if colors.len() < 2 || colors.len() > 4 {
            return None;
        }
        let mut found = Vec::new();
        for subset in 1..(1usize << colors.len()) - 1 {
            let selected: Vec<u32> = colors
                .iter()
                .enumerate()
                .filter(|(i, _)| subset & (1 << i) != 0)
                .map(|(_, c)| *c)
                .collect();
            let bitmap: Vec<bool> = (y..y + height)
                .flat_map(|yy| (x..x + width).map(move |xx| pixels[yy * WIDTH + xx]))
                .map(|c| selected.contains(&c))
                .collect();
            let Some((key, ink)) = normalized(&bitmap, width, height) else {
                continue;
            };
            let Some(templates) = self.glyphs.get(&key) else {
                continue;
            };
            for glyph in templates {
                if !height.is_multiple_of(glyph.slot_height) {
                    continue;
                }
                let scale = height / glyph.slot_height;
                if scale == 0
                    || scale > 8
                    || width != glyph.slot_width * scale
                    || ink.width != glyph.ink.width * scale
                    || ink.height != glyph.ink.height * scale
                    || ink.x != glyph.ink.x * scale
                    || ink.y != glyph.ink.y * scale
                {
                    continue;
                }
                if !(0..height).all(|yy| {
                    (0..width).all(|xx| {
                        bitmap[yy * width + xx]
                            == glyph.bitmap[(yy / scale) * glyph.slot_width + xx / scale]
                    })
                }) {
                    continue;
                }
                found.push((glyph, ink, selected.clone()));
            }
        }
        let rank = found.iter().map(|v| v.0.rank).max()?;
        found.retain(|v| v.0.rank == rank);
        let ink = found[0].1;
        let selected = found[0].2.clone();
        let mut choices: Vec<char> = found.iter().map(|v| v.0.character).collect();
        choices.sort_unstable();
        choices.dedup();
        let color = brightest(&selected);
        Some(Letter {
            ink: Bounds {
                x: x + ink.x,
                y: y + ink.y,
                width: ink.width,
                height: ink.height,
            },
            left: x as i32,
            top: y as i32,
            width,
            height,
            choices,
            color,
            colors: selected,
            complete: true,
            rank,
        })
    }

    fn recognize_words(&self, pixels: &[u32]) -> Vec<TextRun> {
        let mut result = Vec::new();
        for word in &self.words {
            for y in 0..=HEIGHT - word.height {
                for x in (0..=WIDTH - word.width).step_by(8) {
                    let mut palette = [None; 4];
                    let mut check = |xx: usize, yy: usize| {
                        let expected = word.pixels[yy * word.width + xx] as usize;
                        let actual = pixels[(y + yy) * WIDTH + x + xx];
                        match palette[expected] {
                            Some(color) => color == actual,
                            None => {
                                if palette.iter().enumerate().any(|(i, c)| {
                                    *c == Some(actual)
                                        && (word.foreground & (1 << i) != 0)
                                            != (word.foreground & (1 << expected) != 0)
                                }) {
                                    return false;
                                }
                                palette[expected] = Some(actual);
                                true
                            }
                        }
                    };
                    // Reject almost all positions on the first tile before inspecting a word.
                    if !(0..8).all(|yy| (0..8).all(|xx| check(xx, yy)))
                        || !(0..word.height).all(|yy| (0..word.width).all(|xx| check(xx, yy)))
                    {
                        continue;
                    }
                    let mut colors: Vec<u32> = palette
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| word.foreground & (1 << i) != 0)
                        .filter_map(|(_, &c)| c)
                        .collect();
                    colors.sort_unstable();
                    colors.dedup();
                    if colors.is_empty() || !palette.iter().flatten().any(|c| !colors.contains(c)) {
                        continue;
                    }
                    let ink = Bounds {
                        x: x + word.ink.x,
                        y: y + word.ink.y,
                        width: word.ink.width,
                        height: word.ink.height,
                    };
                    result.push(TextRun {
                        bounds: Bounds {
                            x,
                            y,
                            width: word.width,
                            height: word.height,
                        },
                        text: word.text.into(),
                        height: word.height,
                        letters: vec![Letter {
                            ink,
                            top: y as i32,
                            left: x as i32,
                            width: word.width,
                            height: word.height,
                            choices: Vec::new(),
                            color: brightest(&colors),
                            colors,
                            complete: true,
                            rank: 4,
                        }],
                    });
                }
            }
        }
        result
    }

    pub fn recognize(&mut self, pixels: &[u32]) -> Result<Vec<TextRun>> {
        if pixels.len() != PIXELS {
            return Err("Expected 256x240 frame".into());
        }
        self.visited.fill(false);
        self.owners.fill(usize::MAX);
        let mut components = Vec::new();
        for start in 0..PIXELS {
            if self.visited[start] {
                continue;
            }
            self.queue.clear();
            self.queue.push(start);
            self.visited[start] = true;
            let color = pixels[start];
            let (mut x0, mut y0, mut x1, mut y1) = (WIDTH, HEIGHT, 0, 0);
            let mut cursor = 0;
            while cursor < self.queue.len() {
                let pos = self.queue[cursor];
                cursor += 1;
                let (x, y) = (pos % WIDTH, pos / WIDTH);
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx < 0 || ny < 0 || nx >= WIDTH as i32 || ny >= HEIGHT as i32 {
                            continue;
                        }
                        let next = ny as usize * WIDTH + nx as usize;
                        if !self.visited[next] && pixels[next] == color {
                            self.visited[next] = true;
                            self.queue.push(next);
                        }
                    }
                }
            }
            if x1 - x0 <= 96 && y1 - y0 <= 96 && self.queue.len() <= 4096 {
                for &position in &self.queue {
                    self.owners[position] = components.len();
                }
                components.push((
                    Bounds {
                        x: x0,
                        y: y0,
                        width: x1 - x0,
                        height: y1 - y0,
                    },
                    color,
                ));
            }
        }
        if components.len() > 8192 {
            return Ok(Vec::new());
        }
        let mut letters = Vec::new();
        for &(ink, color) in &components {
            if let Some(letter) = self.match_component(pixels, ink, color) {
                letters.push(letter);
            }
            // Combine dots/accents with their stem, without joining neighbouring letters.
            if ink.height < 3 {
                continue;
            }
            let mut neighbours = HashSet::new();
            for y in ink.y.saturating_sub(5)..ink.y {
                for x in ink.x.saturating_sub(1)..(ink.x + ink.width + 1).min(WIDTH) {
                    let owner = self.owners[y * WIDTH + x];
                    if owner != usize::MAX {
                        neighbours.insert(owner);
                    }
                }
            }
            for neighbour in neighbours {
                let (accent, c) = components[neighbour];
                if c != color
                    || accent.y + accent.height >= ink.y
                    || ink.y - (accent.y + accent.height) > 2
                    || accent.height > 3
                    || accent.x < ink.x.saturating_sub(1)
                    || accent.x + accent.width > ink.x + ink.width + 1
                {
                    continue;
                }
                let x = ink.x.min(accent.x);
                let merged = Bounds {
                    x,
                    y: accent.y,
                    width: (ink.x + ink.width).max(accent.x + accent.width) - x,
                    height: ink.y + ink.height - accent.y,
                };
                if let Some(letter) = self.match_component(pixels, merged, color) {
                    letters.push(letter);
                }
            }
        }
        if letters.len() > 2048 {
            return Ok(Vec::new());
        }
        // A matched letter establishes a pixel grid. Match complete neighbouring
        // slots as well, including disconnected strokes and connected outlines.
        let phases: BTreeSet<_> = letters
            .iter()
            .filter(|l| l.top >= 0 && l.height <= 32 && l.width <= 32)
            .map(|l| {
                (
                    l.left.rem_euclid(l.width as i32) as usize,
                    l.top as usize,
                    l.width,
                    l.height,
                )
            })
            .collect();
        for (phase, y, width, height) in phases.into_iter().take(128) {
            if y + height > HEIGHT {
                continue;
            }
            for x in (phase..WIDTH - width + 1).step_by(width) {
                if let Some(letter) = self.match_slot(pixels, x, y, width, height) {
                    letters.push(letter);
                }
            }
        }
        // Deduplicate different foreground planes and prefer complete glyphs over their stems.
        letters.sort_by_key(|l| (l.top, l.left, !l.complete, std::cmp::Reverse(l.ink.height)));
        let mut slots = HashSet::new();
        letters.retain(|l| slots.insert((l.top, l.left, l.width, l.height)));
        let mut runs: Vec<Vec<Letter>> = Vec::new();
        for letter in letters {
            if letter.choices.len() > 1 && !letter.choices.iter().all(|c| "O0I1l".contains(*c)) {
                continue;
            }
            let row = runs.iter_mut().find(|r| {
                let last = r.last().unwrap();
                last.top.abs_diff(letter.top) <= 1
                    && last.height == letter.height
                    && letter.ink.x >= last.ink.x + last.ink.width
                    && letter.ink.x - (last.ink.x + last.ink.width) <= letter.height * 2
                    && {
                        let start = (last.left + last.width as i32).max(0) as usize;
                        let end = letter.left.max(0) as usize;
                        (start..end).all(|x| {
                            (last.ink.y..last.ink.y + last.ink.height)
                                .all(|y| !last.colors.contains(&pixels[y * WIDTH + x]))
                        })
                    }
            });
            if let Some(row) = row {
                row.push(letter);
            } else {
                runs.push(vec![letter]);
            }
        }
        let mut result = self.recognize_words(pixels);
        let mut occupied = HashSet::new();
        for run in &result {
            let b = run.bounds;
            occupied.extend(
                (b.y..b.y + b.height)
                    .flat_map(|y| (b.x..b.x + b.width).map(move |x| y * WIDTH + x)),
            );
        }
        for letters in runs {
            let distinct_letters = letters
                .iter()
                .filter(|l| l.choices.len() == 1 && l.choices[0].is_alphanumeric())
                .count();
            let numeric = letters
                .iter()
                .all(|l| l.choices.iter().any(|c| c.is_ascii_digit()));
            let trusted = letters
                .iter()
                .all(|l| l.rank >= 4 && l.complete && l.choices.len() == 1);
            if !trusted && (distinct_letters < 2 || letters.len() < if numeric { 2 } else { 3 }) {
                continue;
            }
            // Do not present the surviving middle of an occluded word as a complete label.
            if !numeric {
                let first = &letters[0];
                let last = letters.last().unwrap();
                let has_ink = |l: &Letter, start: usize, end: usize| {
                    (start..end).any(|x| {
                        (l.ink.y..l.ink.y + l.ink.height)
                            .any(|y| l.colors.contains(&pixels[y * WIDTH + x]))
                    })
                };
                let left = first.left.max(0) as usize;
                let right = (last.left + last.width as i32).clamp(0, WIDTH as i32) as usize;
                if has_ink(first, left.saturating_sub(first.width), left)
                    || has_ink(last, right, (right + last.width).min(WIDTH))
                {
                    continue;
                }
            }
            let alphabetical = letters
                .iter()
                .any(|l| l.choices.len() == 1 && l.choices[0].is_alphabetic());
            let mut text = String::new();
            for (i, letter) in letters.iter().enumerate() {
                if i > 0 {
                    let prev = &letters[i - 1];
                    let gap = letter.ink.x - (prev.ink.x + prev.ink.width);
                    if gap >= letter.height * 3 / 4 {
                        text.push(' ');
                    }
                }
                let selected = if letter.choices.len() == 1 {
                    Some(letter.choices[0])
                } else {
                    letter.choices.iter().copied().find(|c| {
                        if alphabetical {
                            c.is_alphabetic()
                        } else {
                            c.is_ascii_digit()
                        }
                    })
                };
                let Some(c) = selected else {
                    text.clear();
                    break;
                };
                text.push(c);
            }
            if text.is_empty() {
                continue;
            }
            let x = letters.iter().map(|l| l.ink.x).min().unwrap();
            let y = letters.iter().map(|l| l.ink.y).min().unwrap();
            let right = letters.iter().map(|l| l.ink.x + l.ink.width).max().unwrap();
            let bottom = letters
                .iter()
                .map(|l| l.ink.y + l.ink.height)
                .max()
                .unwrap();
            if (y..bottom).any(|yy| (x..right).any(|xx| occupied.contains(&(yy * WIDTH + xx)))) {
                continue;
            }
            occupied.extend((y..bottom).flat_map(|yy| (x..right).map(move |xx| yy * WIDTH + xx)));
            result.push(TextRun {
                bounds: Bounds {
                    x,
                    y,
                    width: right - x,
                    height: bottom - y,
                },
                text,
                height: letters[0].height,
                letters,
            });
        }
        Ok(result)
    }

    pub fn apply(
        &mut self,
        pixels: &[u32],
        frame: &mut AsciiFrame,
        color: bool,
    ) -> Result<Vec<TextRun>> {
        if frame.cols == 0
            || frame.rows == 0
            || frame.cols > WIDTH
            || frame.rows > HEIGHT
            || frame.cols.checked_mul(frame.rows) != Some(frame.chars.len())
            || frame.colors.len() != frame.chars.len()
        {
            return Err("Invalid text destination frame".into());
        }
        let runs = self.recognize(pixels)?;
        for run in &runs {
            let b = run.bounds;
            let left = b.x * frame.cols / WIDTH;
            let right = ((b.x + b.width) * frame.cols)
                .div_ceil(WIDTH)
                .min(frame.cols);
            let top = b.y * frame.rows / HEIGHT;
            let bottom = ((b.y + b.height) * frame.rows)
                .div_ceil(HEIGHT)
                .min(frame.rows);
            let text: Vec<char> = run.text.chars().collect();
            let normal = run.height * frame.rows <= HEIGHT && text.len() <= right - left;
            if normal {
                for row in top..bottom {
                    for col in left..right {
                        frame.put_character(row * frame.cols + col, ' ');
                    }
                }
                let row = ((b.y + b.height / 2) * frame.rows / HEIGHT).min(frame.rows - 1);
                for (i, &c) in text.iter().enumerate() {
                    let index = row * frame.cols + left + i;
                    frame.put_character(index, c);
                    let rgb = run.letters[0].color;
                    frame.colors[index] = if color {
                        (rgb & 255) << 16 | (rgb & 0xff00) | (rgb >> 16) & 255
                    } else {
                        MONO_COLOR
                    };
                }
            } else {
                // Draw recognized large text from its actual foreground geometry,
                // with a small contour alphabet instead of arbitrary ASCII texture.
                let mut ink = vec![false; b.width * b.height];
                for y in b.y..b.y + b.height {
                    for x in b.x..b.x + b.width {
                        ink[(y - b.y) * b.width + x - b.x] = run.letters.iter().any(|l| {
                            x >= l.ink.x
                                && x < l.ink.x + l.ink.width
                                && y >= l.ink.y
                                && y < l.ink.y + l.ink.height
                                && l.colors.contains(&pixels[y * WIDTH + x])
                        });
                    }
                }
                for row in top..bottom {
                    for col in left..right {
                        let mut points = Vec::new();
                        let (x0, x1) = (col * WIDTH / frame.cols, (col + 1) * WIDTH / frame.cols);
                        let (y0, y1) = (row * HEIGHT / frame.rows, (row + 1) * HEIGHT / frame.rows);
                        for y in y0.max(b.y)..y1.min(b.y + b.height) {
                            for x in x0.max(b.x)..x1.min(b.x + b.width) {
                                let (xx, yy) = (x - b.x, y - b.y);
                                if ink[yy * b.width + xx]
                                    && (xx == 0
                                        || yy == 0
                                        || xx + 1 == b.width
                                        || yy + 1 == b.height
                                        || !ink[yy * b.width + xx - 1]
                                        || !ink[yy * b.width + xx + 1]
                                        || !ink[(yy - 1) * b.width + xx]
                                        || !ink[(yy + 1) * b.width + xx])
                                {
                                    points.push((x - x0, y - y0));
                                }
                            }
                        }
                        let character = contour(&points, x1 - x0, y1 - y0);
                        let index = row * frame.cols + col;
                        frame.put_character(index, character);
                        let rgb = run.letters[0].color;
                        frame.colors[index] = if color {
                            (rgb & 255) << 16 | (rgb & 0xff00) | (rgb >> 16) & 255
                        } else {
                            MONO_COLOR
                        };
                    }
                }
            }
        }
        Ok(runs)
    }
}

fn brightest(colors: &[u32]) -> u32 {
    *colors
        .iter()
        .max_by_key(|&&c| 2126 * (c & 255) + 7152 * ((c >> 8) & 255) + 722 * ((c >> 16) & 255))
        .unwrap()
}

fn contour(points: &[(usize, usize)], width: usize, height: usize) -> char {
    if points.is_empty() {
        return ' ';
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.0 as f64).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1 as f64).sum::<f64>() / n;
    let vx = points
        .iter()
        .map(|p| (p.0 as f64 - mx).powi(2))
        .sum::<f64>()
        / n;
    let vy = points
        .iter()
        .map(|p| (p.1 as f64 - my).powi(2))
        .sum::<f64>()
        / n;
    let covariance = points
        .iter()
        .map(|p| (p.0 as f64 - mx) * (p.1 as f64 - my))
        .sum::<f64>()
        / n;
    if n > (width * height) as f64 * 0.8 {
        return '#';
    }
    if vx < 0.5 && vy > 0.7 {
        return '|';
    }
    if vy < 0.5 {
        return if my >= height as f64 * 0.5 { '_' } else { '-' };
    }
    if covariance.abs() > 0.6 * (vx * vy).sqrt() {
        return if covariance > 0.0 { '\\' } else { '/' };
    }
    '#'
}
