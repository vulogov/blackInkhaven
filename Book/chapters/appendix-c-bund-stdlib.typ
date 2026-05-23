#import "../design.typ": *

#appendix(letter: "C", title: "Bund stdlib + hooks")

#dropcap("Q")uick reference for the `ink.*` stdlib and the
hook lambda names. Stack signatures only — the full
language tutorial lives at
`Documentation/Bund/BUND_TUTORIAL.md`.

#section("Tree (ink.node.*, ink.tree.*)")

#chord_table((
  chord_row("ink.node.list", "( -- list )"),
  chord_row("ink.node.get", "( uuid -- hash | NODATA )"),
  chord_row("ink.node.children", "( parent_uuid_or_empty -- list )"),
  chord_row("ink.path.to_uuid", "( path -- uuid | NODATA )"),
  chord_row("ink.tree.add", "( parent_path kind title -- uuid )"),
  chord_row("ink.tree.delete", "( uuid -- )"),
  chord_row("ink.tree.rename", "( uuid new_title -- )"),
  chord_row("ink.tree.move_up / move_down", "( uuid -- )"),
  chord_row("ink.tree.morph", "( uuid new_kind -- )"),
))

#section("Paragraph (ink.paragraph.*)")

#chord_table((
  chord_row("ink.paragraph.text", "( uuid -- string | NODATA )"),
  chord_row("ink.paragraph.save", "( path body -- )"),
  chord_row("ink.paragraph.set_status", "( path status -- )"),
  chord_row("ink.paragraph.set_target", "( path int -- )"),
  chord_row("ink.paragraph.target", "( path -- int | NODATA )"),
))

#section("Tags (ink.tag.*, 1.2.6+)")

#chord_table((
  chord_row("ink.tag.list", "( -- list )"),
  chord_row("ink.tag.list_for", "( path -- list | NODATA )"),
  chord_row("ink.tag.search", "( tag -- list-of-paths )"),
  chord_row("ink.tag.add", "( path tag -- )"),
  chord_row("ink.tag.remove", "( path tag -- )"),
))

#section("Events (ink.event.*, 1.2.7+)")

#chord_table((
  chord_row("ink.event.list", "( -- list )"),
  chord_row("ink.event.list_orphans", "( -- list )"),
  chord_row("ink.event.add", "( book title spec -- uuid )"),
  chord_row("ink.event.set_end", "( uuid spec -- )"),
  chord_row("ink.event.set_precision", "( uuid prec -- )"),
  chord_row("ink.event.set_track", "( uuid track -- )"),
  chord_row("ink.event.link_paragraph", "( uuid path -- )"),
))

#section("Story (ink.story.*, 1.2.6+)")

#chord_table((
  chord_row("ink.story.render", "( book-name path -- )"),
))

Stack-only output; writes the PNG to `path`. Policy:
`fs_write`.

#section("Editor (ink.editor.*)")

#chord_table((
  chord_row("ink.editor.cursor", "( -- list[row col] | NODATA )"),
  chord_row("ink.editor.goto", "( row col -- )      // 0-based"),
  chord_row("ink.editor.set_cursor", "( row col -- )  // 1-based (1.2.6+)"),
  chord_row("ink.editor.insert", "( text -- )"),
  chord_row("ink.editor.replace", "( text -- )"),
  chord_row("ink.editor.replace_all", "( pattern replacement -- )"),
  chord_row("ink.editor.delete_line / delete_to_bol / delete_to_eol", "( -- )"),
  chord_row("ink.editor.scroll", "( delta_rows -- )"),
))

#section("Search (ink.search.*)")

#chord_table((
  chord_row("ink.search.text", "( query limit -- list )"),
))

#section("Snapshots (ink.snapshot.*)")

#chord_table((
  chord_row("ink.snapshot.list", "( uuid -- list )"),
))

#section("Filesystem (ink.fs.*)")

#chord_table((
  chord_row("ink.fs.read", "( path -- string )"),
  chord_row("ink.fs.write", "( path body -- )       // fs_write policy"),
))

#section("AI (ink.ai.*)")

#chord_table((
  chord_row("ink.ai.send_blocking", "( prompt -- response )  // ai_write policy"),
  chord_row("ink.ai.set_system_prompt", "( prompt -- )       // ai_write policy"),
))

#section("Keymap (ink.key.*)")

#chord_table((
  chord_row("ink.key.bind", "( chord action -- )    // keymap policy"),
  chord_row("ink.key.unbind", "( chord -- )         // keymap policy"),
))

#section("Pane + input")

#chord_table((
  chord_row("ink.pane.open", "( title -- handle )"),
  chord_row("ink.pane.write", "( handle text -- )"),
  chord_row("ink.pane.close", "( handle -- )"),
  chord_row("ink.input", "( prompt -- string )     // pops a modal"),
))

#section("DB (ink.db.*)")

#chord_table((
  chord_row("ink.db.sync", "( -- )"),
  chord_row("ink.db.checkpoint", "( -- )"),
  chord_row("ink.db.reindex", "( -- )"),
))

#section("Hooks (fire signatures)")

Each hook is registered with `register`. The body sees the
fire-time arguments on the stack, bottom-first.

#chord_table((
  chord_row("hook.on_create", "( uuid kind -- )"),
  chord_row("hook.on_save", "( uuid -- )"),
  chord_row("hook.on_rename", "( uuid new_title -- )"),
  chord_row("hook.on_snapshot", "( parent_uuid snapshot_uuid -- )"),
  chord_row("hook.on_delete", "( uuid -- )         // per id in subtree"),
  chord_row("hook.on_status_promoted", "( uuid new_status -- )"),
  chord_row("hook.on_goal_hit", "( word_count_today -- )"),
  chord_row("hook.on_streak_break", "( -- )"),
  chord_row("hook.on_assemble", "( book_uuid path -- )"),
  chord_row("hook.on_take", "( book_uuid pdf_path -- )"),
  chord_row("hook.on_diagnostic", "( uuid count first-message -- )  // 1.2.6+"),
  chord_row("hook.on_event_added", "( uuid -- )                       // 1.2.7+"),
  chord_row("hook.on_event_orphaned", "( uuid -- )                    // 1.2.7+"),
))

#section("Policy categories")

#chord_table((
  chord_row("store_read", "Default allowed. Tree + paragraph + event reads."),
  chord_row("store_write", "Default denied. ink.tree.*, ink.paragraph.save, ink.event.add/set_*, ink.tag.add/remove."),
  chord_row("fs_read", "Default allowed. ink.fs.read."),
  chord_row("fs_write", "Default denied. ink.fs.write, ink.story.render."),
  chord_row("net", "Default denied."),
  chord_row("shell", "Default denied."),
  chord_row("code_eval", "Default denied. Bund's `eval`."),
  chord_row("keymap", "Default denied. ink.key.bind."),
  chord_row("ai_write", "Default denied. ink.ai.*."),
  chord_row("editor_write", "Default allowed. ink.editor.*."),
  chord_row("theme_write", "Default denied."),
))
