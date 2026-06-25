//! BOOK_RAG-1 — Chat with Your Book.
//!
//! The AI pane's **Book scope** is retrieval-augmented: a prompt in Book
//! scope retrieves the semantically relevant paragraphs (via the existing
//! `Store::search_text` over the vecstore HNSW index), expands them with a
//! little surrounding context, composes a focused, token-budgeted context,
//! and grounds the LLM's answer in those passages with markdown citations
//! — instead of sending the entire book. It is the generalisation of the
//! shipped Facts semantic-search-grounds-a-chat pattern (`search_facts`)
//! from the Facts book to the manuscript.
//!
//! This module holds the pure pieces (passage type, token estimate,
//! context composition, system prompt) plus the retrieval core
//! ([`retrieval::retrieve`]), shared by the TUI Book scope
//! (`book_rag_impl.rs`) and the `inkhaven book-rag` CLI.

pub mod retrieval;

/// One retrieved paragraph, ready to compose into the LLM context.
#[derive(Debug, Clone)]
pub struct RetrievedPassage {
    /// Paragraph node id — also its citation anchor (`[id](#id)`).
    pub id: uuid::Uuid,
    /// `chapter-slug/paragraph-slug`-style breadcrumb for the heading.
    pub breadcrumb: String,
    /// The paragraph's prose (`.typ` body).
    pub body: String,
    /// Vecstore similarity score (0..1). Expansion paragraphs carry the
    /// score of the hit that pulled them in. Surfaced by the transparency
    /// section.
    pub score: f64,
    /// True for a direct semantic hit; false for a context-expansion
    /// neighbour pulled in around a hit.
    pub is_hit: bool,
}

/// Rough token estimate (≈ chars / 4) — there is no tokenizer in-tree, and
/// the budget only needs to be approximately right.
pub fn estimate_tokens(s: &str) -> usize {
    s.chars().count() / 4
}

/// Compose the retrieved passages into the context block prepended to the
/// user's prompt. Each passage is labelled with its breadcrumb and citation
/// id so the LLM can cite it as `[id](#id)`.
pub fn compose_context_prefix(passages: &[RetrievedPassage]) -> String {
    if passages.is_empty() {
        return "── Retrieved passages ──\n(No passages in this book matched the \
                query semantically.)\n── end retrieved passages ──"
            .to_string();
    }
    let mut out = String::from("── Retrieved passages (grounding evidence) ──\n");
    for p in passages {
        let marker = if p.is_hit { " ★" } else { "" };
        out.push_str(&format!(
            "\n[{id}] {breadcrumb}{marker}\n{body}\n",
            id = p.id,
            breadcrumb = p.breadcrumb,
            marker = marker,
            body = p.body.trim(),
        ));
    }
    out.push_str("\n── end retrieved passages ──");
    out
}

/// The set of paragraph ids cited by the retrieval — used by the citation
/// validator to flag any cited id the LLM invented.
pub fn cited_ids(passages: &[RetrievedPassage]) -> std::collections::HashSet<String> {
    passages.iter().map(|p| p.id.to_string()).collect()
}

