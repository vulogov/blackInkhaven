//! Orchestrator: walk a parsed Scrivener binder and materialise
//! the hierarchy as inkhaven nodes.
//!
//! Public entry point: `import_scrivener_project`. Reads the
//! `.scrivx`, classifies every `BinderItem`, opens each Text
//! node's `.rtf`, converts to Typst, creates the inkhaven
//! node via `Store::create_node` (which handles slug
//! uniqueness, file layout, and metadata persistence).
//!
//! Errors are reported per-item in `ImportReport.errors` —
//! the import never aborts on a single failure. A corrupted RTF
//! produces an empty paragraph rather than dropping the whole
//! tree.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::Config;
use crate::scrivener::binder::{parse_project, BinderItem};
use crate::scrivener::mapping::{classify, node_kind_for, Classification};
use crate::scrivener::rtf::rtf_to_typst;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{EventData, Node, NodeKind};
use crate::store::{InsertPosition, Store, reconcile_event_orphan_tag, SYSTEM_TAG_NOTES, SYSTEM_TAG_PLACES, SYSTEM_TAG_CHARACTERS, SYSTEM_TAG_ARTEFACTS};
use crate::timeline::Calendar;

#[derive(Debug, Clone, Default)]
pub struct ImportOpts {
    /// Override the title used for the user book created from
    /// the Draft folder. None → use the Scrivener Draft folder's
    /// own title.
    pub draft_as_book: Option<String>,
    /// Skip the entire "outside Draft" subtree (Research,
    /// Characters, Places, etc.). Useful when the user only
    /// wants the manuscript.
    pub skip_research: bool,
    /// Don't write anything; just report what would happen.
    pub dry_run: bool,
}

#[derive(Debug, Default)]
pub struct ImportReport {
    pub books_created: usize,
    pub chapters_created: usize,
    pub subchapters_created: usize,
    pub paragraphs_created: usize,
    pub paragraphs_skipped: usize,
    pub errors: Vec<String>,
}

/// Top-level entry point. `scriv_root` is the `.scriv` directory.
pub fn import_scrivener_project(
    scriv_root: &Path,
    store: &Store,
    cfg: &Config,
    opts: &ImportOpts,
) -> Result<ImportReport> {
    let binder = parse_project(scriv_root)
        .with_context(|| format!("parse .scrivx in {}", scriv_root.display()))?;
    let docs_dir = scriv_root.join("Files").join("Docs");
    let mut report = ImportReport::default();
    // 1.2.8+ — when the timeline is opted-in, pre-build the
    // project's Calendar once. The importer feeds CustomMeta
    // date-field values to it during paragraph creation;
    // values that parse become `EventData` on the resulting
    // node. When the timeline is disabled we skip the whole
    // pass even if the .scrivx carries CustomMeta dates.
    let calendar = if cfg.timeline.enabled {
        Some(Calendar::from_config(cfg.timeline.calendar.clone()))
    } else {
        None
    };
    let mut ctx = WalkCtx {
        docs_dir,
        store,
        cfg,
        opts,
        report: &mut report,
        calendar: calendar.as_ref(),
    };
    // Each top-level item is either the Draft, a system-book-
    // mappable folder, or skip. Walk them in order.
    for item in &binder {
        ctx.walk_top(item)?;
    }
    Ok(report)
}

struct WalkCtx<'a> {
    docs_dir: PathBuf,
    store: &'a Store,
    cfg: &'a Config,
    opts: &'a ImportOpts,
    report: &'a mut ImportReport,
    /// 1.2.8+ — present when `timeline.enabled = true`. The
    /// CustomMeta date-extract pass in `create_paragraph` only
    /// runs when this is `Some`.
    calendar: Option<&'a Calendar>,
}

impl<'a> WalkCtx<'a> {
    fn walk_top(&mut self, item: &BinderItem) -> Result<()> {
        // Top-level items: Draft folder or "outside" buckets.
        let is_draft = item.kind == "DraftFolder";
        if is_draft {
            // Draft is the manuscript root → a user Book.
            let title = self
                .opts
                .draft_as_book
                .clone()
                .unwrap_or_else(|| item.title.clone());
            let book_id = self.create_book(&title, None)?;
            self.report.books_created += 1;
            // Walk children with depth-in-Draft semantics.
            for (i, child) in item.children.iter().enumerate() {
                self.walk_in_draft(child, book_id, 1, i as u32)?;
            }
            return Ok(());
        }
        if self.opts.skip_research {
            return Ok(());
        }
        match classify(item, None) {
            Classification::SystemBook(tag) => {
                self.import_into_system_book(item, tag)?;
            }
            Classification::SkipKeepChildren => {
                // Walk children at the top level too — they
                // might be valid Folders themselves.
                for child in &item.children {
                    self.walk_top(child)?;
                }
            }
            _ => {
                // Anything else at the top level is dropped.
            }
        }
        Ok(())
    }

