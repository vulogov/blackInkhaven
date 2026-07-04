//! RESRCH-5 (R5-F) — dead-source detection. Checks the recoverable target in a
//! fact's provenance (`web` URLs via HTTP, `document` files on disk) and reports
//! the ones that no longer resolve, so link rot in a research corpus surfaces
//! before it undermines the trust ladder.
//!
//! Conservative on purpose: only HTTP 404/410 and hard network failures count as
//! dead. 403/429 (bot-blocks) and transient 5xx are treated as "reachable" so
//! the sweep doesn't cry wolf.

use std::time::Duration;

/// One dead source found during a `/deadsources` sweep.
pub(super) struct DeadLink {
    /// The fact's slug-path (or id) in the Facts tree.
    pub loc: String,
    /// `web` | `document`.
    pub origin: String,
    /// The URL or file path that failed.
    pub target: String,
    /// Why it's considered dead (`HTTP 404`, `unreachable`, `file missing`).
    pub reason: String,
}

/// A shared client with a bounded timeout and redirect budget.
pub(super) fn client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("inkhaven-research (dead-source check)")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
}

/// Classify a single web URL: `None` = reachable (or blocked/transient — not our
/// call to flag); `Some(reason)` = confirmed dead.
pub(super) async fn check_web(client: &reqwest::Client, url: &str) -> Option<String> {
    match client.head(url).send().await {
        Ok(resp) => {
            // Some servers reject HEAD (405) — confirm with a GET before judging.
            if resp.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
                match client.get(url).send().await {
                    Ok(r2) => dead_status(r2.status()),
                    Err(e) => classify_err(&e),
                }
            } else {
                dead_status(resp.status())
            }
        }
        Err(e) => classify_err(&e),
    }
}

fn dead_status(s: reqwest::StatusCode) -> Option<String> {
    if s == reqwest::StatusCode::NOT_FOUND || s == reqwest::StatusCode::GONE {
        Some(format!("HTTP {}", s.as_u16()))
    } else {
        None
    }
}

fn classify_err(e: &reqwest::Error) -> Option<String> {
    if e.is_timeout() {
        Some("timeout".into())
    } else if e.is_connect() {
        Some("unreachable".into())
    } else {
        Some("request failed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn only_404_and_410_count_as_dead() {
        assert_eq!(dead_status(StatusCode::NOT_FOUND), Some("HTTP 404".into()));
        assert_eq!(dead_status(StatusCode::GONE), Some("HTTP 410".into()));
        // Reachable, blocked, or transient — never flagged.
        assert_eq!(dead_status(StatusCode::OK), None);
        assert_eq!(dead_status(StatusCode::FORBIDDEN), None);
        assert_eq!(dead_status(StatusCode::TOO_MANY_REQUESTS), None);
        assert_eq!(dead_status(StatusCode::INTERNAL_SERVER_ERROR), None);
        assert_eq!(dead_status(StatusCode::MOVED_PERMANENTLY), None);
    }
}
