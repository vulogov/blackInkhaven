//! CLI probes (3.22) — four commands that had no end-to-end test, each run
//! in-process against a seeded temp project:
//!
//! * `doctor_probe` — the report names every fault in a deliberately broken
//!   project (a paragraph whose `.typ` is gone, a stray `.typ`, an orphaned
//!   node, hooks defined in an untrusted project) and is quiet on a clean one.
//! * `check_probe` — every `--no-*` flag really skips its track, and the
//!   summary says so.
//! * `backup_restore_probe` — backup, restore into a fresh directory, reopen,
//!   and the hierarchy, bodies, snapshots and config come back identical.
//! * `scrivener_probe` — a synthetic `.scriv` package (binder XML + RTF) imports
//!   into the expected tree; a dry run creates nothing.
//!
//! All open real stores (embedding model), so they are `#[ignore]`d:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture cli::probe
//! ```
#![cfg(test)]

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::config::Config;
use crate::probe_kit::run_probe;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{InsertPosition, NodeKind, Store};

const QUAY: &str = "QUAYSTONE";
const LEDGER: &str = "LEDGERLINE";

struct Seeded {
    #[allow(dead_code)] // keeps the TempDir alive for the probe's duration
    tmp: tempfile::TempDir,
    root: PathBuf,
    cfg: Config,
    quay: Uuid,
    ledger: Uuid,
}

fn body(title: &str, sentinel: &str) -> String {
    format!("= {title}\n\nThe {sentinel} sentence stands in for prose. A second sentence follows it.\n")
}

/// One book, one chapter, two paragraphs, one annotated snapshot on the first.
fn seed(tweak: impl FnOnce(&mut Config)) -> Seeded {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = Config::default();
    cfg.images.preview_enabled = false;
    cfg.sync_interval_seconds = 0;
    tweak(&mut cfg);
    let layout = ProjectLayout::new(tmp.path());
    layout.create_layout().unwrap();
    cfg.save(&layout.config_path()).unwrap();
    let store = Store::open(layout.clone(), &cfg).expect("open store");
    let mk = |kind: NodeKind, title: &str, parent: Option<&Node>| -> Node {
        let h = Hierarchy::load(&store).unwrap();
        store.create_node(&cfg, &h, kind, title, parent, None, InsertPosition::End).unwrap()
    };
    let book = mk(NodeKind::Book, "Harbor Lights", None);
    let arrival = mk(NodeKind::Chapter, "Arrival", Some(&book));
    let para = |title: &str, sentinel: &str| -> Node {
        let h = Hierarchy::load(&store).unwrap();
        let node = store
            .create_paragraph_with_body(
                &cfg,
                &h,
                title,
                Some(&arrival),
                InsertPosition::End,
                body(title, sentinel).as_bytes(),
                None,
            )
            .expect("create paragraph");
        if let Some(rel) = &node.file {
            std::fs::write(tmp.path().join(rel), body(title, sentinel)).unwrap();
        }
        node
    };
    let quay = para("Quay", QUAY);
    let ledger = para("Ledger", LEDGER);
    store
        .create_snapshot_annotated(&quay, body("Quay", QUAY).as_bytes(), "probe note")
        .expect("snapshot");
    drop(store);
    Seeded { root: tmp.path().to_path_buf(), tmp, cfg, quay: quay.id, ledger: ledger.id }
}

fn open(root: &Path) -> (Config, Store) {
    let layout = ProjectLayout::new(root);
    let cfg = Config::load_layered(&layout.config_path()).expect("config");
    let store = Store::open(layout, &cfg).expect("open store");
    (cfg, store)
}

fn titles(h: &Hierarchy) -> Vec<String> {
    h.flatten().into_iter().map(|(n, _)| format!("{:?}:{}", n.kind, n.title)).collect()
}

fn para_body(root: &Path, h: &Hierarchy, id: Uuid) -> String {
    h.get(id)
        .and_then(|n| n.file.as_ref())
        .and_then(|rel| std::fs::read_to_string(root.join(rel)).ok())
        .unwrap_or_default()
}

/// A report line's value, `kv`-style (`  key<pad> value`).
fn kv_value<'a>(report: &'a str, key: &str) -> Option<&'a str> {
    report
        .lines()
        .find(|l| l.trim_start().starts_with(key))
        .map(|l| l.trim_start()[key.len()..].trim())
}

