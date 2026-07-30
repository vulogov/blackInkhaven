//! A minimal, reusable full-screen-TUI shell: the raw-mode / alternate-screen
//! lifecycle and the draw → poll → dispatch event loop that inkhaven's
//! terminal companions share. Everything app-specific — panes, async pollers,
//! key handling — lives behind [`TuiHost`]; the shell owns only the boilerplate
//! every such app repeats verbatim.
//!
//! The Research companion (`research::app::ResearchApp`) is the first consumer;
//! the 1.7 Linguistic companion is the second. Both restore the terminal
//! cleanly on panic via [`with_terminal`], and both run the exact cadence the
//! Research TUI has used since R-P1 — extracted here, not reinvented per app.
//!
//! Out of scope: the main editor TUI (`tui::app`) keeps its own bespoke loop;
//! this shell is for the lighter companion apps.

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseEvent};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::{Backend, CrosstermBackend};

/// An application [`run_loop`] can drive. Implementors supply only what is
/// genuinely app-specific; the loop supplies the draw/poll/dispatch cadence.
pub trait TuiHost {
    /// The loop stops when this returns `true`.
    fn should_quit(&self) -> bool;

    /// Drain any background / async work before drawing. Default: nothing —
    /// override to poll channels, advance streams, etc.
    fn poll_async(&mut self) {}

    /// Per-frame bookkeeping, run after [`poll_async`](Self::poll_async) and
    /// before the draw (e.g. spinner cadence, elapsed-time tracking). Default:
    /// nothing.
    fn tick(&mut self) {}

    /// Render the current frame. Immutable — rendering must not mutate state.
    fn render(&self, frame: &mut Frame);

    /// Handle a pressed key. `Release` events are filtered out before this is
    /// called, so implementors see presses and repeats only.
    fn on_key(&mut self, key: KeyEvent);

    /// Handle a mouse event. Default no-op — a host only receives these if it
    /// enables mouse capture on its terminal (most companions don't, so their
    /// native selection/scroll is preserved).
    fn on_mouse(&mut self, _mouse: MouseEvent) {}

    /// How long each frame blocks waiting for input. Default 100 ms (≈10 fps
    /// when idle); lower it for snappier animation, raise it to idle cheaper.
    fn poll_interval(&self) -> Duration {
        Duration::from_millis(100)
    }
}

/// Where the loop gets its key events. Abstracted so [`run_loop`] can run
/// against the real terminal in production and a scripted source in tests
/// (crossterm's global event reader needs a real tty and cannot be mocked).
pub trait InputSource {
    /// Wait up to `timeout` for input. Return `Ok(Some(key))` for a pressed or
    /// repeated key, `Ok(None)` if the wait elapsed with nothing dispatchable
    /// (no event, or a filtered `Release`).
    fn next_key(&mut self, timeout: Duration) -> Result<Option<KeyEvent>>;

    /// Wait up to `timeout` for any host input (key or mouse). The default only
    /// yields keys (via [`next_key`](InputSource::next_key)); the production
    /// [`CrosstermInput`] overrides it to also surface mouse events.
    fn next_input(&mut self, timeout: Duration) -> Result<Option<HostInput>> {
        Ok(self.next_key(timeout)?.map(HostInput::Key))
    }
}

/// A dispatchable input event from an [`InputSource`].
pub enum HostInput {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

/// The production input source: crossterm's global terminal event reader.
pub struct CrosstermInput;

impl InputSource for CrosstermInput {
    fn next_key(&mut self, timeout: Duration) -> Result<Option<KeyEvent>> {
        match self.next_input(timeout)? {
            Some(HostInput::Key(key)) => Ok(Some(key)),
            _ => Ok(None),
        }
    }

    fn next_input(&mut self, timeout: Duration) -> Result<Option<HostInput>> {
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    return Ok(Some(HostInput::Key(key)));
                }
                Event::Mouse(m) => return Ok(Some(HostInput::Mouse(m))),
                _ => {}
            }
        }
        Ok(None)
    }
}

/// Drive `host` on `terminal` until it asks to quit, reading input from the real
/// terminal. Each iteration: poll async work, tick, draw, then wait up to
/// [`poll_interval`](TuiHost::poll_interval) for a keypress and dispatch it.
/// Returns when [`should_quit`](TuiHost::should_quit) goes true or a terminal
/// I/O error occurs.
pub fn run_loop<B, H>(host: &mut H, terminal: &mut Terminal<B>) -> Result<()>
where
    B: Backend,
    H: TuiHost,
{
    run_loop_with(host, terminal, &mut CrosstermInput)
}

