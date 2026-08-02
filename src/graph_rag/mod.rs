//! GRAPHMIND GM-P4 — Chat with Your Graph (templated graph-RAG).
//!
//! The AI pane's **Graph scope** is the relational sibling of Book scope
//! ([`crate::book_rag`]). A prompt in Graph scope retrieves the semantically
//! relevant paragraphs — the same vecstore retrieval Book scope uses — and then,
//! for each, folds in *what the knowledge graph records about it*: the edges
//! touching that node (contradicts / sourced_from / links_to / cites / …). The
//! LLM answers grounded not just in the prose but in how the pieces connect —
//! "which of my claims contradict each other", "what grounds this scene", "how
//! is this fact sourced" — questions the flat manuscript can't answer.
//!
//! This module holds the pure pieces (the passage-plus-relations type, context
//! composition, the grounding system prompt). The retrieval + edge gathering
//! live in the TUI layer (`graph_rag_impl.rs`), which has the store, the
//! hierarchy, and the endpoint labels. The **citation contract is reused
//! verbatim** from Book scope: the citable tokens are the retrieved passages'
//! location paths, validated by [`crate::book_rag::validate_citations`].

pub mod ask;

use std::collections::HashSet;

use crate::book_rag::RetrievedPassage;

/// A retrieved passage together with the graph relations touching its node —
/// pre-rendered by the TUI layer (which owns the endpoint labels) as readable
/// lines like `→ contradicts fact "The lantern was lit" — opposes §3`.
#[derive(Debug, Clone)]
pub struct GraphPassage {
    /// The retrieved passage (prose + location breadcrumb + score). Its
    /// breadcrumb is the citation token, exactly as in Book scope.
    pub passage: RetrievedPassage,
    /// The one-hop graph relations touching this passage's node, already
    /// formatted for the prompt. Empty when the node has no edges.
    pub relations: Vec<String>,
}

/// The citable tokens the retrieval makes available — the passages' location
/// paths, identical to Book scope. Reuses [`crate::book_rag::cited_ids`] so the
/// finaliser's [`crate::book_rag::validate_citations`] flags any invented ones.
pub fn cited_ids(passages: &[GraphPassage]) -> HashSet<String> {
    crate::book_rag::cited_ids(
        &passages.iter().map(|g| g.passage.clone()).collect::<Vec<_>>(),
    )
}

/// Compose the retrieved passages **and their graph relations** into the context
/// block prepended to the user's prompt. Each passage carries its location path
/// (the citation token), its prose, then an indented `relations:` block — the
/// graph's answer to "how does this connect to everything else".
pub fn compose_graph_context(passages: &[GraphPassage]) -> String {
    if passages.is_empty() {
        return "── Graph context ──\n(No passages in this book matched the query \
                semantically, so there is no neighbourhood to walk. Populate the \
                graph with `graph rebuild` / `graph lexical`.)\n── end graph context ──"
            .to_string();
    }
    let mut out = String::from("── Graph context (grounding evidence) ──\n");
    for p in passages {
        let marker = if p.passage.is_hit { " ★" } else { "" };
        out.push_str(&format!(
            "\n[{crumb}]{marker}\n{body}\n",
            crumb = p.passage.breadcrumb,
            marker = marker,
            body = p.passage.body.trim(),
        ));
        if p.relations.is_empty() {
            out.push_str("  relations: (none recorded — run `graph rebuild`)\n");
        } else {
            out.push_str("  relations:\n");
            for r in &p.relations {
                out.push_str(&format!("    {r}\n"));
            }
        }
    }
    out.push_str("\n── end graph context ──");
    out
}

/// The Graph-scope system prompt: ground answers in the retrieved passages AND
/// the graph relations, cite passage location labels, and be honest when the
/// graph doesn't record the relationship asked about. Localised to the
/// multilingual baseline (EN/RU/ES/FR/DE); anything else falls back to English.
/// The language code is matched on its leading two letters.
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
You are helping the author of this book interrogate their work's KNOWLEDGE GRAPH \
— the typed relationships between its parts. You have been given passages \
retrieved by semantic similarity to the author's question, and under each, a \
`relations:` block listing the graph edges touching that passage: what it \
`contradicts`, is `sourced_from`, `links_to`, `cites`, and so on. Each passage \
is labelled with its location in square brackets, like [act-two/the-storm]. The \
passages are Typst markup — read through it to the prose.

Answer using BOTH the prose AND the relations. When the author asks how things \
connect — what contradicts what, what grounds a claim, what a scene links to — \
the `relations:` blocks are your primary evidence; read them carefully and \
report what the graph actually records. Every claim about the book MUST cite at \
least one retrieved passage by repeating its bracketed location label exactly — \
for example [act-two/the-storm]. Never invent a location label you weren't given.

Be precise about the graph's own limits. If the relations don't record the \
connection the author is asking about, say so plainly — \"The graph doesn't \
record a contradiction here\" — rather than inferring one from the prose alone; \
note that the graph is only as complete as what `graph rebuild` / confront / \
`graph link` have populated. Distinguish what the GRAPH asserts from what you'd \
infer yourself, and mark the latter clearly.

