//! 3.0.6 — end-to-end smoke test for the store-backed `ink.*` Bund words.
//!
//! Most `ink.*` words are covered only by the compile + policy-classification
//! guards and (for the pure ones) `scripting::eval`. This module exercises the
//! *store-backed* words against a real project: build a tiny manuscript, then
//! run each word and assert it neither panics nor returns the wrong top-of-stack
//! shape. It is the runtime trust layer for the Bund coverage.
//!
//! Why `#[ignore]` (run locally with `cargo test -- --ignored bund_words`):
//! - `Store::open` builds the embedding engine (MultilingualE5Small), which
//!   downloads over the network on a cold cache — unfit for the default CI suite.
//! - The scripting active-store globals (`ACTIVE_STORE`/`ACTIVE_CONFIG`/`POLICY`)
//!   are process-wide `OnceLock`s: the first `Store::open` wins and later ones
//!   no-op. So this must be ONE test that opens ONE project — it cannot run
//!   alongside any other real-store test in the same process.

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::{InsertPosition, Store};

#[test]
#[ignore = "opens a real Store (embedding-model download) and uses the process-global active store; run locally with --ignored"]
fn bund_words_smoke_over_a_real_project() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = Config::default();
    // Store::open ALSO registers the scripting active store/config/policy globals.
    // The eval VM (`ADAM`) is then built under this default policy, so the
    // write words (import/backup.make/doctor.autofix — all default-denied) can't
    // be exercised here; they are covered by the doctor_scan unit tests and the
    // policy classification guards. This smoke covers the default-allowed reads.
    let store = Store::open(ProjectLayout::new(tmp.path()), &cfg).expect("open store");

    // Build Book → Chapter → Paragraph (reload the hierarchy between inserts so
    // the next create_node sees its parent).
    let h = Hierarchy::load(&store).unwrap();
    let book = store
        .create_node(&cfg, &h, NodeKind::Book, "Test Book", None, None, InsertPosition::End)
        .unwrap();
    let h = Hierarchy::load(&store).unwrap();
    let chapter = store
        .create_node(&cfg, &h, NodeKind::Chapter, "Chapter One", Some(&book), None, InsertPosition::End)
        .unwrap();
    let h = Hierarchy::load(&store).unwrap();
    let mut para = store
        .create_node(&cfg, &h, NodeKind::Paragraph, "Para One", Some(&chapter), None, InsertPosition::End)
        .unwrap();
    store
        .update_paragraph_content(
            &mut para,
            b"= Para One\n\nThe harbor froze in January. Either it was a blessing or a curse.\n",
        )
        .unwrap();

    // A panic inside any word unwinds and fails the test; a clean script Err is
    // acceptable for setup-dependent words (handled by `runs`).
    let top_json = |code: &str| -> serde_json::Value {
        let out = crate::scripting::eval(code).unwrap_or_else(|e| panic!("{code}: {e:#}"));
        let top = out.top.unwrap_or_else(|| panic!("{code}: left no value on the stack"));
        crate::scripting::value_to_json(&top)
    };
    let expect_list = |code: &str| {
        assert!(top_json(code).is_array(), "{code} should push a list");
    };
    let expect_dict = |code: &str| {
        assert!(top_json(code).is_object(), "{code} should push a dict");
    };
    let expect_string = |code: &str| {
        assert!(top_json(code).is_string(), "{code} should push a string");
    };
    // Must not panic; Ok or clean Err are both fine (nodata / uninstalled index).
    let runs = |code: &str| {
        let _ = crate::scripting::eval(code);
    };

    // ── reader words that return a list ──
    expect_list("rigor.scan");
    expect_list("\"Either you win or you lose, there is no middle ground.\" rigor.paragraph");
    expect_list("cost.usage");
    expect_list("wordnet.list");
    expect_list("research.facts");
    expect_list("research.undisputed");
    expect_list("\"harbor\" 5 research.sources");
    expect_list("locorum.build");
    expect_list("locorum.malformed");
    expect_list("verborum.build");
    expect_list("doctor.scan");
    expect_list("backup.list");
    expect_list("planning.frameworks");
    expect_list("\"three_act\" planning.beats");
    expect_list("\"\" ink.words");

    // ── reader words that return a dict ──
    expect_dict("rigor.check");
    expect_dict("goals.streak");
    expect_dict("goals.snapshot");
    expect_dict("cost.caps");
    expect_dict("cost.today");
    expect_dict("companions.findings");
    expect_dict("companions.promotions");
    expect_dict("companions.world");
    expect_dict("companions.summary");
    expect_dict("doctor.integrity");
    expect_dict("doctor.vectors");

    // ── reader words that return a string ──
    expect_string("research.report");
    expect_string("\"md\" locorum.render");
    expect_string("\"md\" verborum.render");

    // ── must-not-panic (nodata / index-dependent) ──
    runs("\"00000000-0000-0000-0000-000000000000\" research.provenance");
    runs("backup.last");
    runs("\"cat\" \"en\" wordnet.lookup");
    runs("\"cat\" \"en\" wordnet.suggest");
    // huge k must be clamped, not overflow-panic (F1 regression guard).
    expect_list("\"harbor\" 9223372036854775807 research.sources");
    // planning.check/gaps resolve the first user book then error cleanly on a
    // project with no beats ("run plan init") — must not panic. (The dict/book
    // shape is covered by planning.rs unit tests where beats exist.)
    runs("planning.check");
    runs("planning.gaps");
}
