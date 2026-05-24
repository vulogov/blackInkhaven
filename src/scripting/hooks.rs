//! Bund hook dispatch.
//!
//! Every `Store` mutation that has a corresponding well-known hook
//! name (`hook.on_save`, `hook.on_rename`, …) calls [`fire`] after
//! the mutation succeeds. If the user has defined a Bund lambda
//! with that name (via the bootstrap script in `inkhaven.hjson`),
//! we push the supplied args onto the workbench and run it.
//!
//! ## Failure model
//!
//! Hooks must not break the editor. Every failure path is logged
//! at WARN and swallowed — a misbehaving hook never aborts a save,
//! a rename, or a snapshot. The user sees the log line and the
//! store mutation still completes.
//!
//! ## Recursion guard
//!
//! A hook lambda can in principle invoke an `ink.*` write word in
//! a future phase (P5+), which would re-enter the store mutation
//! that fired the hook — and recurse. We track depth in a
//! thread-local counter and refuse to dispatch past [`MAX_DEPTH`].
//! Today's read-only `ink.*` words can't trigger this, but the
//! guard is forward-looking.
//!
//! ## Single-threaded by design
//!
//! Hooks fire synchronously on the calling thread, which holds
//! Adam's write lock for the duration. That means a slow hook is
//! a slow save — felt by the writer. Backgrounding via an
//! ephemeral worker pool is P6.

use std::cell::Cell;

use rust_dynamic::value::Value;

/// Recursion cap. Picked to be large enough that legitimate
/// nested workflows (a save hook that calls a rename hook that
/// calls a snapshot hook) keep working, small enough that a
/// runaway recursion is bounded in cost.
const MAX_DEPTH: u32 = 4;

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Fire `name` against Adam if a lambda with that name is defined.
/// Silent no-op when:
///
/// - Adam hasn't been initialised yet (called before TUI startup).
/// - No lambda named `name` is registered.
/// - The recursion depth cap has been reached.
///
/// `args` are pushed onto the workbench stack **in order** before
/// the lambda runs, so the lambda body sees the first arg at the
/// bottom of its workbench and the last on top. Hooks that need
/// no args pass an empty `Vec`.
pub fn fire(name: &str, args: Vec<Value>) {
    // If we're already inside a `bund::eval` on this thread, the
    // mutation that triggered the hook originated from the script
    // itself. Firing the hook now would re-enter Adam's write
    // lock and deadlock. Skip silently — the script is in
    // explicit control of its own side effects.
    if super::is_in_eval() {
        return;
    }
    let depth = DEPTH.with(|c| c.get());
    if depth >= MAX_DEPTH {
        tracing::warn!(
            target: "inkhaven::scripting::hooks",
            "hook {} skipped: depth {} >= max {}",
            name,
            depth,
            MAX_DEPTH
        );
        return;
    }

    // Lazy init — first hook fire after register_active_store
    // builds Adam, applies policy, evals bootstrap (which is
    // where the user's hook lambdas typically come from).
    if let Err(e) = super::init_adam() {
        tracing::warn!(
            target: "inkhaven::scripting::hooks",
            "hook {} init_adam failed: {}",
            name,
            e
        );
        return;
    }

    DEPTH.with(|c| c.set(depth + 1));
    // 1.2.6+ — drain anything in the print buffer BEFORE
    // the hook eval so we can attribute output that arrived
    // afterwards to this specific fire.
    let _ = super::stdlib::io::drain_print_buffer();
    let outcome = super::with_adam(|bund| {
        if !bund.vm.lambdas.contains_key(name) {
            // No-op when the user hasn't installed this hook.
            return Ok(());
        }
        for arg in args {
            let _ = bund.vm.stack.push(arg);
        }
        bund.eval(name.to_string())
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("eval: {e}"))
    });
    DEPTH.with(|c| c.set(depth));
    // Surface any print / println output the hook produced.
    // CLI / TUI surfaces don't drain this buffer on their
    // own; without this drain, hook stdout vanishes
    // silently. We route through tracing::info so the
    // output respects RUST_LOG (and in the TUI lands in
    // .inkhaven.log; in the CLI it flushes to stderr).
    let stdout = super::stdlib::io::drain_print_buffer();
    if !stdout.is_empty() {
        for line in stdout.lines() {
            if !line.is_empty() {
                tracing::info!(target: "inkhaven::hook::out", hook = name, "{line}");
            }
        }
    }

    match outcome {
        Some(Ok(())) => {}
        Some(Err(e)) => {
            tracing::warn!(
                target: "inkhaven::scripting::hooks",
                "hook {} failed: {}",
                name,
                e
            );
        }
        None => {
            // Adam wasn't constructed yet — init_adam should have
            // built it above, so this only happens if init_adam
            // succeeded but ADAM didn't get installed (race we
            // can't actually hit with OnceLock). Be safe.
        }
    }
}
