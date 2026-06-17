//! 1.3.2 PLANNING-1 P0 — `inkhaven plan` subcommand.
//!
//! `plan init` scaffolds a story-structure framework's beats into the
//! `Planning` system book as HJSON-fronted paragraphs (the Threads
//! pattern).  The deterministic coverage/pacing report (`plan check`) and
//! the AI analyze pass arrive in later phases.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::planning::Framework;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::{InsertPosition, Store, SYSTEM_TAG_PLANNING};

use super::{PlanCommand, PlanSceneCommand};

pub fn run(project: &Path, cmd: PlanCommand) -> Result<()> {
    match cmd {
        PlanCommand::Init { framework } => init(project, framework.as_deref()),
        PlanCommand::Check {
            book_name,
            json,
            drift,
        } => check(project, book_name.as_deref(), json, drift),
        PlanCommand::Analyze {
            book_name,
            provider,
        } => analyze(project, book_name.as_deref(), provider.as_deref()),
        PlanCommand::Map {
            beat,
            chapter,
            threads,
            status,
            book_name,
        } => map(
            project,
            &beat,
            &chapter,
            threads,
            status.as_deref(),
            book_name.as_deref(),
        ),
        PlanCommand::Unmap { beat } => unmap(project, &beat),
        PlanCommand::Scaffold {
            premise,
            chapters,
            book_name,
            framework,
            provider,
        } => {
            if premise.is_none() && !chapters {
                return Err(Error::Store(
                    "plan scaffold: pass --premise (fill intentions) and/or --chapters \
                     (scaffold chapter shells)"
                        .into(),
                ));
            }
            if let Some(p) = premise.as_deref() {
                scaffold_intentions(project, p, framework.as_deref(), provider.as_deref())?;
            }
            if chapters {
                scaffold_chapters(project, book_name.as_deref())?;
            }
            Ok(())
        }
        PlanCommand::Scene { cmd } => scene_cmd(project, cmd),
    }
}

/// AI structure analysis: over the book digest + the framework, the LLM
/// maps beats to chapters and names the structural problems.
fn analyze(project: &Path, book_name: Option<&str>, provider: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let book = super::resolve_user_book(&h, book_name, "plan analyze")
        .map_err(Error::Store)?
        .clone();

    // Validates beats+chapters exist + yields the framework slug.
    let (_report, fw_slug, _n) = build_report(&store, &layout, &h, &book, 0.10)?;
    let fw = Framework::parse(&fw_slug)
        .ok_or_else(|| Error::Store(format!("plan analyze: unknown framework `{fw_slug}`")))?;

    let digest =
        crate::cli::submission::ensure_digest(&layout, &cfg, &store, &h, &book, provider, false)?;

    let system = resolve_plan_prompt(
        &store,
        &h,
        &layout,
        crate::planning::ANALYZE_SLUG,
        crate::planning::analyze_system_prompt(),
    );
    let prompt = crate::planning::analyze_user_prompt(fw, &digest.as_context());
    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven plan analyze · {} · model: {model}", fw.label());
    let raw = run_blocking(&ai, model, &system, &prompt)?;
    println!("{}", raw.trim());
    Ok(())
}

/// Three-tier prompt resolution: Prompts book → `prompts.hjson` →
/// `builtin` (parity with the TUI).
fn resolve_plan_prompt(
    store: &Store,
    h: &Hierarchy,
    layout: &ProjectLayout,
    slug: &str,
    builtin: &str,
) -> String {
    super::resolve_book_prompt(store, h, slug)
        .or_else(|| {
            let path = layout.root.join("prompts.hjson");
            crate::ai::prompts::PromptLibrary::load(&path)
                .ok()
                .and_then(|lib| lib.find(slug).map(|p| p.template.clone()))
                .filter(|t| !t.trim().is_empty())
        })
        .unwrap_or_else(|| builtin.to_string())
}

