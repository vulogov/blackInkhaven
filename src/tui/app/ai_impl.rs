//! AI / inference orchestration on `App` — kicks off chat
//! streams (regular, lexicon-RAG, grammar-check, help-RAG,
//! diagnostic-explain, timeline-critique), turn-selection
//! navigation, chat history search, and the apply / diff-review
//! flow that lands an inference back in the editor buffer.
//! The streaming protocol itself lives in `crate::ai::stream`;
//! this is the App-side state machinery around it. Extracted
//! from `tui::app` in the 1.2.7 refactor, Phase 3 batch 7.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{Modifier, Style};
use tui_textarea::TextArea;
use uuid::Uuid;

use super::{
    critique_changes_default_prompt, critique_edit_default_prompt, current_word_or_selection,
    explain_diagnostic_default_prompt, extract_corrected_text, grammar_check_default_prompt,
    select_apply_text, sentence_rhythm_rewrite_default_prompt, FULL_SYSTEM_PROMPT,
    GRAMMAR_CHECK_SYSTEM_PROMPT, HELP_SYSTEM_PROMPT, LOCAL_SYSTEM_PROMPT,
};

use crate::ai::stream::{spawn_chat_stream, ChatTurn};
use crate::store::node::NodeKind;

use super::super::focus::Focus;
use super::super::inference::{
    AiMode, Inference, InferenceAction, InferenceMode, InferenceStatus,
};
use super::super::lexicon_build::LexiconKind;
use super::super::modal::{Modal, PromptBody, PromptCandidate, PromptSource};
use super::super::search_results::SearchHit;

impl super::App {

    pub(super) fn inference_done_with_text(&self) -> bool {
        matches!(
            self.inference.as_ref().map(|i| (&i.status, i.response.is_empty())),
            Some((InferenceStatus::Done, false))
        )
    }

    pub(super) fn apply_inference(&mut self, action: InferenceAction) {
        let Some(inf) = self.inference.as_ref() else {
            return;
        };
        let raw = inf.response.clone();
        if matches!(action, InferenceAction::CopyOnly) {
            // Copy keeps the original markdown — the user might paste it
            // somewhere that expects markdown, not Typst.
            if let Some(cb) = self.clipboard.as_mut() {
                let _ = cb.set_text(raw.clone());
            }
            self.status = "copied AI result to clipboard".into();
            return;
        }
        // 1.2.6+ — diff review gate. Only intercepts the
        // buffer-replacing actions (Replace / ReplaceCorrected);
        // additive actions (Insert / Top / Bottom) fall
        // through to the original direct path. The Modal::AiDiffReview
        // dispatcher calls `apply_inference_direct` after
        // the user accepts.
        if self.cfg.ai.diff_review_on_apply
            && matches!(action, InferenceAction::Replace | InferenceAction::ReplaceCorrected)
        {
            self.open_ai_diff_review(action, &raw);
            return;
        }
        self.apply_inference_direct(action, raw);
    }

    pub(super) fn apply_inference_direct(&mut self, action: InferenceAction, raw: String) {
        let Some(doc) = self.opened.as_mut() else {
            self.status = "no paragraph open — apply needs a focused paragraph".into();
            return;
        };

        // ReplaceCorrected has its own pipeline: pull just the corrected
        // paragraph (no commentary), skip the markdown→typst conversion
        // because the grammar prompt instructs the model to keep Typst
        // markup verbatim, and overwrite the buffer wholesale. Before
        // overwriting, snapshot the pre-correction buffer into
        // `correction_baseline` so the renderer can highlight what
        // changed in `theme.grammar_change_fg`. The highlight survives
        // saves (autosave or manual) and is dismissed only by switching
        // paragraphs or by Ctrl+B C — saving an accepted correction
        // shouldn't yank the visual diff out from under the user.
        if matches!(action, InferenceAction::ReplaceCorrected) {
            let Some(corrected) = extract_corrected_text(&raw) else {
                self.status =
                    "couldn't find corrected text in the response \
                     (expected `<<<CORRECTED>>>` block or fenced code)"
                        .into();
                return;
            };
            let baseline = doc.textarea.lines().to_vec();
            // Build a fresh TextArea from the corrected text rather than
            // shuffling cursor + selection inside the existing one — the
            // cut/select dance was leaving stray characters in the buffer
            // (the "In _The H" duplication seen in user reports).
            let corrected_lines: Vec<String> = if corrected.is_empty() {
                vec![String::new()]
            } else {
                corrected.split('\n').map(String::from).collect()
            };
            let mut new_ta = TextArea::new(corrected_lines);
            new_ta.set_cursor_line_style(
                Style::default().add_modifier(Modifier::REVERSED),
            );
            new_ta.set_line_number_style(
                Style::default().fg(self.theme.line_number_fg),
            );
            doc.textarea = new_ta;
            doc.correction_baseline = Some(baseline);
            doc.dirty = true;
            // Bump activity so idle autosave doesn't fire on the very
            // next tick (which would otherwise lose the freshness of
            // the diff before the user has had a chance to read it).
            doc.last_activity = std::time::Instant::now();
            self.status = format!(
                "applied AI result ({}) — changes highlighted; Ctrl+B C dismisses",
                action.label()
            );
            self.change_focus(Focus::Editor);
            return;
        }

        // 1.2.6+ — Replace runs through `select_apply_text`
        // so a grammar-style response with markers / fence /
        // "Corrected" heading lands ONLY the discrete block,
        // even when the user pressed `r` (which used to paste
        // the whole reply, commentary included). Insert / Top /
        // Bottom still take the full markdown→typst converted
        // body because additive applies are usually meant to
        // surface commentary too.
        let replace_payload: Option<String> =
            if matches!(action, InferenceAction::Replace) {
                match select_apply_text(&raw, false) {
                    Ok((s, _)) => Some(s),
                    Err(msg) => {
                        self.status = msg.into();
                        return;
                    }
                }
            } else {
                None
            };
        let text = super::super::markdown::markdown_to_typst(&raw);
        match action {
            InferenceAction::Replace => {
                if doc.textarea.selection_range().is_some() {
                    doc.textarea.cut();
                } else {
                    // No selection: replace the whole document.
                    use tui_textarea::CursorMove;
                    doc.textarea.move_cursor(CursorMove::Top);
                    doc.textarea.start_selection();
                    doc.textarea.move_cursor(CursorMove::Bottom);
                    doc.textarea.cut();
                }
                doc.textarea
                    .set_yank_text(replace_payload.unwrap_or_else(|| text.clone()));
                doc.textarea.paste();
            }
            InferenceAction::Insert => {
                doc.textarea.set_yank_text(text);
                doc.textarea.paste();
            }
            InferenceAction::Top => {
                use tui_textarea::CursorMove;
                doc.textarea.move_cursor(CursorMove::Top);
                doc.textarea.move_cursor(CursorMove::Head);
                doc.textarea.set_yank_text(format!("{text}\n\n"));
                doc.textarea.paste();
            }
            InferenceAction::Bottom => {
                use tui_textarea::CursorMove;
                doc.textarea.move_cursor(CursorMove::Bottom);
                doc.textarea.move_cursor(CursorMove::End);
                doc.textarea.set_yank_text(format!("\n\n{text}"));
                doc.textarea.paste();
            }
            InferenceAction::CopyOnly | InferenceAction::ReplaceCorrected => unreachable!(),
        }
        doc.dirty = true;
        // 1.2.12+ Phase D — text-insertion / paste of AI
        // output can land enough new content to change the
        // dominant-language signal; let the delta-guard
        // decide whether re-detection is warranted.  No-op
        // when the effective mode is `book_defined`.
        self.maybe_redetect_paragraph_language();
        self.status = format!("applied AI result ({})", action.label());
        self.change_focus(Focus::Editor);
    }

