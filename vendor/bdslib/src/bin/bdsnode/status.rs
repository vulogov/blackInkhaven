use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

pub struct NodeState {
    pub node_id:              String,
    pub hostname:             String,
    pub started_at:           Instant,
    pub current_file:         Arc<Mutex<Option<String>>>,
    pub current_syslog_file:  Arc<Mutex<Option<String>>>,
}

static STATE: OnceLock<NodeState> = OnceLock::new();

/// Initialise the node state.  Must be called once early in `main` before any
/// `v2/status` request can be served.
pub fn init(node_id: String) {
    let _ = STATE.set(NodeState {
        node_id,
        hostname:            resolve_hostname(),
        started_at:          Instant::now(),
        current_file:        Arc::new(Mutex::new(None)),
        current_syslog_file: Arc::new(Mutex::new(None)),
    });
}

/// Return the global node state.  Panics if [`init`] was not called first.
pub fn get() -> &'static NodeState {
    STATE.get().expect("status::init() not called before status::get()")
}

fn resolve_hostname() -> String {
    // Prefer the HOSTNAME env var (set by most shells).
    if let Ok(h) = std::env::var("HOSTNAME") {
        let h = h.trim().to_string();
        if !h.is_empty() {
            return h;
        }
    }
    // /etc/hostname is canonical on Linux.
    if let Ok(h) = std::fs::read_to_string("/etc/hostname") {
        let h = h.trim().to_string();
        if !h.is_empty() {
            return h;
        }
    }
    // Fall back to the `hostname` command (available on macOS and POSIX Linux).
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