Tone: helpful, grounded, specific. Answer in the language of the author's question.";

const RU_SYSTEM_PROMPT: &str = "\
Вы помогаете автору этой книги исследовать ГРАФ ЗНАНИЙ его произведения — \
типизированные связи между его частями. Вам даны фрагменты, отобранные по \
семантическому сходству с вопросом автора, и под каждым — блок `relations:` со \
связями графа, касающимися этого фрагмента: чему он `contradicts` \
(противоречит), из чего `sourced_from` (взят), с чем `links_to` (связан), что \
`cites` (цитирует) и так далее. Каждый фрагмент помечен меткой расположения в \
квадратных скобках, например [act-two/the-storm]. Фрагменты — это разметка \
Typst; читайте сквозь неё саму прозу.

Отвечайте, опираясь И на прозу, И на связи. Когда автор спрашивает, как вещи \
связаны — что чему противоречит, что обосновывает утверждение, с чем связана \
сцена, — блоки `relations:` являются вашим основным свидетельством; читайте их \
внимательно и сообщайте то, что граф действительно фиксирует. Каждое \
утверждение о книге ДОЛЖНО ссылаться хотя бы на один отобранный фрагмент, \
дословно повторяя его метку расположения в скобках — например \
[act-two/the-storm]. Не выдумывайте метку, которая вам не была дана.

Будьте точны в отношении пределов самого графа. Если связи не фиксируют то \
соединение, о котором спрашивает автор, прямо скажите об этом — «Граф не \
фиксирует здесь противоречия» — а не выводите его из одной лишь прозы; \
отметьте, что граф полон лишь настолько, насколько его наполнили \
`graph rebuild` / confront / `graph link`. Отличайте то, что утверждает ГРАФ, \
от того, что вы вывели бы сами, и чётко помечайте последнее.

Тон: полезный, обоснованный, конкретный. Отвечайте на языке вопроса автора.";

const ES_SYSTEM_PROMPT: &str = "\
Estás ayudando al autor de este libro a interrogar el GRAFO DE CONOCIMIENTO de \
su obra — las relaciones tipadas entre sus partes. Se te han dado pasajes \
recuperados por similitud semántica con la pregunta del autor y, bajo cada uno, \
un bloque `relations:` con las aristas del grafo que tocan ese pasaje: qué \
`contradicts` (contradice), de qué está `sourced_from` (extraído), con qué \
`links_to` (enlaza), qué `cites` (cita), etc. Cada pasaje está etiquetado con \
su ubicación entre corchetes, como [act-two/the-storm]. Los pasajes son marcado \
Typst; lee a través de él la prosa.

Responde usando TANTO la prosa COMO las relaciones. Cuando el autor pregunte \
cómo se conectan las cosas — qué contradice a qué, qué fundamenta una \
afirmación, con qué enlaza una escena — los bloques `relations:` son tu \
evidencia principal; léelos con cuidado e informa lo que el grafo realmente \
registra. Toda afirmación sobre el libro DEBE citar al menos un pasaje \
recuperado repitiendo exactamente su etiqueta de ubicación — por ejemplo \
[act-two/the-storm]. Nunca inventes una etiqueta que no se te haya dado.

Sé preciso sobre los límites del propio grafo. Si las relaciones no registran \
la conexión que el autor pregunta, dilo con claridad — «El grafo no registra \
una contradicción aquí» — en lugar de inferirla solo de la prosa; señala que el \
grafo solo está tan completo como lo hayan poblado `graph rebuild` / confront / \
`graph link`. Distingue lo que el GRAFO afirma de lo que tú inferirías, y marca \
esto último claramente.

Tono: útil, fundamentado, concreto. Responde en el idioma de la pregunta del autor.";

const FR_SYSTEM_PROMPT: &str = "\
Vous aidez l'auteur de ce livre à interroger le GRAPHE DE CONNAISSANCES de son \
œuvre — les relations typées entre ses parties. On vous a donné des passages \
retrouvés par similarité sémantique avec la question de l'auteur et, sous \
chacun, un bloc `relations:` listant les arêtes du graphe qui touchent ce \
passage : ce qu'il `contradicts` (contredit), ce dont il est `sourced_from` \
(tiré), ce à quoi il `links_to` (est lié), ce qu'il `cites` (cite), etc. Chaque \
passage est étiqueté avec son emplacement entre crochets, comme \
[act-two/the-storm]. Les passages sont du balisage Typst ; lisez au-delà la prose.

Répondez en vous appuyant À LA FOIS sur la prose ET sur les relations. Quand \
l'auteur demande comment les choses se connectent — ce qui contredit quoi, ce \
qui fonde une affirmation, ce à quoi une scène est liée — les blocs \
`relations:` sont votre preuve principale ; lisez-les attentivement et \
rapportez ce que le graphe enregistre réellement. Toute affirmation sur le \
livre DOIT citer au moins un passage retrouvé en répétant exactement son \
étiquette d'emplacement — par exemple [act-two/the-storm]. N'inventez jamais \
une étiquette qui ne vous a pas été donnée.