    /// Union of system-level prompts (from prompts.hjson) and paragraphs
    /// nested under the "Prompts" system book. System prompts come first so
    /// the user's mental model — "well-known commands at the top, project-
    /// specific scratch prompts below" — is preserved. Filtered by the
    /// substring after `/` in `ai_input`.
    pub(super) fn prompt_picker_matches(&self) -> Vec<PromptCandidate> {
        let q = self.ai_input.as_str();
        let filter = q.strip_prefix('/').unwrap_or("").trim().to_lowercase();

        // 1.2.4+: rank candidates so prefix matches beat
        // mid-word substring matches. Empty filter → keep
        // insertion order (system before book). Match scores:
        //   3 = name starts with filter
        //   2 = description starts with filter (after splitting
        //       on whitespace — so "summarize selection" matches
        //       a /sel prefix on the second word)
        //   1 = name or description contains filter
        //   0 = no match (excluded)
        let score = |name: &str, desc: &str| -> i32 {
            if filter.is_empty() {
                return 1;
            }
            let nl = name.to_lowercase();
            let dl = desc.to_lowercase();
            if nl.starts_with(&filter) {
                return 3;
            }
            if dl.split_whitespace().any(|w| w.starts_with(&filter)) {
                return 2;
            }
            if nl.contains(&filter) || dl.contains(&filter) {
                return 1;
            }
            0
        };

        let mut scored: Vec<(i32, PromptCandidate)> = Vec::new();
        // 1) prompts.hjson (system)
        for p in &self.prompts.prompts {
            let s = score(&p.name, &p.description);
            if s > 0 {
                scored.push((s, PromptCandidate {
                    name: p.name.clone(),
                    description: p.description.clone(),
                    body: PromptBody::Static(p.template.clone()),
                    source: PromptSource::System,
                    // 1.2.12+ Phase C — propagate the
                    // language tag so the picker can
                    // section + chip.
                    language: p.language.clone(),
                }));
            }
        }
        // 2) Paragraphs under the Prompts system book
        if let Some(book_id) = self.system_book_id(crate::store::SYSTEM_TAG_PROMPTS) {
            for id in self.hierarchy.collect_subtree(book_id) {
                if id == book_id {
                    continue;
                }
                let Some(node) = self.hierarchy.get(id) else {
                    continue;
                };
                if node.kind != NodeKind::Paragraph {
                    continue;
                }
                let name = node.slug.clone();
                let title = node.title.clone();
                let s = score(&name, &title);
                if s > 0 {
                    // Pull `lang:<code>` tag if present.
                    let language = node.tags.iter().find_map(|t| {
                        let lc = t.to_lowercase();
                        let rest = lc.strip_prefix("lang:")?;
                        let code = rest.trim();
                        if code.is_empty() {
                            None
                        } else {
                            Some(code.to_string())
                        }
                    });
                    scored.push((s, PromptCandidate {
                        name,
                        description: title,
                        body: PromptBody::BookParagraph(node.id),
                        source: PromptSource::Book,
                        language,
                    }));
                }
            }
        }
        // 1.2.12+ Phase C — three-tier sort:
        //   1. language-priority bucket (active → untagged → other)
        //   2. score (existing prefix > word-prefix > substring)
        //   3. stable insertion order (system before book)
        //
        // The user sees in-language prompts first, untagged
        // (back-compat) below them, other-language matches
        // at the bottom.  Section headers in the renderer
        // make the split visible.
        let active = self.active_prompt_language();
        let bucket = |lang: &Option<String>| -> u8 {
            match lang.as_deref() {
                Some(l) if l.eq_ignore_ascii_case(&active) => 0,
                None => 1,
                Some(_) => 2,
            }
        };
        scored.sort_by(|a, b| {
            bucket(&a.1.language)
                .cmp(&bucket(&b.1.language))
                .then(b.0.cmp(&a.0))
        });
        let out: Vec<PromptCandidate> = scored.into_iter().map(|(_, c)| c).collect();
        out
    }

