//! 1.3.1 SUBMISSION-1 — `inkhaven submissions` subcommand.
//!
//! The structured submission tracker (the `.inkhaven/submissions.json`
//! sidecar via [`crate::submissions`]).  Add a submission, list the log,
//! move a record's status, or remove one.  The generated *drafts* live in
//! the `Submissions` system book; this tracks where they went.

use std::path::Path;

use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::submissions::{today, SubmissionLog, SubmissionRecord, SubmissionStatus};

use super::SubmissionsCommand;

pub fn run(project: &Path, cmd: SubmissionsCommand) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let root = &layout.root;
    let mut log = SubmissionLog::load(root).map_err(Error::Store)?;

    match cmd {
        SubmissionsCommand::Add {
            market,
            agent,
            draft,
            status,
            date_sent,
            next,
            notes,
        } => {
            let status = match status.as_deref() {
                Some(s) => {
                    SubmissionStatus::parse(s).ok_or_else(|| bad_status(s))?
                }
                None => SubmissionStatus::Drafting,
            };
            // A submission you're recording as already sent gets today's
            // date unless one was given.
            let date_sent = date_sent.or_else(|| {
                (status == SubmissionStatus::Sent).then(today)
            });
            let id = log.next_id();
            log.records.push(SubmissionRecord {
                id: id.clone(),
                market,
                agent,
                draft_ref: draft,
                date_sent,
                status,
                response_date: None,
                next_action_date: next,
                notes,
            });
            log.save(root).map_err(Error::Store)?;
            println!("submissions: added {id} ({})", status.label());
            Ok(())
        }
        SubmissionsCommand::List { json, status, open } => {
            let want = match status.as_deref() {
                Some(s) => Some(SubmissionStatus::parse(s).ok_or_else(|| bad_status(s))?),
                None => None,
            };
            let rows: Vec<&SubmissionRecord> = log
                .records
                .iter()
                .filter(|r| want.is_none_or(|w| r.status == w))
                .filter(|r| !open || r.status.is_open())
                .collect();
            if json {
                let out = serde_json::to_string_pretty(&rows)
                    .map_err(|e| Error::Store(format!("submissions: {e}")))?;
                println!("{out}");
            } else if rows.is_empty() {
                println!("submissions: (none)");
            } else {
                for r in rows {
                    print_row(r);
                }
            }
            Ok(())
        }
        SubmissionsCommand::Status {
            id,
            status,
            response_date,
        } => {
            let new = SubmissionStatus::parse(&status).ok_or_else(|| bad_status(&status))?;
            let rec = log
                .find_mut(&id)
                .ok_or_else(|| Error::Store(format!("submissions: no record `{id}`")))?;
            rec.status = new;
            // A terminal response stamps a response date if none given.
            if matches!(new, SubmissionStatus::Rejected | SubmissionStatus::Offer) {
                rec.response_date = response_date.or_else(|| Some(today()));
            } else if let Some(d) = response_date {
                rec.response_date = Some(d);
            }
            let label = new.label();
            log.save(root).map_err(Error::Store)?;
            println!("submissions: {id} → {label}");
            Ok(())
        }
        SubmissionsCommand::Remove { id } => {
            if log.remove(&id) {
                log.save(root).map_err(Error::Store)?;
                println!("submissions: removed {id}");
                Ok(())
            } else {
                Err(Error::Store(format!("submissions: no record `{id}`")))
            }
        }
    }
}

fn bad_status(s: &str) -> Error {
    Error::Store(format!(
        "submissions: unknown status `{s}` (drafting|sent|rejected|offer|withdrawn)"
    ))
}

fn print_row(r: &SubmissionRecord) {
    let mut line = format!("{:<4} {:<10} {}", r.id, r.status.label(), r.market);
    if let Some(a) = &r.agent {
        line.push_str(&format!(" · {a}"));
    }
    if let Some(d) = &r.date_sent {
        line.push_str(&format!(" · sent {d}"));
    }
    if let Some(d) = &r.response_date {
        line.push_str(&format!(" · heard {d}"));
    }
    if let Some(d) = &r.next_action_date {
        line.push_str(&format!(" · next {d}"));
    }
    println!("{line}");
    if let Some(n) = &r.notes {
        println!("       {n}");
    }
}
