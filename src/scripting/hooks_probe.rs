//! Bund hooks probe (3.22) — the lifecycle hooks the tutorial lists
//! (`hook.on_create`, `on_save`, `on_rename`, `on_snapshot`, `on_delete`),
//! fired by real `Store` mutations on a temp project whose bootstrap defines
//! a lambda for each. Every lambda records what it saw in a Bund variable
//! the probe reads back, so the check is on the arguments the hook received,
//! not just on "something ran". Also: an `ink.*` read word inside a hook, a
//! hook that underflows the stack (the mutation must still succeed and later
//! hooks must still fire), a hook fired directly through `hooks::fire`, and a
//! hook nobody registered.
//!
//! Adam, the policy and the active store are process-wide singletons set by
//! the first `Store::open`, so this runs alone:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture hooks_probe
//! ```
#![cfg(test)]

use rust_dynamic::value::Value;

use crate::config::Config;
use crate::probe_kit::run_probe;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{InsertPosition, NodeKind, Store};

/// Each hook stores its arguments under `seen.*` (`"name" value var`
/// registers a variable; the top of the workbench is the last argument).
const BOOTSTRAP: &str = r#"
"hook.on_create" { "seen.create_kind" swap var "seen.create_uuid" swap var } register
"hook.on_save" { dup ink.paragraph.text "seen.save_text" swap var "seen.save_uuid" swap var } register
"hook.on_rename" { "seen.rename_title" swap var "seen.rename_uuid" swap var } register
"hook.on_snapshot" { "seen.snapshot_id" swap var "seen.snapshot_parent" swap var } register
"hook.on_delete" { drop drop drop } register
"hook.probe_custom" { "seen.custom" swap var } register
"seen.boot" "yes" var
"#;

/// The value of Bund variable `name`, or the eval error.
fn var(name: &str) -> Result<String, String> {
    let out = crate::scripting::eval(&format!("\"{name}\" var?")).map_err(|e| e.to_string())?;
    let v = out.top.ok_or_else(|| "nothing on the stack".to_string())?;
    v.cast_string().map_err(|e| e.to_string())
}