/// Flag hallucinated citations inline. Scans the LLM response for markdown
/// fragment links — `](#id)` — and, for any `id` NOT in `valid_ids` (the
/// retrieval set), appends a visible `[citation could not be validated: id]`
/// after the link so the author sees what's grounded vs. invented. A
/// structural commitment to grounding integrity (RFC §8.3).
pub fn validate_citations(
    response: &str,
    valid_ids: &std::collections::HashSet<String>,
) -> String {
    const OPEN: &str = "](#";
    let mut out = String::with_capacity(response.len() + 32);
    let mut rest = response;
    while let Some(pos) = rest.find(OPEN) {
        let frag_start = pos + OPEN.len();
        let after = &rest[frag_start..];
        let Some(end) = after.find(')') else {
            out.push_str(rest); // unterminated link — leave verbatim
            return out;
        };
        let id = &after[..end];
        // Copy through the closing ')'.
        out.push_str(&rest[..frag_start + end + 1]);
        if !id.is_empty() && !valid_ids.contains(id) {
            out.push_str(&format!(" [citation could not be validated: {id}]"));
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The Book-RAG system prompt: ground answers in the retrieved passages,
/// cite with markdown links, and be honest when the passages don't address
/// the question. Localised to the multilingual baseline (EN/RU/ES/FR/DE);
/// any other language falls back to English. The language code is matched on
/// its leading two letters, so `en-US`, `ru`, `pt-BR` all resolve correctly.
pub fn system_prompt(lang: &str) -> &'static str {
    let code: String = lang.chars().take(2).flat_map(|c| c.to_lowercase()).collect();
    match code.as_str() {
        "ru" => RU_SYSTEM_PROMPT,
        "es" => ES_SYSTEM_PROMPT,
        "fr" => FR_SYSTEM_PROMPT,
        "de" => DE_SYSTEM_PROMPT,
        _ => EN_SYSTEM_PROMPT,
    }
}

const EN_SYSTEM_PROMPT: &str = "\
You are helping the author of this book think about their own work. You have \
been given relevant passages from the book, retrieved by semantic similarity \
to the author's question and marked with a citation id like [ch07-p042]. The \
passages are the book's prose in Typst markup — `= heading`, `*strong*`, \
`_emphasis_`, `#footnote[…]` — read through the markup to the prose beneath it.

Answer the author's question using the retrieved passages as primary \
evidence. Every claim about the book MUST cite at least one retrieved \
passage as a markdown link: [ch07-p042](#ch07-p042). Cite multiple passages \
when a claim spans them. Never state something about the book without citing.

When the retrieved passages don't address the question, say so plainly — \
\"The retrieved passages don't address that directly\" — then either ask the \
author to refine the question or offer general knowledge clearly marked as \
not from the book (\"Setting the book aside, in general…\").

Tone: helpful, grounded, specific. The author is consulting their own work, \
not asking you to invent it. Answer in the language of the author's question.";

const RU_SYSTEM_PROMPT: &str = "\
Вы помогаете автору этой книги размышлять над его собственным произведением. \
Вам даны релевантные фрагменты книги, отобранные по семантическому сходству с \
вопросом автора и помеченные идентификатором цитирования вида [ch07-p042]. \
Фрагменты — это проза книги в разметке Typst (`= заголовок`, `*полужирный*`, \
`_курсив_`, `#footnote[…]`); читайте сквозь разметку саму прозу под ней.

Отвечайте на вопрос автора, опираясь на отобранные фрагменты как на основное \
свидетельство. Каждое утверждение о книге ДОЛЖНО ссылаться хотя бы на один \
отобранный фрагмент в виде markdown-ссылки: [ch07-p042](#ch07-p042). \
Ссылайтесь на несколько фрагментов, когда утверждение охватывает их. Никогда \
не утверждайте ничего о книге без ссылки.

Если отобранные фрагменты не отвечают на вопрос, прямо скажите об этом — \
«Отобранные фрагменты не затрагивают это напрямую» — и либо попросите автора \
уточнить вопрос, либо предложите общие знания, чётко помеченные как взятые не \
из книги («Если отложить книгу в сторону, в общем случае…»).

Тон: полезный, обоснованный, конкретный. Автор обращается к собственному \
произведению, а не просит вас его сочинить. Отвечайте на языке вопроса автора.";

const ES_SYSTEM_PROMPT: &str = "\
Estás ayudando al autor de este libro a reflexionar sobre su propia obra. Se \
te han dado pasajes relevantes del libro, recuperados por similitud semántica \
con la pregunta del autor y marcados con un identificador de cita como \
[ch07-p042]. Los pasajes son la prosa del libro en marcado Typst (`= título`, \
`*fuerte*`, `_énfasis_`, `#footnote[…]`); lee a través del marcado la prosa \
que hay debajo.

Responde a la pregunta del autor usando los pasajes recuperados como \
evidencia principal. Toda afirmación sobre el libro DEBE citar al menos un \
pasaje recuperado como enlace markdown: [ch07-p042](#ch07-p042). Cita varios \
pasajes cuando una afirmación los abarque. Nunca afirmes algo sobre el libro \
sin citarlo.

Cuando los pasajes recuperados no aborden la pregunta, dilo con claridad — \
«Los pasajes recuperados no tratan eso directamente» — y luego pide al autor \
que precise la pregunta u ofrece conocimiento general claramente marcado como \
ajeno al libro («Dejando el libro a un lado, en general…»).

Tono: útil, fundamentado, concreto. El autor consulta su propia obra, no te \
pide que la inventes. Responde en el idioma de la pregunta del autor.";

const FR_SYSTEM_PROMPT: &str = "\
Vous aidez l'auteur de ce livre à réfléchir à sa propre œuvre. On vous a donné \
des passages pertinents du livre, retrouvés par similarité sémantique avec la \
question de l'auteur et marqués d'un identifiant de citation comme \
[ch07-p042]. Les passages sont la prose du livre en balisage Typst \
(`= titre`, `*gras*`, `_emphase_`, `#footnote[…]`) ; lisez au-delà du balisage \
la prose qui se trouve dessous.

Répondez à la question de l'auteur en vous appuyant sur les passages retrouvés \
comme preuve principale. Toute affirmation sur le livre DOIT citer au moins un \
passage retrouvé sous forme de lien markdown : [ch07-p042](#ch07-p042). Citez \
plusieurs passages lorsqu'une affirmation les traverse. N'affirmez jamais rien \
sur le livre sans citation.

Lorsque les passages retrouvés ne répondent pas à la question, dites-le \
clairement — « Les passages retrouvés n'abordent pas cela directement » — puis \
demandez à l'auteur de préciser la question ou proposez des connaissances \
générales clairement signalées comme extérieures au livre (« En laissant le \
livre de côté, de manière générale… »).

Ton : utile, fondé, précis. L'auteur consulte sa propre œuvre, il ne vous \
demande pas de l'inventer. Répondez dans la langue de la question de l'auteur.";

const DE_SYSTEM_PROMPT: &str = "\
Sie helfen dem Autor dieses Buches, über sein eigenes Werk nachzudenken. Sie \
haben relevante Passagen des Buches erhalten, die per semantischer Ähnlichkeit \
zur Frage des Autors abgerufen und mit einer Zitat-Kennung wie [ch07-p042] \
markiert sind. Die Passagen sind die Prosa des Buches in Typst-Auszeichnung \
(`= Überschrift`, `*stark*`, `_Betonung_`, `#footnote[…]`); lesen Sie durch \
die Auszeichnung hindurch die darunterliegende Prosa.

Beantworten Sie die Frage des Autors, indem Sie die abgerufenen Passagen als \
primäre Belege nutzen. Jede Aussage über das Buch MUSS mindestens eine \
abgerufene Passage als Markdown-Link zitieren: [ch07-p042](#ch07-p042). \
Zitieren Sie mehrere Passagen, wenn eine Aussage sie umspannt. Behaupten Sie \
niemals etwas über das Buch ohne Zitat.

Wenn die abgerufenen Passagen die Frage nicht behandeln, sagen Sie es \
unumwunden — „Die abgerufenen Passagen behandeln das nicht direkt“ — und \
bitten Sie den Autor dann, die Frage zu präzisieren, oder bieten Sie \
Allgemeinwissen an, das klar als nicht aus dem Buch stammend gekennzeichnet \
ist („Lassen wir das Buch beiseite, im Allgemeinen…“).

Ton: hilfreich, fundiert, konkret. Der Autor konsultiert sein eigenes Werk \
und bittet Sie nicht, es zu erfinden. Antworten Sie in der Sprache der Frage \
des Autors.";

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn passage(body: &str, is_hit: bool) -> RetrievedPassage {
        RetrievedPassage {
            id: Uuid::new_v4(),
            breadcrumb: "ch1/opening".into(),
            body: body.into(),
            score: 0.8,
            is_hit,
        }
    }

    #[test]
    fn estimate_tokens_is_chars_over_four() {
        assert_eq!(estimate_tokens(&"a".repeat(40)), 10);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn compose_labels_passages_with_id_and_hit_marker() {
        let ps = vec![passage("the road was long", true), passage("it rained", false)];
        let out = compose_context_prefix(&ps);
        assert!(out.contains("Retrieved passages"));
        assert!(out.contains(&format!("[{}]", ps[0].id)));
        assert!(out.contains("★"), "hit should be starred");
        assert!(out.contains("the road was long"));
        assert!(out.contains("it rained"));
    }

    #[test]
    fn empty_retrieval_composes_a_no_match_notice() {
        let out = compose_context_prefix(&[]);
        assert!(out.to_lowercase().contains("no passages"));
    }

    #[test]
    fn validate_flags_only_uncited_ids() {
        let mut valid = std::collections::HashSet::new();
        valid.insert("ch07-p042".to_string());
        let resp = "She returned [here](#ch07-p042) and again [later](#ch15-p103).";
        let out = validate_citations(resp, &valid);
        // The valid citation is untouched…
        assert!(out.contains("[here](#ch07-p042)"));
        assert!(!out.contains("ch07-p042]"), "valid id must not be flagged");
        // …the invented one is flagged inline.
        assert!(out.contains("[later](#ch15-p103) [citation could not be validated: ch15-p103]"));
    }

    #[test]
    fn validate_no_citations_is_unchanged() {
        let valid = std::collections::HashSet::new();
        assert_eq!(validate_citations("plain text, no links", &valid), "plain text, no links");
    }

    #[test]
    fn validate_unterminated_link_does_not_panic() {
        let valid = std::collections::HashSet::new();
        let out = validate_citations("oops [x](#unterminated", &valid);
        assert!(out.contains("#unterminated"));
    }

    #[test]
    fn system_prompt_localises_on_two_letter_code() {
        // Each baseline language gets its own contract…
        assert_ne!(system_prompt("ru"), EN_SYSTEM_PROMPT);
        assert_ne!(system_prompt("es"), EN_SYSTEM_PROMPT);
        assert_ne!(system_prompt("fr"), EN_SYSTEM_PROMPT);
        assert_ne!(system_prompt("de"), EN_SYSTEM_PROMPT);
        // …region/case suffixes resolve to the same variant…
        assert_eq!(system_prompt("ru-RU"), system_prompt("ru"));
        assert_eq!(system_prompt("DE"), DE_SYSTEM_PROMPT);
        // …and anything outside the baseline falls back to English.
        assert_eq!(system_prompt("ja"), EN_SYSTEM_PROMPT);
        assert_eq!(system_prompt(""), EN_SYSTEM_PROMPT);
    }

    #[test]
    fn cited_ids_collects_every_passage_id() {
        let ps = vec![passage("a", true), passage("b", false)];
        let ids = cited_ids(&ps);
        assert!(ids.contains(&ps[0].id.to_string()));
        assert!(ids.contains(&ps[1].id.to_string()));
    }
}
