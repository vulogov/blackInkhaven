//! `inkhaven bund <code>` — evaluate a Bund expression and print the
//! top of the workbench.
//!
//! Phase-0 smoke entry point. Stays minimal on purpose: takes a
//! script string, runs it against the Adam VM, prints either the
//! popped result or "(no result)" if the workbench was empty.
//!
//! No project or store dependency — Adam lives in process memory.
//! Later phases (P4 hooks, P5 first-class script nodes) will need
//! the store; this command will gain a `--project` then.

use anyhow::Result;

pub fn run(code: &str) -> Result<()> {
    match crate::scripting::eval(code)? {
        Some(value) => println!("{}", crate::scripting::format_value(&value)),
        None => println!("(no result)"),
    }
    Ok(())
}
