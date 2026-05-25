use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Parsed representation of a keybinding string from the HJSON config such as
/// `"Ctrl+s"`, `"Ctrl+Shift+c"`, `"Tab"`, `"PageUp"`, `"F2"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChord {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyChord {
    /// Format the chord in the same `Ctrl+key` / `Shift+F1` /
    /// `PageUp` shape the parser accepts. Used by `ink.key.list`
    /// and the auto-generated status-bar hints so the user can
    /// copy-paste an output chord back into HJSON unchanged.
    pub fn to_display_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl".into());
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt".into());
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift".into());
        }
        if self.modifiers.contains(KeyModifiers::SUPER) {
            parts.push("Super".into());
        }
        let key = match self.code {
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "Tab".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::Insert => "Insert".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::F(n) => format!("F{n}"),
            KeyCode::Null => "Null".to_string(),
            other => format!("{other:?}"),
        };
        parts.push(key);
        parts.join("+")
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let mut mods = KeyModifiers::empty();
        let mut code: Option<KeyCode> = None;
        let mut shift_present = false;

        for raw in s.split('+') {
            let token = raw.trim();
            if token.is_empty() {
                continue;
            }
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => mods.insert(KeyModifiers::CONTROL),
                "shift" => {
                    shift_present = true;
                }
                "alt" | "meta" | "option" => mods.insert(KeyModifiers::ALT),
                "super" | "cmd" | "command" => mods.insert(KeyModifiers::SUPER),
                _ => {
                    if code.is_some() {
                        return Err(format!("more than one key in `{s}`"));
                    }
                    code = Some(parse_code(token)?);
                }
            }
        }

        let mut code = code.ok_or_else(|| format!("no key code in `{s}`"))?;

        // Normalize: a single-letter chord like "Shift+a" stores Char('A') in
        // many terminals, while "Ctrl+Shift+a" stores Char('a') with both
        // CONTROL and SHIFT. Make matching predictable by always upper-casing
        // a Char when Shift is part of the chord and lower-casing otherwise.
        if let KeyCode::Char(c) = code {
            if shift_present {
                mods.insert(KeyModifiers::SHIFT);
                code = KeyCode::Char(c.to_ascii_uppercase());
            } else if c.is_ascii_alphabetic() {
                code = KeyCode::Char(c.to_ascii_lowercase());
            }
        } else if shift_present {
            mods.insert(KeyModifiers::SHIFT);
        }

        Ok(Self { code, modifiers: mods })
    }

    pub fn matches(&self, ev: &KeyEvent) -> bool {
        // BackTab arrives when Shift+Tab is pressed in some terminals.
        let mut ev_mods = ev.modifiers;
        let ev_code = match ev.code {
            KeyCode::BackTab => {
                ev_mods.insert(KeyModifiers::SHIFT);
                KeyCode::Tab
            }
            KeyCode::Char(c) if c.is_ascii_alphabetic() => {
                // Different terminals diverge:
                //   * Kitty / iTerm2 disambiguation: Shift+p → Char('P') + SHIFT.
                //   * Most TTYs: Shift+p → Char('P'), no SHIFT modifier flag
                //     (case carries the info; SHIFT is implicit).
                //   * `Ctrl+Shift+p` in any terminal: Char('p') + CONTROL+SHIFT.
                // Normalize all three onto "Char(uppercase) + SHIFT" when the
                // letter is upper, "Char(lowercase) without SHIFT" otherwise,
                // so chord lookup is deterministic.
                if ev_mods.contains(KeyModifiers::SHIFT) {
                    KeyCode::Char(c.to_ascii_uppercase())
                } else if c.is_ascii_uppercase() {
                    // Terminal sent a capital letter without the explicit
                    // SHIFT bit (the common legacy path). Promote so a
                    // chord parsed as `Shift+p` (stored as Char('P') + SHIFT)
                    // still matches.
                    ev_mods.insert(KeyModifiers::SHIFT);
                    KeyCode::Char(c)
                } else {
                    KeyCode::Char(c.to_ascii_lowercase())
                }
            }
            other => other,
        };
        // Restrict to modifiers we care about (ignore NUM_LOCK, etc.).
        let mask = KeyModifiers::CONTROL
            | KeyModifiers::SHIFT
            | KeyModifiers::ALT
            | KeyModifiers::SUPER;
        ev_code == self.code && (ev_mods & mask) == (self.modifiers & mask)
    }
}

