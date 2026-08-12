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
            b'"' | b'\'' => in_str = Some(c),
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
