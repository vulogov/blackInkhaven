use bundcore::bundcore::Bund;
use easy_error::{Error, err_msg};
use parking_lot::RwLock;
use std::sync::OnceLock;

pub(crate) static BUND: OnceLock<RwLock<Bund>> = OnceLock::new();

/// Process-wide [`result_queue::ResultQueue`] used by `v2/results.*` for
/// short-lived per-id FIFOs of `rust_dynamic` values.  Lazily initialised
/// on first access via [`results`].
pub static RESULTS: OnceLock<result_queue::ResultQueue> = OnceLock::new();

/// Convenience accessor — returns a clone of the singleton `ResultQueue`,
/// initialising it on first call.  Cheap: `ResultQueue` is `Arc`-backed.
pub fn results() -> result_queue::ResultQueue {
    RESULTS.get_or_init(result_queue::ResultQueue::new).clone()
}

pub mod context;
pub mod ephemeral;
pub mod eval;
pub mod helpers;
pub mod result_queue;
pub mod stdlib;
pub mod vm;
pub mod workers;

pub use vm::init_adam;

/// Initialise the BUND VM (if not already done) and evaluate `code`.
///
/// Calls [`init_adam`] on the first invocation, so callers do not need to
/// initialise the singleton separately.
pub fn bund_eval(code: &str) -> Result<(), Error> {
    init_adam()?;
    let bund = BUND
        .get()
        .ok_or_else(|| err_msg("BUND VM not initialised"))?;
    let mut guard = bund.write();
    helpers::eval::bund_compile_and_eval(&mut guard.vm, code.to_string())?;
    Ok(())
}
