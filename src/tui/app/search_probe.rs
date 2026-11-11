//! Vector-search probe (3.21) — opens a real project with the real embedding
//! model and checks what the search functions and their UI promise:
//!
//! * `search_text` ranks the paragraph about the queried topic first, scores
//!   fall monotonically, no id repeats, every hit resolves in the hierarchy;
//! * an edit, a delete and a move are reflected on the next search (the
//!   index stays in lockstep with the store, including the `:meta` slot);
//! * the scoped searches (Facts, similar paragraphs, Book RAG) find their
//!   hits even when the manuscript outranks them globally, and the similar
//!   picker never lists a system-book paragraph or the open paragraph;
//! * the search bar's overlay: Enter opens the selected hit, the cursor keys
//!   stay in range, typing closes it, `ink.search.load` honours its index.
//!
//! Needs the cached embedding model, so it is `#[ignore]`d:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture search_probe
//! ```
use super::modal_probe::{k, probe_app, render, send, write_body};
use super::pane_probe::run_probe;
use super::*;
use crate::store::hierarchy::Hierarchy;
use crossterm::event::KeyCode;
use std::collections::HashSet;

fn reload(app: &mut App) {
    app.hierarchy = Hierarchy::load(&app.store).unwrap();
    app.rebuild_rows_preserving_cursor();
}

fn node_by_title(app: &App, title: &str) -> Node {
    app.hierarchy.iter().find(|n| n.title == title).cloned().unwrap_or_else(|| panic!("no node {title}"))
}

fn seed_paragraph(app: &App, root: &std::path::Path, parent: &Node, title: &str, body: &str) -> Uuid {
    let h = Hierarchy::load(&app.store).unwrap();
    let mut n = app
        .store
        .create_node(&app.cfg, &h, NodeKind::Paragraph, title, Some(parent), None, InsertPosition::End)
        .unwrap();
    write_body(&app.store, root, &mut n, format!("= {title}\n\n{body}\n").as_bytes());
    n.id
}

fn hits(app: &App, query: &str, limit: usize) -> Vec<SearchHit> {
    app.store.search_text(query, limit).unwrap().iter().filter_map(SearchHit::parse).collect()
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let na: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

/// The two strings a document is embedded from: its metadata fingerprint and
/// its body (`DocumentStorage::add_document`).
fn slots(app: &App, id: Uuid) -> (String, String) {
    let node = app.hierarchy.get(id).expect("node");
    let meta = crate::storage::fingerprint::json_fingerprint(&node.to_json());
    let body = String::from_utf8_lossy(&app.store.get_content(id).unwrap().unwrap_or_default()).into_owned();
    (meta, body)
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        send(app, k(KeyCode::Char(c))).unwrap();
    }
}

/// Monotone scores, unique ids, every id live: the contract of any result list.
fn list_invariants(label: &str, app: &App, hits: &[SearchHit], limit: usize, problems: &mut Vec<String>) {
    if hits.len() > limit {
        problems.push(format!("[{label}] {} hits for limit {limit}", hits.len()));
    }
    let mut seen = HashSet::new();
    for w in hits.windows(2) {
        if w[1].score > w[0].score + 1e-6 {
            problems.push(format!("[{label}] scores rise: {} then {}", w[0].score, w[1].score));
        }
    }
    for h in hits {
        if !seen.insert(h.id) {
            problems.push(format!("[{label}] id {} repeats", h.id));
        }
        if app.hierarchy.get(h.id).is_none() {
            problems.push(format!("[{label}] hit {} ({}) is not in the hierarchy", h.id, h.title));
        }
        if !(-1.01..=1.01).contains(&h.score) {
            problems.push(format!("[{label}] score {} out of the cosine range", h.score));
        }
    }
}