/// PLANNING-2 P3 — materialize a chapter shell per beat under the
/// manuscript book, seed each with its intention, and back-link the beat.
/// Opt-in + guarded: refuses if the book already has chapters.
fn scaffold_chapters(project: &Path, book_name: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h0 = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let pairs = load_beats(&store, &h0);
    if pairs.is_empty() {
        return Err(Error::Store(
            "plan scaffold: no beats — run `inkhaven plan init` first".into(),
        ));
    }
    let book = super::resolve_user_book(&h0, book_name, "plan scaffold")
        .map_err(Error::Store)?
        .clone();
    let existing = h0
        .children_of(Some(book.id))
        .iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .count();
    if existing > 0 {
        return Err(Error::Store(format!(
            "plan scaffold --chapters: `{}` already has {existing} chapter(s) — refusing to \
             clobber. Scaffold into a fresh book.",
            book.title
        )));
    }

    let mut created = 0usize;
    for (beat_id, beat) in &pairs {
        // Reload so each new chapter orders after the previous one.
        let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
        let book_node = h
            .get(book.id)
            .cloned()
            .ok_or_else(|| Error::Store("plan scaffold: book vanished".into()))?;
        let chapter = store.create_node(
            &cfg,
            &h,
            NodeKind::Chapter,
            &beat.beat,
            Some(&book_node),
            None,
            InsertPosition::End,
        )?;
        let chapter_slug = chapter.slug.clone();

        // A starter paragraph seeded with the beat's intention.
        let mut para = store.create_node(
            &cfg,
            &h,
            NodeKind::Paragraph,
            &beat.beat,
            Some(&chapter),
            None,
            InsertPosition::End,
        )?;
        let intention = if beat.notes.trim().is_empty() {
            "(plan this beat)".to_string()
        } else {
            beat.notes.clone()
        };
        let body = format!("= {}\n\n{intention}\n", beat.beat);
        if let Some(rel) = &para.file {
            std::fs::write(store.project_root().join(rel), body.as_bytes()).map_err(Error::Io)?;
        }
        store
            .update_paragraph_content(&mut para, body.as_bytes())
            .map_err(|e| Error::Store(format!("plan scaffold: seed paragraph: {e}")))?;

        // Back-link the beat to its new chapter.
        if let Some(mut bn) = h.get(*beat_id).cloned() {
            edit_beat(&store, &mut bn, |b| b.mapped_chapter = Some(chapter_slug.clone()))?;
        }
        created += 1;
    }

    println!(
        "plan scaffold: created {created} chapter shell(s) under `{}` and mapped {created} beat(s)",
        book.title
    );
    Ok(())
}

/// PLANNING-2 P2 — fill each beat's intention from a premise.
fn scaffold_intentions(
    project: &Path,
    premise: &str,
    framework_override: Option<&str>,
    provider: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let pairs = load_beats(&store, &h);
    if pairs.is_empty() {
        return Err(Error::Store(
            "plan scaffold: no beats — run `inkhaven plan init` first".into(),
        ));
    }
    let fw_slug = framework_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| pairs[0].1.framework.clone());
    let fw = Framework::parse(&fw_slug)
        .ok_or_else(|| Error::Store(format!("plan scaffold: unknown framework `{fw_slug}`")))?;
    let beats: Vec<crate::planning::Beat> = pairs.iter().map(|(_, b)| b.clone()).collect();

    let system = resolve_plan_prompt(
        &store,
        &h,
        &layout,
        crate::planning::SCAFFOLD_SLUG,
        crate::planning::scaffold_system_prompt(),
    );
    let prompt = crate::planning::scaffold_user_prompt(fw, premise);
    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven plan scaffold · {} · model: {model}", fw.label());
    let raw = run_blocking(&ai, model, &system, &prompt)?;
    let intentions = crate::planning::parse_scaffold(&raw, &beats);

    let mut filled = 0usize;
    for (id, beat) in &pairs {
        if let Some((_, intention)) = intentions.iter().find(|(n, _)| n == &beat.beat) {
            let intention = intention.clone();
            if let Some(mut node) = h.get(*id).cloned() {
                edit_beat(&store, &mut node, |b| b.notes = intention)?;
                filled += 1;
            }
        }
    }
    println!("plan scaffold: filled {filled}/{} beat intentions", beats.len());
    if filled < beats.len() {
        eprintln!(
            "  ({} beat(s) unmatched — fill those by hand, or re-run)",
            beats.len() - filled
        );
    }
    Ok(())
}

fn run_blocking(
    ai: &crate::ai::AiClient,
    model: &str,
    system: &str,
    prompt: &str,
) -> Result<String> {
    crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(system.to_string()),
        prompt.to_string(),
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))
}

fn check(project: &Path, book_name: Option<&str>, json: bool, drift_pct: Option<u32>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let book = super::resolve_user_book(&h, book_name, "plan check")
        .map_err(Error::Store)?
        .clone();
    let drift = (drift_pct.unwrap_or(10) as f32) / 100.0;
    let (report, fw, n_chapters) = build_report(&store, &layout, &h, &book, drift)?;

    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("plan check: {e}")))?;
        println!("{out}");
    } else {
        render(&report, &book.title, n_chapters, &fw, drift);
    }
    Ok(())
}

