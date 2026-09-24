//! A cheap depth pre-check for untrusted HJSON.
//!
//! `serde_hjson`'s parser (and our comment-preserving config walker) recurse
//! once per object/array nesting level. Deeply-nested input therefore overflows
//! the native stack into an **uncatchable `SIGABRT`** — `std::panic::catch_unwind`
//! does not catch a stack overflow, so the graceful config-load fallbacks cannot
//! save it and the process dies (at startup, if it's `inkhaven.hjson`).
//!
//! Scanning the brace/bracket depth first — before handing the string to the
//! recursive parser — turns that hard crash into a clean, recoverable error.

/// Maximum object/array nesting depth accepted from an HJSON document. Real
/// config / world / lexicon files nest a handful of levels; 128 is far above any
/// legitimate use and far below the stack-overflow threshold.
pub const MAX_HJSON_DEPTH: usize = 128;

/// Return an error if `s` nests objects/arrays deeper than [`MAX_HJSON_DEPTH`].
///
/// A single cheap pass that skips braces inside quoted strings and `#` / `//` /
/// `/* */` comments. It is deliberately conservative: a stray unbalanced brace
/// inside a *quoteless* string is counted (leaning toward a false reject rather
/// than under-protecting), but the 128-level headroom absorbs that for any real
/// document — while a genuinely deep `{{{…}}}` payload is caught before it can
/// reach the recursive parser.
pub fn check_hjson_depth(s: &str) -> Result<(), String> {
    let b = s.as_bytes();
    let mut depth: usize = 0;
    let mut in_str: Option<u8> = None; // Some(quote byte) while inside a "…" / '…'
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = in_str {
            if c == b'\\' {
                i += 2; // skip the escaped char
                continue;
            }
            if c == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match c {
            // A quote opens a string only where a VALUE or KEY can start —
            // after `:` `,` `[` `{` or at a line start. An apostrophe inside a
            // quoteless value (`note: don't`) must not swallow the rest of the
            // document, which would blind the guard to every brace after it.
            b'"' | b'\'' => {
                let prev = b[..i].iter().rev().find(|&&p| p != b' ' && p != b'\t').copied();
                if matches!(prev, None | Some(b':') | Some(b',') | Some(b'[') | Some(b'{') | Some(b'\n')) {
                    in_str = Some(c);
                }
            }
            // After `key:`, a value that does not start a nested object/array,
            // a quoted string, or a comment is a QUOTELESS string running to the
            // end of the line — braces and quotes inside it are text, not
            // structure, so skip the line whole.
            b':' => {
                let mut j = i + 1;
                while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
                    j += 1;
                }
                let starts_structure = j >= b.len()
                    || matches!(b[j], b'{' | b'[' | b'"' | b'\'' | b'\n' | b'\r' | b'#')
                    || (b[j] == b'/' && j + 1 < b.len() && matches!(b[j + 1], b'/' | b'*'));
                if !starts_structure {
                    while j < b.len() && b[j] != b'\n' {
                        j += 1;
                    }
                    i = j;
                    continue;
                }
            }
            b'#' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_HJSON_DEPTH {
                    return Err(format!(
                        "HJSON nests deeper than {MAX_HJSON_DEPTH} levels — refusing to \
                         parse (stack-overflow guard)"
                    ));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_apostrophe_in_a_quoteless_value_does_not_blind_the_guard() {
        let deep = "[".repeat(MAX_HJSON_DEPTH + 5);
        let doc = format!("{{\n  note: don't\n  a: {deep}\n}}");
        assert!(check_hjson_depth(&doc).is_err(), "depth past the apostrophe must still count");
        let doc = format!("{{\n  note: 5\" tall\n  a: {deep}\n}}");
        assert!(check_hjson_depth(&doc).is_err(), "an unbalanced double quote in a quoteless value too");
        // Legit quoteless values with braces/quotes inside stay fine.
        let ok = "{\n  name: The {Ninth} Lantern\n  note: it's \"fine\"\n  n: { x: 1 }\n}";
        assert!(check_hjson_depth(ok).is_ok());
        // Quoted keys/values still open strings normally.
        let ok = "{ \"a\": \"{{{{{{\", 'b': '[[[[', c: [ { d: 1 } ] }";
        assert!(check_hjson_depth(ok).is_ok());
    }

    #[test]
    fn accepts_ordinary_nesting() {
        assert!(check_hjson_depth("{ a: { b: [ { c: 1 } ] } }").is_ok());
        assert!(check_hjson_depth("{}").is_ok());
        // braces inside a quoted string don't count
        assert!(check_hjson_depth(r#"{ url: "http://x/{{{{{{" }"#).is_ok());
        // braces inside a comment don't count
        assert!(check_hjson_depth("{ a: 1 } # {{{{{{{{{").is_ok());
    }

    #[test]
    fn rejects_pathological_nesting() {
        let deep = "{".repeat(MAX_HJSON_DEPTH + 5);
        assert!(check_hjson_depth(&deep).is_err());
    }

    #[test]
    fn boundary_is_inclusive() {
        assert!(check_hjson_depth(&"[".repeat(MAX_HJSON_DEPTH)).is_ok());
        assert!(check_hjson_depth(&"[".repeat(MAX_HJSON_DEPTH + 1)).is_err());
    }
}