    pub(super) fn start_inference(&mut self) {
        let raw = self.ai_input.as_str().trim().to_string();
        if raw.is_empty() {
            self.status = "empty prompt".into();
            return;
        }
        // 1.2.4+: stash the raw prompt in the history ring for
        // Up/Down recall. Avoids dupes-against-most-recent so
        // the list stays useful when the user re-sends the same
        // prompt repeatedly.
        if self.ai_prompt_history.last() != Some(&raw) {
            self.ai_prompt_history.push(raw.clone());
            // Cap the history so a long session doesn't grow
            // unbounded. 500 entries is past any reasonable
            // recall horizon.
            if self.ai_prompt_history.len() > 500 {
                let drop_n = self.ai_prompt_history.len() - 500;
                self.ai_prompt_history.drain(..drop_n);
            }
        }
        self.ai_prompt_history_cursor = None;
        // "Help!" prefix (case-sensitive) reroutes through the F1 Help-book
        // RAG flow. The rest of the line becomes the question; the AI pane
        // shows the same grounded answer the F1 modal produces.
        if let Some(rest) = raw.strip_prefix("Help!") {
            let question = rest.trim().to_string();
            self.ai_input.clear();
            if question.is_empty() {
                self.status = "Help: type a question after `Help!`".into();
                return;
            }
            self.start_help_inference(&question);
            return;
        }
        let user_query = if raw.starts_with('/') {
            // 1.2.12+ — `/name [extra args]` form routes through
            // the language-aware resolver so a user with
            // `lang:ru` tagged prompts gets the Russian variant
            // first when working on Russian prose.  Embedded-
            // fallback floor doesn't apply here: arbitrary user
            // names have no embedded counterpart, so a miss is
            // surfaced as a status message instead.
            let after = raw.trim_start_matches('/').trim();
            let want_lang = self.active_prompt_language();
            match self.resolve_prompt_optional(after, &want_lang) {
                Some(found) => self.render_template(&found.template),
                None => {
                    self.status = format!(
                        "no prompt `{after}` — type `/` to see the list"
                    );
                    return;
                }
            }
        } else {
            raw
        };

        // Prepend the AI scope context if one is set. Failures (no
        // selection, etc.) abort the submission with a status message; the
        // scope sticks around so the user can fix the cause and re-submit.
        let prompt_text = match self.build_ai_mode_context() {
            Ok(Some(prefix)) => format!("{prefix}\n\n{user_query}"),
            Ok(None) => user_query,
            Err(reason) => {
                self.status = reason;
                return;
            }
        };
        // Lift any pending Place/Character RAG prefix (set by Ctrl+B P / C
        // when the AI prompt was empty). Consumes it — one-shot.
        let prompt_text = match self.pending_rag_prefix.take() {
            Some(rag) => format!("{rag}\n\n{prompt_text}"),
            None => prompt_text,
        };
        let mode_used = self.ai_mode;

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();

        // Replay the accumulated chat history before this new user message
        // so the model has continuous context across turns.
        let mut history = self.chat_history.clone();

        // 1.2.6+ — per-paragraph AI memory. When this is a
        // Paragraph-scoped prompt AND the feature is on AND
        // there's an open paragraph, prepend the paragraph's
        // stored memory turns to the chat history so the
        // model sees the prior paragraph-specific context.
        // Also stash the target id so `pump_inference` can
        // stamp the new turns onto it after the stream
        // completes.
        let memory_target: Option<Uuid> = if self.cfg.ai.per_paragraph_memory
            && self.cfg.ai.per_paragraph_memory_max_turns > 0
            && mode_used == AiMode::Paragraph
        {
            self.opened.as_ref().map(|d| d.id)
        } else {
            None
        };
        if let Some(target_id) = memory_target {
            if let Some(node) = self.hierarchy.get(target_id) {
                let mut memory_history: Vec<ChatTurn> =
                    Vec::with_capacity(node.ai_memory.len());
                for turn in &node.ai_memory {
                    match turn.role.as_str() {
                        "user" => memory_history
                            .push(ChatTurn::User(turn.text.clone())),
                        "assistant" => memory_history
                            .push(ChatTurn::Assistant(turn.text.clone())),
                        _ => {}
                    }
                }
                // Memory comes BEFORE the visible chat
                // history — these are older turns from prior
                // sessions, so they're the prologue.
                memory_history.append(&mut history);
                history = memory_history;
            }
        }
        self.pending_paragraph_memory_target = memory_target;
        // System prompt depends on the inference mode. Local clamps the
        // model to supplied context only; Full lets it augment with
        // general knowledge while still treating context as ground truth.
        // `ink.ai.set_system_prompt` overrides both via a Bund script.
        let system_prompt = self
            .system_prompt_override
            .clone()
            .or_else(|| match self.inference_mode {
                InferenceMode::Local => Some(LOCAL_SYSTEM_PROMPT.to_string()),
                InferenceMode::Full => Some(FULL_SYSTEM_PROMPT.to_string()),
            });
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            system_prompt,
            history,
            prompt_text.clone(),
        );

        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        // Remember the user message so we can pair it with the assistant
        // turn once the stream finishes.
        self.pending_chat_user_msg = Some(prompt_text);
        // Reset chat-history scroll so the user always sees the
        // streaming reply (if they'd PageUp'd to look at earlier turns
        // before sending, the new turn would otherwise land off-screen).
        self.chat_history_scroll = 0;
        // Stay on the AI prompt pane so follow-up questions are one keystroke
        // away. Esc bounces to the AI pane to read/scroll the answer.
        self.change_focus(Focus::AiPrompt);
        let depth = self.chat_history.len() / 2 + 1;
        let scope_note = if mode_used == AiMode::None {
            String::new()
        } else {
            format!(" · scope={}", mode_used.label())
        };
        self.status = format!(
            "streaming from {provider} (chat turn #{depth}{scope_note})…"
        );
        // Auto-reset the scope so the next prompt isn't surprised by stale
        // context. The user re-cycles with F9 to pick a new scope.
        self.ai_mode = AiMode::None;
        // Clear the prompt so the next inference starts fresh.
        self.ai_input.clear();
    }

    /// Path used by the chat-history persistence hooks. Lives next
    /// to `.inkhaven-backup.json` and `.session.json` inside the
    /// project root.
    pub(super) fn chat_history_path(&self) -> std::path::PathBuf {
        self.layout.root.join(".inkhaven-chat.json")
    }

    /// Editor meta `Ctrl+B P` (Places) / `Ctrl+B C` (Characters). Treats
    /// the editor's selection (or the word under the cursor) as a lookup
    /// term, sweeps matching paragraphs in the named system book, builds
    /// a RAG context block, and either fires the inference immediately
    /// (if the AI prompt already has a query) or stashes the context as
    /// `pending_rag_prefix` and refocuses the AI prompt for the user to
    /// type a query (item 4 in the spec).
    pub(super) fn start_lexicon_inference(&mut self, kind: LexiconKind) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = format!("{} RAG needs an open paragraph", kind.label());
            return;
        };
        let lookup = current_word_or_selection(doc);
        if lookup.trim().is_empty() {
            self.status = format!(
                "{} RAG: select a name or place the cursor on one first",
                kind.label()
            );
            return;
        }

        let Some(book_id) = self.system_book_id(kind.system_tag()) else {
            self.status = format!(
                "{} book is missing — re-open the project to seed it",
                kind.label()
            );
            return;
        };

        // Case-insensitive substring match against paragraph titles. A
        // selection of "Москва" finds both "Москва" and "Москва-Сити",
        // which is usually the user's intent.
        let needle = lookup.to_lowercase();
        let mut chunks: Vec<String> = Vec::new();
        for id in self.hierarchy.collect_subtree(book_id) {
            if id == book_id {
                continue;
            }
            let Some(node) = self.hierarchy.get(id) else {
                continue;
            };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            if !node.title.to_lowercase().contains(&needle) {
                continue;
            }
            let body = match self.store.get_content(node.id) {
                Ok(Some(b)) => String::from_utf8_lossy(&b).to_string(),
                _ => continue,
            };
            chunks.push(format!(
                "── {}: {} ──\n{}\n── end {} ──",
                kind.label(),
                node.title,
                body,
                kind.label().to_lowercase()
            ));
        }
        if chunks.is_empty() {
            self.status = format!(
                "{} RAG: no entry titled like `{lookup}` in the {} book",
                kind.label(),
                kind.label()
            );
            return;
        }
        let prefix = format!(
            "── {} context for `{lookup}` ({} match(es)) ──\n\n{}",
            kind.label(),
            chunks.len(),
            chunks.join("\n\n")
        );

        // Item 4: if the AI prompt is empty, arm the prefix and let the
        // user type their question. Otherwise send immediately with the
        // current prompt as the question.
        let prompt_present = !self.ai_input.as_str().trim().is_empty();
        if prompt_present {
            self.pending_rag_prefix = Some(prefix);
            self.start_inference();
            // start_inference moves focus to AiPrompt; bounce to AI pane
            // so the user can watch the streamed answer per spec.
            self.change_focus(Focus::Ai);
        } else {
            self.pending_rag_prefix = Some(prefix);
            self.change_focus(Focus::AiPrompt);
            self.status = format!(
                "{} RAG armed for `{lookup}` — type your question and Enter",
                kind.label()
            );
        }
    }

    /// Run a grammar check on the currently-open paragraph. Resolves a
    /// "Grammar check" prompt template by precedence:
    ///   1. Paragraph titled / slugged `grammar-check` (or `Grammar check`)
    ///      under the Prompts system book.
    ///   2. Same-named entry in `prompts.hjson` (`name: "grammar-check"`).
    ///   3. Built-in fallback that constrains the LLM to checking syntax
    ///      and punctuation in `cfg.language` while preserving any Typst
    ///      formatting.
    ///
    /// In all three cases the paragraph body is appended verbatim. After
    /// streaming starts focus jumps to the AI pane so the user can watch
    /// the result render in real time.
    pub(super) fn start_grammar_check(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "grammar check needs an open paragraph".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        if body.trim().is_empty() {
            self.status = "grammar check: paragraph is empty".into();
            return;
        }

        // 1.2.12+ — Phase A: route through the three-pass
        // language-aware resolver.  Same observable behaviour as
        // the legacy 4-step pattern for projects without any
        // `lang:*` tags or `language: <code>` HJSON entries
        // (Pass 2 picks up every untagged prompt); projects that
        // *do* add per-language prompts get them preferred.
        const NAME: &str = "grammar-check";
        let want_lang = self.active_prompt_language();
        let template = self
            .resolve_prompt(NAME, &want_lang, || {
                // 1.2.12+ Phase B — embedded floor is now
                // language-aware; pass the ISO code from
                // `active_prompt_language`.
                grammar_check_default_prompt(&want_lang).to_string()
            })
            .template;

        // Render placeholders ({{selection}} / {{context}}) and then
        // append the paragraph body so the model has a single trailing
        // block to work on regardless of whether the template already
        // referenced it.
        let rendered = self.render_template(&template);
        let prompt_text = format!(
            "{rendered}\n\n── Paragraph: {title} ──\n{body}\n── end paragraph ──",
            title = doc.title
        );

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("grammar check: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        // Grammar check is a one-shot: don't replay chat history, don't
        // append the turn to history. Behaviour matches Help in that
        // sense.
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            Some(GRAMMAR_CHECK_SYSTEM_PROMPT.to_string()),
            Vec::new(),
            prompt_text,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        // Per spec: focus moves to the AI pane so the user can watch the
        // streamed result. Esc bounces back to AiPrompt for follow-ups.
        self.change_focus(Focus::Ai);
        self.status = format!(
            "Grammar check: streaming from {provider} ({})…",
            self.cfg.language
        );
    }

    pub(super) fn start_help_inference(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            self.status = "Help: empty question".into();
            return;
        }

        // Locate the Help book; required as the RAG source.
        let Some(help_id) = self.system_book_id(crate::store::SYSTEM_TAG_HELP) else {
            self.status = "Help book not present — re-open the project to seed it".into();
            return;
        };
        let help_subtree: std::collections::HashSet<Uuid> =
            self.hierarchy.collect_subtree(help_id).into_iter().collect();

        // Search broadly, then filter to nodes inside the Help subtree. We
        // ask for more than we'll actually feed to the LLM so the post-filter
        // doesn't starve us if many hits are outside Help.
        let raw_hits = match self.store.search_text(query, 40) {
            Ok(hits) => hits,
            Err(e) => {
                self.status = format!("Help: search failed: {e}");
                return;
            }
        };
        let mut chosen: Vec<SearchHit> = raw_hits
            .iter()
            .filter_map(SearchHit::parse)
            .filter(|h| help_subtree.contains(&h.id))
            .collect();
        // Keep only paragraphs — branches don't have prose to ground on.
        chosen.retain(|h| h.kind == NodeKind::Paragraph);
        // Cap context size to avoid blowing the model's window.
        const MAX_CONTEXT_PARAGRAPHS: usize = 8;
        const MAX_CHARS_PER_PARAGRAPH: usize = 2000;
        chosen.truncate(MAX_CONTEXT_PARAGRAPHS);

        // Fetch full content for the chosen paragraphs and assemble the
        // grounded context block.
        let mut context = String::new();
        let mut included = 0usize;
        for hit in &chosen {
            let body = match self.store.get_content(hit.id) {
                Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).to_string(),
                _ => continue,
            };
            let trimmed = if body.chars().count() > MAX_CHARS_PER_PARAGRAPH {
                let mut t: String = body.chars().take(MAX_CHARS_PER_PARAGRAPH).collect();
                t.push('…');
                t
            } else {
                body
            };
            let breadcrumb = self.title_breadcrumb(hit.id);
            context.push_str(&format!(
                "── Help excerpt: {} (path: {}) ──\n{}\n\n",
                hit.title, breadcrumb, trimmed
            ));
            included += 1;
        }

        if included == 0 {
            self.status = format!(
                "Help: no entries found for `{}`. Try a different question.",
                query
            );
            return;
        }

        let system_prompt = HELP_SYSTEM_PROMPT.to_string();
        let user_prompt = format!(
            "Question: {query}\n\nContext (Inkhaven Help excerpts — your ONLY allowed source):\n\n{context}\nAnswer using only the context above. If it does not contain the answer, say so plainly and suggest which part of the Help book might be relevant."
        );

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("Help: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();

        // Help is a one-shot RAG inference — no chat history is replayed
        // (so the strict grounding system prompt isn't diluted), and the
        // turn does not accumulate into `chat_history`.
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            Some(system_prompt),
            Vec::new(),
            user_prompt,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        // Land on the AI prompt pane so the user can immediately ask a
        // follow-up Help question; Esc flips to the AI pane to read.
        self.change_focus(Focus::AiPrompt);
        self.status = format!(
            "Help: streaming answer from {provider} (grounded on {included} excerpt(s))…"
        );
    }

    /// Ctrl+F12 (1.2.6+) — send the typst diagnostic at the
    /// cursor (or the closest one) to the AI pane with the
    /// configured explain-or-fix prompt. Surrounds the
    /// diagnostic with ±5 context lines so the model sees the
    /// problem and what's around it without the whole file.
    /// Used to live on bare F11; macOS grabs F11 (Mission
    /// Control / Show Desktop) so the chord never made it
    /// into the TUI.
    pub(super) fn start_explain_diagnostic(&mut self) {
        // Force a refresh so we explain the live state, not the
        // cached one.
        self.refresh_typst_diagnostics_for_opened();
        let (diag, body, title) = match self.opened.as_ref() {
            Some(doc) => {
                if doc.typst_diagnostics.is_empty() {
                    self.status =
                        "Ctrl+F12 explain: no typst diagnostics in this buffer".into();
                    return;
                }
                let (cur_row, _) = doc.textarea.cursor();
                let cur1 = cur_row + 1;
                // Pick the diagnostic closest to the cursor row.
                let picked = doc
                    .typst_diagnostics
                    .iter()
                    .min_by_key(|d| {
                        ((d.line as i64) - (cur1 as i64)).abs()
                    })
                    .cloned();
                let Some(d) = picked else {
                    self.status =
                        "Ctrl+F12 explain: no diagnostic to anchor on".into();
                    return;
                };
                let body = doc.textarea.lines().join("\n");
                (d, body, doc.title.clone())
            }
            None => {
                self.status = "Ctrl+F12 explain: no paragraph open".into();
                return;
            }
        };

        // ±5 lines of context around the diagnostic.
        let lines: Vec<&str> = body.lines().collect();
        let lo = diag.line.saturating_sub(6); // 0-based
        let hi = (diag.line + 4).min(lines.len()); // exclusive
        let mut context = String::new();
        for (idx_zero, line) in lines.iter().enumerate().take(hi).skip(lo) {
            let lineno = idx_zero + 1;
            let mark = if lineno == diag.line { ">> " } else { "   " };
            context.push_str(&format!("{mark}{lineno:>4}  {line}\n"));
        }

        // 1.2.12+ Phase B — capture the want-lang code so the
        // language-aware embedded floor can pick the right
        // variant.  `resolve_prompt_template` itself reads
        // `active_prompt_language` internally for its
        // language target.
        let want_lang = self.active_prompt_language();
        let template = self.resolve_prompt_template("explain-diagnostic", || {
            explain_diagnostic_default_prompt(&want_lang).to_string()
        });
        let rendered = self.render_template(&template);
        let prompt_text = format!(
            "{rendered}\n\n── Diagnostic ──\nline {line}:{col} — {msg}\n── end ──\n\n── Context (paragraph: {title}) ──\n{context}── end context ──",
            line = diag.line,
            col = diag.col,
            msg = diag.message,
        );

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("Ctrl+F12 explain: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            None,
            Vec::new(),
            prompt_text,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        self.change_focus(Focus::Ai);
        self.status = format!(
            "Explaining typst diagnostic at line {}:{} via {provider}…",
            diag.line, diag.col,
        );
    }

    /// F12 (1.2.6+) — AI critique of the open paragraph. Mode-
    /// aware: when split-edit (F4) is active, sends the
    /// "evaluate-changes" prompt with both the snapshot and
    /// the live buffer; otherwise sends the "critique-edit"
    /// prompt with just the live buffer. Both prompt names
    /// resolve via the standard Prompts-book → prompts.hjson
    /// → embedded precedence.
    pub(super) fn start_critique(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "F12 critique: no paragraph open".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        if body.trim().is_empty() {
            self.status = "F12 critique: paragraph is empty".into();
            return;
        }
        let title = doc.title.clone();
        let split_baseline = doc
            .split
            .as_ref()
            .map(|s| s.snapshot_lines.join("\n"));

        // 1.2.12+ Phase D — split-view with two distinct
        // paragraphs picks the `critique-compare` flow:
        // bundle both bodies and ask the LLM for a
        // comparative critique (translation faithfulness,
        // draft-vs-draft strength).  Same precedence
        // rules as the other flows — Prompts book →
        // prompts.hjson → embedded.  Skips the comparison
        // when secondary is empty or holds the same body
        // as primary (avoids self-compare); falls back to
        // single-paragraph critique-edit in those cases.
        let primary_id = doc.id;
        let split_compare: Option<(String, String, String)> = if self.split_view {
            self.secondary.as_ref().and_then(|sec| {
                if sec.id == primary_id {
                    return None;
                }
                let sec_body = sec.textarea.lines().join("\n");
                if sec_body.trim().is_empty() {
                    return None;
                }
                Some((sec.title.clone(), sec_body, sec.id.to_string()))
            })
        } else {
            None
        };

        // 1.2.12+ Phase B — embedded floors are language-
        // aware; capture the want-lang ISO and pass it into
        // whichever variant fires.  Function pointers can't
        // close over local state, so the dispatch is the
        // local `match` below.
        let want_lang = self.active_prompt_language();
        let prompt_name = if split_compare.is_some() {
            "critique-compare"
        } else if split_baseline.is_some() {
            "critique-changes"
        } else {
            "critique-edit"
        };
        let template = self
            .resolve_prompt_template(prompt_name, || match prompt_name {
                "critique-changes" => {
                    critique_changes_default_prompt(&want_lang).to_string()
                }
                "critique-compare" => {
                    super::super::app::critique_compare_default_prompt(&want_lang)
                        .to_string()
                }
                _ => critique_edit_default_prompt(&want_lang).to_string(),
            });
        let rendered = self.render_template(&template);

        let prompt_text = match (&split_compare, split_baseline.as_ref()) {
            (Some((sec_title, sec_body, _)), _) => format!(
                "{rendered}\n\n── Left (`{title}`) ──\n{body}\n── end left ──\n\n── Right (`{sec_title}`) ──\n{sec_body}\n── end right ──",
            ),
            (None, Some(baseline)) => format!(
                "{rendered}\n\n── Before (snapshot) ──\n{baseline}\n── end before ──\n\n── After (current buffer of `{title}`) ──\n{body}\n── end after ──",
            ),
            (None, None) => format!(
                "{rendered}\n\n── Paragraph: {title} ──\n{body}\n── end paragraph ──",
            ),
        };

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("F12 critique: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            None,
            Vec::new(),
            prompt_text,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        self.change_focus(Focus::Ai);
        self.status = format!(
            "F12 critique (`{prompt_name}`): streaming from {provider}…",
        );
    }

    /// 1.2.9+ — Ctrl+B Shift+T action: AI-driven
    /// show-don't-tell scan of the open paragraph.
    /// Mirrors `start_critique` exactly — builds a
    /// system prompt, spawns the streaming AI client,
    /// routes the response into the AI pane.  Uses
    /// the `show-dont-tell` prompt name; falls back
    /// to the embedded template
    /// (`show_dont_tell_default_prompt`) when the
    /// project / global prompts library doesn't
    /// provide one.
    pub(super) fn start_show_dont_tell_scan(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "show↛tell scan: no paragraph open".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        if body.trim().is_empty() {
            self.status = "show↛tell scan: paragraph is empty".into();
            return;
        }
        let title = doc.title.clone();
        let template = self.resolve_prompt_template(
            "show-dont-tell",
            || {
                let want_lang = self.active_prompt_language();
                super::super::app::show_dont_tell_default_prompt(&want_lang)
                    .to_string()
            },
        );
        let rendered = self.render_template(&template);
        let prompt_text = format!(
            "{rendered}\n\n── Paragraph: {title} ──\n{body}\n── end paragraph ──",
        );
        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("show↛tell scan: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            None,
            Vec::new(),
            prompt_text,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        self.change_focus(Focus::Ai);
        self.status =
            format!("show↛tell scan: streaming from {provider}…");
    }

    /// 1.2.11+ — Ctrl+B Shift+M.  AI-driven sentence-
    /// rhythm rewrite of the open paragraph.
    ///
    /// Prompt resolution follows the standard
    /// pattern (`resolve_prompt_template`):
    ///   1. Paragraph in the project's Prompts
    ///      system book with slug or title
    ///      `sentence-rhythm-rewrite` /
    ///      `sentence rhythm rewrite`.
    ///   2. Entry in the project's
    ///      `prompts.hjson`.
    ///   3. Embedded multilingual fallback —
    ///      `sentence_rhythm_rewrite_default_prompt`,
    ///      language-aware via `cfg.language`.
    ///
    /// Unlike show-don't-tell (which leaves the
    /// response in the AI pane for the user to
    /// read), the rewrite flow auto-opens an AI
    /// diff modal when streaming completes.  The
    /// modal carries `post_accept_snapshot =
    /// Some("Sentence rhythm rewrite")` so the
    /// apply step creates an annotated F6-
    /// discoverable snapshot of the pre-rewrite
    /// state BEFORE the buffer is replaced.
    pub(super) fn start_sentence_rhythm_rewrite(&mut self) {
        // 1.2.11+ — chord can fire from inside the
        // `Ctrl+B Shift+H` rhythm-gauge modal
        // (the natural diagnose-then-rewrite
        // workflow).  Dismiss the gauge before
        // spawning the inference so the user
        // sees the AI pane streaming the rewrite,
        // not a stale gauge frozen on a verdict.
        if matches!(self.modal, Modal::SentenceRhythm { .. }) {
            self.modal = Modal::None;
        }
        let Some(doc) = self.opened.as_ref() else {
            self.status = "rhythm rewrite: no paragraph open".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        if body.trim().is_empty() {
            self.status = "rhythm rewrite: paragraph is empty".into();
            return;
        }
        let title = doc.title.clone();
        let language = self.cfg.language.clone();
        // 1.2.12+ — Phase A: route through the language-aware
        // resolver.  Same precedence the F7 grammar-check flow
        // uses; both slug and title forms are tried inside each
        // pass by the resolver.
        const NAME: &str = "sentence-rhythm-rewrite";
        let want_lang = self.active_prompt_language();
        let template = self
            .resolve_prompt(NAME, &want_lang, || {
                // 1.2.12+ Phase B — embedded floor is now
                // a 5-language match keyed by ISO code.
                sentence_rhythm_rewrite_default_prompt(&want_lang).to_string()
            })
            .template;
        let rendered = self.render_template(&template);
        let prompt_text = format!(
            "{rendered}\n\n── Paragraph: {title} ──\n{body}\n── end paragraph ──",
        );
        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("rhythm rewrite: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            None,
            Vec::new(),
            prompt_text,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        // Flag this inference as "auto-apply via
        // diff review" — pump_inference watches
        // for the transition to Done and opens
        // the diff modal automatically.
        self.pending_rhythm_rewrite = true;
        self.change_focus(Focus::Ai);
        self.status = format!(
            "rhythm rewrite ({language}): streaming from {provider} · diff review on completion…"
        );
    }

    pub(super) fn ai_diff_review_handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if let Modal::AiDiffReview { scroll, .. } = &mut self.modal {
                    if *scroll > 0 {
                        *scroll -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if let Modal::AiDiffReview {
                    before_lines,
                    after_lines,
                    scroll,
                    wrapped_total,
                    ..
                } = &mut self.modal
                {
                    // 1.2.11+ — bound by wrapped row count
                    // when the renderer has populated it
                    // (every frame after first render).
                    // Fall back to source-line count on
                    // the very first key press before any
                    // render — it's a safe lower bound
                    // since wrapping only adds rows.
                    let max = if *wrapped_total > 0 {
                        *wrapped_total
                    } else {
                        before_lines.len().max(after_lines.len())
                    };
                    if *scroll + 1 < max {
                        *scroll += 1;
                    }
                }
            }
            KeyCode::PageUp => {
                if let Modal::AiDiffReview { scroll, .. } = &mut self.modal {
                    *scroll = scroll.saturating_sub(10);
                }
            }
            KeyCode::PageDown => {
                if let Modal::AiDiffReview {
                    before_lines,
                    after_lines,
                    scroll,
                    wrapped_total,
                    ..
                } = &mut self.modal
                {
                    let max = if *wrapped_total > 0 {
                        *wrapped_total
                    } else {
                        before_lines.len().max(after_lines.len())
                    };
                    *scroll = (*scroll + 10).min(max.saturating_sub(1));
                }
            }
            KeyCode::Home => {
                if let Modal::AiDiffReview { scroll, .. } = &mut self.modal {
                    *scroll = 0;
                }
            }
            KeyCode::End => {
                if let Modal::AiDiffReview {
                    before_lines,
                    after_lines,
                    scroll,
                    wrapped_total,
                    ..
                } = &mut self.modal
                {
                    let max = if *wrapped_total > 0 {
                        *wrapped_total
                    } else {
                        before_lines.len().max(after_lines.len())
                    };
                    *scroll = max.saturating_sub(1);
                }
            }
            // Accept — commit via the original direct path AND
            // refocus the editor pane so the user lands on the
            // freshly-edited buffer ready to type. (`e` is kept
            // as an alias for muscle memory; both behave the
            // same since 1.2.6 batch 7.)
            KeyCode::Char('a')
            | KeyCode::Char('A')
            | KeyCode::Char('e')
            | KeyCode::Char('E')
            | KeyCode::Enter => {
                let taken = std::mem::replace(&mut self.modal, Modal::None);
                if let Modal::AiDiffReview {
                    after_lines,
                    action,
                    post_accept_snapshot,
                    ..
                } = taken
                {
                    // 1.2.11+ — when the rewrite flow
                    // requested it, snapshot the
                    // pre-rewrite buffer with the
                    // supplied annotation BEFORE
                    // replacing.  This is how
                    // Ctrl+B Shift+M (sentence-
                    // rhythm rewrite) preserves the
                    // user's old prose with a
                    // labelled F6-discoverable
                    // entry.
                    if let Some(annotation) = post_accept_snapshot {
                        self.snapshot_open_paragraph_with_annotation(
                            &annotation,
                        );
                    }
                    let after = after_lines.join("\n");
                    self.apply_ai_diff_accepted(action, after, true);
                }
            }
            // Reject — close and leave the buffer alone.
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.modal = Modal::None;
                self.status = "AI diff: rejected — buffer unchanged".into();
            }
            _ => {}
        }
    }

    pub(super) fn chat_selection_step(&mut self, delta: isize) {
        let Some(sel) = self.chat_selection else { return };
        let total = self.chat_history.len();
        if total == 0 {
            self.chat_selection = None;
            return;
        }
        let new_turn = if delta < 0 {
            sel.turn.saturating_sub(delta.unsigned_abs())
        } else {
            (sel.turn + delta as usize).min(total - 1)
        };
        if let Some(s) = self.chat_selection.as_mut() {
            s.turn = new_turn;
        }
        let label = self.chat_turn_label(new_turn);
        self.status = format!("chat selection: {} {}/{total}", label, new_turn + 1);
    }

    pub(super) fn chat_selection_jump(&mut self, target: usize) {
        let Some(_sel) = self.chat_selection else { return };
        let total = self.chat_history.len();
        if total == 0 {
            self.chat_selection = None;
            return;
        }
        let new_turn = target.min(total - 1);
        if let Some(s) = self.chat_selection.as_mut() {
            s.turn = new_turn;
        }
        let label = self.chat_turn_label(new_turn);
        self.status = format!("chat selection: {} {}/{total}", label, new_turn + 1);
    }

    pub(super) fn chat_turn_label(&self, idx: usize) -> &'static str {
        match self.chat_history.get(idx) {
            Some(ChatTurn::User(_)) => "User",
            Some(ChatTurn::Assistant(_)) => "Assistant",
            None => "?",
        }
    }

    /// `c` / `C` action: copy the selected turn's text to the system
    /// clipboard. Silently no-op when no clipboard is available
    /// (headless host); status bar reports the outcome either way.
    pub(super) fn chat_selection_copy(&mut self) {
        let Some(sel) = self.chat_selection else { return };
        let Some(turn) = self.chat_history.get(sel.turn) else { return };
        let text = match turn {
            ChatTurn::User(s) | ChatTurn::Assistant(s) => s.clone(),
        };
        match self.clipboard.as_mut() {
            Some(cb) => match cb.set_text(text.clone()) {
                Ok(()) => {
                    self.status = format!(
                        "copied {} turn ({} chars)",
                        self.chat_turn_label(sel.turn),
                        text.chars().count()
                    );
                }
                Err(e) => {
                    self.status = format!("clipboard copy failed: {e}");
                }
            },
            None => {
                self.status =
                    "no system clipboard available — copy unavailable on this host".into();
            }
        }
    }

    /// `t` / `T` action: insert the selected turn's text at the
    /// editor cursor. Useful when an Assistant reply is the right
    /// next paragraph or when a User question becomes the new
    /// prompt body. Requires an open paragraph in the editor.
    pub(super) fn chat_selection_into_editor(&mut self) {
        let Some(sel) = self.chat_selection else { return };
        let Some(turn) = self.chat_history.get(sel.turn) else { return };
        let text = match turn {
            ChatTurn::User(s) | ChatTurn::Assistant(s) => s.clone(),
        };
        let label = self.chat_turn_label(sel.turn);
        if self.opened.is_none() {
            self.status =
                "no paragraph open — switch off AI fullscreen (Ctrl+B K) and pick one".into();
            return;
        }
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.insert_str(&text);
            doc.dirty = true;
        }
        self.status = format!(
            "inserted {label} turn into editor ({} chars)",
            text.chars().count()
        );
    }

    /// Find every line index in the rendered chat-history pane
    /// whose text contains `query` (case-insensitive). Render runs
    /// against the same shape `draw_chat_history` produces so the
    /// indices map 1-1 to rendered rows.
    pub(super) fn chat_search_matches(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle = query.to_lowercase();
        let (lines, _) = self.build_chat_history_lines();
        let mut out: Vec<usize> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if text.to_lowercase().contains(&needle) {
                out.push(i);
            }
        }
        out
    }

}