    fn walk_in_draft(
        &mut self,
        item: &BinderItem,
        parent_id: uuid::Uuid,
        depth: usize,
        order_hint: u32,
    ) -> Result<()> {
        let _ = order_hint;
        let classification = classify(item, Some(depth));
        match classification {
            Classification::Paragraph => {
                self.create_paragraph(item, parent_id)?;
                self.report.paragraphs_created += 1;
            }
            Classification::Chapter | Classification::Subchapter => {
                let kind = node_kind_for(&classification).unwrap();
                let branch_id = self.create_branch(kind, &item.title, Some(parent_id))?;
                match kind {
                    NodeKind::Chapter => self.report.chapters_created += 1,
                    NodeKind::Subchapter => self.report.subchapters_created += 1,
                    _ => {}
                }
                for (i, child) in item.children.iter().enumerate() {
                    self.walk_in_draft(child, branch_id, depth + 1, i as u32)?;
                }
            }
            Classification::SkipKeepChildren => {
                for (i, child) in item.children.iter().enumerate() {
                    self.walk_in_draft(child, parent_id, depth, i as u32)?;
                }
            }
            Classification::SkipSubtree => {
                self.report.paragraphs_skipped += 1;
            }
            // These don't apply inside the Draft but Rust insists
            // on exhaustive matching.
            Classification::UserBook | Classification::SystemBook(_) => {}
        }
        Ok(())
    }

    fn create_book(
        &mut self,
        title: &str,
        system_tag: Option<&str>,
    ) -> Result<uuid::Uuid> {
        if self.opts.dry_run {
            return Ok(uuid::Uuid::nil());
        }
        let hierarchy = Hierarchy::load(self.store)
            .map_err(|e| anyhow::anyhow!("hierarchy: {e}"))?;
        let mut node = self
            .store
            .create_node(
                self.cfg,
                &hierarchy,
                NodeKind::Book,
                title,
                None,
                None,
                InsertPosition::End,
            )
            .map_err(|e| anyhow::anyhow!("create_node book `{title}`: {e}"))?;
        if let Some(tag) = system_tag {
            node.system_tag = Some(tag.to_string());
            node.protected = true;
            self.store
                .raw()
                .update_metadata(node.id, node.to_json())
                .map_err(|e| anyhow::anyhow!("tag book `{title}`: {e}"))?;
        }
        Ok(node.id)
    }

    fn create_branch(
        &mut self,
        kind: NodeKind,
        title: &str,
        parent_id: Option<uuid::Uuid>,
    ) -> Result<uuid::Uuid> {
        if self.opts.dry_run {
            return Ok(uuid::Uuid::nil());
        }
        let hierarchy = Hierarchy::load(self.store)
            .map_err(|e| anyhow::anyhow!("hierarchy: {e}"))?;
        let parent_node: Option<Node> = parent_id
            .and_then(|id| hierarchy.get(id).cloned());
        let parent_ref = parent_node.as_ref();
        let node = self
            .store
            .create_node(
                self.cfg,
                &hierarchy,
                kind,
                title,
                parent_ref,
                None,
                InsertPosition::End,
            )
            .map_err(|e| {
                anyhow::anyhow!("create_node {:?} `{title}`: {e}", kind)
            })?;
        Ok(node.id)
    }