/// Build the structure report for `book`: load the Planning-book beats,
/// measure them against the book's chapter positions, and analyze.
/// Returns `(report, framework_slug, chapter_count)`.  Shared by `plan
/// check` and the TUI structure-outline view.
pub(crate) fn build_report(
    store: &Store,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &crate::store::node::Node,
    drift: f32,
) -> Result<(crate::planning::PlanReport, String, usize)> {
    let pairs = load_beats(store, h);
    if pairs.is_empty() {
        return Err(Error::Store(
            "plan: no beats yet — run `inkhaven plan init` first".into(),
        ));
    }
    let beats: Vec<crate::planning::Beat> = pairs.into_iter().map(|(_, b)| b).collect();
    let chapters = chapter_positions(layout, h, book);
    if chapters.is_empty() {
        return Err(Error::Store(format!(
            "plan: `{}` has no chapters to measure against",
            book.title
        )));
    }
    // Known thread slugs — lets analyze flag beats referencing a thread
    // that doesn't exist.
    let known_threads = known_thread_slugs(h);

    let fw = beats.first().map(|b| b.framework.clone()).unwrap_or_default();
    let n = chapters.len();
    let mut report = crate::planning::analyze(&beats, &chapters, drift, &known_threads);
    // Tension curve (1.3.4 P0): expected (framework) vs actual (open-
    // obligation density). Flat-beat findings join the report's warnings.
    let curve = build_tension_curve(store, layout, h, book, &beats, &chapters);
    report.warnings.extend(curve.warnings.iter().cloned());
    report.tension = Some(curve);
    // Scene cards (P3): the weak-scene findings join the warnings.
    let scenes: Vec<crate::planning::Scene> =
        load_scenes(store, h).into_iter().map(|(_, s)| s).collect();
    let (scene_status, scene_warnings) = crate::planning::analyze_scenes(&scenes);
    report.warnings.extend(scene_warnings);
    report.scenes = scene_status;
    Ok((report, fw, n))
}

/// Assemble the open-obligation spans — the tension ledger (one open
/// question = weight 1.0) plus open Threads (weight = the thread's 0–10
/// tension / 10) — and build the tension curve for `book`.
fn build_tension_curve(
    store: &Store,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &crate::store::node::Node,
    beats: &[crate::planning::Beat],
    chapters: &[crate::planning::ChapterPos],
) -> crate::planning::TensionCurve {
    use crate::planning::OpenSpan;
    let mut spans: Vec<OpenSpan> = Vec::new();
    let idx_pos = |i: usize| chapters.get(i).map(|c| c.start).unwrap_or(1.0);

    // 1) Ledger obligations.
    if let Ok(ledger) = crate::tension::TensionLedger::load(&layout.root) {
        let lang = if ledger.language.trim().is_empty() {
            "english".to_string()
        } else {
            ledger.language.clone()
        };
        for (intro, resolve) in crate::tension::obligation_spans(&ledger, &lang) {
            let start = idx_pos(intro);
            let end = match resolve {
                Some(r) if r > intro => idx_pos(r),
                // resolved in its own chapter → still open across it
                Some(_) => idx_pos(intro + 1),
                None => 1.0,
            };
            spans.push(OpenSpan { start, end: end.max(start), weight: 1.0 });
        }
    }

    // 2) Open threads.
    spans.extend(thread_spans(store, h, book, chapters));

    crate::planning::tension_curve(beats, chapters, &spans, FLAT_THRESHOLD)
}

/// A beat is flagged *flat* when its actual intensity falls this far below
/// its (high) expected intensity. Shared by the curve builder + renderer.
const FLAT_THRESHOLD: f32 = 0.25;

/// For each Threads-book arc the manuscript actually references, an
/// `OpenSpan` from the first to the last chapter that links it, weighted by
/// the thread's `tension` (0–10 → 0..1). Best-effort: a thread nothing
/// links contributes nothing.
fn thread_spans(
    store: &Store,
    h: &Hierarchy,
    book: &crate::store::node::Node,
    chapters: &[crate::planning::ChapterPos],
) -> Vec<crate::planning::OpenSpan> {
    use crate::planning::OpenSpan;
    let Some(threads_book) = h.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_THREADS)
    }) else {
        return Vec::new();
    };
    let chapter_idx: std::collections::HashMap<&str, usize> = chapters
        .iter()
        .enumerate()
        .map(|(i, c)| (c.slug.as_str(), i))
        .collect();

    let mut spans = Vec::new();
    for thread in h.children_of(Some(threads_book.id)) {
        if thread.kind != NodeKind::Paragraph {
            continue;
        }
        let tid = thread.id;
        let (mut first, mut last): (Option<usize>, Option<usize>) = (None, None);
        for ch in h.children_of(Some(book.id)) {
            if ch.kind != NodeKind::Chapter {
                continue;
            }
            let Some(&ci) = chapter_idx.get(ch.slug.as_str()) else {
                continue;
            };
            let references = h.collect_subtree(ch.id).into_iter().any(|pid| {
                h.get(pid)
                    .map(|n| n.linked_paragraphs.contains(&tid))
                    .unwrap_or(false)
            });
            if references {
                first.get_or_insert(ci);
                last = Some(ci);
            }
        }
        if let (Some(f), Some(l)) = (first, last) {
            let start = chapters.get(f).map(|c| c.start).unwrap_or(0.0);
            // open through the last referencing chapter (to the next start)
            let end = chapters.get(l + 1).map(|c| c.start).unwrap_or(1.0);
            spans.push(OpenSpan {
                start,
                end: end.max(start),
                weight: thread_tension_weight(store, tid),
            });
        }
    }
    spans
}

