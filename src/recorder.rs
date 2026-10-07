use serde_json::json;
use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// Records only output successfully flushed to stdout. Existing files are
/// never truncated, so a mistaken --record pointing at the ROM is harmless.
pub struct Recorder {
    file: BufWriter<File>,
    started: Instant,
}

impl Recorder {
    pub fn create(path: &Path, width: usize, height: usize) -> io::Result<Self> {
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let mut file = BufWriter::new(file);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let header = json!({ "version": 2, "width": width, "height": height, "timestamp": timestamp,
            "env": { "TERM": std::env::var("TERM").unwrap_or_default(), "SHELL": std::env::var("SHELL").unwrap_or_default() } });
        writeln!(file, "{header}")?;
        file.flush()?;
        Ok(Self {
            file,
            started: Instant::now(),
        })
    }

    pub fn record(&mut self, data: &str) -> io::Result<()> {
        writeln!(
            self.file,
            "{}",
            json!([self.started.elapsed().as_secs_f64(), "o", data])
        )?;
        self.file.flush()
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
