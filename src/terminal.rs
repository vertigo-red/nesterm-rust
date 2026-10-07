use crate::{
    Result,
    ansi::{ENTER_SEQUENCE, EXIT_SEQUENCE, format_frame},
    cli::Options,
    emulator::Emulator,
    input::{Control, InputController, InputEvent, InputParser, Key},
    recorder::Recorder,
    renderer::{AsciiFrame, AsciiRenderer},
};
use std::{
    io::{self, IsTerminal, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub trait RawMode {
    fn enable(&mut self) -> io::Result<()>;
    fn disable(&mut self) -> io::Result<()>;
}
pub struct SystemRawMode;
impl RawMode for SystemRawMode {
    fn enable(&mut self) -> io::Result<()> {
        crossterm::terminal::enable_raw_mode()
    }
    fn disable(&mut self) -> io::Result<()> {
        crossterm::terminal::disable_raw_mode()
    }
}

/// RAII restoration also runs during unwinding. Explicit close propagates
/// output/recording errors while always attempting to restore input settings.
pub struct TerminalSession<W: Write, R: RawMode> {
    output: W,
    raw: R,
    recorder: Option<Recorder>,
    entered: bool,
    raw_enabled: bool,
}

impl<W: Write, R: RawMode> TerminalSession<W, R> {
    pub fn new(output: W, raw: R, recorder: Option<Recorder>) -> Self {
        Self {
            output,
            raw,
            recorder,
            entered: false,
            raw_enabled: false,
        }
    }

    fn output(&mut self, data: &str) -> io::Result<()> {
        self.output.write_all(data.as_bytes())?;
        self.output.flush()?;
        if let Some(recorder) = &mut self.recorder {
            recorder.record(data)?;
        }
        Ok(())
    }

    pub fn enter(&mut self, raw: bool) -> io::Result<()> {
        if self.entered {
            return Ok(());
        }
        if raw {
            self.raw.enable()?;
            self.raw_enabled = true;
        }
        self.entered = true;
        self.output(ENTER_SEQUENCE)
    }

    pub fn draw(
        &mut self,
        frame: &AsciiFrame,
        color: bool,
        previous: Option<&AsciiFrame>,
    ) -> Result<bool> {
        let data = format_frame(frame, color, previous)?;
        if data.is_empty() {
            return Ok(false);
        }
        self.output(&data)?;
        Ok(true)
    }

    pub fn close(&mut self) -> io::Result<()> {
        let mut error = None;
        if self.entered {
            if let Err(e) = self.output(EXIT_SEQUENCE) {
                // A transient failure must not prevent a second restoration
                // attempt. Do not recursively invoke the recorder on retry.
                let _ = self.output.write_all(EXIT_SEQUENCE.as_bytes());
                let _ = self.output.flush();
                error = Some(e);
            }
            self.entered = false;
        }
        if self.raw_enabled {
            match self.raw.disable() {
                Ok(()) => self.raw_enabled = false,
                Err(e) => {
                    if error.is_none() {
                        error = Some(e);
                    }
                }
            }
        }
        if let Some(recorder) = &mut self.recorder
            && let Err(e) = recorder.flush()
            && error.is_none()
        {
            error = Some(e);
        }
        error.map_or(Ok(()), Err)
    }
}

impl<W: Write, R: RawMode> Drop for TerminalSession<W, R> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

pub fn assert_size(actual: Option<(u16, u16)>, cols: usize, rows: usize) -> Result<()> {
    if let Some((width, height)) = actual
        && ((width as usize) < cols || (height as usize) < rows)
    {
        return Err(format!(
            "terminal is too small: {width}x{height}; need at least {cols}x{rows}"
        )
        .into());
    }
    Ok(())
}

enum Message {
    Keys(Vec<InputEvent>),
    Error(io::Error),
}

#[cfg(not(windows))]
fn spawn_input(
    sender: mpsc::SyncSender<Message>,
    _stopping: Arc<AtomicBool>,
    _tty: bool,
) -> io::Result<()> {
    use std::io::Read;
    std::thread::Builder::new()
        .name("terminal-input".into())
        .spawn(move || {
            let mut input = io::stdin().lock();
            let mut parser = InputParser::default();
            let mut bytes = [0; 1024];
            loop {
                match input.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(count) => {
                        let events = parser.feed(&bytes[..count]);
                        if !events.is_empty() && sender.send(Message::Keys(events)).is_err() {
                            break;
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        let _ = sender.send(Message::Error(e));
                        break;
                    }
                }
            }
        })?;
    Ok(())
}

#[cfg(windows)]
fn spawn_input(
    sender: mpsc::SyncSender<Message>,
    stopping: Arc<AtomicBool>,
    tty: bool,
) -> io::Result<()> {
    use crate::input::{Action, Button};
    use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    std::thread::Builder::new()
        .name("terminal-input".into())
        .spawn(move || {
            if !tty {
                return;
            }
            while !stopping.load(Ordering::Relaxed) {
                let result = (|| -> io::Result<()> {
                    if !event::poll(Duration::from_millis(20))? {
                        return Ok(());
                    }
                    let Event::Key(k) = event::read()? else {
                        return Ok(());
                    };
                    let key = match k.code {
                        KeyCode::Up => Some(Key::Button(Button::Up)),
                        KeyCode::Down => Some(Key::Button(Button::Down)),
                        KeyCode::Left => Some(Key::Button(Button::Left)),
                        KeyCode::Right => Some(Key::Button(Button::Right)),
                        KeyCode::Enter => Some(Key::Button(Button::Start)),
                        KeyCode::Char(c) => {
                            if c.eq_ignore_ascii_case(&'c')
                                && k.modifiers.contains(KeyModifiers::CONTROL)
                            {
                                Some(Key::Control(Control::Quit))
                            } else {
                                InputParser::default()
                                    .feed(&[c.to_ascii_lowercase() as u8])
                                    .first()
                                    .map(|e| e.key)
                            }
                        }
                        _ => None,
                    };
                    if let Some(key) = key {
                        let action = match k.kind {
                            KeyEventKind::Press => Action::Press,
                            KeyEventKind::Repeat => Action::Repeat,
                            KeyEventKind::Release => Action::Release,
                        };
                        // Windows key-up reporting varies by terminal. Apply the
                        // legacy deadline as a fallback even if key-up is available.
                        let _ = sender.send(Message::Keys(vec![InputEvent {
                            key,
                            action,
                            extended: false,
                        }]));
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    let _ = sender.send(Message::Error(e));
                    break;
                }
            }
        })?;
    Ok(())
}

pub fn run(options: &Options) -> Result<()> {
    let output_tty = io::stdout().is_terminal();
    let input_tty = io::stdin().is_terminal();
    #[cfg(windows)]
    if output_tty && !crossterm::ansi_support::supports_ansi() {
        return Err("This console does not support ANSI output. Use Windows Terminal.".into());
    }
    let terminal_size = || {
        if output_tty {
            crossterm::terminal::size().ok()
        } else {
            None
        }
    };
    assert_size(terminal_size(), options.cols, options.rows)?;
    let bytes = std::fs::read(options.rom.as_ref().ok_or("a ROM path is required")?)?;
    let mut emulator = Emulator::new();
    emulator.load(&bytes)?;
    let mut renderer = AsciiRenderer::new(options.cols, options.rows, options.mode, options.color)?;
    let stopping = Arc::new(AtomicBool::new(false));
    let signal_flag = Arc::clone(&stopping);
    ctrlc::set_handler(move || signal_flag.store(true, Ordering::Relaxed))?;
    let recorder = options
        .record
        .as_ref()
        .map(|path| Recorder::create(path, options.cols, options.rows))
        .transpose()?;
    let stdout = io::stdout();
    let mut session = TerminalSession::new(stdout.lock(), SystemRawMode, recorder);
    let mut controller = InputController::default();
    let (sender, receiver) = mpsc::sync_channel(8);
    let result = (|| -> Result<()> {
        session.enter(input_tty)?;
        spawn_input(sender, Arc::clone(&stopping), input_tty)?;
        let started = Instant::now();
        let frame_period = emulator.frame_period().as_secs_f64();
        let render_period = 1.0 / options.fps;
        let (mut next_frame, mut next_render) = (0.0, 0.0);
        let mut previous: Option<AsciiFrame> = None;
        let mut size = terminal_size();
        let mut paused = false;
        while !stopping.load(Ordering::Relaxed)
            && options
                .seconds
                .is_none_or(|limit| started.elapsed().as_secs_f64() < limit)
        {
            for message in receiver.try_iter() {
                match message {
                    Message::Error(e) => return Err(e.into()),
                    Message::Keys(events) => {
                        for event in events {
                            if paused && matches!(event.key, Key::Button(_)) {
                                continue;
                            }
                            match controller.handle(event, &mut emulator, Instant::now()) {
                                Some(Control::Quit) => stopping.store(true, Ordering::Relaxed),
                                Some(Control::Pause) => {
                                    paused = !paused;
                                    if paused {
                                        controller.release_all(&mut emulator);
                                    }
                                }
                                None => {}
                            }
                        }
                    }
                }
            }
            if stopping.load(Ordering::Relaxed) {
                break;
            }
            controller.expire(&mut emulator, Instant::now());
            let actual = terminal_size();
            assert_size(actual, options.cols, options.rows)?;
            if actual != size {
                size = actual;
                previous = None;
            }
            let mut now = started.elapsed().as_secs_f64();
            if paused {
                next_frame = now + frame_period;
            } else {
                let mut catchups = 0;
                while now >= next_frame && catchups < 5 {
                    emulator.frame()?;
                    next_frame += frame_period;
                    catchups += 1;
                }
                if now >= next_frame && catchups == 5 {
                    next_frame = now + frame_period;
                }
            }
            now = started.elapsed().as_secs_f64();
            if emulator.frames() > 0 && (previous.is_none() || now >= next_render) {
                let frame = renderer.render(emulator.pixels())?;
                session.draw(&frame, options.color, previous.as_ref())?;
                previous = Some(frame);
                next_render = (next_render + render_period).max(now + render_period);
            }
            let wake = next_frame
                .min(next_render)
                .min(options.seconds.unwrap_or(f64::INFINITY));
            let delay = (wake - started.elapsed().as_secs_f64()).clamp(0.0005, 0.010);
            std::thread::sleep(Duration::from_secs_f64(delay));
        }
        Ok(())
    })();
    stopping.store(true, Ordering::Relaxed);
    controller.release_all(&mut emulator);
    let cleanup = session.close();
    result?;
    cleanup?;
    Ok(())
}