#[test]
#[ignore = "probe: opens a real Store with the embedding model; run with --ignored --nocapture"]
fn search_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("search", |out| {
        let (tmp, mut app, _pid) = probe_app();
        std::env::set_current_dir(tmp.path()).unwrap();
        let mut problems: Vec<String> = Vec::new();
        let root = tmp.path().to_path_buf();

        // ── Seed: a harbor-heavy manuscript, three distinct topics, two facts ─
        let chapter = node_by_title(&app, "Chapter One");
        let book = node_by_title(&app, "Probe Book");
        let facts_book = app
            .hierarchy
            .iter()
            .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_FACTS))
            .cloned()
            .expect("the Facts system book");
        let mut harbor_ids = Vec::new();
        for i in 0..60 {
            let body = format!(
                "The harbor froze again on day {i}. Ships waited for the ice to break while the harbor master counted \
                 the frozen hulls along the quay, and the harbor pilots stayed ashore."
            );
            harbor_ids.push(seed_paragraph(&app, &root, &chapter, &format!("Harbor {i}"), &body));
        }
        let violin = seed_paragraph(&app, &root, &chapter, "Violin Lesson", "She tightened the bow, rubbed rosin along the hair, and tuned the violin's E string before the lesson.");
        let chess = seed_paragraph(&app, &root, &chapter, "Chess Night", "The knight forked the king and rook; three moves later it was checkmate and the chess club went quiet.");
        let desert = seed_paragraph(&app, &root, &chapter, "Desert Crossing", "The caravan crossed the dunes at dusk, camels roped nose to tail under a sky of salt-white stars.");
        let fact_harbor = seed_paragraph(&app, &root, &facts_book, "Harbor pilot fee", "Fact: the harbor pilot fee in the frozen harbor is twelve silver per ship, paid at the quay.");
        let _fact_chess = seed_paragraph(&app, &root, &facts_book, "Chess club night", "Fact: the chess club meets on Thursdays in the back room.");
        reload(&mut app);
        out.push(format!(
            "[SEED] {} rows · {} vectors · stale={}",
            app.store.row_count().unwrap(),
            app.store.vector_count().unwrap(),
            app.store.vector_index_appears_stale()
        ));
        if app.store.vector_index_appears_stale() {
            problems.push("[SEED] the index looks stale after seeding".into());
        }

        // ── RANK: the topic paragraph comes first; list contracts hold ────
        for (query, want, limit, within) in [
            ("violin bow rosin lesson", "Violin Lesson", 5, 1),
            ("knight checkmate chess club", "Chess Night", 5, 1),
            ("a caravan of camels crossing the dunes at dusk", "Desert Crossing", 5, 1),
            // Three bare nouns: short metadata slots of unrelated branch nodes
            // compete; the paragraph must still be near the top.
            ("caravan camels dunes", "Desert Crossing", 8, 3),
        ] {
            let h = hits(&app, query, limit);
            list_invariants("RANK", &app, &h, limit, &mut problems);
            match h.iter().position(|x| x.title == want) {
                Some(p) if p < within => {}
                p => problems.push(format!("[RANK] `{query}` → {want} at {p:?}, expected within the top {within}: {:?}", h.iter().map(|x| format!("{} ({})", x.title, x.kind.as_str())).collect::<Vec<_>>())),
            }
            out.push(format!("[RANK] `{query}` → {}", h.iter().take(4).map(|h| format!("{} ({}) {:.3}", h.title, h.kind.as_str(), h.score)).collect::<Vec<_>>().join(" · ")));
        }

        // ── COSINE: the engine's score is the cosine of the embedded strings ─
        {
            let query = "violin bow rosin lesson";
            let h = hits(&app, query, 3);
            let top = &h[0];
            let (meta, body) = slots(&app, top.id);
            let v = app.store.embed_batch(&[query, &meta, &body]).unwrap();
            let (cm, cb) = (cosine(&v[0], &v[1]), cosine(&v[0], &v[2]));
            let want = cm.max(cb);
            if (top.score - want).abs() > 2e-3 {
                problems.push(format!("[COSINE] engine score {:.4} ≠ max(meta {cm:.4}, body {cb:.4})", top.score));
            }
            out.push(format!("[COSINE] {} score {:.4} = max(meta {cm:.4}, body {cb:.4})", top.title, top.score));
        }

        // ── SLOTS: which slot carries a branch node this high (diagnostic) ──
        {
            let query = "caravan camels dunes";
            let h = hits(&app, query, 6);
            let mut lines = Vec::new();
            let mut texts: Vec<String> = vec![query.to_string(), format!("query: {query}")];
            let mut idx = Vec::new();
            for x in h.iter().chain(hits(&app, "caravan", 1).iter().filter(|_| false)) {
                let (meta, body) = slots(&app, x.id);
                idx.push((x.title.clone(), x.kind.as_str().to_string(), x.score, texts.len()));
                texts.push(meta.clone());
                texts.push(body.clone());
                texts.push(format!("passage: {meta}"));
                texts.push(format!("passage: {body}"));
            }
            let (dm, db) = slots(&app, desert);
            idx.push(("Desert Crossing".into(), "paragraph".into(), f64::NAN, texts.len()));
            texts.extend([dm.clone(), db.clone(), format!("passage: {dm}"), format!("passage: {db}")]);
            let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
            let v = app.store.embed_batch(&refs).unwrap();
            for (title, kind, score, i) in idx {
                lines.push(format!(
                    "    {title} ({kind}) engine {score:.3} · plain meta {:.3} body {:.3} · e5-prefixed meta {:.3} body {:.3}",
                    cosine(&v[0], &v[i]), cosine(&v[0], &v[i + 1]), cosine(&v[1], &v[i + 2]), cosine(&v[1], &v[i + 3])
                ));
            }
            out.push(format!("[SLOTS] `{query}`:\n{}", lines.join("\n")));
        }
        let h = hits(&app, "the harbor froze and ships waited for the ice", 50);
        list_invariants("RANK harbor", &app, &h, 50, &mut problems);
        if h.len() < 40 {
            problems.push(format!("[RANK] only {} hits for a 60-paragraph topic at limit 50", h.len()));
        }
        if h.iter().take(5).any(|x| x.title == "Violin Lesson" || x.title == "Chess Night") {
            problems.push("[RANK] an off-topic paragraph sits in the harbor top five".into());
        }
        out.push(format!("[RANK] harbor query → {} hits, top {:.3}, 50th {:.3}", h.len(), h[0].score, h.last().unwrap().score));

        // ── EDIT: a rewritten body is what the next search sees ───────────
        {
            let mut n = app.hierarchy.get(desert).cloned().unwrap();
            write_body(&app.store, &root, &mut n, b"= Desert Crossing\n\nThe lighthouse keeper trimmed the lamp wick and watched the fog roll over the rocks.\n");
            reload(&mut app);
            let h = hits(&app, "lighthouse keeper lamp fog", 5);
            if h.first().map(|x| x.id) != Some(desert) {
                problems.push(format!("[EDIT] the rewritten paragraph is not the top hit for its new text: {:?}", h.first().map(|x| x.title.clone())));
            }
            let h = hits(&app, "caravan camels dunes", 5);
            if h.first().map(|x| x.id) == Some(desert) {
                problems.push("[EDIT] the old text still ranks the rewritten paragraph first".into());
            }
            out.push("[EDIT] rewritten body replaces the old embedding".into());
        }

        // ── DELETE: a deleted paragraph never surfaces again ──────────────
        {
            app.focus = Focus::Tree;
            let row = app.rows.iter().position(|(id, _)| *id == chess).expect("chess row");
            app.tree_cursor = row;
            app.open_delete_modal();
            app.commit_delete();
            reload(&mut app);
            if app.hierarchy.get(chess).is_some() {
                problems.push("[DELETE] the paragraph is still in the hierarchy".into());
            }
            let h = hits(&app, "knight checkmate chess club", 10);
            if h.iter().any(|x| x.id == chess) {
                problems.push("[DELETE] the deleted paragraph still surfaces".into());
            }
            out.push(format!("[DELETE] chess paragraph gone · `chess` now → {:?}", h.first().map(|x| x.title.clone())));
        }

        // ── MOVE: the `:meta` slot follows the paragraph to its new chapter ─
        {
            let h = Hierarchy::load(&app.store).unwrap();
            let zebra = app
                .store
                .create_node(&app.cfg, &h, NodeKind::Chapter, "Zebra Quokka Chapter", Some(&book), None, InsertPosition::End)
                .unwrap();
            let h = Hierarchy::load(&app.store).unwrap();
            app.store.move_node_to_parent(&app.cfg, &h, violin, Some(zebra.id)).unwrap();
            reload(&mut app);
            let h = hits(&app, "zebra quokka chapter", 6);
            if !h.iter().any(|x| x.id == violin) {
                problems.push(format!(
                    "[MOVE] the moved paragraph's metadata slot still describes the old path: {:?}",
                    h.iter().map(|x| x.title.clone()).collect::<Vec<_>>()
                ));
            }
            out.push(format!("[MOVE] `zebra quokka chapter` → {:?}", h.iter().map(|x| x.title.clone()).collect::<Vec<_>>()));
        }

        // ── FACTS: scoped search finds the fact the manuscript outranks ───
        {
            let entries = app.search_facts("the harbor froze and ships waited for the ice", 25);
            if !entries.iter().any(|e| e.id == fact_harbor) {
                problems.push(format!(
                    "[FACTS] the harbor fact is missing from a Facts search ({} entries: {:?})",
                    entries.len(),
                    entries.iter().map(|e| e.title.clone()).collect::<Vec<_>>()
                ));
            }
            if entries.iter().any(|e| !app.hierarchy.under_system_book(e.id)) {
                problems.push("[FACTS] a manuscript paragraph leaked into the Facts search".into());
            }
            out.push(format!("[FACTS] {} entries, top {:?}", entries.len(), entries.first().map(|e| e.title.clone())));
        }

        // ── SIMILAR: never the open paragraph, never a system book ────────
        {
            let body = String::from_utf8_lossy(&app.store.get_content(harbor_ids[0]).unwrap().unwrap()).into_owned();
            let entries = app.find_similar_paragraphs(harbor_ids[0], &body, 20).unwrap();
            if entries.is_empty() {
                problems.push("[SIMILAR] no neighbours for a harbor paragraph".into());
            }
            if entries.iter().any(|e| e.id == harbor_ids[0]) {
                problems.push("[SIMILAR] the paragraph itself is listed".into());
            }
            for e in &entries {
                if app.hierarchy.under_system_book(e.id) {
                    problems.push(format!("[SIMILAR] system-book paragraph listed: {}", e.title));
                }
            }
            if entries.len() < 20 {
                problems.push(format!("[SIMILAR] only {} of 20 neighbours with 59 siblings on the topic", entries.len()));
            }
            out.push(format!("[SIMILAR] {} neighbours, top {:?} {:.3}", entries.len(), entries.first().map(|e| e.title.clone()), entries.first().map(|e| e.score).unwrap_or(0.0)));
        }

        // ── BOOK RAG: the scope is the book, nothing else ─────────────────
        {
            let cfg = crate::config::BookRagConfig { top_k: 5, context_expansion: 1, max_context_tokens: 4000, include_system_books: vec![], exclude_system_books: vec![] };
            let passages = crate::book_rag::retrieval::retrieve(&app.store, &app.hierarchy, &cfg, book.id, "violin bow rosin lesson").unwrap();
            if !passages.iter().any(|p| p.id == violin && p.is_hit) {
                problems.push("[RAG] the violin paragraph is not a hit for its own topic".into());
            }
            if passages.iter().any(|p| app.hierarchy.under_system_book(p.id)) {
                problems.push("[RAG] a system-book paragraph is in the book scope".into());
            }
            let with_facts = crate::config::BookRagConfig { include_system_books: vec![crate::store::SYSTEM_TAG_FACTS.into()], ..cfg.clone() };
            let passages = crate::book_rag::retrieval::retrieve(&app.store, &app.hierarchy, &with_facts, book.id, "harbor pilot fee twelve silver per ship").unwrap();
            if !passages.iter().any(|p| p.id == fact_harbor) {
                problems.push("[RAG] with the Facts book included, the pilot-fee fact is not retrieved".into());
            }
            out.push(format!("[RAG] {} passages for the fee query, hits {:?}", passages.len(), passages.iter().filter(|p| p.is_hit).map(|p| p.breadcrumb.clone()).collect::<Vec<_>>()));
        }

        // ── UI: the search bar and its overlay ────────────────────────────
        {
            app.modal = Modal::None;
            app.focus = Focus::SearchBar;
            app.search_input.clear();
            type_str(&mut app, "violin bow rosin lesson");
            send(&mut app, k(KeyCode::Enter)).unwrap();
            if !app.show_results_overlay || app.results.is_empty() {
                problems.push("[UI] Enter did not open a results overlay".into());
            }
            if app.results.first().map(|h| h.id) != Some(violin) {
                problems.push(format!("[UI] top result is {:?}", app.results.first().map(|h| h.title.clone())));
            }
            let n = app.results.len();
            for _ in 0..3 {
                send(&mut app, k(KeyCode::Down)).unwrap();
            }
            if app.results_cursor != 3.min(n - 1) {
                problems.push(format!("[UI] Down×3 → cursor {}", app.results_cursor));
            }
            send(&mut app, k(KeyCode::End)).unwrap();
            if app.results_cursor != n - 1 {
                problems.push(format!("[UI] End → cursor {} of {n}", app.results_cursor));
            }
            for _ in 0..3 {
                send(&mut app, k(KeyCode::Down)).unwrap();
            }
            if app.results_cursor != n - 1 {
                problems.push("[UI] Down past the end moved the cursor".into());
            }
            let frame = render(&mut app, 80, 24);
            if let Err(e) = frame {
                problems.push(format!("[UI] render with the cursor at the end: {e}"));
            }
            // Enter on a paragraph hit opens it; on a branch hit it moves the
            // tree cursor there instead. Walk to the first paragraph after the
            // top result and open it, then to the first branch hit.
            send(&mut app, k(KeyCode::Home)).unwrap();
            let para_pos = app.results.iter().skip(1).position(|h| h.kind == NodeKind::Paragraph).map(|p| p + 1).expect("a second paragraph hit");
            for _ in 0..para_pos {
                send(&mut app, k(KeyCode::Down)).unwrap();
            }
            let want = app.results[para_pos].id;
            send(&mut app, k(KeyCode::Enter)).unwrap();
            if app.show_results_overlay {
                problems.push("[UI] Enter left the overlay open".into());
            }
            if app.opened.as_ref().map(|d| d.id) != Some(want) {
                problems.push(format!("[UI] Enter opened {:?}, expected result {para_pos}", app.opened.as_ref().map(|d| d.id)));
            }
            app.focus = Focus::SearchBar;
            send(&mut app, k(KeyCode::Enter)).unwrap();
            if let Some(branch_pos) = app.results.iter().position(|h| h.kind != NodeKind::Paragraph) {
                for _ in 0..branch_pos {
                    send(&mut app, k(KeyCode::Down)).unwrap();
                }
                let branch = app.results[branch_pos].id;
                send(&mut app, k(KeyCode::Enter)).unwrap();
                if app.focus != Focus::Tree || app.rows.get(app.tree_cursor).map(|(id, _)| *id) != Some(branch) {
                    problems.push(format!("[UI] Enter on a branch hit: focus {:?}, tree cursor on {:?}", app.focus, app.rows.get(app.tree_cursor)));
                }
            }
            // Typing after a search closes the overlay; Esc closes it too.
            app.focus = Focus::SearchBar;
            send(&mut app, k(KeyCode::Enter)).unwrap();
            if !app.show_results_overlay {
                problems.push("[UI] a repeated Enter did not search again".into());
            }
            send(&mut app, k(KeyCode::Char('x'))).unwrap();
            if app.show_results_overlay {
                problems.push("[UI] typing kept the overlay open".into());
            }
            send(&mut app, k(KeyCode::Backspace)).unwrap();
            send(&mut app, k(KeyCode::Enter)).unwrap();
            send(&mut app, k(KeyCode::Esc)).unwrap();
            if app.show_results_overlay || app.focus != Focus::SearchBar {
                problems.push(format!("[UI] Esc: overlay {} focus {:?}", app.show_results_overlay, app.focus));
            }
            // An empty query is refused, not searched.
            app.search_input.clear();
            send(&mut app, k(KeyCode::Enter)).unwrap();
            if app.show_results_overlay {
                problems.push("[UI] an empty query opened the overlay".into());
            }
            out.push(format!("[UI] overlay: {n} results · Enter opens the selected hit · typing / Esc close"));
        }

        // ── INK: `ink.search.load` honours its index ──────────────────────
        {
            match app.ink_search_load("violin bow rosin lesson", 0) {
                Ok(true) if app.opened.as_ref().map(|d| d.id) == Some(violin) => {}
                other => problems.push(format!("[INK] index 0 → {other:?}, opened {:?}", app.opened.as_ref().map(|d| d.id))),
            }
            if app.ink_search_load("violin bow rosin lesson", 500) != Ok(false) {
                problems.push("[INK] an out-of-range index did not report false".into());
            }
            if app.ink_search_load("   ", 0).is_ok() {
                problems.push("[INK] an empty query was accepted".into());
            }
            out.push("[INK] ink.search.load: index 0 opens the top hit, 500 → false, blank → error".into());
        }

        out.push(format!("PROBLEMS ({}):", problems.len()));
        out.extend(problems.iter().cloned());
        assert!(problems.is_empty(), "{} problem(s) — see the report", problems.len());
    });
}