    fn create_paragraph(
        &mut self,
        item: &BinderItem,
        parent_id: uuid::Uuid,
    ) -> Result<()> {
        if self.opts.dry_run {
            return Ok(());
        }
        let title = item.title.as_str();
        let scriv_uuid = &item.uuid;
        // Convert the source RTF first. Missing file isn't fatal
        // — Scrivener routinely leaves "empty" Text items with
        // no .rtf at all; just create an empty paragraph.
        let rtf_path = self.docs_dir.join(format!("{}.rtf", scriv_uuid));
        let body = if rtf_path.is_file() {
            match std::fs::read(&rtf_path) {
                Ok(bytes) => match rtf_to_typst(&bytes) {
                    Ok(s) => s,
                    Err(e) => {
                        self.report.errors.push(format!(
                            "rtf `{}`: {e}",
                            rtf_path.display()
                        ));
                        String::new()
                    }
                },
                Err(e) => {
                    self.report.errors.push(format!(
                        "read `{}`: {e}",
                        rtf_path.display()
                    ));
                    String::new()
                }
            }
        } else {
            String::new()
        };

        let hierarchy = Hierarchy::load(self.store)
            .map_err(|e| anyhow::anyhow!("hierarchy: {e}"))?;
        let parent_node = hierarchy
            .get(parent_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("parent {parent_id} missing"))?;
        let mut node = self
            .store
            .create_node(
                self.cfg,
                &hierarchy,
                NodeKind::Paragraph,
                title,
                Some(&parent_node),
                None,
                InsertPosition::End,
            )
            .map_err(|e| anyhow::anyhow!("create_node paragraph: {e}"))?;
        // Write the body bytes to the new node's on-disk file
        // + the store blob. `update_paragraph_content` handles
        // both halves; `put_node`-was-create already wrote the
        // initial template — overwrite it.
        if !body.is_empty() {
            // Write the file on disk first so the next bdslib
            // re-embed sees the right bytes.
            if let Some(rel) = node.file.as_ref() {
                let abs = self.store.project_root().join(rel);
                if let Err(e) = std::fs::write(&abs, body.as_bytes()) {
                    self.report.errors.push(format!(
                        "write {}: {e}",
                        abs.display()
                    ));
                }
            }
            if let Err(e) = self
                .store
                .update_paragraph_content(&mut node, body.as_bytes())
            {
                self.report.errors.push(format!(
                    "store update `{title}`: {e}"
                ));
            }
        }

        // 1.2.6+ — propagate Scrivener keywords → inkhaven
        // tags. Empty list = no-op (skip the metadata update).
        // Tag values are trimmed + de-duped during the binder
        // parse, so we can write them through as-is.
        if !item.keywords.is_empty() {
            node.tags = item.keywords.clone();
            if let Err(e) = self
                .store
                .raw()
                .update_metadata(node.id, node.to_json())
            {
                self.report.errors.push(format!(
                    "tags persist for `{title}`: {e}"
                ));
            }
        }

        // 1.2.8+ — propagate Scrivener CustomMeta date fields
        // → inkhaven `EventData`. The mapping table is
        // `cfg.scrivener.date_fields` (case-insensitive title
        // match); the value is parsed by the project's
        // Calendar. Only runs when `timeline.enabled = true`.
        // Errors are warnings — bad date values do NOT abort
        // the import of the rest of the paragraph; they land
        // on `ImportReport.errors` for the user to chase.
        // The first matching field on the item wins; multiple
        // date fields with conflicting values would be a user
        // setup bug rather than something the importer should
        // try to resolve.
        if let Some(calendar) = self.calendar {
            if !item.custom_meta.is_empty() {
                let date_fields: Vec<String> = self
                    .cfg
                    .scrivener
                    .date_fields
                    .iter()
                    .map(|s| s.to_lowercase())
                    .collect();
                let event_opt = item.custom_meta.iter().find_map(|(field, value)| {
                    let f_lower = field.to_lowercase();
                    if !date_fields.iter().any(|d| d == &f_lower) {
                        return None;
                    }
                    match calendar.parse(value) {
                        Ok((point, precision)) => Some((field.clone(), value.clone(), point, precision)),
                        Err(e) => {
                            self.report.errors.push(format!(
                                "date `{title}` (Scrivener field `{field}` = `{value}`): {e}"
                            ));
                            None
                        }
                    }
                });
                if let Some((field, _value, point, precision)) = event_opt {
                    node.event = Some(EventData {
                        start_ticks: point.ticks(),
                        end_ticks: None,
                        precision,
                        characters: Vec::new(),
                        places: Vec::new(),
                        // No track override — Scrivener CustomMeta
                        // has no equivalent concept; the timeline
                        // view's `default_track` carries it.
                        track: None,
                    });
                    reconcile_event_orphan_tag(&mut node);
                    node.modified_at = chrono::Utc::now();
                    if let Err(e) = self
                        .store
                        .raw()
                        .update_metadata(node.id, node.to_json())
                    {
                        self.report.errors.push(format!(
                            "event persist for `{title}` from `{field}`: {e}"
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn import_into_system_book(
        &mut self,
        item: &BinderItem,
        tag: &str,
    ) -> Result<()> {
        // Look up the existing system book by tag; if it doesn't
        // exist (older project pre-seeding), create one.
        let hierarchy = Hierarchy::load(self.store)
            .map_err(|e| anyhow::anyhow!("hierarchy: {e}"))?;
        let book_id = hierarchy
            .iter()
            .find(|n| {
                n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(tag)
            })
            .map(|n| n.id);
        let book_id = match book_id {
            Some(id) => id,
            None => {
                let title = match tag {
                    "places" => "Places",
                    "characters" => "Characters",
                    "notes" => "Notes",
                    "artefacts" => "Artefacts",
                    other => other,
                };
                self.create_book(title, Some(tag))?
            }
        };
        // Walk children as paragraphs (no chapter / subchapter
        // levels — system books are flat).
        for child in &item.children {
            self.flatten_into_system_book(child, book_id)?;
        }
        Ok(())
    }

    fn flatten_into_system_book(
        &mut self,
        item: &BinderItem,
        book_id: uuid::Uuid,
    ) -> Result<()> {
        if item.kind == "Text" {
            self.create_paragraph(item, book_id)?;
            self.report.paragraphs_created += 1;
        }
        for child in &item.children {
            self.flatten_into_system_book(child, book_id)?;
        }
        Ok(())
    }
}

// Re-export system tags constants for callers that need to
// reference them by string. Kept here to surface the
// `mapping`/`import` coupling at the module boundary.
#[allow(dead_code)]
pub const SYSTEM_TAGS: &[&str] = &[
    SYSTEM_TAG_NOTES,
    SYSTEM_TAG_PLACES,
    SYSTEM_TAG_CHARACTERS,
    SYSTEM_TAG_ARTEFACTS,
];
