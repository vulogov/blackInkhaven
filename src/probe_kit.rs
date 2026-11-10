//! Shared pieces of the headless TUI probes (3.20): the key battery, the
//! terminal sizes, and a probe runner that keeps the last panic message so a
//! failed assertion inside a battery is still reported. The main-editor probes
//! under `tui/app/` predate this module and carry their own copies.
#![cfg(test)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

pub(crate) fn k(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
pub(crate) fn km(code: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, m)
}
pub(crate) fn ctrl(c: char) -> KeyEvent {
    km(KeyCode::Char(c), KeyModifiers::CONTROL)
}
pub(crate) fn alt(c: char) -> KeyEvent {
    km(KeyCode::Char(c), KeyModifiers::ALT)
}

pub(crate) fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic>".into()
    }
}

/// The terminal sizes every probe renders at, from "too small for anything"
/// to a wide desktop window.
pub(crate) const SIZES: [(u16, u16); 9] =
    [(8, 3), (20, 6), (40, 12), (60, 20), (79, 24), (80, 24), (100, 30), (120, 40), (200, 60)];

/// Every key a probe presses in a given state: navigation, printable
/// characters, editing keys, function keys, Ctrl and Alt chords.
pub(crate) fn battery() -> Vec<(String, KeyEvent)> {
    let mut v: Vec<(String, KeyEvent)> = Vec::new();
    let mut push = |n: &str, e: KeyEvent| v.push((n.to_string(), e));
    for _ in 0..3 {
        push("Up", k(KeyCode::Up));
    }
    for _ in 0..8 {
        push("Down", k(KeyCode::Down));
    }
    for _ in 0..3 {
        push("PageDown", k(KeyCode::PageDown));
    }
    push("End", k(KeyCode::End));
    push("Down", k(KeyCode::Down));
    for _ in 0..3 {
        push("PageUp", k(KeyCode::PageUp));
    }
    push("Home", k(KeyCode::Home));
    push("Up", k(KeyCode::Up));
    for _ in 0..3 {
        push("Left", k(KeyCode::Left));
    }
    for _ in 0..6 {
        push("Right", k(KeyCode::Right));
    }
    for _ in 0..3 {
        push("Tab", k(KeyCode::Tab));
    }
    push("BackTab", k(KeyCode::BackTab));
    for c in [
        'a', 'b', 'Z', '0', '/', '?', ' ', '.', ':', '-', '+', 'x', 'y', 'n', 'd', 'v', 's', 'e', 'r', 'g',
        'G', 'j', 'k', 'q', 'p', 'c', 't', 'm', 'w', 'f', 'o', 'u', 'i', 'h', 'l', '#', '9', 'é',
    ] {
        push(&format!("Char({c:?})"), k(KeyCode::Char(c)));
    }
    for _ in 0..3 {
        push("Backspace", k(KeyCode::Backspace));
    }
    for _ in 0..2 {
        push("Delete", k(KeyCode::Delete));
    }
    push("Insert", k(KeyCode::Insert));
    for n in 1..=5u8 {
        push(&format!("F{n}"), k(KeyCode::F(n)));
    }
    for c in "abcdefghijklmnoprstuvwxyz".chars() {
        push(&format!("Ctrl+{c}"), ctrl(c));
    }
    push("Alt+b", alt('b'));
    push("Alt+f", alt('f'));
    push("Alt+d", alt('d'));
    push("Shift+Up", km(KeyCode::Up, KeyModifiers::SHIFT));
    push("Shift+Down", km(KeyCode::Down, KeyModifiers::SHIFT));
    push("Shift+Enter", km(KeyCode::Enter, KeyModifiers::SHIFT));
    push("Ctrl+Enter", km(KeyCode::Enter, KeyModifiers::CONTROL));
    push("Ctrl+Right", km(KeyCode::Right, KeyModifiers::CONTROL));
    push("Ctrl+Left", km(KeyCode::Left, KeyModifiers::CONTROL));
    push("Ctrl+Up", km(KeyCode::Up, KeyModifiers::CONTROL));
    push("Ctrl+Down", km(KeyCode::Down, KeyModifiers::CONTROL));
    push("Ctrl+Backspace", km(KeyCode::Backspace, KeyModifiers::CONTROL));
    push("Alt+Backspace", km(KeyCode::Backspace, KeyModifiers::ALT));
    v
}

/// Render one frame with `draw` on a `TestBackend` of the given size,
/// returning the screen text (rows joined by `\n`) or the panic message.
pub(crate) fn render_text(w: u16, h: u16, draw: impl FnOnce(&mut ratatui::Frame)) -> Result<String, String> {
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(draw).map(|_| ())?;
        let buf = term.backend().buffer().clone();
        let mut rows = Vec::with_capacity(h as usize);
        for y in 0..h {
            let mut s = String::new();
            for x in 0..w {
                s.push_str(buf[(x, y)].symbol());
            }
            rows.push(s);
        }
        Ok::<String, std::io::Error>(rows.join("\n"))
    }));
    match r {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => Err(format!("io: {e}")),
        Err(p) => Err(format!("PANIC {}", panic_text(p))),
    }
}

static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

/// Silence panics (the batteries catch hundreds of them on purpose) but keep
/// the last message, so an assertion that fails mid-probe is still reported.
pub(crate) fn install_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic>".into()
        };
        let at = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        *LAST_PANIC.lock().unwrap() = Some(format!("{msg}{at}"));
    }));
}

/// Run a probe body, write + print its report whatever happens, and re-raise
/// a body panic with its message.
pub(crate) fn run_probe(name: &str, body: impl FnOnce(&mut Vec<String>)) {
    install_hook();
    let mut out: Vec<String> = Vec::new();
    let r = catch_unwind(AssertUnwindSafe(|| body(&mut out)));
    let report = out.join("\n");
    let report_path = std::env::temp_dir().join(format!("inkhaven-{name}-probe.txt"));
    std::fs::write(&report_path, &report).unwrap();
    println!("{report}");
    println!("report: {}", report_path.display());
    if r.is_err() {
        let msg = LAST_PANIC.lock().unwrap().clone().unwrap_or_default();
        let _ = std::panic::take_hook();
        panic!("{name} probe failed: {msg}");
    }
}
