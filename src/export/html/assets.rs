//! TDOC-4 — image assets. Rewrite `<img src="…">` for local images to
//! `assets/<basename>` and record the sources so the orchestrator can copy them
//! into the site. External URLs and already-rewritten paths are left alone.

use std::collections::BTreeSet;
use std::path::Path;

/// Rewrite local `<img src>` to `assets/<basename>`, collecting the original
/// (project-relative) sources into `collected`.
pub fn rewrite_images(html: &str, collected: &mut BTreeSet<String>) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(pos) = rest.find("<img ") {
        out.push_str(&rest[..pos]);
        let tag = &rest[pos..];
        if let Some(src_rel) = tag.find("src=\"") {
            let abs = src_rel + 5;
            if let Some(end) = tag[abs..].find('"') {
                let src = &tag[abs..abs + end];
                if is_local(src) {
                    if let Some(base) = Path::new(src).file_name().and_then(|s| s.to_str()) {
                        collected.insert(src.to_string());
                        out.push_str(&tag[..abs]);
                        out.push_str("assets/");
                        out.push_str(base);
                        rest = &tag[abs + end..];
                        continue;
                    }
                }
            }
        }
        // No rewrite — emit `<img ` and keep scanning after it.
        out.push_str("<img ");
        rest = &tag[5..];
    }
    out.push_str(rest);
    out
}

fn is_local(src: &str) -> bool {
    !src.starts_with("http://")
        && !src.starts_with("https://")
        && !src.starts_with("data:")
        && !src.starts_with("assets/")
}

/// Copy every collected image into `<out_dir>/assets/`. Missing files warn, never fail.
pub fn copy_assets(project_root: &Path, out_dir: &Path, collected: &BTreeSet<String>) -> std::io::Result<()> {
    if collected.is_empty() {
        return Ok(());
    }
    let assets = out_dir.join("assets");
    std::fs::create_dir_all(&assets)?;
    for src in collected {
        let from = project_root.join(src);
        let Some(base) = Path::new(src).file_name() else { continue };
        if from.exists() {
            let _ = std::fs::copy(&from, assets.join(base));
        } else {
            tracing::warn!(target: "inkhaven::html", "html: image `{}` not found — skipped", from.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_local_leaves_remote() {
        let mut c = BTreeSet::new();
        let html = rewrite_images(
            "<p><img src=\"img/map.png\" alt=\"m\"> and <img src=\"https://x/y.png\" alt=\"r\"></p>",
            &mut c,
        );
        assert!(html.contains("src=\"assets/map.png\""));
        assert!(html.contains("src=\"https://x/y.png\""));
        assert!(c.contains("img/map.png"));
        assert_eq!(c.len(), 1);
    }
}
