//! NARR-1 — `inkhaven prose …` commands: narrative-voice profiling.
//!
//! - `profile` — (re)compute and print a book's voice profile.
//! - `refresh` — recompute stale profiles, summary only.
//! - `drift`   — chapter-to-chapter (or vs a reference project) voice drift.
//! - `suggest` — per-metric interpretation guide (English UI text).

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::prose::{
    ProseLanguage, ProseStore, VoiceProfile, VoiceScope, refresh_book, resolve_prose_language,
};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{NodeKind, Store};

use super::ProseCommand;

pub fn run(project: &Path, cmd: ProseCommand) -> Result<()> {
    match cmd {
        ProseCommand::Profile { book, deep, json, language } => {
            profile(project, book.as_deref(), deep, json, language.as_deref())
        }
        ProseCommand::Refresh { book, deep, language } => {
            refresh(project, book.as_deref(), deep, language.as_deref())
        }
        ProseCommand::Drift { book, json, language, mode, reference } => drift(
            project,
            book.as_deref(),
            json,
            language.as_deref(),
            &mode,
            reference.as_deref(),
        ),
        ProseCommand::Suggest { .. } => {
            print!("{}", suggest_text());
            Ok(())
        }
    }
}

fn open(project: &Path) -> Result<(ProjectLayout, Config, Store, Hierarchy)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    Ok((layout, cfg, store, h))
}

fn computed_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn refresh_profiles(
    layout: &ProjectLayout,
    cfg: &Config,
    store: &Store,
    h: &Hierarchy,
    book: &Node,
    language: Option<&str>,
    deep: bool,
) -> Result<Vec<VoiceProfile>> {
    let pstore = ProseStore::open(store.project_root()).map_err(|e| Error::Store(e.to_string()))?;
    refresh_book(&pstore, layout, h, cfg, book, language, deep, cfg.prose.mattr_window, &computed_now())
        .map_err(|e| Error::Store(e.to_string()))
}

fn profile(project: &Path, book_name: Option<&str>, deep: bool, json: bool, language: Option<&str>) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "prose")
        .map_err(Error::Store)?
        .clone();
    let lang_override = language.or(cfg.prose.language.as_deref());
    let deep = deep || cfg.prose.deep_metrics;
    let profiles = refresh_profiles(&layout, &cfg, &store, &h, &book, lang_override, deep)?;
    let (lang, note) = resolve_prose_language(lang_override, &cfg.language);
    let n_chapters = h
        .children_of(Some(book.id))
        .iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .count();

    if json {
        let arr: Vec<_> = profiles.iter().map(profile_to_json).collect();
        println!(
            "{}",
            serde_json::json!({
                "book": book.title,
                "prose_language": lang.as_code(),
                "note": note,
                "profiles": arr,
            })
        );
    } else {
        print!("{}", format_profile_text(&book.title, n_chapters, &lang, note.as_deref(), &profiles));
    }
    Ok(())
}

fn refresh(project: &Path, book_name: Option<&str>, deep: bool, language: Option<&str>) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "prose")
        .map_err(Error::Store)?
        .clone();
    let lang_override = language.or(cfg.prose.language.as_deref());
    let deep = deep || cfg.prose.deep_metrics;
    let profiles = refresh_profiles(&layout, &cfg, &store, &h, &book, lang_override, deep)?;
    let (lang, _) = resolve_prose_language(lang_override, &cfg.language);
    println!(
        "prose: refreshed {} profile(s) for `{}` [{}]",
        profiles.len(),
        book.title,
        lang.as_code()
    );
    Ok(())
}

