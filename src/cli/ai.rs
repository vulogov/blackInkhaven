use std::io::Write;
use std::path::Path;

use crate::ai::AiClient;
use crate::ai::stream::{StreamMsg, spawn_chat_stream};
use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;

pub fn run(project: &Path, prompt: &str, provider: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env_var) = ai.resolve_provider(&cfg.llm, provider)?;

    let mut rx = spawn_chat_stream(
        ai.client.clone(),
        model.to_string(),
        None,
        prompt.to_string(),
    );

    let mut stdout = std::io::stdout().lock();
    let mut wrote_anything = false;
    while let Some(msg) = rx.blocking_recv() {
        match msg {
            StreamMsg::Token(t) => {
                let _ = stdout.write_all(t.as_bytes());
                let _ = stdout.flush();
                wrote_anything = true;
            }
            StreamMsg::Done => break,
            StreamMsg::Error(e) => {
                let _ = stdout.write_all(b"\n");
                eprintln!("inference error: {e}");
                return Ok(());
            }
        }
    }
    if wrote_anything {
        let _ = stdout.write_all(b"\n");
    } else {
        eprintln!("(no tokens received)");
    }
    Ok(())
}