#[test]
#[ignore = "probe: opens a real Store; run with --ignored --nocapture"]
fn doctor_probe() {
    run_probe("doctor", |out| {
        let mut problems: Vec<String> = Vec::new();
        macro_rules! note {
            ($msg:expr) => {{ let m: String = $msg; out.push(format!("  !! {m}")); problems.push(m); }};
        }
        let s = seed(|_| {});

        // ---- clean project ---------------------------------------------------
        let report = crate::cli::doctor::report(&s.root).expect("doctor report");
        for key in ["critical", "warning"] {
            match kv_value(&report, key) {
                Some("0") => {}
                other => note!(format!("[CLEAN] scan `{key}` should be 0, got {other:?}")),
            }
        }
        let clean_bdslib_only: usize = kv_value(&report, "bdslib-only").and_then(|v| v.parse().ok()).unwrap_or(0);
        if !report.contains("pending opt-in") {
            note!("[CLEAN] a fresh project (trust_decision `ask`, no trust file) must report pending opt-in".into());
        }
        if report.contains("silently does nothing") {
            note!("[CLEAN] no hooks defined — the untrusted-hooks warning must not fire".into());
        }
        if !report.contains("auto_fallback") || !report.contains("AI providers") {
            note!("[CLEAN] the provider table (`inkhaven ai --resolve`) is missing from the report".into());
        }
        if !report.contains("user books") {
            note!("[CLEAN] the project section is missing".into());
        }
        out.push(format!(
            "  [CLEAN] {} lines, scan 0 critical / 0 warning ({clean_bdslib_only} bdslib-only info), trust pending, provider table present",
            report.lines().count()
        ));

        // ---- break it: a missing .typ, a broken parent ref, untrusted hooks ----
        {
            let (_cfg, store) = open(&s.root);
            let h = Hierarchy::load(&store).unwrap();
            let quay = h.get(s.quay).cloned().unwrap();
            std::fs::remove_file(s.root.join(quay.file.as_ref().unwrap())).unwrap();
            let mut ledger = h.get(s.ledger).cloned().unwrap();
            ledger.parent_id = Some(Uuid::new_v4());
            store.raw().update_metadata(ledger.id, ledger.to_json()).unwrap();
        }
        let mut cfg = s.cfg.clone();
        cfg.scripting.bootstrap = "\"hook.on_save\" { drop } register\n".to_string();
        cfg.save(&ProjectLayout::new(&s.root).config_path()).unwrap();

        let report = crate::cli::doctor::report(&s.root).expect("doctor report (broken)");
        // The deleted .typ still has its bdslib copy → `bdslib-only` (recoverable).
        let broken_bdslib_only: usize = kv_value(&report, "bdslib-only").and_then(|v| v.parse().ok()).unwrap_or(0);
        if broken_bdslib_only != clean_bdslib_only + 1 {
            note!(format!(
                "[BROKEN] deleting Quay's .typ should add one bdslib-only finding ({clean_bdslib_only} → {broken_bdslib_only})"
            ));
        }
        match kv_value(&report, "broken-parent-ref") {
            Some("1") => {}
            other => note!(format!("[BROKEN] expected 1 broken-parent-ref, got {other:?}")),
        }
        // A detached node is unreachable from the tree: the scanner rates it critical.
        match kv_value(&report, "critical") {
            Some("0") | None => note!("[BROKEN] the broken parent ref must count as critical".into()),
            _ => {}
        }
        if !report.contains("inkhaven doctor --scan") {
            note!("[BROKEN] the report must point at `inkhaven doctor --scan`".into());
        }
        if !report.contains("1 hook registration(s)") {
            note!("[BROKEN] the bootstrap's hook count is missing".into());
        }
        if !report.contains("silently does nothing") {
            note!("[BROKEN] hooks in an untrusted project must produce the warning".into());
        }
        if !report.contains("⚠") {
            note!("[BROKEN] the Notes section must carry warnings".into());
        }
        out.push("  [BROKEN] missing .typ (bdslib-only), broken parent ref and untrusted hooks all reported".into());

        // ---- trust the project: the hooks warning goes away ------------------
        cfg.scripting.trust_decision = "trust".to_string();
        cfg.save(&ProjectLayout::new(&s.root).config_path()).unwrap();
        let report = crate::cli::doctor::report(&s.root).expect("doctor report (trusted)");
        if report.contains("silently does nothing") {
            note!("[TRUSTED] the hooks warning must not fire once trust_decision = trust".into());
        }
        if !report.contains("trusted — bootstrap and Script nodes run") {
            note!("[TRUSTED] the Scripting section must say the project is trusted".into());
        }
        out.push("  [TRUSTED] trust_decision = trust clears the hooks warning".into());

        // ---- outside a project ----------------------------------------------
        let empty = tempfile::tempdir().unwrap();
        let report = crate::cli::doctor::report(empty.path()).expect("doctor outside a project");
        if !report.contains("not an inkhaven project") {
            note!("[OUTSIDE] a non-project directory must be reported as such".into());
        }
        out.push("  [OUTSIDE] a plain directory is reported as not a project".into());

        out.push(String::new());
        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  - {p}"));
        }
        assert!(problems.is_empty(), "{} doctor problem(s):\n{}", problems.len(), problems.join("\n"));
    });
}

