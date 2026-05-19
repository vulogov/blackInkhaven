//! Cron-driven scheduler for stored BUND scripts.
//!
//! The scheduler reads the script registry exposed by
//! [`ShardsManager::scripts`], parses each `schedule` field as a crontab
//! expression via [`croner::Cron`], and submits any script whose next
//! occurrence falls within the current minute to the persistent
//! [`crate::vm::workers::BundWorkerPool`].
//!
//! Each submission re-uses the script's storage UUID as the result-queue id
//! (via [`submit_script_with_id`]) so that callers can locate the latest
//! workbench output by querying the well-known queue.
//!
//! The scheduler itself is stateless — every tick rebuilds the in-memory
//! `(uuid → Cron)` map from the registry, so newly added scripts are picked
//! up immediately and deleted scripts disappear without restart.
//!
//! ## Tick cadence
//!
//! [`Scheduler::run`] is meant to be invoked once per minute; running it
//! more often will fire scripts with cron `* * * * *` more than once per
//! minute (because the same minute boundary is "current" for multiple ticks).
//! For sub-minute precision, switch to a smaller cadence and supply
//! second-level cron patterns (`croner` supports the optional 6-field form
//! with `with_seconds_required()` if needed).

use crate::common::error::{err_msg, Result};
use crate::shardsmanager::ShardsManager;
use crate::vm::workers::submit_script_with_id;
use chrono::{Duration, Local, Timelike};
use croner::Cron;
use std::collections::HashMap;
use uuid::Uuid;

/// Cron-driven dispatcher of stored BUND scripts.
///
/// Holds a clone of the `ShardsManager` so each tick can read the script
/// registry and fetch script bodies without requiring the global singleton.
pub struct Scheduler {
    db: ShardsManager,
}

impl Scheduler {
    /// Construct a scheduler bound to the given `ShardsManager`.
    pub fn new(db: ShardsManager) -> Self {
        Self { db }
    }

    /// Single tick: enumerate stored scripts, fire those whose cron pattern
    /// resolves to a moment within the current minute, and return how many
    /// scripts were dispatched this tick.
    ///
    /// Errors fetching individual scripts or parsing individual cron strings
    /// are logged and skipped — one bad entry never aborts the whole tick.
    pub fn run(&self) -> Result<usize> {
        let now = Local::now();
        let minute_start = now
            .with_nanosecond(0)
            .and_then(|t| t.with_second(0))
            .ok_or_else(|| err_msg("scheduler: minute truncation failed"))?;
        let minute_end = minute_start + Duration::minutes(1);

        // Snapshot the registry: (uuid, schedule_string) pairs.
        let entries = self
            .db
            .scripts()
            .map_err(|e| err_msg(format!("scheduler: scripts() failed: {e}")))?;

        // Build the ephemeral (uuid → Cron) map. Invalid patterns are logged
        // and dropped rather than failing the whole tick.
        let mut crons: HashMap<Uuid, Cron> = HashMap::with_capacity(entries.len());
        for (id, schedule) in &entries {
            match Cron::new(schedule).parse() {
                Ok(cron) => {
                    crons.insert(*id, cron);
                }
                Err(e) => log::warn!(
                    "[scheduler] invalid cron schedule {schedule:?} for script {id}: {e}"
                ),
            }
        }

        let mut fired = 0usize;
        for (id, cron) in &crons {
            // `find_next_occurrence` with `inclusive=true` returns the first
            // occurrence at or after `minute_start`. If that moment falls
            // before `minute_end`, the cron pattern fires this minute.
            let next = match cron.find_next_occurrence(&minute_start, true) {
                Ok(t) => t,
                Err(e) => {
                    log::warn!(
                        "[scheduler] find_next_occurrence failed for script {id}: {e}"
                    );
                    continue;
                }
            };
            if next < minute_end {
                let body = match self.db.script(*id) {
                    Ok(Some(b)) => b,
                    Ok(None) => {
                        log::warn!(
                            "[scheduler] script {id} disappeared between scripts() and script(); skipping"
                        );
                        continue;
                    }
                    Err(e) => {
                        log::warn!("[scheduler] script({id}) lookup failed: {e}");
                        continue;
                    }
                };

                match submit_script_with_id(*id, &body) {
                    Ok(_) => {
                        log::info!(
                            "[scheduler] submitted script {id} (cron tick at {next})"
                        );
                        fired += 1;
                    }
                    Err(e) => {
                        log::warn!(
                            "[scheduler] submit_script_with_id({id}) failed: {e}"
                        );
                    }
                }
            }
        }

        Ok(fired)
    }
}
