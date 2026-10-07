use nesterm_rust::{
    ansi::{ENTER_SEQUENCE, EXIT_SEQUENCE},
    renderer::AsciiFrame,
    terminal::{RawMode, TerminalSession, assert_size},
};
use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
};

#[derive(Default)]
struct State {
    bytes: Vec<u8>,
    raw: bool,
    writes: usize,
    fail_write: Option<usize>,
    fail_disable_once: bool,
}
struct Writer(Rc<RefCell<State>>);
impl Write for Writer {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let mut state = self.0.borrow_mut();
        state.writes += 1;
        if state.fail_write == Some(state.writes) {
            return Err(io::Error::other("simulated output failure"));
        }
        let n = data.len().min(3);
        state.bytes.extend_from_slice(&data[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct Raw(Rc<RefCell<State>>);
impl RawMode for Raw {
    fn enable(&mut self) -> io::Result<()> {
        self.0.borrow_mut().raw = true;
        Ok(())
    }
    fn disable(&mut self) -> io::Result<()> {
        let mut state = self.0.borrow_mut();
        if state.fail_disable_once {
            state.fail_disable_once = false;
            return Err(io::Error::other("simulated raw disable failure"));
        }
        state.raw = false;
        Ok(())
    }
}
fn session(state: Rc<RefCell<State>>) -> TerminalSession<Writer, Raw> {
    TerminalSession::new(Writer(state.clone()), Raw(state), None)
}

#[test]
fn short_writes_restore_terminal_and_raw_mode_exactly_once() {
    let state = Rc::new(RefCell::new(State::default()));
    {
        let mut terminal = session(state.clone());
        terminal.enter(true).unwrap();
        terminal.enter(true).unwrap();
        assert!(state.borrow().raw);
        terminal.close().unwrap();
        terminal.close().unwrap();
    }
    assert!(!state.borrow().raw);
    assert_eq!(
        state.borrow().bytes,
        format!("{ENTER_SEQUENCE}{EXIT_SEQUENCE}").as_bytes()
    );
}

#[test]
fn dropping_after_draw_error_attempts_exit_and_restores_raw_mode() {
    let state = Rc::new(RefCell::new(State::default()));
    {
        let mut terminal = session(state.clone());
        terminal.enter(true).unwrap();
        let writes = state.borrow().writes;
        state.borrow_mut().fail_write = Some(writes + 1);
        let frame = AsciiFrame {
            cols: 1,
            rows: 1,
            chars: vec![b'X'],
            colors: vec![0xffffff],
        };
        assert!(terminal.draw(&frame, true, None).is_err());
    }
    assert!(!state.borrow().raw);
    assert!(state.borrow().bytes.ends_with(EXIT_SEQUENCE.as_bytes()));
}

#[test]
fn closing_retries_failed_exit_and_drop_retries_failed_raw_disable() {
    let state = Rc::new(RefCell::new(State::default()));
    {
        let mut terminal = session(state.clone());
        terminal.enter(true).unwrap();
        let writes = state.borrow().writes;
        state.borrow_mut().fail_write = Some(writes + 1);
        state.borrow_mut().fail_disable_once = true;
        assert!(terminal.close().is_err());
        assert!(state.borrow().raw);
    }
    assert!(!state.borrow().raw);
    assert!(state.borrow().bytes.ends_with(EXIT_SEQUENCE.as_bytes()));
}

#[test]
fn failed_enter_restores_raw_mode_on_drop() {
    let state = Rc::new(RefCell::new(State {
        fail_write: Some(1),
        ..State::default()
    }));
    {
        let mut terminal = session(state.clone());
        assert!(terminal.enter(true).is_err());
    }
    assert!(!state.borrow().raw);
    assert!(state.borrow().bytes.ends_with(EXIT_SEQUENCE.as_bytes()));
}

#[test]
fn exact_size_is_allowed_and_small_or_shrunken_size_is_rejected() {
    for (cols, rows) in [(40, 25), (64, 30)] {
        assert!(assert_size(Some((cols, rows)), cols as usize, rows as usize).is_ok());
        assert!(assert_size(Some((cols - 1, rows)), cols as usize, rows as usize).is_err());
        assert!(assert_size(Some((cols, rows - 1)), cols as usize, rows as usize).is_err());
        assert!(assert_size(None, cols as usize, rows as usize).is_ok());
    }
}
