use nesterm_rust::input::{
    Action, Button, Control, Gamepad, InputController, InputEvent, InputParser, Key, LEGACY_RELEASE,
};
use std::time::Instant;

#[derive(Default)]
struct Pad([bool; 8]);
impl Gamepad for Pad {
    fn set_button(&mut self, button: Button, pressed: bool) {
        self.0[button as usize] = pressed;
    }
}
fn event(button: Button, action: Action, extended: bool) -> InputEvent {
    InputEvent {
        key: Key::Button(button),
        action,
        extended,
    }
}

#[test]
fn legacy_controls_uppercase_and_application_arrows() {
    let events = InputParser::default().feed(b"WASDXZ\r\n \x1b[A\x1bOBpq\x03");
    let keys: Vec<_> = events.iter().map(|e| e.key).collect();
    assert_eq!(
        keys,
        vec![
            Key::Button(Button::Up),
            Key::Button(Button::Left),
            Key::Button(Button::Down),
            Key::Button(Button::Right),
            Key::Button(Button::A),
            Key::Button(Button::B),
            Key::Button(Button::Start),
            Key::Button(Button::Start),
            Key::Button(Button::Select),
            Key::Button(Button::Up),
            Key::Button(Button::Down),
            Key::Control(Control::Pause),
            Key::Control(Control::Quit),
            Key::Control(Control::Quit)
        ]
    );
    assert!(events.iter().all(|e| !e.extended));
}

#[test]
fn kitty_release_shift_and_arrows() {
    let events = InputParser::default()
        .feed(b"\x1b[120;1:1u\x1b[120;1:3u\x1b[57441;1:1u\x1b[57447;1:3u\x1b[1;1:2D\x1b[1;1:3D");
    assert_eq!(
        events,
        vec![
            event(Button::A, Action::Press, true),
            event(Button::A, Action::Release, true),
            event(Button::Select, Action::Press, true),
            event(Button::Select, Action::Release, true),
            event(Button::Left, Action::Repeat, true),
            event(Button::Left, Action::Release, true)
        ]
    );
}

#[test]
fn kitty_alternate_key_fields_and_control_c() {
    let events = InputParser::default().feed(b"\x1b[120:88;1:1;120u\x1b[99;5u\x1b[57352;1:3u");
    assert_eq!(
        events,
        vec![
            event(Button::A, Action::Press, true),
            InputEvent {
                key: Key::Control(Control::Quit),
                action: Action::Press,
                extended: true
            },
            event(Button::Up, Action::Release, true)
        ]
    );
}

#[test]
fn every_split_boundary_preserves_events() {
    let source = b"\x1b[120;1:3u\x1b[1;1:2D\x1b[57441;1:1u\x1b[A";
    let expected = InputParser::default().feed(source);
    for split in 0..=source.len() {
        let mut parser = InputParser::default();
        let mut events = parser.feed(&source[..split]);
        events.extend(parser.feed(&source[split..]));
        assert_eq!(events, expected, "split={split}");
    }
    let mut parser = InputParser::default();
    let actual: Vec<_> = source
        .iter()
        .flat_map(|byte| parser.feed(&[*byte]))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn unknown_sequences_do_not_become_buttons_and_long_prefix_is_bounded() {
    let mut parser = InputParser::default();
    assert!(parser.feed(b"\x1b[?1u\x1b[15~\x1b[>3u").is_empty());
    let mut invalid = b"\x1b[".to_vec();
    invalid.extend(vec![b'1'; 2048]);
    assert!(parser.feed(&invalid).is_empty());
    assert_eq!(
        parser.feed(b"x"),
        vec![event(Button::A, Action::Press, false)]
    );
}

#[test]
fn legacy_deadline_resets_on_repeat_and_extended_keys_have_no_deadline() {
    let now = Instant::now();
    let mut pad = Pad::default();
    let mut input = InputController::default();
    input.handle(event(Button::Up, Action::Press, false), &mut pad, now);
    input.expire(&mut pad, now + LEGACY_RELEASE / 2);
    assert!(pad.0[Button::Up as usize]);
    input.handle(
        event(Button::Up, Action::Repeat, false),
        &mut pad,
        now + LEGACY_RELEASE / 2,
    );
    input.expire(&mut pad, now + LEGACY_RELEASE);
    assert!(pad.0[Button::Up as usize]);
    input.expire(&mut pad, now + LEGACY_RELEASE * 2);
    assert!(!pad.0[Button::Up as usize]);
    input.handle(event(Button::A, Action::Press, true), &mut pad, now);
    input.handle(event(Button::B, Action::Press, true), &mut pad, now);
    input.expire(&mut pad, now + LEGACY_RELEASE * 100);
    assert!(pad.0[0] && pad.0[1]);
    input.handle(event(Button::A, Action::Release, true), &mut pad, now);
    assert!(!pad.0[0] && pad.0[1]);
    input.release_all(&mut pad);
    assert!(!pad.0.iter().any(|x| *x));
}

#[test]
fn pause_and_quit_ignore_repeat_and_release() {
    let now = Instant::now();
    let mut input = InputController::default();
    let mut pad = Pad::default();
    for control in [Control::Pause, Control::Quit] {
        for action in [Action::Press, Action::Repeat, Action::Release] {
            let result = input.handle(
                InputEvent {
                    key: Key::Control(control),
                    action,
                    extended: true,
                },
                &mut pad,
                now,
            );
            assert_eq!(
                result,
                if action == Action::Press {
                    Some(control)
                } else {
                    None
                }
            );
        }
    }
}
