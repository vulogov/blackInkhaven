//! GRAPHMIND GM-P5 — the graph-query **tool-loop** ("chat with your graph", the
//! *traversing* form). Where GM-P4's Graph scope does a single retrieval and
//! folds in one hop of relations, `graph ask` lets the model *walk* the graph:
//! it searches for seed nodes, then issues read-only graph queries (neighbours,
//! contradictions, loci, paths) turn by turn until it can answer — grounding
//! the answer in what it actually observed.
//!
//! There is no native function-calling in the LLM layer, so the loop is
//! text-driven exactly like [`crate::research::agentic`]: each turn the model
//! emits ONE JSON action, Rust executes it against the graph and feeds the
//! observation back. This module is the pure orchestration — the LLM is a
//! closure and the graph is the [`GraphOracle`] trait, so the whole loop is
//! unit-testable with a scripted model and a fake graph.

use std::collections::HashMap;

use uuid::Uuid;

/// One read-only action the model can take each turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Semantic search for seed nodes (returns handles `n1`, `n2`, …).
    Search(String),
    /// A node's one-hop neighbourhood.
    Neighbors(String),
    /// The stance clashes touching a node.
    Contradicting(String),
    /// The primary-source loci a node cites.
    Loci(String),
    /// A bounded citation/link path between two nodes.
    Paths(String, String),
    /// Finish: the grounded answer.
    Answer(String),
}

/// The read-only graph surface the loop drives. Implemented over the real
/// `Store` in the CLI; faked in tests. Every method returns display text (the
/// observation fed back to the model), except `search`, which also yields the
/// node ids so the loop can mint handles.
pub trait GraphOracle {
    /// Semantic node search → `(id, label)`, best first, at most `limit`.
    fn search(&self, query: &str, limit: usize) -> Vec<(Uuid, String)>;
    /// A node's one-hop neighbourhood, rendered.
    fn neighbors(&self, node: Uuid) -> String;
    /// The stance clashes (contradicts / in_tension) touching a node.
    fn contradicting(&self, node: Uuid) -> String;
    /// The primary-source loci a node cites.
    fn loci(&self, node: Uuid) -> String;
    /// A bounded path between two nodes.
    fn paths(&self, from: Uuid, to: Uuid) -> String;
    /// A human label for a node id.
    fn label(&self, node: Uuid) -> String;
}

/// The result of a `graph ask` run.
#[derive(Debug, Clone)]
pub struct AskOutcome {
    /// The model's final grounded answer.
    pub answer: String,
    /// A human transcript of the exploration (one entry per action taken).
    pub steps: Vec<String>,
    /// How many LLM turns the loop used.
    pub llm_calls: usize,
    /// True when the loop ran out of steps before the model chose to answer
    /// (the answer is then the forced final synthesis).
    pub forced: bool,
}

