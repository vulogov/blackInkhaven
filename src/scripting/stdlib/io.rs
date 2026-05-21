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
use rust_multistackvm::multistackvm::VM;
use std::cell::RefCell;

thread_local! {
    static PRINT_BUFFER: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Register inkhaven's `print` / `println` overrides on `vm`.
/// Replaces bundcore's defaults via `register_inline` upsert.
pub fn register(vm: &mut VM) -> Result<()> {
    vm.register_inline("print".to_string(), ink_print)
        .map_err(|e| anyhow!("register print: {e}"))?;
    vm.register_inline("println".to_string(), ink_println)
        .map_err(|e| anyhow!("register println: {e}"))?;
    Ok(())
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
    PRINT_BUFFER.with(|b| {
        let mut g = b.borrow_mut();
        g.push_str(&text);
        if newline {
            g.push('\n');
        }
    });
    Ok(vm)
}
