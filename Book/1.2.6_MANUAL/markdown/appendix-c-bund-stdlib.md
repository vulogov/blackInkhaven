# Appendix C — Bund stdlib + hooks

Quick reference for the `ink.*` stdlib and the hook lambda names. Stack signatures only — the full language tutorial lives at `Documentation/Bund/BUND_TUTORIAL.md`.

## Tree (ink.node.*, ink.tree.*)

| Word | Signature |
|------|-----------|
| ink.node.list | `( -- list )` |
| ink.node.get | `( uuid -- hash | NODATA )` |
| ink.node.children | `( parent_uuid_or_empty -- list )` |
| ink.path.to_uuid | `( path -- uuid | NODATA )` |
| ink.tree.add | `( parent_path kind title -- uuid )` |
| ink.tree.delete | `( uuid -- )` |
| ink.tree.rename | `( uuid new_title -- )` |
| ink.tree.move_up / move_down | `( uuid -- )` |
| ink.tree.morph | `( uuid new_kind -- )` |

## Paragraph (ink.paragraph.*)

| Word | Signature |
|------|-----------|
| ink.paragraph.text | `( uuid -- string | NODATA )` |
| ink.paragraph.save | `( path body -- )` |
| ink.paragraph.set_status | `( path status -- )` |
| ink.paragraph.set_target | `( path int -- )` |
| ink.paragraph.target | `( path -- int | NODATA )` |

## Tags (ink.tag.*, 1.2.6+)

| Word | Signature |
|------|-----------|
| ink.tag.list | `( -- list )` |
| ink.tag.list_for | `( path -- list | NODATA )` |
| ink.tag.search | `( tag -- list-of-paths )` |
| ink.tag.add | `( path tag -- )` |
| ink.tag.remove | `( path tag -- )` |

## Events (ink.event.*, 1.2.6+)

| Word | Signature |
|------|-----------|
| ink.event.list | `( -- list )` |
| ink.event.list_orphans | `( -- list )` |
| ink.event.add | `( book title spec -- uuid )` |
| ink.event.set_end | `( uuid spec -- )` |
| ink.event.set_precision | `( uuid prec -- )` |
| ink.event.set_track | `( uuid track -- )` |
| ink.event.link_paragraph | `( uuid path -- )` |

## Story (ink.story.*, 1.2.6+)

| Word | Signature |
|------|-----------|
| ink.story.render | `( book-name path -- )` |

Stack-only output; writes the PNG to `path`. Policy: `fs_write`.

## Editor (ink.editor.*)

| Word | Signature |
|------|-----------|
| ink.editor.cursor | `( -- list[row col] | NODATA )` |
| ink.editor.goto | `( row col -- )      // 0-based` |
| ink.editor.set_cursor | `( row col -- )  // 1-based (1.2.6+)` |
| ink.editor.insert | `( text -- )` |
| ink.editor.replace | `( text -- )` |
| ink.editor.replace_all | `( pattern replacement -- )` |
| ink.editor.delete_line / delete_to_bol / delete_to_eol | `( -- )` |
| ink.editor.scroll | `( delta_rows -- )` |

## Search (ink.search.*)

| Word | Signature |
|------|-----------|
| ink.search.text | `( query limit -- list )` |

## Snapshots (ink.snapshot.*)

| Word | Signature |
|------|-----------|
| ink.snapshot.list | `( uuid -- list )` |

## Filesystem (ink.fs.*)

| Word | Signature |
|------|-----------|
| ink.fs.read | `( path -- string )` |
| ink.fs.write | `( path body -- )       // fs_write policy` |

## AI (ink.ai.*)

| Word | Signature |
|------|-----------|
| ink.ai.send_blocking | `( prompt -- response )  // ai_write policy` |
| ink.ai.set_system_prompt | `( prompt -- )       // ai_write policy` |

## Keymap (ink.key.*)

| Word | Signature |
|------|-----------|
| ink.key.bind | `( chord action -- )    // keymap policy` |
| ink.key.unbind | `( chord -- )         // keymap policy` |

## Pane + input

| Word | Signature |
|------|-----------|
| ink.pane.open | `( title -- handle )` |
| ink.pane.write | `( handle text -- )` |
| ink.pane.close | `( handle -- )` |
| ink.input | `( prompt -- string )     // pops a modal` |

## DB (ink.db.*)

| Word | Signature |
|------|-----------|
| ink.db.sync | `( -- )` |
| ink.db.checkpoint | `( -- )` |
| ink.db.reindex | `( -- )` |

## Hooks (fire signatures)

Each hook is registered with `register`. The body sees the fire-time arguments on the stack, bottom-first.

| Hook | Signature |
|------|-----------|
| hook.on_create | `( uuid kind -- )` |
| hook.on_save | `( uuid -- )` |
| hook.on_rename | `( uuid new_title -- )` |
| hook.on_snapshot | `( parent_uuid snapshot_uuid -- )` |
| hook.on_delete | `( uuid -- )         // per id in subtree` |
| hook.on_status_promoted | `( uuid new_status -- )` |
| hook.on_goal_hit | `( word_count_today -- )` |
| hook.on_streak_break | `( -- )` |
| hook.on_assemble | `( book_uuid path -- )` |
| hook.on_take | `( book_uuid pdf_path -- )` |
| hook.on_diagnostic | `( uuid count first-message -- )  // 1.2.6+` |
| hook.on_event_added | `( uuid -- )                       // 1.2.6+` |
| hook.on_event_orphaned | `( uuid -- )                    // 1.2.6+` |

## Policy categories

| Category | Default | Words |
|----------|---------|-------|
| store_read | Allowed | Tree + paragraph + event reads. |
| store_write | Denied | ink.tree.*, ink.paragraph.save, ink.event.add/set_*, ink.tag.add/remove. |
| fs_read | Allowed | ink.fs.read. |
| fs_write | Denied | ink.fs.write, ink.story.render. |
| net | Denied | (network words). |
| shell | Denied | (shell words). |
| code_eval | Denied | Bund's `eval`. |
| keymap | Denied | ink.key.bind. |
| ai_write | Denied | ink.ai.*. |
| editor_write | Allowed | ink.editor.*. |
| theme_write | Denied | |