/// [`run_loop`] with an injectable [`InputSource`] — the testable core.
pub fn run_loop_with<B, H, I>(
    host: &mut H,
    terminal: &mut Terminal<B>,
    input: &mut I,
) -> Result<()>
where
    B: Backend,
    H: TuiHost,
    I: InputSource,
{
    while !host.should_quit() {
        host.poll_async();
        host.tick();
        terminal.draw(|f| host.render(f))?;
        match input.next_input(host.poll_interval())? {
            Some(HostInput::Key(key)) => host.on_key(key),
            Some(HostInput::Mouse(m)) => host.on_mouse(m),
            None => {}
        }
    }
    Ok(())
}

/// Enter raw mode + the alternate screen (registering a crash-restore hook),
/// run `body` with a ready [`Terminal`], then restore the terminal no matter
/// how `body` returns. The crash hook guarantees a panic inside `body` still
/// leaves the user's terminal usable.
pub fn with_terminal<F, R>(body: F) -> Result<R>
where
    F: FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<R>,
{
    crate::crash::set_terminal_restore(Some(Box::new(|| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    })));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = body(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    crate::crash::set_terminal_restore(None);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;

    /// Counts hook invocations and records dispatched keys. `tick` decrements a
    /// frame budget so the loop terminates deterministically.
    struct RecordingHost {
        remaining: u32,
        ticks: u32,
        polls: u32,
        keys: Vec<KeyCode>,
    }

    impl RecordingHost {
        fn new(frames: u32) -> Self {
            Self { remaining: frames, ticks: 0, polls: 0, keys: Vec::new() }
        }
    }

    impl TuiHost for RecordingHost {
        fn should_quit(&self) -> bool {
            self.remaining == 0
        }
        fn poll_async(&mut self) {
            self.polls += 1;
        }
        fn tick(&mut self) {
            self.ticks += 1;
            self.remaining = self.remaining.saturating_sub(1);
        }
        fn render(&self, frame: &mut Frame) {
            frame.render_widget(Paragraph::new("hi"), frame.area());
        }
        fn on_key(&mut self, key: KeyEvent) {
            self.keys.push(key.code);
        }
        fn poll_interval(&self) -> Duration {
            Duration::from_millis(0)
        }
    }

    /// Yields queued keys one per frame, then `None` forever.
    struct ScriptedInput {
        queue: std::collections::VecDeque<KeyEvent>,
    }

    impl ScriptedInput {
        fn new(keys: Vec<KeyEvent>) -> Self {
            Self { queue: keys.into() }
        }
    }

    impl InputSource for ScriptedInput {
        fn next_key(&mut self, _timeout: Duration) -> Result<Option<KeyEvent>> {
            Ok(self.queue.pop_front())
        }
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn run_loop_ticks_polls_and_draws_each_frame_until_quit() {
        let backend = TestBackend::new(8, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut host = RecordingHost::new(3);
        let mut input = ScriptedInput::new(vec![]);
        run_loop_with(&mut host, &mut terminal, &mut input).unwrap();
        // Three frames: poll_async + tick fire once per frame; then quit.
        assert_eq!(host.ticks, 3);
        assert_eq!(host.polls, 3);
    }

    #[test]
    fn run_loop_is_a_noop_when_already_quit() {
        let backend = TestBackend::new(4, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut host = RecordingHost::new(0);
        let mut input = ScriptedInput::new(vec![]);
        run_loop_with(&mut host, &mut terminal, &mut input).unwrap();
        assert_eq!(host.polls, 0, "quit-on-entry polls nothing");
        assert_eq!(host.ticks, 0);
    }

    #[test]
    fn run_loop_dispatches_queued_keys_to_on_key() {
        let backend = TestBackend::new(8, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut host = RecordingHost::new(3);
        let mut input =
            ScriptedInput::new(vec![press(KeyCode::Char('a')), press(KeyCode::Char('b'))]);
        run_loop_with(&mut host, &mut terminal, &mut input).unwrap();
        assert_eq!(host.keys, vec![KeyCode::Char('a'), KeyCode::Char('b')]);
    }
}