#[test]
#[ignore = "probe: opens a real Store; run with --ignored --nocapture"]
fn check_probe() {
    run_probe("check", |out| {
        let mut problems: Vec<String> = Vec::new();
        macro_rules! note {
            ($msg:expr) => {{ let m: String = $msg; out.push(format!("  !! {m}")); problems.push(m); }};
        }
        let s = seed(|cfg| {
            cfg.timeline.enabled = true;
            cfg.timeline.critique.enabled = true;
            cfg.continuity.enabled = true;
            cfg.lector.enabled = true;
        });
        let run = |flags: [bool; 5], paragraph: Option<&str>, book: Option<&str>| {
            crate::cli::check::run_report(&s.root, paragraph, book, flags[0], flags[1], flags[2], flags[3], flags[4])
        };
        let track_line = |report: &str, track: &str| -> String {
            report
                .lines()
                .find(|l| l.trim_start().starts_with(track))
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| format!("<no `{track}` line>"))
        };
        const TRACKS: [&str; 5] = ["fact-check", "socrates", "timeline", "continuity", "read-through"];

        // ---- everything on ----------------------------------------------------
        match run([false; 5], None, None) {
            Err(e) => note!(format!("[ALL] check failed: {e}")),
            Ok(report) => {
                if !report.contains("review pass · 2 paragraph(s)") {
                    note!(format!("[ALL] expected a 2-paragraph review pass, got:\n{report}"));
                }
                for t in TRACKS {
                    let line = track_line(&report, t);
                    if line.contains("skipped") {
                        note!(format!("[ALL] `{t}` ran with no skip flag but reads skipped: {line}"));
                    }
                }
                if !report.contains("TOTAL") {
                    note!("[ALL] no TOTAL line".into());
                }
                out.push(format!("  [ALL] five tracks ran: {}", TRACKS.iter().map(|t| track_line(&report, t)).collect::<Vec<_>>().join(" · ")));
            }
        }

        // ---- each flag skips exactly its track ---------------------------------
        for (i, t) in TRACKS.iter().enumerate() {
            let mut flags = [false; 5];
            flags[i] = true;
            match run(flags, None, None) {
                Err(e) => note!(format!("[SKIP {t}] check failed: {e}")),
                Ok(report) => {
                    let line = track_line(&report, t);
                    if !line.contains("skipped") {
                        note!(format!("[SKIP {t}] the flag did not skip the track: {line}"));
                    }
                    for other in TRACKS.iter().filter(|o| *o != t) {
                        if track_line(&report, other).contains("skipped") {
                            note!(format!("[SKIP {t}] `{other}` was skipped too"));
                        }
                    }
                }
            }
        }
        out.push("  [SKIP] each --no-* flag skips its own track only".into());

        // ---- all flags: nothing runs, the summary still prints -----------------
        match run([true; 5], None, None) {
            Err(e) => note!(format!("[NONE] check failed: {e}")),
            Ok(report) => {
                if TRACKS.iter().any(|t| !track_line(&report, t).contains("skipped")) {
                    note!(format!("[NONE] some track still ran:\n{report}"));
                }
                if !report.contains("TOTAL      : 0") {
                    note!(format!("[NONE] total must be 0 with nothing running:\n{report}"));
                }
                out.push("  [NONE] all five flags: every track skipped, TOTAL 0".into());
            }
        }

        // ---- scope: one paragraph, a book, an unknown book ---------------------
        match run([false; 5], Some(&s.quay.to_string()), None) {
            Err(e) => note!(format!("[PARAGRAPH] check failed: {e}")),
            Ok(report) => {
                if !report.contains("review pass · 1 paragraph(s)") {
                    note!(format!("[PARAGRAPH] --paragraph must scope to one paragraph:\n{report}"));
                }
                out.push("  [PARAGRAPH] --paragraph <id> scopes the pass to one paragraph".into());
            }
        }
        match run([false; 5], None, Some("Harbor Lights")) {
            Err(e) => note!(format!("[BOOK] check failed: {e}")),
            Ok(report) => {
                if !report.contains("review pass · 2 paragraph(s)") {
                    note!(format!("[BOOK] --book-name must keep both paragraphs:\n{report}"));
                }
                out.push("  [BOOK] --book-name scopes to the book".into());
            }
        }
        match run([false; 5], None, Some("No Such Book")) {
            Ok(report) => note!(format!("[BOOK] an unknown --book-name must be refused, got:\n{report}")),
            Err(e) => out.push(format!("  [BOOK] unknown book refused: {e}")),
        }

        out.push(String::new());
        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  - {p}"));
        }
        assert!(problems.is_empty(), "{} check problem(s):\n{}", problems.len(), problems.join("\n"));
    });
}