/// A thread's tension weight: its HJSON `tension` (0–10) / 10, default 0.5
/// when absent or unparseable.
fn thread_tension_weight(store: &Store, thread_id: uuid::Uuid) -> f32 {
    #[derive(serde::Deserialize)]
    struct T {
        tension: Option<i32>,
    }
    store
        .get_content(thread_id)
        .ok()
        .flatten()
        .and_then(|b| serde_hjson::from_str::<T>(&String::from_utf8_lossy(&b)).ok())
        .and_then(|t| t.tension)
        .map(|t| (t as f32 / 10.0).clamp(0.0, 1.0))
        .unwrap_or(0.5)
}

/// Each chapter's slug + start position as a fraction of the book's total
/// words (scene-break markers excluded, as the manuscript export does).
fn chapter_positions(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &crate::store::node::Node,
) -> Vec<crate::planning::ChapterPos> {
    let mut raw: Vec<(String, usize)> = Vec::new();
    for ch in h.children_of(Some(book.id)) {
        if ch.kind != NodeKind::Chapter {
            continue;
        }
        let words: usize = crate::cli::book_walk::chapter_paragraphs_raw(layout, h, ch.id)
            .iter()
            .map(|p| crate::audiobook::typst_to_plain(p))
            .filter(|p| !crate::manuscript::is_scene_break(p))
            .map(|p| crate::progress::count_words(&p).max(0) as usize)
            .sum();
        raw.push((ch.slug.clone(), words));
    }
    let total = raw.iter().map(|(_, w)| *w).sum::<usize>().max(1);
    let mut cum = 0usize;
    raw.into_iter()
        .map(|(slug, w)| {
            let start = cum as f32 / total as f32;
            cum += w;
            crate::planning::ChapterPos { slug, start }
        })
        .collect()
}

fn render(
    report: &crate::planning::PlanReport,
    book_title: &str,
    chapter_count: usize,
    framework_slug: &str,
    drift: f32,
) {
    let fw = Framework::parse(framework_slug)
        .map(|f| f.label().to_string())
        .unwrap_or_else(|| framework_slug.to_string());
    println!("plan check · {book_title} · {fw} · {chapter_count} chapter(s)");
    println!("\nBEATS");
    for b in &report.beats {
        let (icon, detail) = match (&b.mapped_chapter, b.actual_position, b.drift) {
            (None, _, _) => ('✗', "(unmapped)".to_string()),
            (Some(ch), Some(a), Some(d)) => {
                let icon = if d.abs() > drift { '⚠' } else { '✓' };
                (icon, format!("→ {ch} ({:.0}%, {:+.0}%)", a * 100.0, d * 100.0))
            }
            (Some(ch), _, _) => ('?', format!("→ {ch} (chapter not found)")),
        };
        let threads = if b.threads.is_empty() {
            String::new()
        } else {
            format!("  ↪ {}", b.threads.join(", "))
        };
        println!(
            "  {icon} {:<28} act {}  target {:>3.0}%  {detail}{threads}",
            b.beat,
            b.act,
            b.target_position * 100.0,
        );
    }
    println!("\nPACING (act word-share)");
    for p in &report.acts {
        let actual = p.actual.map(|a| format!("{:.0}%", a * 100.0)).unwrap_or_else(|| "?".into());
        let flag = match p.actual {
            Some(a) if (a - p.expected).abs() > drift => {
                if a > p.expected {
                    " ⚠ long"
                } else {
                    " ⚠ short"
                }
            }
            _ => "",
        };
        println!(
            "  Act {}  expected {:>3.0}%  actual {:>4}{flag}",
            p.act,
            p.expected * 100.0,
            actual,
        );
    }

    // TENSION (1.3.4): the framework's expected intensity vs the actual
    // open-obligation density. Deterministic — the overlay (Ctrl+V Shift+K)
    // draws the same curve graphically.
    if let Some(t) = &report.tension {
        println!("\nTENSION (expected vs actual intensity)");
        if !t.has_actual {
            println!(
                "  (no tension data yet — run `inkhaven tension scan`, or link threads to\n   \
                 beats, to chart the actual curve against the expected shape below)"
            );
        }
        for p in &t.points {
            let actual = match p.actual {
                Some(a) => format!("{:>3.0}%", a * 100.0),
                None => "  —".to_string(),
            };
            let flat = matches!((p.gap, p.actual), (Some(g), Some(_)) if p.expected >= 0.5 && g > FLAT_THRESHOLD)
                .then_some(" ⚠ flat")
                .unwrap_or("");
            println!(
                "  {:<28} expected {:>3.0}%  actual {}{flat}",
                p.beat,
                p.expected * 100.0,
                actual,
            );
        }
    }

    // SCENES (1.3.4 P3): the goal/conflict/disaster spine per scene card,
    // with the weak-scene (no-turn) flag.
    if !report.scenes.is_empty() {
        let mark = |present: bool| if present { '●' } else { '○' };
        println!("\nSCENES (goal · conflict · disaster)");
        for s in &report.scenes {
            println!(
                "  {} {:<26} G{} C{} D{}{}",
                if s.no_turn { '⚠' } else { '·' },
                s.title,
                mark(s.has_goal),
                mark(s.has_conflict),
                mark(s.has_disaster),
                if s.no_turn { "  no turn" } else { "" },
            );
        }
    }

    println!();
    if report.warnings.is_empty() {
        println!("✓ no structural warnings");
    } else {
        println!("{} finding(s):", report.warnings.len());
        for w in &report.warnings {
            println!("  ⚠ {w}");
        }
    }

    // The slugs to copy into a beat's `mapped_chapter` / `threads` — so you
    // can map without hunting for them.
    println!("\nCHAPTER SLUGS (set `mapped_chapter:` to one of these)");
    for c in &report.chapters {
        println!("  {:<32} {:>3.0}%", c.slug, c.position * 100.0);
    }
    if report.available_threads.is_empty() {
        println!("\nTHREAD SLUGS: (none — add arcs with `inkhaven thread add`)");
    } else {
        println!("\nTHREAD SLUGS (add to a beat's `threads:` list)");
        for t in &report.available_threads {
            println!("  {t}");
        }
    }
}

