use std::path::Path;

use serde_json::Value as JsonValue;

use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;
use crate::store::Store;

pub fn run(project: &Path, query: &str, limit: usize) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;

    let results = store.search_text(query, limit)?;

    if results.is_empty() {
        eprintln!("No results.");
        return Ok(());
    }

    for r in &results {
        print_result(r);
    }
    Ok(())
}

fn print_result(r: &JsonValue) {
    let score = r.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let id = r.get("id").and_then(|v| v.as_str()).unwrap_or("?");
    let meta = r.get("metadata");

    let kind = meta
        .and_then(|m| m.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let title = meta
        .and_then(|m| m.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)");
    let path: Vec<&str> = meta
        .and_then(|m| m.get("path"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let slug = meta
        .and_then(|m| m.get("slug"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut crumbs = path.join("/");
    if !crumbs.is_empty() && !slug.is_empty() {
        crumbs.push('/');
    }
    crumbs.push_str(slug);

    let snippet = r
        .get("document")
        .and_then(|v| v.as_str())
        .map(truncate_one_line)
        .unwrap_or_default();

    println!("{score:>5.3}  [{kind:<10}] {crumbs}");
    println!("        {title}");
    if !snippet.is_empty() {
        println!("        {snippet}");
    }
    println!("        id: {id}");
    println!();
}

fn truncate_one_line(s: &str) -> String {
    let single = s.lines().next().unwrap_or("").trim();
    if single.chars().count() > 100 {
        let truncated: String = single.chars().take(100).collect();
        format!("{truncated}…")
    } else {
        single.to_string()
    }
}
