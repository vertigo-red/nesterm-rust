use std::time::{Duration, Instant};

pub const LEGACY_RELEASE: Duration = Duration::from_millis(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Button {
    A,
    B,
    Select,
    Start,
    Up,
    Down,
    Left,
    Right,
}
pub const BUTTONS: [Button; 8] = [
    Button::A,
    Button::B,
    Button::Select,
    Button::Start,
    Button::Up,
    Button::Down,
    Button::Left,
    Button::Right,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Quit,
    Pause,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Button(Button),
    Control(Control),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Press,
    Repeat,
    Release,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputEvent {
    pub key: Key,
    pub action: Action,
    pub extended: bool,
}

fn key_for(code: u32, ctrl: bool) -> Option<Key> {
    use Button::*;
    let c = char::from_u32(code).map(|c| c.to_ascii_lowercase());
    if code == 3 || c == Some('q') || (ctrl && c == Some('c')) {
        return Some(Key::Control(Control::Quit));
    }
    if c == Some('p') {
        return Some(Key::Control(Control::Pause));
    }
    let button = match code {
        57441 | 57447 => Select,
        57352 => Up,
        57353 => Down,
        57354 => Left,
        57355 => Right,
        _ => match c? {
            'w' => Up,
            'a' => Left,
            's' => Down,
            'd' => Right,
            'x' => A,
            'z' => B,
            '\r' | '\n' => Start,
            ' ' => Select,
            _ => return None,
        },
    };
    Some(Key::Button(button))
}

/// Incremental byte parser. Split CSI sequences are carried between reads;
/// unknown complete sequences are consumed without leaking their payload as keys.
#[derive(Default)]
pub struct InputParser {
    rest: Vec<u8>,
}

impl InputParser {
    pub fn feed(&mut self, input: &[u8]) -> Vec<InputEvent> {
        self.rest.extend_from_slice(input);
        let mut events = Vec::new();
        let mut offset = 0;
        while offset < self.rest.len() {
            let remaining = &self.rest[offset..];
            if remaining.starts_with(b"\x1b[") || remaining.starts_with(b"\x1bO") {
                let Some(end) = remaining[2..]
                    .iter()
                    .position(|b| (0x40..=0x7e).contains(b))
                    .map(|n| n + 2)
                else {
                    break;
                };
                let params = &remaining[2..end];
                let final_byte = remaining[end];
                if let Some(event) = parse_csi(params, final_byte) {
                    events.push(event);
                }
                offset += end + 1;
                continue;
            }
            if remaining == b"\x1b" {
                break;
            }
            if let Some(key) = key_for(remaining[0] as u32, false) {
                events.push(InputEvent {
                    key,
                    action: Action::Press,
                    extended: false,
                });
            }
            offset += 1;
        }
        self.rest.drain(..offset);
        // A malformed terminal sequence must not grow memory without bound.
        if self.rest.len() > 1024 {
            self.rest.clear();
        }
        events
    }
}

fn parse_csi(params: &[u8], final_byte: u8) -> Option<InputEvent> {
    let params = std::str::from_utf8(params).ok()?;
    let mut fields = params.split(';');
    let code = fields.next().unwrap_or("").split(':').next().unwrap_or("");
    let mut modifiers = fields.next().unwrap_or("1").split(':');
    let mods = modifiers.next()?.parse::<u32>().ok()?;
    let event_type = modifiers.next();
    let action = match event_type {
        Some("2") => Action::Repeat,
        Some("3") => Action::Release,
        None | Some("1") => Action::Press,
        _ => return None,
    };
    let key = if final_byte == b'u' {
        key_for(code.parse().ok()?, mods.saturating_sub(1) & 4 != 0)?
    } else {
        if !code.is_empty() && code != "1" {
            return None;
        }
        Key::Button(match final_byte {
            b'A' => Button::Up,
            b'B' => Button::Down,
            b'C' => Button::Right,
            b'D' => Button::Left,
            _ => return None,
        })
    };
    Some(InputEvent {
        key,
        action,
        extended: final_byte == b'u' || event_type.is_some(),
    })
}

pub trait Gamepad {
    fn set_button(&mut self, button: Button, pressed: bool);
    fn release_all(&mut self) {
        for button in BUTTONS {
            self.set_button(button, false);
        }
    }
}

#[derive(Default)]
pub struct InputController {
    deadlines: [Option<Instant>; 8],
}

impl InputController {
    pub fn handle(
        &mut self,
        event: InputEvent,
        gamepad: &mut impl Gamepad,
        now: Instant,
    ) -> Option<Control> {
        match event.key {
            Key::Control(control) if event.action == Action::Press => Some(control),
            Key::Control(_) => None,
            Key::Button(button) => {
                self.deadlines[button as usize] =
                    if event.action != Action::Release && !event.extended {
                        Some(now + LEGACY_RELEASE)
                    } else {
                        None
                    };
                gamepad.set_button(button, event.action != Action::Release);
                None
            }
        }
    }

    pub fn expire(&mut self, gamepad: &mut impl Gamepad, now: Instant) {
        for button in BUTTONS {
            if self.deadlines[button as usize].is_some_and(|deadline| now >= deadline) {
                gamepad.set_button(button, false);
                self.deadlines[button as usize] = None;
            }
        }
    }

    pub fn release_all(&mut self, gamepad: &mut impl Gamepad) {
        self.deadlines.fill(None);
        gamepad.release_all();
    }
}