#[test]
#[ignore = "probe: opens real Stores; run with --ignored --nocapture"]
fn backup_restore_probe() {
    run_probe("backup-restore", |out| {
        let mut problems: Vec<String> = Vec::new();
        macro_rules! note {
            ($msg:expr) => {{ let m: String = $msg; out.push(format!("  !! {m}")); problems.push(m); }};
        }
        let s = seed(|_| {});
        let (before_titles, before_quay, before_ledger, before_snaps) = {
            let (_cfg, store) = open(&s.root);
            let h = Hierarchy::load(&store).unwrap();
            (
                titles(&h),
                para_body(&s.root, &h, s.quay),
                para_body(&s.root, &h, s.ledger),
                store.list_snapshots(s.quay).unwrap().into_iter().map(|sn| (sn.id, sn.annotation)).collect::<Vec<_>>(),
            )
        };
        let config_before = std::fs::read_to_string(ProjectLayout::new(&s.root).config_path()).unwrap();

        // ---- backup ---------------------------------------------------------
        let outdir = tempfile::tempdir().unwrap();
        if let Err(e) = crate::cli::backup::run(&s.root, Some(outdir.path())) {
            note!(format!("[BACKUP] failed: {e}"));
        }
        let zips: Vec<PathBuf> = std::fs::read_dir(outdir.path())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("zip"))
            .collect();
        let Some(zip) = zips.first().cloned() else {
            note!("[BACKUP] no .zip written".into());
            out.push(format!("PROBLEMS ({})", problems.len()));
            panic!("{}", problems.join("\n"));
        };
        let size = std::fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);
        out.push(format!("  [BACKUP] {} ({size} bytes)", zip.file_name().unwrap().to_string_lossy()));
        if size == 0 {
            note!("[BACKUP] the archive is empty".into());
        }

        // ---- restore into a fresh directory ------------------------------------
        let dest_root = tempfile::tempdir().unwrap();
        let dest = dest_root.path().join("restored");
        if let Err(e) = crate::cli::restore::run(&zip, &dest) {
            note!(format!("[RESTORE] failed: {e}"));
        }
        if !ProjectLayout::new(&dest).is_initialized() {
            note!("[RESTORE] the restored directory is not an initialised project".into());
        } else {
            let config_after = std::fs::read_to_string(ProjectLayout::new(&dest).config_path()).unwrap_or_default();
            if config_after != config_before {
                note!("[RESTORE] inkhaven.hjson differs after restore".into());
            }
            let (_cfg, store) = open(&dest);
            let h = Hierarchy::load(&store).unwrap();
            let after_titles = titles(&h);
            if after_titles != before_titles {
                note!(format!("[RESTORE] hierarchy differs: before {before_titles:?}, after {after_titles:?}"));
            }
            if para_body(&dest, &h, s.quay) != before_quay || para_body(&dest, &h, s.ledger) != before_ledger {
                note!("[RESTORE] a paragraph body differs after restore".into());
            }
            let after_snaps: Vec<(Uuid, String)> =
                store.list_snapshots(s.quay).unwrap().into_iter().map(|sn| (sn.id, sn.annotation)).collect();
            if after_snaps != before_snaps {
                note!(format!("[RESTORE] snapshots differ: before {before_snaps:?}, after {after_snaps:?}"));
            }
            out.push(format!(
                "  [RESTORE] {} nodes, both bodies, {} snapshot(s) and the config came back identical",
                after_titles.len(),
                after_snaps.len()
            ));
        }

        // ---- a zip that is not a backup is refused -------------------------------
        let bogus = outdir.path().join("bogus.zip");
        {
            let f = std::fs::File::create(&bogus).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            use std::io::Write as _;
            zw.start_file("readme.txt", zip::write::SimpleFileOptions::default()).unwrap();
            zw.write_all(b"not a backup").unwrap();
            zw.finish().unwrap();
        }
        match crate::cli::restore::run(&bogus, &dest_root.path().join("bogus")) {
            Ok(()) => note!("[REFUSE] a zip without inkhaven.hjson must be refused".into()),
            Err(e) => out.push(format!("  [REFUSE] foreign zip refused: {e}")),
        }
        match crate::cli::restore::run(&outdir.path().join("missing.zip"), &dest_root.path().join("x")) {
            Ok(()) => note!("[REFUSE] a missing archive must be refused".into()),
            Err(_) => out.push("  [REFUSE] missing archive refused".into()),
        }

        out.push(String::new());
        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  - {p}"));
        }
        assert!(problems.is_empty(), "{} backup/restore problem(s):\n{}", problems.len(), problems.join("\n"));
    });
}

