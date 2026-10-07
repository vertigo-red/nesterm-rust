//! Framebuffer pixels are 0xAABBGGRR; terminal colors are 0xRRGGBB.
use crate::{
    Result,
    glyphs::{GLYPHS, GLYPHS_TALL},
};
use std::collections::{BTreeMap, HashMap};

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;
pub const PIXELS: usize = WIDTH * HEIGHT;
pub const MONO_COLOR: u32 = 0xd8f0dd;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Shape,
    Ramp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsciiFrame {
    pub cols: usize,
    pub rows: usize,
    pub chars: Vec<u8>,
    pub colors: Vec<u32>,
    /// Optional single-column Unicode cells used by recognized text only.
    pub unicode: BTreeMap<usize, char>,
}

impl AsciiFrame {
    pub fn character(&self, index: usize) -> char {
        self.unicode
            .get(&index)
            .copied()
            .unwrap_or(self.chars[index] as char)
    }

    pub fn put_character(&mut self, index: usize, character: char) {
        if character.is_ascii() {
            self.chars[index] = character as u8;
            self.unicode.remove(&index);
        } else {
            self.chars[index] = b' ';
            self.unicode.insert(index, character);
        }
    }

    pub fn text(&self) -> String {
        (0..self.rows)
            .map(|row| {
                (0..self.cols)
                    .map(|col| self.character(row * self.cols + col))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub struct AsciiRenderer {
    cols: usize,
    rows: usize,
    mode: Mode,
    color: bool,
    shape: ShapeMatcher,
}

impl AsciiRenderer {
    pub fn new(cols: usize, rows: usize, mode: Mode, color: bool) -> Result<Self> {
        Ok(Self {
            cols,
            rows,
            mode,
            color,
            shape: ShapeMatcher::new(cols, rows)?,
        })
    }

    pub fn reset(&mut self) {
        self.shape.reset();
    }

    pub fn render(&mut self, pixels: &[u32]) -> Result<AsciiFrame> {
        if pixels.len() != PIXELS {
            return Err("Expected 256x240 frame".into());
        }
        if self.mode == Mode::Shape {
            return self.shape.render(pixels, self.color);
        }
        let ramp = b" .,:;irsXA253hMHGS#9B&@";
        let mut frame = AsciiFrame {
            cols: self.cols,
            rows: self.rows,
            chars: Vec::with_capacity(self.cols * self.rows),
            colors: Vec::with_capacity(self.cols * self.rows),
            unicode: BTreeMap::new(),
        };
        for cy in 0..self.rows {
            for cx in 0..self.cols {
                let (mut r, mut g, mut b, mut n) = (0.0, 0.0, 0.0, 0.0);
                for y in cy * HEIGHT / self.rows..(cy + 1) * HEIGHT / self.rows {
                    for x in cx * WIDTH / self.cols..(cx + 1) * WIDTH / self.cols {
                        let p = pixels[y * WIDTH + x];
                        r += (p & 255) as f64;
                        g += ((p >> 8) & 255) as f64;
                        b += ((p >> 16) & 255) as f64;
                        n += 1.0;
                    }
                }
                r /= n;
                g /= n;
                b /= n;
                let level = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0;
                frame
                    .chars
                    .push(ramp[((level * ramp.len() as f64).floor() as usize).min(ramp.len() - 1)]);
                frame
                    .colors
                    .push(if self.color { rgb(r, g, b) } else { MONO_COLOR });
            }
        }
        Ok(frame)
    }
}

fn rgb(r: f64, g: f64, b: f64) -> u32 {
    (r.round() as u32) << 16 | (g.round() as u32) << 8 | b.round() as u32
}

fn overlaps(count: usize, extent: usize) -> Vec<Vec<(usize, f64)>> {
    (0..count)
        .map(|i| {
            let a = i as f64 * extent as f64 / count as f64;
            let b = (i + 1) as f64 * extent as f64 / count as f64;
            (a.floor() as usize..b.ceil() as usize)
                .map(|p| (p, (b.min((p + 1) as f64) - a.max(p as f64)) / (b - a)))
                .collect()
        })
        .collect()
}

struct Mask {
    values: Vec<f32>,
    energy: f64,
}

pub struct ShapeMatcher {
    cols: usize,
    rows: usize,
    sample_count: usize,
    masks: Vec<Mask>,
    indices: Vec<usize>,
    weights: Vec<f32>,
    offsets: Vec<usize>,
    previous: Vec<usize>,
    samples: Vec<f32>,
}

impl ShapeMatcher {
    pub fn new(cols: usize, rows: usize) -> Result<Self> {
        if cols == 0 || rows == 0 || cols > WIDTH || rows > HEIGHT {
            return Err("Invalid grid: require 1..256 columns and 1..240 rows".into());
        }
        let height = if (WIDTH as f64 / cols as f64) / (HEIGHT as f64 / rows as f64) <= 0.55 {
            8
        } else {
            6
        };
        let source: Vec<&[u8]> = if height == 8 {
            GLYPHS_TALL.iter().map(|v| v.as_slice()).collect()
        } else {
            GLYPHS.iter().map(|v| v.as_slice()).collect()
        };
        let masks = source
            .iter()
            .map(|raw| {
                let values: Vec<f32> = raw.iter().map(|&v| (v as f64 / 255.0) as f32).collect();
                let energy = values.iter().map(|&v| (v as f64) * (v as f64)).sum();
                Mask { values, energy }
            })
            .collect();
        let xs = overlaps(cols * 4, WIDTH);
        let ys = overlaps(rows * height, HEIGHT);
        let (mut indices, mut weights, mut offsets) = (Vec::new(), Vec::new(), vec![0]);
        for cy in 0..rows {
            for cx in 0..cols {
                for sy in 0..height {
                    for sx in 0..4 {
                        for &(y, wy) in &ys[cy * height + sy] {
                            for &(x, wx) in &xs[cx * 4 + sx] {
                                indices.push(y * WIDTH + x);
                                weights.push((wy * wx) as f32);
                            }
                        }
                        offsets.push(indices.len());
                    }
                }
            }
        }
        Ok(Self {
            cols,
            rows,
            sample_count: height * 4,
            masks,
            indices,
            weights,
            offsets,
            previous: vec![0; cols * rows],
            samples: vec![0.0; height * 4],
        })
    }

    pub fn reset(&mut self) {
        self.previous.fill(0);
    }

    pub fn render(&mut self, pixels: &[u32], color: bool) -> Result<AsciiFrame> {
        if pixels.len() != PIXELS {
            return Err("Expected 256x240 frame".into());
        }
        let mut palette: HashMap<u32, [f64; 4]> = HashMap::with_capacity(64);
        let mut frame = AsciiFrame {
            cols: self.cols,
            rows: self.rows,
            chars: Vec::with_capacity(self.previous.len()),
            colors: Vec::with_capacity(self.previous.len()),
            unicode: BTreeMap::new(),
        };
        for cell in 0..self.previous.len() {
            let (mut mean, mut sum_r, mut sum_g, mut sum_b, mut weight) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for s in 0..self.sample_count {
                let sample = cell * self.sample_count + s;
                let mut value = 0.0;
                for j in self.offsets[sample]..self.offsets[sample + 1] {
                    let p = pixels[self.indices[j]];
                    let c = palette.entry(p).or_insert_with(|| {
                        let r = (p & 255) as f64;
                        let g = ((p >> 8) & 255) as f64;
                        let b = ((p >> 16) & 255) as f64;
                        let v = if r + g + b < 36.0 {
                            0.0
                        } else {
                            (0.2126 * r * r + 0.7152 * g * g + 0.0722 * b * b).sqrt() / 255.0
                        };
                        [v, r * v, g * v, b * v]
                    });
                    let w = self.weights[j] as f64;
                    value += c[0] * w;
                    sum_r += c[1] * w;
                    sum_g += c[2] * w;
                    sum_b += c[3] * w;
                    weight += c[0] * w;
                }
                self.samples[s] = value as f32;
                mean += value;
            }
            mean /= self.sample_count as f64;
            let mut peak: f64 = 0.0;
            for s in &mut self.samples {
                *s = (((*s as f64 - mean).abs() * 1.5) + *s as f64 * 0.1) as f32;
                peak = peak.max(*s as f64);
            }
            let (mut best, mut best_score) = (0, 0.0);
            if peak > 0.045 {
                let gain = 1.0 / peak.max(1.0);
                for (k, mask) in self.masks.iter().enumerate().skip(1) {
                    let dot: f64 = self
                        .samples
                        .iter()
                        .zip(&mask.values)
                        .map(|(&v, &m)| v as f64 * gain * m as f64)
                        .sum();
                    let amplitude = (dot / mask.energy).clamp(0.65, 1.85);
                    let mut score = 2.0 * amplitude * dot - amplitude * amplitude * mask.energy;
                    if self.previous[cell] == k {
                        score += 0.055;
                    }
                    if score > best_score {
                        best_score = score;
                        best = k;
                    }
                }
            }
            self.previous[cell] = best;
            frame.chars.push(32 + best as u8);
            let (r, g, b) = if weight > 0.0 {
                (sum_r / weight, sum_g / weight, sum_b / weight)
            } else {
                (0.0, 0.0, 0.0)
            };
            let boost = (255.0 / r.max(g).max(b).max(1.0)).min(2.4);
            let intensity = (0.56 + mean * 0.65).min(1.0);
            frame.colors.push(if color {
                rgb(
                    r * boost * intensity,
                    g * boost * intensity,
                    b * boost * intensity,
                )
            } else {
                MONO_COLOR
            });
        }
        Ok(frame)
    }
}
