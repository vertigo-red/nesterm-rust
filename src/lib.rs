//! Native, CLI-only NES-to-ASCII rendering.
pub mod ansi;
pub mod cli;
pub mod emulator;
mod glyphs;
pub mod input;
pub mod recorder;
pub mod renderer;
pub mod terminal;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