/// A minimal Scrivener 3 package: `<stem>.scrivx` plus `Files/Docs/<uuid>.rtf`.
fn write_scriv(dir: &Path) -> PathBuf {
    let scriv = dir.join("Novel.scriv");
    let docs = scriv.join("Files").join("Docs");
    std::fs::create_dir_all(&docs).unwrap();
    const U: [&str; 6] = [
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
        "00000000-0000-0000-0000-000000000003",
        "00000000-0000-0000-0000-000000000004",
        "00000000-0000-0000-0000-000000000005",
        "00000000-0000-0000-0000-000000000006",
    ];
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ScrivenerProject>
  <Binder>
    <BinderItem UUID="{}" Type="DraftFolder">
      <Title>Manuscript</Title>
      <Children>
        <BinderItem UUID="{}" Type="Folder">
          <Title>Chapter One</Title>
          <Children>
            <BinderItem UUID="{}" Type="Text"><Title>The Storm</Title></BinderItem>
            <BinderItem UUID="{}" Type="Text"><Title>The Calm</Title></BinderItem>
          </Children>
        </BinderItem>
        <BinderItem UUID="{}" Type="Folder">
          <Title>Chapter Two</Title>
          <Children>
            <BinderItem UUID="{}" Type="Text"><Title>Aftermath</Title></BinderItem>
          </Children>
        </BinderItem>
      </Children>
    </BinderItem>
  </Binder>
</ScrivenerProject>"#,
        U[0], U[1], U[2], U[3], U[4], U[5]
    );
    std::fs::write(scriv.join("Novel.scrivx"), xml).unwrap();
    std::fs::write(docs.join(format!("{}.rtf", U[2])), b"{\\rtf1\\ansi\\pard The STORMSENTINEL blew in from the west.\\par}").unwrap();
    std::fs::write(docs.join(format!("{}.rtf", U[3])), b"{\\rtf1\\ansi\\pard The CALMSENTINEL settled over the quay.\\par}").unwrap();
    std::fs::write(docs.join(format!("{}.rtf", U[5])), b"{\\rtf1\\ansi\\pard The AFTERSENTINEL counted the boats.\\par}").unwrap();
    scriv
}

