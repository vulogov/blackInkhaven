//! Targeted in-place HJSON edits used by the live-toggle
//! chords (`Ctrl+B L` switches LLM provider, `Ctrl+B E`
//! toggles sound). The strategy is a surgical text edit (no
//! full re-serialisation) so the carefully-annotated default
//! HJSON template — comments, key ordering, indentation,
//! trailing per-line comments — survives a toggle.
//!
//! Extracted from `tui::app` in the 1.2.7 refactor.

/// Rewrite `<block>.<key> = <value_lit>` in an existing HJSON
/// config file in place, preserving every other byte.
///
/// `value_lit` is the literal text to write (already quoted /
/// formatted by the caller — e.g. `"ollama"` for a string,
/// `true` for a bool). When the key isn't present we insert
/// it right after the opening `{` of the block.
///
/// Returns Err with a human-readable reason when the file
/// shape doesn't match our expectations (no block of that
/// name, unterminated braces). The brace counter doesn't
/// understand HJSON strings — it would miscount a `{` / `}`
/// inside a quoted string. Fine for our shipped template,
/// which uses braces only for nested objects.
pub(super) fn set_key_in_hjson_block(
    raw: &str,
    block: &str,
    key: &str,
    value_lit: &str,
) -> Result<String, String> {
    let lines: Vec<&str> = raw.split_inclusive('\n').collect();
    if lines.is_empty() {
        return Err("config file is empty".into());
    }

    let block_prefix = format!("{block}:");
    let block_open_idx = lines.iter().position(|l| {
        let trimmed = l.trim_start();
        !trimmed.starts_with("//") && trimmed.starts_with(&block_prefix)
    });
    let block_open_idx = block_open_idx
        .ok_or_else(|| format!("no `{block}:` block found in HJSON"))?;

    // Walk forward tracking brace depth (ignoring `//` line
    // comments) so we know where the block ends.
    let mut depth: i32 = 0;
    let mut block_started = false;
    let mut block_end: Option<usize> = None;
    for (i, line) in lines.iter().enumerate().skip(block_open_idx) {
        let code = line.split("//").next().unwrap_or("");
        for c in code.chars() {
            match c {
                '{' => {
                    depth += 1;
                    block_started = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        if block_started && depth == 0 {
            block_end = Some(i);
            break;
        }
    }
    let block_end = block_end
        .ok_or_else(|| format!("unterminated `{block}: {{` block — check brace balance"))?;

    // Scan for the target key as a *direct* child of the
    // block (depth == 1 at the time the line starts being
    // read).
    let key_unquoted = format!("{key}:");
    let key_quoted = format!("\"{key}\":");
    let mut depth: i32 = 0;
    let mut target_idx: Option<usize> = None;
    for (i, line) in lines.iter().enumerate().take(block_end + 1).skip(block_open_idx) {
        let depth_before = depth;
        let code = line.split("//").next().unwrap_or("");
        for c in code.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if i == block_open_idx {
            continue;
        }
        if depth_before == 1 {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with(&key_unquoted) || trimmed.starts_with(&key_quoted) {
                target_idx = Some(i);
                break;
            }
        }
    }

    let mut out = String::with_capacity(raw.len() + value_lit.len());
    match target_idx {
        Some(idx) => {
            let mut rewrote = false;
            for (i, line) in lines.iter().enumerate() {
                if i == idx {
                    rewrote = true;
                    let (eol, core): (&str, &str) =
                        if let Some(stripped) = line.strip_suffix("\r\n") {
                            ("\r\n", stripped)
                        } else if let Some(stripped) = line.strip_suffix('\n') {
                            ("\n", stripped)
                        } else {
                            ("", *line)
                        };
                    let colon_pos = core.find(':').ok_or_else(|| {
                        format!("`{key}` line missing `:` separator — unexpected HJSON")
                    })?;
                    let head = &core[..=colon_pos]; // includes ":"
                    let tail = &core[colon_pos + 1..];
                    let comment_pos = tail.find("//");
                    let (_old_value, comment_suffix) = match comment_pos {
                        Some(p) => (&tail[..p], &tail[p..]),
                        None => (tail, ""),
                    };
                    if comment_suffix.is_empty() {
                        out.push_str(&format!("{head} {value_lit}{eol}"));
                    } else {
                        // Keep one space between the new value
                        // and the trailing comment so it doesn't
                        // slide left.
                        out.push_str(&format!("{head} {value_lit}  {comment_suffix}{eol}"));
                    }
                } else {
                    out.push_str(line);
                }
            }
            if !rewrote {
                return Err("internal error: target line not rewritten".into());
            }
            Ok(out)
        }
        None => {
            // Insert the missing key right after the block-
            // opening line, using two extra spaces of
            // indentation relative to it.
            let block_indent: String = lines[block_open_idx]
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            let child_indent = format!("{block_indent}  ");
            for (i, line) in lines.iter().enumerate() {
                out.push_str(line);
                if i == block_open_idx {
                    let eol = if line.ends_with("\r\n") {
                        "\r\n"
                    } else if line.ends_with('\n') {
                        "\n"
                    } else {
                        "\n"
                    };
                    out.push_str(&format!("{child_indent}{key}: {value_lit}{eol}"));
                }
            }
            Ok(out)
        }
    }
}

/// Wrapper that quotes `new_default` if needed and delegates
/// to `set_key_in_hjson_block` for the `llm.default` slot.
pub(super) fn set_llm_default_in_hjson(
    raw: &str,
    new_default: &str,
) -> Result<String, String> {
    let quote_needed = new_default.is_empty()
        || !new_default
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let value_lit = if quote_needed {
        format!(
            "\"{}\"",
            new_default.replace('\\', "\\\\").replace('"', "\\\"")
        )
    } else {
        new_default.to_string()
    };
    set_key_in_hjson_block(raw, "llm", "default", &value_lit)
}

/// Set `sound.enabled = true|false` in inkhaven.hjson.
/// Inserts the key (and synthesises the block when missing)
/// when the user has stripped them from an older config.
pub(super) fn set_sound_enabled_in_hjson(
    raw: &str,
    enabled: bool,
) -> Result<String, String> {
    let value_lit = if enabled { "true" } else { "false" };
    match set_key_in_hjson_block(raw, "sound", "enabled", value_lit) {
        Ok(s) => Ok(s),
        Err(reason) if reason.contains("no `sound:` block") => {
            insert_sound_block_before_root_close(raw, value_lit)
        }
        Err(other) => Err(other),
    }
}

/// Append a fresh `sound: { ... }` block just *inside* the
/// root object's closing `}`. Older configs predating the
/// sound feature don't have the block at all — the toggle
/// synthesises one. The previous version of this helper
/// appended after the file end, which landed the block
/// *outside* the root and broke parsing on next launch.
pub(super) fn insert_sound_block_before_root_close(
    raw: &str,
    value_lit: &str,
) -> Result<String, String> {
    let lines: Vec<&str> = raw.split_inclusive('\n').collect();
    // Scan backward for the root object's closing brace —
    // the last line whose first non-whitespace character is
    // `}` and whose code (stripped of `//` comments)
    // contains *only* whitespace + `}`.
    let root_close_idx = lines.iter().enumerate().rev().find_map(|(i, l)| {
        let code = l.split("//").next().unwrap_or("");
        let trimmed = code.trim();
        if trimmed == "}" {
            Some(i)
        } else {
            None
        }
    });
    let root_close_idx = root_close_idx.ok_or_else(|| {
        "no root closing `}` found — file shape unrecognised".to_string()
    })?;

    let block = format!(
        "\n  // Typewriter SFX (Ctrl+B E to toggle).\n  sound: {{\n    enabled: {value_lit}\n    volume: 0.6\n  }}\n"
    );

    let mut out = String::with_capacity(raw.len() + block.len());
    for (i, line) in lines.iter().enumerate() {
        if i == root_close_idx {
            out.push_str(&block);
        }
        out.push_str(line);
    }
    Ok(out)
}