fn drift(
    project: &Path,
    book_name: Option<&str>,
    json: bool,
    language: Option<&str>,
    mode: &str,
    reference: Option<&Path>,
) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "prose")
        .map_err(Error::Store)?
        .clone();
    let lang_override = language.or(cfg.prose.language.as_deref());
    let profiles = refresh_profiles(&layout, &cfg, &store, &h, &book, lang_override, cfg.prose.deep_metrics)?;
    let (lang, _) = resolve_prose_language(lang_override, &cfg.language);
    let baseline_ord = cfg.prose.baseline_chapter;

    // Cross-project reference: current book aggregate vs the reference book.
    if let Some(ref_root) = reference {
        let rstore = ProseStore::open(ref_root).map_err(|e| Error::Store(e.to_string()))?;
        let ref_books = rstore.book_slugs().map_err(|e| Error::Store(e.to_string()))?;
        let ref_slug = ref_books.first().cloned().unwrap_or_default();
        let ref_profiles = rstore.get_all(&ref_slug).map_err(|e| Error::Store(e.to_string()))?;
        let cur = profiles.iter().find(|p| p.scope == VoiceScope::Book);
        let rfp = ref_profiles.iter().find(|p| p.scope == VoiceScope::Book);
        if let (Some(a), Some(b)) = (cur, rfp) {
            if a.prose_language.as_code() != b.prose_language.as_code() {
                eprintln!(
                    "warning: reference profile language ({}) differs from current book ({}). \
                     Language-sensitive comparisons may not be meaningful; rhythm metrics are unaffected.",
                    b.prose_language.as_code(),
                    a.prose_language.as_code()
                );
            }
            print_drift(&book.title, &lang, &[("book vs reference".into(), delta(a, b))], json);
        }
        return Ok(());
    }

    // Within-book drift.
    let mut chapters: Vec<&VoiceProfile> = profiles
        .iter()
        .filter(|p| p.scope.chapter_ord().is_some())
        .collect();
    chapters.sort_by_key(|p| p.scope.chapter_ord());
    let mut rows: Vec<(String, Delta)> = Vec::new();
    match mode {
        "rolling" => {
            for w in chapters.windows(2) {
                let n = w[1].scope.chapter_ord().unwrap_or(0);
                rows.push((format!("ch.{n} vs ch.{}", n.saturating_sub(1)), delta(w[1], w[0])));
            }
        }
        _ => {
            // baseline: each chapter vs the configured baseline chapter.
            if let Some(base) = chapters
                .iter()
                .find(|p| p.scope == VoiceScope::Chapter(baseline_ord))
                .copied()
            {
                for c in &chapters {
                    let n = c.scope.chapter_ord().unwrap();
                    if n == baseline_ord {
                        continue;
                    }
                    rows.push((format!("ch.{n} vs ch.{baseline_ord}"), delta(c, base)));
                }
            }
        }
    }
    print_drift(&book.title, &lang, &rows, json);

    // Threshold crossings (the informational `prose` findings) — baseline mode.
    if !json && mode != "rolling" {
        let v = crate::prose::violations::violations(&profiles, baseline_ord, &cfg.prose.thresholds);
        if !v.is_empty() {
            println!("\nThreshold crossings (info — descriptive, not prescriptive):");
            for x in &v {
                println!(
                    "  ch.{:<3} {:<34} {:+.3}  (baseline {:.3} → {:.3})",
                    x.chapter, x.metric, x.delta, x.baseline, x.value
                );
            }
        }
    }
    Ok(())
}

/// Per-metric drift between two profiles (a − b), for the displayed signals.
struct Delta {
    cv: f32,
    mattr: f32,
    modal: Option<f32>,
    interiority: Option<f32>,
}

fn delta(a: &VoiceProfile, b: &VoiceProfile) -> Delta {
    let dopt = |x: Option<f32>, y: Option<f32>| match (x, y) {
        (Some(x), Some(y)) => Some(x - y),
        _ => None,
    };
    Delta {
        cv: a.cv - b.cv,
        mattr: a.mattr - b.mattr,
        modal: dopt(a.modal_density, b.modal_density),
        interiority: dopt(a.interiority_ratio, b.interiority_ratio),
    }
}