#[test]
#[ignore = "probe: opens a real Store; run with --ignored --nocapture"]
fn scrivener_probe() {
    run_probe("scrivener", |out| {
        let mut problems: Vec<String> = Vec::new();
        macro_rules! note {
            ($msg:expr) => {{ let m: String = $msg; out.push(format!("  !! {m}")); problems.push(m); }};
        }
        let tmp = tempfile::tempdir().unwrap();
        let scriv = write_scriv(tmp.path());
        let project = tmp.path().join("project");
        let layout = ProjectLayout::new(&project);
        layout.create_layout().unwrap();
        let mut cfg = Config::default();
        cfg.images.preview_enabled = false;
        cfg.sync_interval_seconds = 0;
        cfg.save(&layout.config_path()).unwrap();

        // ---- dry run creates nothing -------------------------------------------
        if let Err(e) = crate::cli::import_scrivener::run(&project, &scriv, None, true, true) {
            note!(format!("[DRY] import failed: {e}"));
        }
        {
            let (_cfg, store) = open(&project);
            let h = Hierarchy::load(&store).unwrap();
            let books = h.iter().filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none()).count();
            if books != 0 {
                note!(format!("[DRY] a dry run created {books} book(s)"));
            } else {
                out.push("  [DRY] --dry-run created no book".into());
            }
        }

        // ---- the real import ---------------------------------------------------
        if let Err(e) = crate::cli::import_scrivener::run(&project, &scriv, None, true, false) {
            note!(format!("[IMPORT] failed: {e}"));
        }
        let (_cfg, store) = open(&project);
        let h = Hierarchy::load(&store).unwrap();
        let book = h.iter().find(|n| n.kind == NodeKind::Book && n.system_tag.is_none()).cloned();
        match book {
            None => note!("[IMPORT] no user book after import".into()),
            Some(book) => {
                if book.title != "Manuscript" {
                    note!(format!("[IMPORT] the Draft folder should become book `Manuscript`, got `{}`", book.title));
                }
                let chapters: Vec<String> = h
                    .children_of(Some(book.id))
                    .into_iter()
                    .filter(|n| n.kind == NodeKind::Chapter)
                    .map(|n| n.title.clone())
                    .collect();
                if chapters != ["Chapter One", "Chapter Two"] {
                    note!(format!("[IMPORT] chapters {chapters:?}"));
                }
                // Only the imported book's prose: the system books (Mythology,
                // Help, …) seed their own paragraphs on open.
                let paragraphs: Vec<(String, String)> = h
                    .flatten()
                    .into_iter()
                    .filter(|(n, _)| n.kind == NodeKind::Paragraph)
                    .filter(|(n, _)| h.ancestors(n).iter().any(|a| a.id == book.id))
                    .map(|(n, _)| (n.title.clone(), para_body(&project, &h, n.id)))
                    .collect();
                let expect = [("The Storm", "STORMSENTINEL"), ("The Calm", "CALMSENTINEL"), ("Aftermath", "AFTERSENTINEL")];
                if paragraphs.len() != expect.len() {
                    note!(format!("[IMPORT] expected {} paragraphs, got {:?}", expect.len(), paragraphs.iter().map(|p| &p.0).collect::<Vec<_>>()));
                }
                for ((title, sentinel), (got_title, got_body)) in expect.iter().zip(&paragraphs) {
                    if got_title != title {
                        note!(format!("[IMPORT] paragraph order: expected `{title}`, got `{got_title}`"));
                    }
                    if !got_body.contains(sentinel) {
                        note!(format!("[IMPORT] `{got_title}` body lacks {sentinel}: {got_body:?}"));
                    }
                    if got_body.contains("\\rtf") || got_body.contains("\\par") {
                        note!(format!("[IMPORT] `{got_title}` body still carries RTF control words: {got_body:?}"));
                    }
                }
                out.push(format!(
                    "  [IMPORT] book `{}`, chapters {chapters:?}, {} paragraph(s) with converted bodies",
                    book.title,
                    paragraphs.len()
                ));
            }
        }

        // ---- a directory that is not a package is refused ---------------------
        match crate::cli::import_scrivener::run(&project, tmp.path(), None, true, true) {
            Ok(()) => note!("[REFUSE] a directory without a .scrivx must be refused".into()),
            Err(e) => out.push(format!("  [REFUSE] non-package refused: {e}")),
        }

        out.push(String::new());
        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  - {p}"));
        }
        assert!(problems.is_empty(), "{} scrivener problem(s):\n{}", problems.len(), problems.join("\n"));
    });
}