fn parse_code(name: &str) -> Result<KeyCode, String> {
    let lower = name.to_ascii_lowercase();
    Ok(match lower.as_str() {
        "tab" => KeyCode::Tab,
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdown" | "pgdn" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        // `fN` where N is 1..24. Length bound 2..=3 excludes the
        // bare letter "f" (which is a normal Char chord) but
        // includes "f1" through "f24".
        s if s.starts_with('f') && s.len() >= 2 && s.len() <= 3 => {
            let n: u8 = s[1..]
                .parse()
                .map_err(|_| format!("bad function key `{name}`"))?;
            if !(1..=24).contains(&n) {
                return Err(format!("function key {n} out of range"));
            }
            KeyCode::F(n)
        }
        s if s.chars().count() == 1 => {
            let c = s.chars().next().unwrap();
            KeyCode::Char(c)
        }
        _ => return Err(format!("unknown key `{name}`")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn parse_ctrl_s() {
        let k = KeyChord::parse("Ctrl+s").unwrap();
        assert!(k.matches(&ev(KeyCode::Char('s'), KeyModifiers::CONTROL)));
        assert!(!k.matches(&ev(KeyCode::Char('s'), KeyModifiers::NONE)));
    }

    #[test]
    fn parse_ctrl_slash() {
        let k = KeyChord::parse("Ctrl+/").unwrap();
        assert!(k.matches(&ev(KeyCode::Char('/'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn parse_shift_tab() {
        let k = KeyChord::parse("Shift+Tab").unwrap();
        assert!(k.matches(&ev(KeyCode::Tab, KeyModifiers::SHIFT)));
        assert!(k.matches(&ev(KeyCode::BackTab, KeyModifiers::NONE)));
    }

    #[test]
    fn parse_pageup() {
        let k = KeyChord::parse("PageUp").unwrap();
        assert!(k.matches(&ev(KeyCode::PageUp, KeyModifiers::NONE)));
    }

    #[test]
    fn parse_ctrl_shift_letter() {
        let k = KeyChord::parse("Ctrl+Shift+c").unwrap();
        let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert!(k.matches(&ev(KeyCode::Char('C'), mods)));
        // Some terminals send lowercase + SHIFT instead of uppercase.
        assert!(k.matches(&ev(KeyCode::Char('c'), mods)));
    }

    /// 1.2.8 regression test for the case that exposed the
    /// `Ctrl+V Shift+P` ≡ `Ctrl+V p` bug: legacy terminals
    /// without the kitty disambiguation send Shift+letter
    /// as an uppercase Char WITHOUT the SHIFT modifier flag
    /// — case carries the info instead. The chord matcher
    /// has to treat that as implicit-Shift or the dispatch
    /// table collapses the two chords onto whichever entry
    /// is checked first.
    #[test]
    fn shift_letter_matches_uppercase_without_shift_flag() {
        let k = KeyChord::parse("Shift+p").unwrap();
        // The terminal-as-modern case: explicit SHIFT bit.
        assert!(k.matches(&ev(KeyCode::Char('P'), KeyModifiers::SHIFT)));
        // The legacy case: uppercase letter with no modifier.
        assert!(k.matches(&ev(KeyCode::Char('P'), KeyModifiers::NONE)));
        // Bare lowercase 'p' must NOT match — otherwise the
        // distinct `p` chord can't be bound separately.
        assert!(!k.matches(&ev(KeyCode::Char('p'), KeyModifiers::NONE)));
    }

    #[test]
    fn plain_letter_chord_rejects_uppercase() {
        // Symmetric guard: `p` chord must NOT match Shift+P.
        let k = KeyChord::parse("p").unwrap();
        assert!(k.matches(&ev(KeyCode::Char('p'), KeyModifiers::NONE)));
        assert!(!k.matches(&ev(KeyCode::Char('P'), KeyModifiers::NONE)));
        assert!(!k.matches(&ev(KeyCode::Char('P'), KeyModifiers::SHIFT)));
    }
}
