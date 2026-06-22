//! Inkhaven-flavoured `print` / `println` overrides.
//!
//! Bundcore's default `print` / `println` handlers call Rust's
//! `print!()` macro — fine for CLI usage, fatal in the TUI where
//! stdout is in raw mode and printing under the alternate-screen
//! buffer corrupts the rendered frame.
//!
//! We re-register both words on the Adam VM so they accumulate
//! into a thread-local string buffer instead. Callers (the CLI
//! `bund` subcommand, the TUI Ctrl+Z E modal) read the buffer
//! after `eval` returns and route the captured text to the
//! appropriate channel — terminal stdout for CLI, the status bar
//! for TUI.

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::types::STRING;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use super::helpers::{active_store, pull, push, require_depth, value_to_string};
use crate::pane::output::{kinds, Lifetime, Message, OutputStore, Severity};

thread_local! {
    static PRINT_BUFFER: RefCell<String> = const { RefCell::new(String::new()) };
    /// PANE-1 P1 — the active project's Output store, opened once per process.
    /// Re-opening the same `output.db` would create a second DuckDB instance and
    /// conflict, so it is cached by project root.
    static OUTPUT_STORE: RefCell<Option<(PathBuf, OutputStore)>> = const { RefCell::new(None) };
}

/// Register inkhaven's `print` / `println` overrides plus the `ink.io.*` Output
/// family on `vm`. Replaces bundcore's defaults via `register_inline` upsert.
pub fn register(vm: &mut VM) -> Result<()> {
    vm.register_inline("print".to_string(), ink_print)
        .map_err(|e| anyhow!("register print: {e}"))?;
    vm.register_inline("println".to_string(), ink_println)
        .map_err(|e| anyhow!("register println: {e}"))?;

    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.io.print", w_io_print),
        ("ink.io.log", w_io_log),
        ("ink.io.notify", w_io_notify),
        ("ink.io.message.list", w_io_msg_list),
        ("ink.io.message.count", w_io_msg_count),
        ("ink.io.message.dismiss", w_io_msg_dismiss),
        ("ink.io.message.pin", w_io_msg_pin),
        ("ink.io.message.unpin", w_io_msg_unpin),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f)
            .map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(format!("{e}"))
}

/// The active project's Output store (cached per process; see [`OUTPUT_STORE`]).
fn output_store(tag: &str) -> Result<OutputStore> {
    let store = active_store(tag)?;
    let root = store.project_root().to_path_buf();
    OUTPUT_STORE.with(|c| {
        let mut g = c.borrow_mut();
        if let Some((cached, os)) = g.as_ref() {
            if *cached == root {
                return Ok(os.clone());
            }
        }
        let os = OutputStore::open_for_project(&root).map_err(|e| anyhow!("{tag}: {e}"))?;
        *g = Some((root, os.clone()));
        Ok(os)
    })
}

// ( text -- )  emit a bund_print message to the Output pane
fn w_io_print(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_io_print(vm).map_err(to_bund_err)
}
fn do_io_print(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.io.print";
    require_depth(vm, 1, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    let os = output_store(tag)?;
    let msg = Message::new(
        kinds::BUND_PRINT,
        Severity::Info,
        Lifetime::Session(100),
        serde_json::json!({ "text": text }),
    );
    os.emit(&msg).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ( text level -- )  emit a bund_log message; severity from level
fn w_io_log(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_io_log(vm).map_err(to_bund_err)
}
fn do_io_log(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.io.log";
    require_depth(vm, 2, tag)?;
    let level = value_to_string(pull(vm, tag)?, "level", tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    let severity = match level.as_str() {
        "warn" | "warning" => Severity::Warning,
        "error" => Severity::Contradiction,
        _ => Severity::Info,
    };
    let os = output_store(tag)?;
    let msg = Message::new(
        kinds::BUND_LOG,
        severity,
        Lifetime::Session(200),
        serde_json::json!({ "text": text, "level": level }),
    );
    os.emit(&msg).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ( kind metadata-dict -- id )  emit a structured message of arbitrary kind
fn w_io_notify(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_io_notify(vm).map_err(to_bund_err)
}
fn do_io_notify(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.io.notify";
    require_depth(vm, 2, tag)?;
    let metadata_v = pull(vm, tag)?;
    let kind = value_to_string(pull(vm, tag)?, "kind", tag)?;
    let metadata = crate::scripting::value_to_json(&metadata_v);
    let os = output_store(tag)?;
    let msg = Message::new(kind, Severity::Info, Lifetime::UntilActedOn, metadata);
    let id = os.emit(&msg).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_string(id.to_string()));
    Ok(vm)
}

/// Build a Bund dict view of a message ({id, kind, severity, text}).
fn message_dict(m: &Message) -> Value {
    let mut d: HashMap<String, Value> = HashMap::new();
    d.insert("id".into(), Value::from_string(m.id.to_string()));
    d.insert("kind".into(), Value::from_string(m.kind.clone()));
    d.insert("severity".into(), Value::from_string(m.severity.as_str().to_string()));
    let text =
        m.metadata.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
    d.insert("text".into(), Value::from_string(text));
    Value::from_dict(d)
}

// ( kind -- list )  active messages ("" = all)
fn w_io_msg_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_io_msg_list(vm).map_err(to_bund_err)
}
fn do_io_msg_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.io.message.list";
    require_depth(vm, 1, tag)?;
    let kind = value_to_string(pull(vm, tag)?, "kind", tag)?;
    let os = output_store(tag)?;
    let msgs = if kind.trim().is_empty() {
        os.active()
    } else {
        os.by_kind(&kind)
    }
    .map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_list(msgs.iter().map(message_dict).collect()));
    Ok(vm)
}