/// Extract the first balanced `{…}` JSON object from a model reply, tolerating
/// code fences and surrounding prose. Brace-counting skips braces inside strings.
fn extract_json_object(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let start = raw.find('{')?;
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    for i in start..bytes.len() {
        let c = bytes[i] as char;
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(raw[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse the model's one-action-per-turn reply into an [`Action`]. The first
/// recognised key wins; `answer` takes precedence so a model that both explores
/// and concludes still terminates.
pub fn parse_action(raw: &str) -> Result<Action, String> {
    let json = extract_json_object(raw).ok_or_else(|| "no JSON object in reply".to_string())?;
    let v: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("reply was not valid JSON: {e}"))?;
    let obj = v.as_object().ok_or_else(|| "expected a JSON object".to_string())?;

    if let Some(a) = obj.get("answer").and_then(|x| x.as_str()) {
        return Ok(Action::Answer(a.trim().to_string()));
    }
    for (key, make) in [
        ("search", Action::Search as fn(String) -> Action),
        ("neighbors", Action::Neighbors),
        ("contradicting", Action::Contradicting),
        ("loci", Action::Loci),
    ] {
        if let Some(s) = obj.get(key).and_then(|x| x.as_str()) {
            let s = s.trim();
            if s.is_empty() {
                return Err(format!("`{key}` needs a non-empty value"));
            }
            return Ok(make(s.to_string()));
        }
    }
    if let Some(p) = obj.get("paths").and_then(|x| x.as_array()) {
        let handles: Vec<String> = p
            .iter()
            .filter_map(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if handles.len() == 2 {
            return Ok(Action::Paths(handles[0].clone(), handles[1].clone()));
        }
        return Err("`paths` needs exactly two node handles".to_string());
    }
    Err("no known action — use search / neighbors / contradicting / loci / paths / answer".into())
}

/// The handle registry: readable `n1`/`n2` handles ↔ node ids, so the model
/// never has to echo a UUID.
struct Handles {
    to_id: HashMap<String, Uuid>,
    of_id: HashMap<Uuid, String>,
    next: usize,
}

impl Handles {
    fn new() -> Self {
        Handles { to_id: HashMap::new(), of_id: HashMap::new(), next: 1 }
    }
    /// Register a node, reusing its handle if already seen. Returns the handle.
    fn register(&mut self, id: Uuid) -> String {
        if let Some(h) = self.of_id.get(&id) {
            return h.clone();
        }
        let h = format!("n{}", self.next);
        self.next += 1;
        self.to_id.insert(h.clone(), id);
        self.of_id.insert(id, h.clone());
        h
    }
    /// Resolve a model-supplied handle: an `n#` handle, or a raw UUID.
    fn resolve(&self, handle: &str) -> Option<Uuid> {
        self.to_id
            .get(handle)
            .copied()
            .or_else(|| Uuid::parse_str(handle).ok())
    }
}

/// What to do after feeding an exploration turn's reply back to the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskStep {
    /// Keep exploring — issue another turn with [`AskSession::next_prompt`].
    Continue,
    /// The model gave a final answer directly (its `{"answer":…}`).
    Answer(String),
    /// The exploration budget is spent — the driver must issue ONE terminal
    /// turn with [`AskSession::synthesize_prompt`] and finish via
    /// [`AskSession::on_synthesis`].
    Synthesize,
}

/// GM-P8 — the resumable core of the graph walk, extracted from the blocking
/// loop so a caller can drive it turn by turn: the CLI blocks on each turn (see
/// [`ask`]); the TUI advances it across render frames without blocking. Holds
/// the handle registry + observations + transcript; the LLM and the graph stay
/// outside (the driver calls the model between `next_prompt` and `on_reply`).
pub struct AskSession {
    question: String,
    handles: Handles,
    observations: Vec<String>,
    steps: Vec<String>,
    step: usize,
    max_steps: usize,
    search_width: usize,
}

impl AskSession {
    /// A fresh walk for `question`, bounded by `max_steps` exploration turns and
    /// `search_width` seed nodes per search (both floored at 1).
    pub fn new(question: impl Into<String>, max_steps: usize, search_width: usize) -> Self {
        AskSession {
            question: question.into(),
            handles: Handles::new(),
            observations: Vec::new(),
            steps: Vec::new(),
            step: 0,
            max_steps: max_steps.max(1),
            search_width: search_width.max(1),
        }
    }

    /// The transcript so far — one compact step line per action taken. The live
    /// progress the TUI renders as the walk unfolds.
    pub fn transcript(&self) -> &[String] {
        &self.steps
    }

    /// 1-based index of the exploration turn about to run, and the cap — for a
    /// `turn k/N` status line.
    pub fn turn(&self) -> (usize, usize) {
        ((self.step + 1).min(self.max_steps), self.max_steps)
    }

    fn exhausted(&self) -> bool {
        self.step >= self.max_steps
    }

    /// The user prompt for the next exploration turn.
    pub fn next_prompt(&self) -> String {
        let last = self.step + 1 >= self.max_steps;
        build_prompt(&self.question, &self.handles, &self.observations, last)
    }

    /// The user prompt for the terminal turn: a grounded PROSE answer (not a
    /// JSON action), so the TUI can stream it live and the CLI can print it.
    pub fn synthesize_prompt(&self) -> String {
        format!(
            "{}\n\nProvide your final answer to the question NOW, in prose — grounded only \
             in the observations above, citing the node labels you relied on. Do not reply \
             with a JSON action; write the answer itself.",
            build_prompt(&self.question, &self.handles, &self.observations, true)
        )
    }

    /// Feed back an exploration turn's reply: parse it, run any graph query
    /// against `oracle`, record the observation + transcript step, and decide
    /// what happens next. Never fails — a malformed reply or unknown handle is
    /// fed back as an observation so the model can recover.
    pub fn on_reply(&mut self, reply: &str, oracle: &dyn GraphOracle) -> AskStep {
        let action = match parse_action(reply) {
            Ok(a) => a,
            Err(e) => {
                self.observations.push(format!(
                    "(your last reply was rejected: {e} — reply with exactly one JSON action)"
                ));
                self.steps.push(format!("· malformed reply: {e}"));
                self.step += 1;
                return self.after_step();
            }
        };
        if let Action::Answer(text) = action {
            return AskStep::Answer(text);
        }
        self.execute(action, oracle);
        self.step += 1;
        self.after_step()
    }

    /// Turn the terminal turn's reply into the final answer. The synthesis
    /// prompt asks for prose, so this is usually the reply verbatim; a model
    /// that still wrapped it in `{"answer":…}` is unwrapped anyway.
    pub fn on_synthesis(&self, reply: &str) -> String {
        match parse_action(reply) {
            Ok(Action::Answer(a)) => a,
            _ => reply.trim().to_string(),
        }
    }

    fn after_step(&self) -> AskStep {
        if self.exhausted() {
            AskStep::Synthesize
        } else {
            AskStep::Continue
        }
    }

    /// Run one tool action against the oracle and record it. `Answer` is handled
    /// by the caller and never reaches here.
    fn execute(&mut self, action: Action, oracle: &dyn GraphOracle) {
        match action {
            Action::Answer(_) => {}
            Action::Search(q) => {
                let found = oracle.search(&q, self.search_width);
                let mut lines = vec![format!("search \"{q}\" →")];
                if found.is_empty() {
                    lines.push("  (no matching nodes)".to_string());
                } else {
                    for (id, label) in &found {
                        let h = self.handles.register(*id);
                        lines.push(format!("  {h}  {label}"));
                    }
                }
                self.observations.push(lines.join("\n"));
                self.steps.push(format!("· search \"{q}\" ({} node(s))", found.len()));
            }
            Action::Neighbors(h) => self.node_query(oracle, "neighbors", &h, |o, id| o.neighbors(id)),
            Action::Contradicting(h) => {
                self.node_query(oracle, "contradicting", &h, |o, id| o.contradicting(id))
            }
            Action::Loci(h) => self.node_query(oracle, "loci", &h, |o, id| o.loci(id)),
            Action::Paths(a, b) => match (self.handles.resolve(&a), self.handles.resolve(&b)) {
                (Some(x), Some(y)) => {
                    let out = oracle.paths(x, y);
                    self.observations.push(format!("paths {a} → {b}:\n{out}"));
                    self.steps.push(format!("· paths {a} → {b}"));
                }
                _ => {
                    self.observations.push(format!(
                        "(unknown handle in paths {a}/{b} — search first, or use a listed handle)"
                    ));
                    self.steps.push(format!("· paths {a}/{b}: unknown handle"));
                }
            },
        }
    }

    fn node_query(
        &mut self,
        oracle: &dyn GraphOracle,
        verb: &str,
        handle: &str,
        query: impl Fn(&dyn GraphOracle, Uuid) -> String,
    ) {
        match self.handles.resolve(handle) {
            Some(id) => {
                let out = query(oracle, id);
                self.observations
                    .push(format!("{verb} {handle} ({}):\n{out}", oracle.label(id)));
                self.steps.push(format!("· {verb} {handle}"));
            }
            None => {
                self.observations.push(format!(
                    "(unknown handle `{handle}` — search first, or use a listed handle)"
                ));
                self.steps.push(format!("· {verb} {handle}: unknown handle"));
            }
        }
    }
}

/// Run the graph-query tool-loop to completion (the blocking driver, used by the
/// `graph ask` CLI). `llm(user_prompt) -> reply` bakes in the system prompt (the
/// tool contract); `oracle` executes the graph queries. A thin loop over
/// [`AskSession`]; the TUI drives the same session across frames instead.
pub fn ask(
    oracle: &dyn GraphOracle,
    mut llm: impl FnMut(&str) -> Result<String, String>,
    question: &str,
    max_steps: usize,
    search_limit: usize,
) -> Result<AskOutcome, String> {
    let mut session = AskSession::new(question, max_steps, search_limit);
    let mut llm_calls = 0usize;
    loop {
        let reply = llm(&session.next_prompt())?;
        llm_calls += 1;
        match session.on_reply(&reply, oracle) {
            AskStep::Answer(answer) => {
                return Ok(AskOutcome { answer, steps: session.steps, llm_calls, forced: false });
            }
            AskStep::Continue => {}
            AskStep::Synthesize => break,
        }
    }
    // Exploration budget spent — one terminal synthesis so the run always yields
    // something grounded rather than nothing.
    let reply = llm(&session.synthesize_prompt())?;
    llm_calls += 1;
    let answer = session.on_synthesis(&reply);
    Ok(AskOutcome { answer, steps: session.steps, llm_calls, forced: true })
}

/// Build the per-turn user prompt: the question, the known-node table, the
/// observations gathered so far, and the turn's instruction.
fn build_prompt(question: &str, handles: &Handles, observations: &[String], last: bool) -> String {
    let mut out = format!("Question: {question}\n\n");

    if handles.of_id.is_empty() {
        out.push_str("Known nodes: (none yet — start with a search)\n\n");
    } else {
        out.push_str("Known nodes:\n");
        // Stable order by handle number.
        let mut rows: Vec<(&String, &Uuid)> = handles.to_id.iter().collect();
        rows.sort_by_key(|(h, _)| h.trim_start_matches('n').parse::<usize>().unwrap_or(0));
        for (h, _) in rows {
            // Labels aren't stored here; the observations already carry them.
            out.push_str(&format!("  {h}\n"));
        }
        out.push('\n');
    }

    if observations.is_empty() {
        out.push_str("Observations: (none yet)\n\n");
    } else {
        out.push_str("Observations so far:\n");
        for (i, o) in observations.iter().enumerate() {
            out.push_str(&format!("[{}] {}\n", i + 1, o));
        }
        out.push('\n');
    }

    if last {
        out.push_str(
            "This is your LAST exploration turn. Prefer to answer now with \
             {\"answer\":\"…\"}, grounding it in the observations.",
        );
    } else {
        out.push_str("Take ONE action now (one JSON object).");
    }
    out
}

/// The `graph ask` system prompt (the tool contract). Multilingual baseline
/// (EN/RU/ES/FR/DE); anything else falls back to English. Matched on the
/// leading two letters of `lang`.
pub fn system_prompt(lang: &str) -> &'static str {
    let code: String = lang.chars().take(2).flat_map(|c| c.to_lowercase()).collect();
    match code.as_str() {
        "ru" => RU,
        "es" => ES,
        "fr" => FR,
        "de" => DE,
        _ => EN,
    }
}

const EN: &str = "\
You are exploring a read-only KNOWLEDGE GRAPH to answer the author's question \
about their book. You cannot see the whole graph at once — you must WALK it, one \
query per turn, and ground your final answer in what you actually observe.

Each turn, reply with EXACTLY ONE JSON object and nothing else:
  {\"search\": \"terms\"}            — find seed nodes (returns handles n1, n2, …)
  {\"neighbors\": \"n1\"}            — a node's one-hop relations
  {\"contradicting\": \"n1\"}        — the stance clashes touching a node
  {\"loci\": \"n1\"}                 — the primary-source loci a node cites
  {\"paths\": [\"n1\", \"n2\"]}      — a citation/link path between two nodes
  {\"answer\": \"…\"}                — finish, with your grounded answer

Always START with a search to get node handles; you can only query nodes by a \
handle a prior search returned. Explore only as far as the question needs, then \
answer. In the answer, cite the node labels you relied on, and be honest about \
the graph's limits: if the relations you found don't record what was asked, say \
so plainly rather than inventing a connection — the graph is only as complete as \
`graph rebuild` / confront / `graph link` have made it. Answer in the language of \
the question.";

const RU: &str = "\
Вы исследуете граф знаний (только для чтения), чтобы ответить на вопрос автора о \
его книге. Вы не видите весь граф сразу — его нужно ОБХОДИТЬ, по одному запросу \
за ход, и обосновывать итоговый ответ тем, что вы действительно наблюдаете.

Каждый ход отвечайте РОВНО одним объектом JSON и ничем больше:
  {\"search\": \"термины\"}          — найти узлы (возвращает метки n1, n2, …)
  {\"neighbors\": \"n1\"}            — связи узла на один шаг
  {\"contradicting\": \"n1\"}        — конфликты позиций, касающиеся узла
  {\"loci\": \"n1\"}                 — первоисточники (loci), которые цитирует узел
  {\"paths\": [\"n1\", \"n2\"]}      — путь цитирования/связи между двумя узлами
  {\"answer\": \"…\"}                — завершить обоснованным ответом

Всегда НАЧИНАЙТЕ с поиска, чтобы получить метки узлов; запрашивать узел можно \
только по метке, которую вернул предыдущий поиск. Исследуйте лишь настолько, \
насколько нужно для вопроса, затем отвечайте. В ответе ссылайтесь на метки \
узлов, на которые опирались, и честно говорите о пределах графа: если найденные \
связи не фиксируют спрошенное, прямо скажите об этом, а не выдумывайте связь — \
граф полон лишь настолько, насколько его наполнили `graph rebuild` / confront / \
`graph link`. Отвечайте на языке вопроса.";

const ES: &str = "\
Estás explorando un GRAFO DE CONOCIMIENTO de solo lectura para responder la \
pregunta del autor sobre su libro. No puedes ver todo el grafo a la vez: debes \
RECORRERLO, una consulta por turno, y fundamentar tu respuesta final en lo que \
realmente observas.

Cada turno, responde con EXACTAMENTE un objeto JSON y nada más:
  {\"search\": \"términos\"}         — busca nodos semilla (devuelve n1, n2, …)
  {\"neighbors\": \"n1\"}            — las relaciones de un nodo a un salto
  {\"contradicting\": \"n1\"}        — los choques de postura que tocan un nodo
  {\"loci\": \"n1\"}                 — los loci de fuente primaria que cita un nodo
  {\"paths\": [\"n1\", \"n2\"]}      — un camino de cita/enlace entre dos nodos
  {\"answer\": \"…\"}                — termina, con tu respuesta fundamentada

EMPIEZA siempre con una búsqueda para obtener identificadores; solo puedes \
consultar un nodo por un identificador que una búsqueda previa devolvió. Explora \
solo lo que la pregunta necesita y luego responde. En la respuesta, cita las \
etiquetas de los nodos en que te apoyaste y sé honesto sobre los límites del \
grafo: si las relaciones halladas no registran lo preguntado, dilo con claridad \
en vez de inventar una conexión — el grafo solo está tan completo como lo hayan \
poblado `graph rebuild` / confront / `graph link`. Responde en el idioma de la \
pregunta.";

const FR: &str = "\
Vous explorez un GRAPHE DE CONNAISSANCES en lecture seule pour répondre à la \
question de l'auteur sur son livre. Vous ne voyez pas tout le graphe d'un coup — \
vous devez le PARCOURIR, une requête par tour, et fonder votre réponse finale \
sur ce que vous observez réellement.

À chaque tour, répondez par EXACTEMENT un objet JSON et rien d'autre :
  {\"search\": \"termes\"}           — trouver des nœuds (renvoie n1, n2, …)
  {\"neighbors\": \"n1\"}            — les relations d'un nœud à un saut
  {\"contradicting\": \"n1\"}        — les conflits de position touchant un nœud
  {\"loci\": \"n1\"}                 — les loci de source primaire que cite un nœud
  {\"paths\": [\"n1\", \"n2\"]}      — un chemin de citation/lien entre deux nœuds
  {\"answer\": \"…\"}                — terminer, avec votre réponse fondée

COMMENCEZ toujours par une recherche pour obtenir des identifiants ; vous ne \
pouvez interroger un nœud que par un identifiant renvoyé par une recherche \
précédente. N'explorez que ce que la question exige, puis répondez. Dans la \
réponse, citez les étiquettes des nœuds utilisés et soyez honnête sur les \
limites du graphe : si les relations trouvées n'enregistrent pas ce qui est \
demandé, dites-le clairement au lieu d'inventer un lien — le graphe n'est \
complet que dans la mesure où `graph rebuild` / confront / `graph link` l'ont \
peuplé. Répondez dans la langue de la question.";

const DE: &str = "\
Sie erkunden einen schreibgeschützten WISSENSGRAPHEN, um die Frage des Autors zu \
seinem Buch zu beantworten. Sie sehen den Graphen nicht auf einmal — Sie müssen \
ihn DURCHLAUFEN, eine Abfrage pro Zug, und Ihre endgültige Antwort auf dem \
gründen, was Sie tatsächlich beobachten.

Antworten Sie pro Zug mit GENAU einem JSON-Objekt und sonst nichts:
  {\"search\": \"Begriffe\"}         — Startknoten finden (liefert n1, n2, …)
  {\"neighbors\": \"n1\"}            — die Ein-Schritt-Beziehungen eines Knotens
  {\"contradicting\": \"n1\"}        — die Positions-Konflikte an einem Knoten
  {\"loci\": \"n1\"}                 — die Primärquellen-Loci, die ein Knoten zitiert
  {\"paths\": [\"n1\", \"n2\"]}      — ein Zitat-/Verknüpfungspfad zwischen zwei Knoten
  {\"answer\": \"…\"}                — abschließen, mit Ihrer fundierten Antwort

BEGINNEN Sie stets mit einer Suche, um Knoten-Kennungen zu erhalten; Sie können \
einen Knoten nur über eine Kennung abfragen, die eine frühere Suche geliefert \
hat. Erkunden Sie nur so weit, wie es die Frage erfordert, und antworten Sie \
dann. Zitieren Sie in der Antwort die Knoten-Beschriftungen, auf die Sie sich \
gestützt haben, und seien Sie ehrlich über die Grenzen des Graphen: Wenn die \
gefundenen Beziehungen das Erfragte nicht festhalten, sagen Sie es unumwunden, \
statt eine Verbindung zu erfinden — der Graph ist nur so vollständig, wie \
`graph rebuild` / confront / `graph link` ihn gemacht haben. Antworten Sie in \
der Sprache der Frage.";

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn parse_extracts_actions_through_fences_and_prose() {
        assert_eq!(
            parse_action("```json\n{\"search\": \"the storm\"}\n```").unwrap(),
            Action::Search("the storm".into())
        );
        assert_eq!(
            parse_action("Sure — {\"neighbors\":\"n2\"} next.").unwrap(),
            Action::Neighbors("n2".into())
        );
        assert_eq!(
            parse_action("{\"paths\": [\"n1\", \"n3\"]}").unwrap(),
            Action::Paths("n1".into(), "n3".into())
        );
        // A brace inside a string must not end the object early.
        assert_eq!(
            parse_action("{\"answer\": \"use } wisely\"}").unwrap(),
            Action::Answer("use } wisely".into())
        );
    }

    #[test]
    fn parse_answer_wins_and_bad_input_errs() {
        assert_eq!(
            parse_action("{\"answer\": \"done\", \"search\": \"x\"}").unwrap(),
            Action::Answer("done".into())
        );
        assert!(parse_action("no json here").is_err());
        assert!(parse_action("{\"paths\": [\"n1\"]}").is_err());
        assert!(parse_action("{\"unknown\": \"x\"}").is_err());
    }

    /// A tiny fake graph: two nodes, a→contradicts→b.
    struct FakeGraph {
        a: Uuid,
        b: Uuid,
    }
    impl GraphOracle for FakeGraph {
        fn search(&self, _q: &str, _limit: usize) -> Vec<(Uuid, String)> {
            vec![(self.a, "003. Quiet hour".into()), (self.b, "007. The lantern".into())]
        }
        fn neighbors(&self, node: Uuid) -> String {
            if node == self.a {
                "◆ 003. Quiet hour\n├─ contradicts (1)\n│    ⇄ 007. The lantern".into()
            } else {
                "◆ 007. The lantern\n(no edges)".into()
            }
        }
        fn contradicting(&self, _node: Uuid) -> String {
            "⇄ 007. The lantern — lit at dusk, opposes §3".into()
        }
        fn loci(&self, _node: Uuid) -> String {
            "(no loci)".into()
        }
        fn paths(&self, _from: Uuid, _to: Uuid) -> String {
            "(no path within 8 hops)".into()
        }
        fn label(&self, node: Uuid) -> String {
            if node == self.a { "003. Quiet hour".into() } else { "007. The lantern".into() }
        }
    }

    #[test]
    fn loop_searches_then_queries_then_answers() {
        let g = FakeGraph { a: Uuid::now_v7(), b: Uuid::now_v7() };
        // Scripted model: search → neighbors n1 → answer.
        let script = RefCell::new(vec![
            "{\"search\": \"quiet hour\"}".to_string(),
            "{\"neighbors\": \"n1\"}".to_string(),
            "{\"answer\": \"003 contradicts 007 (the lantern).\"}".to_string(),
        ]);
        let turns = RefCell::new(Vec::<String>::new());
        let out = ask(
            &g,
            |prompt| {
                turns.borrow_mut().push(prompt.to_string());
                Ok(script.borrow_mut().remove(0))
            },
            "what contradicts the quiet hour?",
            8,
            5,
        )
        .unwrap();
        assert!(out.answer.contains("contradicts"));
        assert!(!out.forced);
        assert_eq!(out.llm_calls, 3);
        assert_eq!(out.steps.len(), 2); // search + neighbors (answer isn't a step)
        // The neighbors turn's prompt shows the handle table from the search.
        assert!(turns.borrow()[1].contains("n1"));
    }

    #[test]
    fn loop_forces_an_answer_when_steps_run_out() {
        let g = FakeGraph { a: Uuid::now_v7(), b: Uuid::now_v7() };
        // A model that never chooses to answer — always searches.
        let out = ask(
            &g,
            |_p| Ok("{\"search\": \"again\"}".to_string()),
            "unanswerable?",
            2,
            5,
        )
        .unwrap();
        assert!(out.forced, "should force a final synthesis");
        // 2 exploration turns + 1 forced turn.
        assert_eq!(out.llm_calls, 3);
    }

    #[test]
    fn unknown_handle_is_fed_back_not_fatal() {
        let g = FakeGraph { a: Uuid::now_v7(), b: Uuid::now_v7() };
        let script = RefCell::new(vec![
            // References n1 before any search has minted it.
            "{\"neighbors\": \"n1\"}".to_string(),
            "{\"answer\": \"couldn't explore\"}".to_string(),
        ]);
        let out = ask(
            &g,
            |_p| Ok(script.borrow_mut().remove(0)),
            "q?",
            8,
            5,
        )
        .unwrap();
        assert!(out.steps[0].contains("unknown handle"));
        assert_eq!(out.answer, "couldn't explore");
    }

    #[test]
    fn system_prompt_localises_and_falls_back() {
        assert_ne!(system_prompt("ru"), EN);
        assert_ne!(system_prompt("fr"), EN);
        assert_eq!(system_prompt("de-DE"), DE);
        assert_eq!(system_prompt("ja"), EN);
    }

    #[test]
    fn session_drives_turn_by_turn_and_reports_progress() {
        // The resumable core the TUI frame-driver rides: advance one turn at a
        // time, observe the transcript grow, then answer.
        let g = FakeGraph { a: Uuid::now_v7(), b: Uuid::now_v7() };
        let mut s = AskSession::new("what contradicts the quiet hour?", 8, 5);
        assert_eq!(s.turn(), (1, 8));
        // The first prompt has no handles yet.
        assert!(s.next_prompt().contains("start with a search"));

        assert_eq!(s.on_reply("{\"search\":\"quiet\"}", &g), AskStep::Continue);
        assert_eq!(s.turn(), (2, 8));
        assert_eq!(s.transcript().len(), 1);
        // The search minted handles the next prompt now lists.
        assert!(s.next_prompt().contains("n1"));

        assert_eq!(s.on_reply("{\"neighbors\":\"n1\"}", &g), AskStep::Continue);
        assert_eq!(s.transcript().len(), 2);

        // A direct answer ends the walk without a synthesis turn.
        match s.on_reply("{\"answer\":\"003 contradicts 007\"}", &g) {
            AskStep::Answer(a) => assert!(a.contains("contradicts")),
            other => panic!("expected Answer, got {other:?}"),
        }
    }

    #[test]
    fn session_signals_synthesize_when_budget_spent() {
        let g = FakeGraph { a: Uuid::now_v7(), b: Uuid::now_v7() };
        let mut s = AskSession::new("q?", 2, 5);
        assert_eq!(s.on_reply("{\"search\":\"x\"}", &g), AskStep::Continue);
        // Second (last) exploration turn → the driver must synthesise next.
        assert_eq!(s.on_reply("{\"search\":\"y\"}", &g), AskStep::Synthesize);
        // The synthesis prompt asks for prose, not a JSON action.
        assert!(s.synthesize_prompt().contains("in prose"));
        assert_eq!(s.on_synthesis("Because the lantern was lit."), "Because the lantern was lit.");
    }
}