fn init(project: &Path, framework: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;

    let fw = match framework {
        Some(s) => Framework::parse(s).ok_or_else(|| {
            Error::Store(format!(
                "plan init: unknown framework `{s}` \
                 (three_act|save_the_cat|story_circle|hero_journey|seven_point)"
            ))
        })?,
        None => Framework::ThreeAct,
    };

    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let planning = planning_book(&h)?;
    // Refuse to clobber an existing structure.
    if !h.children_of(Some(planning.id)).is_empty() {
        return Err(Error::Store(
            "plan init: the Planning book already has beats — remove them to re-init".into(),
        ));
    }

    let beats = fw.seed_beats();
    for beat in &beats {
        // Reload before each create so later beats see the earlier ones
        // (slug + order) — the same pattern facts/threads use.
        let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
        let planning = planning_book(&h)?;
        let mut node = store.create_node(
            &cfg,
            &h,
            NodeKind::Paragraph,
            &beat.beat,
            Some(&planning),
            None,
            InsertPosition::End,
        )?;
        let body = crate::planning::beat_body(beat);
        node.content_type = Some("hjson".to_string());
        // Disk write first (the editor reads the .typ off disk), then the
        // bdslib-only metadata/content update — the Threads pattern.
        if let Some(rel) = &node.file {
            let abs = store.project_root().join(rel);
            std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
        }
        store
            .update_paragraph_content(&mut node, body.as_bytes())
            .map_err(|e| Error::Store(format!("plan init: seed beat: {e}")))?;
    }

    println!(
        "plan init: seeded {} {} beats into the Planning book",
        beats.len(),
        fw.label(),
    );
    eprintln!("  next: map each beat to a chapter, then `inkhaven plan check` (P1)");
    Ok(())
}

// ── P0: plan map / unmap (HJSON write-back) ─────────────────────────

/// True when `ident` names this beat — by slug, slugified title, or title
/// (all case-insensitive). So `Midpoint`, `midpoint` both resolve.
fn beat_matches(slug: &str, title: &str, ident: &str) -> bool {
    slug.eq_ignore_ascii_case(ident)
        || slug.eq_ignore_ascii_case(&slug::slugify(ident))
        || title.eq_ignore_ascii_case(ident)
}