#[test]
#[ignore = "probe: builds the process-wide Adam VM over a real store; run alone with --ignored --nocapture"]
fn hooks_probe() {
    run_probe("hooks", |out| {
        let mut problems: Vec<String> = Vec::new();
        macro_rules! check {
            ($cond:expr, $($arg:tt)*) => {{
                let msg = format!($($arg)*);
                if $cond { out.push(format!("  ok   {msg}")); } else { out.push(format!("  !!   {msg}")); problems.push(msg); }
            }};
        }

        let tmp = tempfile::tempdir().expect("tempdir");
        let mut cfg = Config::default();
        cfg.images.preview_enabled = false;
        cfg.sync_interval_seconds = 0;
        cfg.scripting.bootstrap = BOOTSTRAP.to_string();
        // The bootstrap is gated by the project-trust decision (S.6/H1): an
        // untrusted project runs no bootstrap and therefore defines no hooks.
        // A real author marks the project trusted; the probe does the same.
        cfg.scripting.trust_decision = "trust".to_string();
        let layout = ProjectLayout::new(tmp.path());
        layout.create_layout().unwrap();
        cfg.save(&layout.config_path()).unwrap();
        let store = Store::open(layout.clone(), &cfg).expect("open store");
        let root = tmp.path();

        // Nothing has fired yet: the VM is built lazily on the first hook.
        check!(
            !crate::scripting::adam_initialised(),
            "Adam is not built before the first hook fires"
        );

        // ---- create: on_create ( uuid kind -- ) then on_save ( uuid -- ) ----
        let h = Hierarchy::load(&store).unwrap();
        let book = store.create_node(&cfg, &h, NodeKind::Book, "Hooked", None, None, InsertPosition::End).unwrap();
        let h = Hierarchy::load(&store).unwrap();
        let chapter = store
            .create_node(&cfg, &h, NodeKind::Chapter, "Quayside", Some(&book), None, InsertPosition::End)
            .unwrap();
        check!(crate::scripting::adam_initialised(), "the first mutation built Adam (bootstrap present)");
        check!(var("seen.boot").as_deref() == Ok("yes"), "the bootstrap ran (seen.boot = {:?})", var("seen.boot"));
        check!(
            var("seen.create_kind").as_deref() == Ok("chapter"),
            "on_create saw kind `chapter` for the chapter (got {:?})",
            var("seen.create_kind")
        );
        check!(
            var("seen.create_uuid").as_deref() == Ok(chapter.id.to_string().as_str()),
            "on_create saw the chapter's uuid"
        );

        let h = Hierarchy::load(&store).unwrap();
        let body1 = b"= Quay\n\nFrozen ropes, frozen ledgers.\n";
        let mut para = store
            .create_paragraph_with_body(&cfg, &h, "Quay", Some(&chapter), InsertPosition::End, body1, None)
            .unwrap();
        if let Some(rel) = &para.file {
            std::fs::write(root.join(rel), body1).unwrap();
        }
        check!(
            var("seen.create_kind").as_deref() == Ok("paragraph"),
            "on_create saw kind `paragraph` (got {:?})",
            var("seen.create_kind")
        );
        check!(
            var("seen.save_uuid").as_deref() == Ok(para.id.to_string().as_str()),
            "on_save fired for the new paragraph with its uuid (got {:?})",
            var("seen.save_uuid")
        );
        let text = var("seen.save_text").unwrap_or_default();
        check!(
            text.contains("Frozen ropes"),
            "`ink.paragraph.text` inside on_save read the saved body (got {text:?})"
        );

        // ---- save: on_save sees the new content --------------------------------
        let body2 = b"= Quay\n\nThawed ropes, open ledgers.\n";
        if let Some(rel) = &para.file {
            std::fs::write(root.join(rel), body2).unwrap();
        }
        store.update_paragraph_content(&mut para, body2).unwrap();
        let text = var("seen.save_text").unwrap_or_default();
        check!(
            text.contains("Thawed ropes") && !text.contains("Frozen"),
            "on_save after update_paragraph_content reads the new body (got {text:?})"
        );

        // ---- rename: on_rename ( uuid new_title -- ) ------------------------------
        let h = Hierarchy::load(&store).unwrap();
        store.rename_node(&h, para.id, "Pier").unwrap();
        check!(
            var("seen.rename_title").as_deref() == Ok("Pier"),
            "on_rename saw the new title (got {:?})",
            var("seen.rename_title")
        );
        check!(
            var("seen.rename_uuid").as_deref() == Ok(para.id.to_string().as_str()),
            "on_rename saw the renamed uuid"
        );
        let h = Hierarchy::load(&store).unwrap();
        check!(
            h.get(para.id).map(|n| n.title.as_str()) == Some("Pier"),
            "the rename itself landed"
        );

        // ---- snapshot: on_snapshot ( parent_uuid snap_uuid -- ) -------------------
        let h = Hierarchy::load(&store).unwrap();
        let node = h.get(para.id).cloned().unwrap();
        let snap = store.create_snapshot_annotated(&node, body2, "probe note").unwrap();
        check!(
            var("seen.snapshot_id").as_deref() == Ok(snap.to_string().as_str()),
            "on_snapshot saw the snapshot uuid (got {:?})",
            var("seen.snapshot_id")
        );
        check!(
            var("seen.snapshot_parent").as_deref() == Ok(para.id.to_string().as_str()),
            "on_snapshot saw the parent uuid"
        );

        // ---- delete: a hook that underflows the stack must not break the delete --
        let h = Hierarchy::load(&store).unwrap();
        let victim = store
            .create_paragraph_with_body(&cfg, &h, "Victim", Some(&chapter), InsertPosition::End, b"= Victim\n\nGone soon.\n", None)
            .unwrap();
        let fs_rel = victim.file.as_ref().map(std::path::PathBuf::from).unwrap_or_default();
        let deleted = store.delete_subtree(&fs_rel, &[victim.id]);
        check!(deleted.is_ok(), "delete_subtree succeeded although on_delete underflows ({deleted:?})");
        let h = Hierarchy::load(&store).unwrap();
        check!(h.get(victim.id).is_none(), "the victim is gone from the hierarchy");

        // Later hooks still fire after the faulty one (depth counter restored).
        let body3 = b"= Pier\n\nAfter the fault.\n";
        if let Some(rel) = &para.file {
            std::fs::write(root.join(rel), body3).unwrap();
        }
        store.update_paragraph_content(&mut para, body3).unwrap();
        check!(
            var("seen.save_text").unwrap_or_default().contains("After the fault"),
            "on_save still fires after a hook fault"
        );

        // ---- direct fire + an unregistered hook ------------------------------------
        crate::scripting::hooks::fire("hook.probe_custom", vec![Value::from_string("ping")]);
        check!(
            var("seen.custom").as_deref() == Ok("ping"),
            "hooks::fire delivers its argument to a registered lambda (got {:?})",
            var("seen.custom")
        );
        crate::scripting::hooks::fire("hook.nobody_registered_this", vec![Value::from_string("x")]);
        check!(
            var("seen.custom").as_deref() == Ok("ping"),
            "an unregistered hook is a silent no-op"
        );
        // The stack is clean after every hook: nothing left behind by the fires.
        let depth = crate::scripting::with_adam(|b| b.vm.stack.current_stack_len()).unwrap_or(usize::MAX);
        check!(depth == 0, "the workbench is empty after the hooks ran (depth {depth})");

        out.push(String::new());
        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  - {p}"));
        }
        assert!(problems.is_empty(), "{} hook problem(s):\n{}", problems.len(), problems.join("\n"));
    });
}