Soyez précis sur les limites du graphe lui-même. Si les relations \
n'enregistrent pas la connexion demandée, dites-le clairement — « Le graphe \
n'enregistre pas de contradiction ici » — plutôt que de l'inférer de la seule \
prose ; notez que le graphe n'est complet que dans la mesure où \
`graph rebuild` / confront / `graph link` l'ont peuplé. Distinguez ce que le \
GRAPHE affirme de ce que vous inféreriez, et signalez clairement ce dernier.

Ton : utile, fondé, précis. Répondez dans la langue de la question de l'auteur.";

const DE_SYSTEM_PROMPT: &str = "\
Sie helfen dem Autor dieses Buches, den WISSENSGRAPHEN seines Werks zu \
befragen — die typisierten Beziehungen zwischen seinen Teilen. Sie haben \
Passagen erhalten, die per semantischer Ähnlichkeit zur Frage des Autors \
abgerufen wurden, und unter jeder einen `relations:`-Block mit den Graphkanten, \
die diese Passage berühren: was sie `contradicts` (widerspricht), woraus sie \
`sourced_from` (stammt), womit sie `links_to` (verknüpft ist), was sie `cites` \
(zitiert) usw. Jede Passage ist mit ihrem Fundort in eckigen Klammern \
beschriftet, etwa [act-two/the-storm]. Die Passagen sind Typst-Auszeichnung; \
lesen Sie durch sie hindurch die Prosa.

Antworten Sie unter Nutzung SOWOHL der Prosa ALS AUCH der Beziehungen. Wenn der \
Autor fragt, wie die Dinge zusammenhängen — was was widerspricht, was eine \
Aussage stützt, womit eine Szene verknüpft ist — sind die \
`relations:`-Blöcke Ihr primärer Beleg; lesen Sie sie sorgfältig und berichten \
Sie, was der Graph tatsächlich festhält. Jede Aussage über das Buch MUSS \
mindestens eine abgerufene Passage zitieren, indem Sie ihre Fundort-Beschriftung \
exakt wiederholen — zum Beispiel [act-two/the-storm]. Erfinden Sie nie eine \
Beschriftung, die Ihnen nicht gegeben wurde.

Seien Sie präzise über die Grenzen des Graphen selbst. Wenn die Beziehungen die \
erfragte Verbindung nicht festhalten, sagen Sie es unumwunden — „Der Graph hält \
hier keinen Widerspruch fest“ — statt sie allein aus der Prosa zu schließen; \
weisen Sie darauf hin, dass der Graph nur so vollständig ist, wie \
`graph rebuild` / confront / `graph link` ihn gefüllt haben. Unterscheiden Sie, \
was der GRAPH behauptet, von dem, was Sie selbst schließen würden, und \
kennzeichnen Sie Letzteres klar.

Ton: hilfreich, fundiert, konkret. Antworten Sie in der Sprache der Frage des Autors.";

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn gp(crumb: &str, body: &str, is_hit: bool, relations: &[&str]) -> GraphPassage {
        GraphPassage {
            passage: RetrievedPassage {
                id: Uuid::new_v4(),
                breadcrumb: crumb.into(),
                body: body.into(),
                score: 0.8,
                is_hit,
            },
            relations: relations.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn compose_folds_relations_under_each_passage() {
        let ps = vec![
            gp(
                "act-two/the-storm",
                "the lantern was dark",
                true,
                &["→ contradicts fact \"the lantern was lit\" — opposes §3"],
            ),
            gp("act-one/the-harbour", "it rained", false, &[]),
        ];
        let out = compose_graph_context(&ps);
        assert!(out.contains("Graph context"));
        // Citation token is the readable path, never the UUID.
        assert!(out.contains("[act-two/the-storm]"));
        assert!(!out.contains(&ps[0].passage.id.to_string()));
        assert!(out.contains("★"), "hit should be starred");
        // Relations are folded in under the passage…
        assert!(out.contains("relations:"));
        assert!(out.contains("contradicts fact"));
        // …and a relationless passage says so, rather than silently omitting.
        assert!(out.contains("(none recorded"));
    }

    #[test]
    fn empty_retrieval_explains_the_empty_neighbourhood() {
        let out = compose_graph_context(&[]);
        assert!(out.to_lowercase().contains("no passages"));
        assert!(out.contains("graph rebuild"));
    }

    #[test]
    fn cited_ids_are_the_passage_paths() {
        let ps = vec![
            gp("act-one/the-harbour", "a", true, &[]),
            gp("act-two/the-storm", "b", false, &["→ links_to x"]),
        ];
        let toks = cited_ids(&ps);
        assert!(toks.contains("act-one/the-harbour"));
        assert!(toks.contains("act-two/the-storm"));
    }

    #[test]
    fn system_prompt_localises_and_falls_back() {
        assert_ne!(system_prompt("ru"), EN_SYSTEM_PROMPT);
        assert_ne!(system_prompt("de"), EN_SYSTEM_PROMPT);
        assert_eq!(system_prompt("ru-RU"), system_prompt("ru"));
        assert_eq!(system_prompt("ja"), EN_SYSTEM_PROMPT);
        assert_eq!(system_prompt(""), EN_SYSTEM_PROMPT);
    }
}
