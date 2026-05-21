//! Bund stdlib words that mutate App-level state.
//!
//! Phase B of the editor-integration roadmap. Where Phase A
//! (`ink.tree.*`, `ink.paragraph.*`, `ink.db.*`) only touched the
//! project Store — accessible globally via `ACTIVE_STORE` — these
//! words reach into the running `App`'s editor buffer, AI chat
//! history, and Typst scheduler. Access goes through the
//! `with_active_app` helper which dereferences a thread-local
//! raw pointer set by `App::scripting_eval` (see the
//! SAFETY-contract comment on `scripting::ACTIVE_APP`).
//!
//! These words error with "no active App (run inside the TUI)"
//! when invoked from `inkhaven bund` outside a TUI session.
//!
//! ## Categories
//!
//! - `ink.editor.*` → `editor_write` (default-denied)
//! - `ink.editor.text` / `.cursor` / `.find` → `editor_read`
//! - `ink.ai.*` → `ai_write` (default-denied)
//! - `ink.typst.*` → `store_write` — runs Book assembly, which
//!   mutates the artefacts directory.

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{pull, push, require_depth, value_to_i64, value_to_string};

pub fn register(vm: &mut VM) -> Result<()> {
    // ── Editor read ───────────────────────────────────────────
    vm.register_inline("ink.editor.cursor".to_string(), ink_editor_cursor)
        .map_err(|e| anyhow!("register ink.editor.cursor: {e}"))?;
    vm.register_inline("ink.editor.text".to_string(), ink_editor_text)
        .map_err(|e| anyhow!("register ink.editor.text: {e}"))?;
    vm.register_inline("ink.editor.find".to_string(), ink_editor_find)
        .map_err(|e| anyhow!("register ink.editor.find: {e}"))?;

    // ── Editor write ──────────────────────────────────────────
    vm.register_inline("ink.editor.goto".to_string(), ink_editor_goto)
        .map_err(|e| anyhow!("register ink.editor.goto: {e}"))?;
    vm.register_inline("ink.editor.insert".to_string(), ink_editor_insert)
        .map_err(|e| anyhow!("register ink.editor.insert: {e}"))?;
    vm.register_inline("ink.editor.scroll".to_string(), ink_editor_scroll)
        .map_err(|e| anyhow!("register ink.editor.scroll: {e}"))?;
    vm.register_inline("ink.editor.delete_line".to_string(), ink_editor_delete_line)
        .map_err(|e| anyhow!("register ink.editor.delete_line: {e}"))?;
    vm.register_inline("ink.editor.delete_to_bol".to_string(), ink_editor_delete_to_bol)
        .map_err(|e| anyhow!("register ink.editor.delete_to_bol: {e}"))?;
    vm.register_inline("ink.editor.delete_to_eol".to_string(), ink_editor_delete_to_eol)
        .map_err(|e| anyhow!("register ink.editor.delete_to_eol: {e}"))?;

    // ── AI ────────────────────────────────────────────────────
    vm.register_inline("ink.ai.clear_history".to_string(), ink_ai_clear_history)
        .map_err(|e| anyhow!("register ink.ai.clear_history: {e}"))?;
    vm.register_inline("ink.ai.send".to_string(), ink_ai_send)
        .map_err(|e| anyhow!("register ink.ai.send: {e}"))?;
    vm.register_inline("ink.ai.history".to_string(), ink_ai_history)
        .map_err(|e| anyhow!("register ink.ai.history: {e}"))?;
    vm.register_inline(
        "ink.ai.set_system_prompt".to_string(),
        ink_ai_set_system_prompt,
    )
    .map_err(|e| anyhow!("register ink.ai.set_system_prompt: {e}"))?;

    // ── Theme ─────────────────────────────────────────────────
    vm.register_inline("ink.theme.set".to_string(), ink_theme_set)
        .map_err(|e| anyhow!("register ink.theme.set: {e}"))?;

    // ── Editor (Phase C) ──────────────────────────────────────
    vm.register_inline("ink.editor.replace".to_string(), ink_editor_replace)
        .map_err(|e| anyhow!("register ink.editor.replace: {e}"))?;

    // ── Typst ─────────────────────────────────────────────────
    vm.register_inline("ink.typst.assemble".to_string(), ink_typst_assemble)
        .map_err(|e| anyhow!("register ink.typst.assemble: {e}"))?;
    vm.register_inline("ink.typst.build".to_string(), ink_typst_build)
        .map_err(|e| anyhow!("register ink.typst.build: {e}"))?;
    vm.register_inline("ink.typst.take".to_string(), ink_typst_take)
        .map_err(|e| anyhow!("register ink.typst.take: {e}"))?;

    // ── Bund output pane ──────────────────────────────────────
    vm.register_inline("ink.pane.show".to_string(), ink_pane_show)
        .map_err(|e| anyhow!("register ink.pane.show: {e}"))?;
    vm.register_inline("ink.pane.close".to_string(), ink_pane_close)
        .map_err(|e| anyhow!("register ink.pane.close: {e}"))?;
    vm.register_inline("ink.pane.clear".to_string(), ink_pane_clear)
        .map_err(|e| anyhow!("register ink.pane.clear: {e}"))?;
    vm.register_inline("ink.pane.line".to_string(), ink_pane_line)
        .map_err(|e| anyhow!("register ink.pane.line: {e}"))?;

    // ── Bund input modal ──────────────────────────────────────
    vm.register_inline("ink.input".to_string(), ink_input)
        .map_err(|e| anyhow!("register ink.input: {e}"))?;

    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

fn with_app<F, R>(tag: &str, f: F) -> Result<R>
where
    F: FnOnce(&mut crate::tui::app::App) -> Result<R>,
{
    let outcome = crate::scripting::with_active_app(f);
    match outcome {
        Some(r) => r,
        None => Err(anyhow!(
            "{tag}: no active App (run inside the TUI; `inkhaven bund` from the CLI has no editor / AI / typst state)"
        )),
    }
}

// ── ink.editor.cursor ────────────────────────────────────────────────
// Stack: ( -- list[row col] | NODATA )

fn ink_editor_cursor(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_editor_cursor(vm).map_err(to_bund_err)
}

fn do_ink_editor_cursor(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.editor.cursor";
    let cursor = with_app(tag, |app| Ok(app.ink_editor_cursor()))?;
    let val = match cursor {
        Some((row, col)) => Value::from_list(vec![
            Value::from_int(row as i64),
            Value::from_int(col as i64),
        ]),
        None => Value::nodata(),
    };
    push(vm, val);
    Ok(vm)
}

// ── ink.editor.text ──────────────────────────────────────────────────
// Stack: ( -- string | NODATA )

fn ink_editor_text(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_editor_text(vm).map_err(to_bund_err)
}

fn do_ink_editor_text(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.editor.text";
    let text = with_app(tag, |app| Ok(app.ink_editor_text()))?;
    let val = match text {
        Some(s) => Value::from_string(s),
        None => Value::nodata(),
    };
    push(vm, val);
    Ok(vm)
}

// ── ink.editor.find ──────────────────────────────────────────────────
// Stack: ( needle -- list[row col] | NODATA )

fn ink_editor_find(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_editor_find(vm).map_err(to_bund_err)
}

fn do_ink_editor_find(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.editor.find";
    require_depth(vm, 1, tag)?;
    let needle = value_to_string(pull(vm, tag)?, "needle", tag)?;
    let pos = with_app(tag, |app| Ok(app.ink_editor_find(&needle)))?;
    let val = match pos {
        Some((row, col)) => Value::from_list(vec![
            Value::from_int(row as i64),
            Value::from_int(col as i64),
        ]),
        None => Value::nodata(),
    };
    push(vm, val);
    Ok(vm)
}

// ── ink.editor.goto ──────────────────────────────────────────────────
// Stack: ( row col -- )

fn ink_editor_goto(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_editor_goto(vm).map_err(to_bund_err)
}

fn do_ink_editor_goto(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.editor.goto";
    require_depth(vm, 2, tag)?;
    let col = value_to_i64(pull(vm, tag)?, "col", tag)?.max(0) as usize;
    let row = value_to_i64(pull(vm, tag)?, "row", tag)?.max(0) as usize;
    with_app(tag, |app| {
        app.ink_editor_goto(row, col)
            .map_err(|e| anyhow!("{tag}: {e}"))
    })?;
    Ok(vm)
}

// ── ink.editor.insert ────────────────────────────────────────────────
// Stack: ( text -- )

fn ink_editor_insert(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_editor_insert(vm).map_err(to_bund_err)
}

fn do_ink_editor_insert(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.editor.insert";
    require_depth(vm, 1, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    with_app(tag, |app| {
        app.ink_editor_insert(&text)
            .map_err(|e| anyhow!("{tag}: {e}"))
    })?;
    Ok(vm)
}

// ── ink.editor.scroll ────────────────────────────────────────────────
// Stack: ( delta -- )
// Positive scrolls down; negative scrolls up. Clamps at the edges.

fn ink_editor_scroll(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_editor_scroll(vm).map_err(to_bund_err)
}

fn do_ink_editor_scroll(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.editor.scroll";
    require_depth(vm, 1, tag)?;
    let delta = value_to_i64(pull(vm, tag)?, "delta", tag)?;
    with_app(tag, |app| {
        app.ink_editor_scroll(delta as i32)
            .map_err(|e| anyhow!("{tag}: {e}"))
    })?;
    Ok(vm)
}

// ── ink.editor.delete_line / .delete_to_bol / .delete_to_eol ─────────
// Stack: ( -- )

fn ink_editor_delete_line(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_call_unit(vm, "ink.editor.delete_line", |app| app.ink_editor_delete_line())
        .map_err(to_bund_err)
}

fn ink_editor_delete_to_bol(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_call_unit(vm, "ink.editor.delete_to_bol", |app| {
        app.ink_editor_delete_to_bol()
    })
    .map_err(to_bund_err)
}

fn ink_editor_delete_to_eol(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_call_unit(vm, "ink.editor.delete_to_eol", |app| {
        app.ink_editor_delete_to_eol()
    })
    .map_err(to_bund_err)
}

/// Shared shape for words that take no stack args, run one App
/// method that returns `Result<(), String>`, and push nothing.
fn do_call_unit<'a, F>(vm: &'a mut VM, tag: &'static str, f: F) -> Result<&'a mut VM>
where
    F: FnOnce(&mut crate::tui::app::App) -> std::result::Result<(), String>,
{
    with_app(tag, |app| f(app).map_err(|e| anyhow!("{tag}: {e}")))?;
    Ok(vm)
}

// ── ink.ai.clear_history ─────────────────────────────────────────────
// Stack: ( -- )

fn ink_ai_clear_history(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let tag = "ink.ai.clear_history";
    match with_app(tag, |app| {
        app.ink_ai_clear_history();
        Ok::<_, anyhow::Error>(())
    }) {
        Ok(_) => Ok(vm),
        Err(e) => Err(to_bund_err(e)),
    }
}

// ── ink.typst.assemble / .build / .take ──────────────────────────────
// Stack: ( -- )
// Schedule asynchronously — these post a background task; the
// script returns immediately. Result lands on the TUI's status bar.

fn ink_typst_assemble(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    typst_call(vm, "ink.typst.assemble", |app| app.ink_typst_assemble())
}

fn ink_typst_build(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    typst_call(vm, "ink.typst.build", |app| app.ink_typst_build())
}

fn ink_typst_take(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    typst_call(vm, "ink.typst.take", |app| app.ink_typst_take())
}

fn typst_call<'a, F>(
    vm: &'a mut VM,
    tag: &'static str,
    f: F,
) -> std::result::Result<&'a mut VM, BundError>
where
    F: FnOnce(&mut crate::tui::app::App),
{
    match with_app(tag, |app| {
        f(app);
        Ok::<_, anyhow::Error>(())
    }) {
        Ok(_) => Ok(vm),
        Err(e) => Err(to_bund_err(e)),
    }
}

// ─────────────────────────────────────────────────────────────────────
// Phase C — AI / theme / editor.replace
// ─────────────────────────────────────────────────────────────────────

// ── ink.ai.send ──────────────────────────────────────────────────────
// Stack: ( prompt -- )
// Posts a user turn through the same streaming pipeline Ctrl+I /
// AI-prompt-Enter use. Returns immediately — the response is
// async and lands in chat_history once complete.

fn ink_ai_send(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_ai_send(vm).map_err(to_bund_err)
}

fn do_ink_ai_send(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.ai.send";
    require_depth(vm, 1, tag)?;
    let prompt = value_to_string(pull(vm, tag)?, "prompt", tag)?;
    with_app(tag, |app| {
        app.ink_ai_send(&prompt).map_err(|e| anyhow!("{tag}: {e}"))
    })?;
    Ok(vm)
}

// ── ink.ai.history ───────────────────────────────────────────────────
// Stack: ( -- list[hash{role, content}] )

fn ink_ai_history(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_ai_history(vm).map_err(to_bund_err)
}

fn do_ink_ai_history(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.ai.history";
    let turns = with_app(tag, |app| Ok(app.ink_ai_history()))?;
    let items: Vec<Value> = turns
        .into_iter()
        .map(|(role, content)| {
            let mut h = std::collections::HashMap::new();
            h.insert("role".to_string(), Value::from_string(role));
            h.insert("content".to_string(), Value::from_string(content));
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ── ink.ai.set_system_prompt ─────────────────────────────────────────
// Stack: ( text -- )
// Empty string clears the override (falling back to the
// inference-mode default).

fn ink_ai_set_system_prompt(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_ai_set_system_prompt(vm).map_err(to_bund_err)
}

fn do_ink_ai_set_system_prompt(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.ai.set_system_prompt";
    require_depth(vm, 1, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    with_app(tag, |app| {
        app.ink_ai_set_system_prompt(&text);
        Ok::<_, anyhow::Error>(())
    })?;
    Ok(vm)
}

// ── ink.theme.set ────────────────────────────────────────────────────
// Stack: ( field hex -- )
// Mutates one theme colour at runtime. Volatile — not persisted.

fn ink_theme_set(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_theme_set(vm).map_err(to_bund_err)
}

fn do_ink_theme_set(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.theme.set";
    require_depth(vm, 2, tag)?;
    let hex = value_to_string(pull(vm, tag)?, "hex", tag)?;
    let field = value_to_string(pull(vm, tag)?, "field", tag)?;
    with_app(tag, |app| {
        app.ink_theme_set(&field, &hex)
            .map_err(|e| anyhow!("{tag}: {e}"))
    })?;
    Ok(vm)
}

// ── ink.editor.replace ───────────────────────────────────────────────
// Stack: ( find replace -- replaced_bool )
// Replaces the first occurrence of `find` with `replace`. Pushes
// true / false depending on whether a match was found.

fn ink_editor_replace(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_editor_replace(vm).map_err(to_bund_err)
}

fn do_ink_editor_replace(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.editor.replace";
    require_depth(vm, 2, tag)?;
    let replace = value_to_string(pull(vm, tag)?, "replace", tag)?;
    let find = value_to_string(pull(vm, tag)?, "find", tag)?;
    let did = with_app(tag, |app| {
        app.ink_editor_replace(&find, &replace)
            .map_err(|e| anyhow!("{tag}: {e}"))
    })?;
    push(vm, Value::from_bool(did));
    Ok(vm)
}

// ── ink.pane.show ────────────────────────────────────────────────────
// Stack: ( title -- )
// Open the floating Bund output pane. Any subsequent print /
// println from the script (or future scripts while the pane stays
// open) lands in the pane rather than the status bar.
//
// If a pane is already open it's reset to `title` with an empty
// buffer — same shape as Esc-then-show.

fn ink_pane_show(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_pane_show(vm).map_err(to_bund_err)
}

fn do_ink_pane_show(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pane.show";
    require_depth(vm, 1, tag)?;
    let title = value_to_string(pull(vm, tag)?, "title", tag)?;
    with_app(tag, |app| {
        app.open_bund_pane(&title);
        Ok(())
    })?;
    Ok(vm)
}

// ── ink.pane.close ───────────────────────────────────────────────────
// Stack: ( -- )

fn ink_pane_close(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_pane_close(vm).map_err(to_bund_err)
}

fn do_ink_pane_close(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pane.close";
    with_app(tag, |app| {
        app.close_bund_pane();
        Ok(())
    })?;
    Ok(vm)
}

// ── ink.pane.clear ───────────────────────────────────────────────────
// Stack: ( -- cleared_bool )
// Empties the line buffer; pane stays open. Returns false if no
// pane is open (script wrote to a closed pane by mistake).

fn ink_pane_clear(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_pane_clear(vm).map_err(to_bund_err)
}

fn do_ink_pane_clear(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pane.clear";
    let cleared = with_app(tag, |app| Ok(app.clear_bund_pane()))?;
    push(vm, Value::from_bool(cleared));
    Ok(vm)
}

// ── ink.pane.line ────────────────────────────────────────────────────
// Stack: ( text -- routed_bool )
// Append `text` to the pane as a fresh line. Returns false if no
// pane is open (lets scripts branch on visibility without first
// calling ink.pane.show).

fn ink_pane_line(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_pane_line(vm).map_err(to_bund_err)
}

fn do_ink_pane_line(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pane.line";
    require_depth(vm, 1, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    let routed = with_app(tag, |app| Ok(app.append_to_bund_pane(&text, true)))?;
    push(vm, Value::from_bool(routed));
    Ok(vm)
}

// ── ink.input ────────────────────────────────────────────────────────
// Stack: ( prompt hook -- )
// Open the BundInput modal showing `prompt`. When the user
// presses Enter, the typed string is pushed onto Adam's
// workbench and the lambda named `hook` is invoked. Esc closes
// the modal without firing. Hook-driven rather than synchronous
// because a blocking modal would freeze autosave + inference
// polling for as long as the user takes to type.

fn ink_input(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_input(vm).map_err(to_bund_err)
}

fn do_ink_input(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.input";
    require_depth(vm, 2, tag)?;
    let hook = value_to_string(pull(vm, tag)?, "hook", tag)?;
    let prompt = value_to_string(pull(vm, tag)?, "prompt", tag)?;
    with_app(tag, |app| {
        app.open_bund_input(&prompt, &hook);
        Ok(())
    })?;
    Ok(vm)
}