// ( kind -- n )  count active messages ("" = all)
fn w_io_msg_count(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_io_msg_count(vm).map_err(to_bund_err)
}
fn do_io_msg_count(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.io.message.count";
    require_depth(vm, 1, tag)?;
    let kind = value_to_string(pull(vm, tag)?, "kind", tag)?;
    let os = output_store(tag)?;
    let k = (!kind.trim().is_empty()).then_some(kind.as_str());
    let n = os.count_active(k).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_int(n as i64));
    Ok(vm)
}

fn pull_id(vm: &mut VM, tag: &str) -> Result<uuid::Uuid> {
    let s = value_to_string(pull(vm, tag)?, "id", tag)?;
    uuid::Uuid::parse_str(&s).map_err(|e| anyhow!("{tag}: bad id `{s}`: {e}"))
}

// ( id -- )  dismiss a message
fn w_io_msg_dismiss(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_io_msg_dismiss(vm).map_err(to_bund_err)
}
fn do_io_msg_dismiss(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.io.message.dismiss";
    require_depth(vm, 1, tag)?;
    let id = pull_id(vm, tag)?;
    output_store(tag)?.dismiss(id).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ( id -- )  pin a message
fn w_io_msg_pin(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_io_msg_pin(vm).map_err(to_bund_err)
}
fn do_io_msg_pin(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.io.message.pin";
    require_depth(vm, 1, tag)?;
    let id = pull_id(vm, tag)?;
    output_store(tag)?.set_pinned(id, true).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ( id -- )  unpin a message
fn w_io_msg_unpin(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_io_msg_unpin(vm).map_err(to_bund_err)
}
fn do_io_msg_unpin(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.io.message.unpin";
    require_depth(vm, 1, tag)?;
    let id = pull_id(vm, tag)?;
    output_store(tag)?.set_pinned(id, false).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

/// Drain and return the captured buffer. Resets the buffer to
/// empty as a side effect — call exactly once per eval cycle.
pub fn drain_print_buffer() -> String {
    PRINT_BUFFER.with(|b| std::mem::take(&mut *b.borrow_mut()))
}

fn ink_print(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    capture(vm, false)
}

fn ink_println(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    capture(vm, true)
}

/// Pull the top of stack, format as STRING, append to the buffer.
/// Empty stack errors with the same wording bundcore's default
/// emits so existing scripts see the same exception shape.
fn capture(vm: &mut VM, newline: bool) -> std::result::Result<&mut VM, BundError> {
    let value = match vm.stack.pull() {
        Some(v) => v,
        None => {
            return Err(easy_error::err_msg(
                "PRINT returns: NO DATA",
            ));
        }
    };
    // Mirror bundcore's default: convert any type to STRING first
    // (so `42 println` works), then read the converted Value as a
    // plain Rust String for the buffer.
    let str_value = value
        .conv(STRING)
        .map_err(|e| easy_error::err_msg(format!("PRINT conv: {e}")))?;
    let text = str_value
        .cast_string()
        .map_err(|e| easy_error::err_msg(format!("PRINT cast: {e}")))?;
    // Smart routing: if a Bund output pane is open on the
    // active App, append there so multi-line script output
    // doesn't clobber the status bar. Otherwise fall back to
    // the print buffer that the eval caller drains.
    let routed_to_pane = crate::scripting::with_active_app(|app| {
        app.append_to_bund_pane(&text, newline)
    })
    .unwrap_or(false);
    if !routed_to_pane {
        PRINT_BUFFER.with(|b| {
            let mut g = b.borrow_mut();
            g.push_str(&text);
            if newline {
                g.push('\n');
            }
        });
    }
    Ok(vm)
}