fn find_beat(h: &Hierarchy, planning_id: uuid::Uuid, ident: &str) -> Option<crate::store::node::Node> {
    h.children_of(Some(planning_id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Paragraph && beat_matches(&n.slug, &n.title, ident))
        .cloned()
}

/// Thread paragraph slugs in the Threads book.
fn known_thread_slugs(h: &Hierarchy) -> std::collections::BTreeSet<String> {
    h.iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_THREADS)
        })
        .map(|tb| {
            h.collect_subtree(tb.id)
                .into_iter()
                .filter_map(|id| h.get(id))
                .filter(|n| n.kind == NodeKind::Paragraph)
                .map(|n| n.slug.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// The Planning-book beats as `(node id, Beat)`, in order — the report's
/// `beats` aligns with this by index.  Shared by `build_report` and the
/// TUI (which needs the node ids to write a mapping back).
pub(crate) fn load_beats(
    store: &Store,
    h: &Hierarchy,
) -> Vec<(uuid::Uuid, crate::planning::Beat)> {
    let Ok(planning) = planning_book(h) else {
        return Vec::new();
    };
    h.children_of(Some(planning.id))
        .iter()
        .filter(|n| n.kind == NodeKind::Paragraph)
        .filter_map(|n| {
            store
                .get_content(n.id)
                .ok()
                .flatten()
                .and_then(|b| crate::planning::parse_beat(&String::from_utf8_lossy(&b)))
                .map(|beat| (n.id, beat))
        })
        .collect()
}

/// Load a beat's HJSON, apply `edit`, and write it back (disk-first, then
/// the bdslib content update).  The interactive-mapping primitive.
pub(crate) fn edit_beat(
    store: &Store,
    node: &mut crate::store::node::Node,
    edit: impl FnOnce(&mut crate::planning::Beat),
) -> Result<()> {
    let body = store
        .get_content(node.id)
        .map_err(|e| Error::Store(e.to_string()))?
        .ok_or_else(|| Error::Store("plan: beat has no content".into()))?;
    let mut beat = crate::planning::parse_beat(&String::from_utf8_lossy(&body))
        .ok_or_else(|| Error::Store("plan: beat body is not valid HJSON".into()))?;
    edit(&mut beat);
    save_beat(store, node, &beat)
}

/// Re-render a beat into its Planning-book paragraph (disk-first, then the
/// bdslib content update — the Threads pattern).
fn save_beat(
    store: &Store,
    node: &mut crate::store::node::Node,
    beat: &crate::planning::Beat,
) -> Result<()> {
    let body = crate::planning::beat_body(beat);
    node.content_type = Some("hjson".to_string());
    if let Some(rel) = &node.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
    }
    store
        .update_paragraph_content(node, body.as_bytes())
        .map_err(|e| Error::Store(format!("plan: save beat: {e}")))?;
    Ok(())
}

// ── P3: scene cards ─────────────────────────────────────────────────

/// The Planning-book chapter that holds scene cards (so they're separate
/// from the beat paragraphs, which are direct children of Planning).
const SCENES_CHAPTER: &str = "Scenes";

fn scene_cmd(project: &Path, cmd: PlanSceneCommand) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    match cmd {
        PlanSceneCommand::Add {
            title,
            chapter,
            goal,
            conflict,
            disaster,
        } => scene_add(&store, &cfg, &h, &title, &chapter, goal, conflict, disaster),
        PlanSceneCommand::List => scene_list(&store, &h),
        PlanSceneCommand::Set {
            title,
            chapter,
            goal,
            conflict,
            disaster,
            status,
        } => scene_set(&store, &h, &title, chapter, goal, conflict, disaster, status),
        PlanSceneCommand::Remove { title } => scene_remove(&store, &h, &title),
    }
}

fn scenes_chapter_id(h: &Hierarchy) -> Option<uuid::Uuid> {
    let planning = planning_book(h).ok()?;
    h.children_of(Some(planning.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case(SCENES_CHAPTER))
        .map(|n| n.id)
}

/// Find (or create) the `Scenes` chapter under the Planning book.
fn ensure_scenes_chapter(store: &Store, cfg: &Config, h: &Hierarchy) -> Result<crate::store::node::Node> {
    let planning = planning_book(h)?;
    if let Some(ch) = h
        .children_of(Some(planning.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case(SCENES_CHAPTER))
    {
        return Ok(ch.clone());
    }
    store
        .create_node(cfg, h, NodeKind::Chapter, SCENES_CHAPTER, Some(&planning), None, InsertPosition::End)
        .map_err(|e| Error::Store(e.to_string()))
}

/// The Planning book's scene cards as `(node id, Scene)`.
pub(crate) fn load_scenes(store: &Store, h: &Hierarchy) -> Vec<(uuid::Uuid, crate::planning::Scene)> {
    let Some(ch_id) = scenes_chapter_id(h) else {
        return Vec::new();
    };
    h.children_of(Some(ch_id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Paragraph)
        .filter_map(|n| {
            store
                .get_content(n.id)
                .ok()
                .flatten()
                .and_then(|b| crate::planning::parse_scene(&String::from_utf8_lossy(&b)))
                .map(|s| (n.id, s))
        })
        .collect()
}

fn find_scene(h: &Hierarchy, title: &str) -> Option<crate::store::node::Node> {
    let ch_id = scenes_chapter_id(h)?;
    h.children_of(Some(ch_id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Paragraph
                && (n.title.eq_ignore_ascii_case(title)
                    || n.slug.eq_ignore_ascii_case(&slug::slugify(title)))
        })
        .cloned()
}

fn save_scene(
    store: &Store,
    node: &mut crate::store::node::Node,
    scene: &crate::planning::Scene,
) -> Result<()> {
    let body = crate::planning::scene_body(scene);
    node.content_type = Some("hjson".to_string());
    if let Some(rel) = &node.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
    }
    store
        .update_paragraph_content(node, body.as_bytes())
        .map_err(|e| Error::Store(format!("plan scene: save: {e}")))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn scene_add(
    store: &Store,
    cfg: &Config,
    h: &Hierarchy,
    title: &str,
    chapter: &str,
    goal: Option<String>,
    conflict: Option<String>,
    disaster: Option<String>,
) -> Result<()> {
    if find_scene(h, title).is_some() {
        return Err(Error::Store(format!(
            "plan scene: `{title}` already exists — use `plan scene set`"
        )));
    }
    let parent = ensure_scenes_chapter(store, cfg, h)?;
    // Reload so the (possibly just-created) Scenes chapter is visible.
    let h = Hierarchy::load(store).map_err(|e| Error::Store(e.to_string()))?;
    let parent = h.get(parent.id).cloned().unwrap_or(parent);
    let mut node = store
        .create_node(cfg, &h, NodeKind::Paragraph, title, Some(&parent), None, InsertPosition::End)
        .map_err(|e| Error::Store(e.to_string()))?;
    let scene = crate::planning::Scene {
        chapter: chapter.to_string(),
        title: title.to_string(),
        goal: goal.unwrap_or_default(),
        conflict: conflict.unwrap_or_default(),
        disaster: disaster.unwrap_or_default(),
        status: "planned".to_string(),
    };
    save_scene(store, &mut node, &scene)?;
    println!("plan scene: added `{title}` under {chapter}");
    if scene.goal.trim().is_empty() || scene.disaster.trim().is_empty() {
        eprintln!("  tip: fill --goal and --disaster so the scene turns (`plan scene set`)");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn scene_set(
    store: &Store,
    h: &Hierarchy,
    title: &str,
    chapter: Option<String>,
    goal: Option<String>,
    conflict: Option<String>,
    disaster: Option<String>,
    status: Option<String>,
) -> Result<()> {
    let Some(mut node) = find_scene(h, title) else {
        return Err(Error::Store(format!(
            "plan scene: no scene `{title}` (see `plan scene list`)"
        )));
    };
    let body = store
        .get_content(node.id)
        .map_err(|e| Error::Store(e.to_string()))?
        .ok_or_else(|| Error::Store("plan scene: empty".into()))?;
    let mut scene = crate::planning::parse_scene(&String::from_utf8_lossy(&body))
        .ok_or_else(|| Error::Store("plan scene: body is not valid HJSON".into()))?;
    if let Some(c) = chapter {
        scene.chapter = c;
    }
    if let Some(g) = goal {
        scene.goal = g;
    }
    if let Some(c) = conflict {
        scene.conflict = c;
    }
    if let Some(d) = disaster {
        scene.disaster = d;
    }
    if let Some(s) = status {
        scene.status = s;
    }
    save_scene(store, &mut node, &scene)?;
    println!("plan scene: updated `{title}`");
    Ok(())
}

fn scene_remove(store: &Store, h: &Hierarchy, title: &str) -> Result<()> {
    let Some(node) = find_scene(h, title) else {
        return Err(Error::Store(format!("plan scene: no scene `{title}`")));
    };
    let fs_rel = node
        .file
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    store
        .delete_subtree(&fs_rel, &[node.id])
        .map_err(|e| Error::Store(e.to_string()))?;
    println!("plan scene: removed `{title}`");
    Ok(())
}

fn scene_list(store: &Store, h: &Hierarchy) -> Result<()> {
    let scenes: Vec<crate::planning::Scene> =
        load_scenes(store, h).into_iter().map(|(_, s)| s).collect();
    if scenes.is_empty() {
        println!(
            "plan scene: no scene cards yet — add with \
             `inkhaven plan scene add <title> --chapter <slug>`"
        );
        return Ok(());
    }
    let (statuses, warnings) = crate::planning::analyze_scenes(&scenes);
    let mark = |present: bool| if present { '●' } else { '○' };
    use std::collections::BTreeMap;
    let mut by_chapter: BTreeMap<String, Vec<&crate::planning::Scene>> = BTreeMap::new();
    for s in &scenes {
        by_chapter.entry(s.chapter.clone()).or_default().push(s);
    }
    for (ch, list) in &by_chapter {
        println!("{}", if ch.is_empty() { "(no chapter)" } else { ch });
        for s in list {
            let no_turn = statuses
                .iter()
                .find(|x| x.title == s.title)
                .map(|x| x.no_turn)
                .unwrap_or(false);
            println!(
                "  {} {:<28} G{} C{} D{}{}",
                if no_turn { '⚠' } else { '·' },
                s.title,
                mark(!s.goal.trim().is_empty()),
                mark(!s.conflict.trim().is_empty()),
                mark(!s.disaster.trim().is_empty()),
                if no_turn { "  no turn" } else { "" },
            );
        }
    }
    if !warnings.is_empty() {
        println!("\n{} weak scene(s):", warnings.len());
        for w in &warnings {
            println!("  ⚠ {w}");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn map(
    project: &Path,
    beat_ident: &str,
    chapter: &str,
    threads: Option<Vec<String>>,
    status: Option<&str>,
    book_name: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let planning = planning_book(&h)?;
    let mut node = find_beat(&h, planning.id, beat_ident).ok_or_else(|| {
        Error::Store(format!("plan map: no beat `{beat_ident}` (see `inkhaven plan check`)"))
    })?;

    // Validate the chapter slug against the target book.
    let book = super::resolve_user_book(&h, book_name, "plan map")
        .map_err(Error::Store)?
        .clone();
    let chapter_slugs: std::collections::BTreeSet<String> = chapter_positions(&layout, &h, &book)
        .into_iter()
        .map(|c| c.slug)
        .collect();
    if !chapter_slugs.contains(chapter) {
        return Err(Error::Store(format!(
            "plan map: no chapter `{chapter}` in `{}` — run `inkhaven plan check` for the slug list",
            book.title
        )));
    }
    // Validate thread slugs (if given).
    if let Some(ts) = &threads {
        let known = known_thread_slugs(&h);
        for t in ts {
            if !known.contains(t) {
                return Err(Error::Store(format!(
                    "plan map: no thread `{t}` — add it with `inkhaven thread add`"
                )));
            }
        }
    }

    let body = store
        .get_content(node.id)
        .map_err(|e| Error::Store(e.to_string()))?
        .ok_or_else(|| Error::Store("plan map: beat has no content".into()))?;
    let mut beat = crate::planning::parse_beat(&String::from_utf8_lossy(&body))
        .ok_or_else(|| Error::Store("plan map: beat body is not valid HJSON".into()))?;
    beat.mapped_chapter = Some(chapter.to_string());
    if let Some(ts) = threads {
        beat.threads = ts;
    }
    if let Some(st) = status {
        beat.status = st.to_string();
    }
    save_beat(&store, &mut node, &beat)?;
    println!("plan map: {} → {chapter}", beat.beat);
    Ok(())
}

fn unmap(project: &Path, beat_ident: &str) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let planning = planning_book(&h)?;
    let mut node = find_beat(&h, planning.id, beat_ident).ok_or_else(|| {
        Error::Store(format!("plan unmap: no beat `{beat_ident}`"))
    })?;
    let body = store
        .get_content(node.id)
        .map_err(|e| Error::Store(e.to_string()))?
        .ok_or_else(|| Error::Store("plan unmap: beat has no content".into()))?;
    let mut beat = crate::planning::parse_beat(&String::from_utf8_lossy(&body))
        .ok_or_else(|| Error::Store("plan unmap: beat body is not valid HJSON".into()))?;
    beat.mapped_chapter = None;
    save_beat(&store, &mut node, &beat)?;
    println!("plan unmap: {} is now an open gap", beat.beat);
    Ok(())
}

fn planning_book(h: &Hierarchy) -> Result<crate::store::node::Node> {
    h.iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_PLANNING)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store("plan init: Planning book missing — reopen the project to seed it".into())
        })
}

#[cfg(test)]
mod tests {
    use super::beat_matches;

    #[test]
    fn beat_ident_matches_slug_title_and_name() {
        // node slug "midpoint", title "Midpoint"
        assert!(beat_matches("midpoint", "Midpoint", "Midpoint"));
        assert!(beat_matches("midpoint", "Midpoint", "midpoint"));
        assert!(beat_matches("midpoint", "Midpoint", "MIDPOINT"));
        // multi-word: slug "all-is-lost", user types the name or slug
        assert!(beat_matches("all-is-lost", "All Is Lost", "All Is Lost"));
        assert!(beat_matches("all-is-lost", "All Is Lost", "all-is-lost"));
        assert!(!beat_matches("midpoint", "Midpoint", "climax"));
    }
}