fn print_drift(title: &str, lang: &ProseLanguage, rows: &[(String, Delta)], json: bool) {
    if json {
        let arr: Vec<_> = rows
            .iter()
            .map(|(label, d)| {
                serde_json::json!({
                    "pair": label, "d_cv": d.cv, "d_mattr": d.mattr,
                    "d_modal_density": d.modal, "d_interiority_ratio": d.interiority,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({ "book": title, "prose_language": lang.as_code(), "drift": arr })
        );
        return;
    }
    let modal_h = match lang {
        ProseLanguage::Fr => "ΔHedging(fr)",
        ProseLanguage::Es => "ΔHedging(es)",
        _ => "ΔModal",
    };
    println!("Voice drift — \"{title}\" [{}]", lang.as_code());
    println!("{:<18} {:>8} {:>8} {:>10} {:>12}", "Pair", "ΔCV", "ΔMATTR", modal_h, "ΔInterior");
    let f = |o: Option<f32>| o.map(|v| format!("{v:+.3}")).unwrap_or_else(|| "—".into());
    for (label, d) in rows {
        println!(
            "{:<18} {:>+8.3} {:>+8.3} {:>10} {:>12}",
            label, d.cv, d.mattr, f(d.modal), f(d.interiority)
        );
    }
    if rows.is_empty() {
        println!("(need at least two chapters with profiles)");
    }
}

fn profile_to_json(p: &VoiceProfile) -> serde_json::Value {
    serde_json::json!({
        "scope": p.scope.as_str(),
        "chapter_ord": p.scope.chapter_ord(),
        "prose_language": p.prose_language.as_code(),
        "word_count": p.word_count,
        "sentence_count": p.sentence_count,
        "sent_len": { "p10": p.p10, "p25": p.p25, "p50": p.p50, "p75": p.p75, "p90": p.p90 },
        "cv": p.cv,
        "burstiness_b": p.burstiness,
        "mattr": p.mattr,
        "modal_density": p.modal_density,
        "interiority_ratio": p.interiority_ratio,
        "de_erlebte_rede_particle_density": p.de_erlebte_rede_particle_density,
        "tier2": p.tier2.map(|t| serde_json::json!({
            "sensory": {
                "visual": t.sensory[0], "auditory": t.sensory[1], "olfactory": t.sensory[2],
                "tactile": t.sensory[3], "kinesthetic": t.sensory[4],
            },
            "active_passive_ratio": t.active_passive_ratio,
        })),
    })
}

fn format_profile_text(
    title: &str,
    n_chapters: usize,
    lang: &ProseLanguage,
    note: Option<&str>,
    profiles: &[VoiceProfile],
) -> String {
    let book = profiles.iter().find(|p| p.scope == VoiceScope::Book);
    let words = book.map(|p| p.word_count).unwrap_or(0);
    let mut out = format!(
        "Prose voice profile — \"{title}\" ({n_chapters} chapters, {words} words) [{}]\n",
        lang.as_code()
    );
    if let Some(n) = note {
        out += &format!("note: {n}\n");
    }
    let rule = "─".repeat(64);
    out += &rule;
    out += "\n";
    let Some(p) = book else {
        out += "(no prose to profile)\n";
        return out;
    };
    out += &format!("Sentence CV          {:.3}\n", p.cv);
    out += &format!("Burstiness B         {:+.3}\n", p.burstiness);
    out += &format!("MATTR (w=100)        {:.3}\n", p.mattr);
    let modal_label = match lang {
        ProseLanguage::Fr => "Epistemic hedging   ",
        ProseLanguage::Es => "Epistemic hedging   ",
        _ => "Modal density       ",
    };
    match p.modal_density {
        Some(m) => {
            out += &format!("{modal_label} {m:.3}");
            if matches!(lang, ProseLanguage::Fr | ProseLanguage::Es) {
                out += "   (lexical hedging only; conditionnel/subjuntivo not detected without a parser)";
            }
            out += "\n";
        }
        None => out += &format!("{modal_label} —  (language not supported)\n"),
    }
    if let Some(i) = p.interiority_ratio {
        out += &format!("Interiority ratio    {i:.3}\n");
    }
    if let Some(d) = p.de_erlebte_rede_particle_density {
        out += &format!("  └ particle density {d:.3}  (erlebte Rede modal particles)\n");
    }
    out += &rule;
    out += "\n";
    match &p.tier2 {
        Some(t) => {
            out += &format!(
                "Sensory   V {:.3}  A {:.3}  O {:.3}  T {:.3}  K {:.3}\n",
                t.sensory[0], t.sensory[1], t.sensory[2], t.sensory[3], t.sensory[4]
            );
            out += &format!("Active/passive ratio {:.3}\n", t.active_passive_ratio);
        }
        None => {
            out += "Tier-2 not computed. Run with --deep for sensory balance + active/passive ratio.\n";
        }
    }
    out
}

fn suggest_text() -> String {
    "Prose voice metrics — how to read them\n\
     ───────────────────────────────────────\n\
     Sentence CV       below ~0.35 is metronomic; above ~0.5 is varied rhythm.\n\
                       A falling CV across chapters = the voice narrowing.\n\
     Burstiness B      bounded [-1,+1] companion to CV (B = (CV-1)/(CV+1)).\n\
     MATTR             length-corrected lexical diversity; a drop may signal\n\
                       vocabulary fatigue late in a draft.\n\
     Modal density     epistemic hedging; a rise = a more uncertain/distanced\n\
                       narrator (FR/ES: lexical items only).\n\
     Interiority       share of sentences accessing inner life; track against\n\
                       your intended POV closeness.\n\
     Sensory balance   (--deep) which senses the prose leans on; a flat profile\n\
                       may read visually monotonous.\n\
     Active/passive    (--deep) a rising ratio = more passive constructions.\n\
     \n\
     All figures are descriptive, never prescriptive — they tell you WHERE the\n\
     voice moved, not that it is wrong.\n"
        .to_string()
}
