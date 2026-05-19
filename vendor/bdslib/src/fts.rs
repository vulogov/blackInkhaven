use crate::common::error::{err_msg, Result};
use parking_lot::Mutex;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, OwnedValue, Schema, STORED, STRING, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};
use uuid::Uuid;

const WRITER_HEAP_BYTES: usize = 50_000_000;

struct FTSState {
    index: Index,
    writer: IndexWriter<TantivyDocument>,
    reader: IndexReader,
    id_field: Field,
    body_field: Field,
}

/// Thread-safe full-text search engine backed by Tantivy.
///
/// Every document is assigned a UUIDv7 at insertion time. All three
/// operations (`add_document`, `drop_document`, `search`) are immediately
/// consistent: a commit + reader reload is issued after every write.
///
/// The underlying Tantivy index (50 MB writer heap) is opened lazily on the
/// first FTS read or write, so shards opened for pure DuckDB or template
/// queries pay no Tantivy initialisation cost.
pub struct FTSEngine {
    path: String,
    state: Mutex<Option<FTSState>>,
}

impl FTSEngine {
    /// Create an FTS engine rooted at `path`.
    ///
    /// Pass `":memory:"` for a RAM-only index (lost on drop).
    /// Any other value is treated as a filesystem directory path.
    ///
    /// The Tantivy index is NOT opened here — it is initialised lazily on the
    /// first read or write operation.
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            state: Mutex::new(None),
        })
    }

    fn open_state(path: &str) -> Result<FTSState> {
        let mut builder = Schema::builder();
        let id_field = builder.add_text_field("id", STRING | STORED);
        let body_field = builder.add_text_field("body", TEXT | STORED);
        let schema = builder.build();

        let index = if path == ":memory:" {
            Index::create_in_ram(schema)
        } else {
            let dir = Path::new(path);
            std::fs::create_dir_all(dir)
                .map_err(|e| err_msg(format!("Cannot create index directory: {e}")))?;
            if dir.join("meta.json").exists() {
                Index::open_in_dir(dir)
                    .map_err(|e| err_msg(format!("Cannot open index at {path}: {e}")))?
            } else {
                Index::create_in_dir(dir, schema)
                    .map_err(|e| err_msg(format!("Cannot create index at {path}: {e}")))?
            }
        };

        let writer: IndexWriter<TantivyDocument> = index
            .writer(WRITER_HEAP_BYTES)
            .map_err(|e| err_msg(format!("Cannot create index writer: {e}")))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| err_msg(format!("Cannot create index reader: {e}")))?;

        Ok(FTSState { index, writer, reader, id_field, body_field })
    }

    fn ensure<'a>(guard: &'a mut Option<FTSState>, path: &str) -> Result<&'a mut FTSState> {
        if guard.is_none() {
            *guard = Some(Self::open_state(path)?);
        }
        Ok(guard.as_mut().unwrap())
    }

    /// Index `text` and return its assigned UUIDv7.
    pub fn add_document(&self, text: &str) -> Result<Uuid> {
        let id = Uuid::now_v7();
        let mut guard = self.state.lock();
        let s = Self::ensure(&mut guard, &self.path)?;

        let mut doc = TantivyDocument::default();
        doc.add_text(s.id_field, id.to_string());
        doc.add_text(s.body_field, text);
        s.writer
            .add_document(doc)
            .map_err(|e| err_msg(format!("Failed to stage document: {e}")))?;
        s.writer
            .commit()
            .map_err(|e| err_msg(format!("Failed to commit add: {e}")))?;
        s.reader
            .reload()
            .map_err(|e| err_msg(format!("Failed to reload reader after add: {e}")))?;
        Ok(id)
    }

    /// Remove the document with the given UUIDv7 from the index.
    ///
    /// Succeeds silently if the UUID does not exist.
    pub fn drop_document(&self, id: Uuid) -> Result<()> {
        let mut guard = self.state.lock();
        let s = Self::ensure(&mut guard, &self.path)?;

        let term = Term::from_field_text(s.id_field, &id.to_string());
        s.writer.delete_term(term);
        s.writer
            .commit()
            .map_err(|e| err_msg(format!("Failed to commit delete: {e}")))?;
        s.reader
            .reload()
            .map_err(|e| err_msg(format!("Failed to reload reader after delete: {e}")))?;
        Ok(())
    }

    /// Flush all pending changes to the on-disk index directory and reload the reader.
    ///
    /// No-op when the index has never been opened (no FTS writes have occurred).
    pub fn sync(&self) -> Result<()> {
        let mut guard = self.state.lock();
        let Some(s) = guard.as_mut() else { return Ok(()); };
        s.writer
            .commit()
            .map_err(|e| err_msg(format!("Sync commit failed: {e}")))?;
        s.reader
            .reload()
            .map_err(|e| err_msg(format!("Failed to reload reader after sync: {e}")))?;
        Ok(())
    }

    /// Index `text` under a caller-supplied `id`, replacing any existing entry for that id.
    ///
    /// Unlike [`add_document`], which generates a fresh UUIDv7, this method stores the
    /// document under the UUID you provide.
    ///
    /// [`add_document`]: FTSEngine::add_document
    pub fn add_document_with_id(&self, id: Uuid, text: &str) -> Result<()> {
        let mut guard = self.state.lock();
        let s = Self::ensure(&mut guard, &self.path)?;

        let term = Term::from_field_text(s.id_field, &id.to_string());
        let mut doc = TantivyDocument::default();
        doc.add_text(s.id_field, id.to_string());
        doc.add_text(s.body_field, text);
        s.writer.delete_term(term);
        s.writer
            .add_document(doc)
            .map_err(|e| err_msg(format!("Failed to stage document {id}: {e}")))?;
        s.writer
            .commit()
            .map_err(|e| err_msg(format!("Failed to commit add_document_with_id: {e}")))?;
        s.reader
            .reload()
            .map_err(|e| err_msg(format!("Failed to reload reader after add_document_with_id: {e}")))?;
        Ok(())
    }

    /// Index multiple `(id, text)` pairs in a single commit + reader reload.
    ///
    /// No-op when `docs` is empty.
    pub fn add_documents_batch(&self, docs: &[(Uuid, String)]) -> Result<()> {
        if docs.is_empty() {
            return Ok(());
        }
        let mut guard = self.state.lock();
        let s = Self::ensure(&mut guard, &self.path)?;

        for (id, text) in docs {
            let term = Term::from_field_text(s.id_field, &id.to_string());
            let mut doc = TantivyDocument::default();
            doc.add_text(s.id_field, id.to_string());
            doc.add_text(s.body_field, text);
            s.writer.delete_term(term);
            s.writer
                .add_document(doc)
                .map_err(|e| err_msg(format!("Failed to stage document {id}: {e}")))?;
        }
        s.writer
            .commit()
            .map_err(|e| err_msg(format!("Failed to commit batch add: {e}")))?;
        s.reader
            .reload()
            .map_err(|e| err_msg(format!("Failed to reload reader after batch add: {e}")))?;
        Ok(())
    }

    /// Search the index and return up to `limit` matching UUIDv7s, ranked by relevance.
    ///
    /// `query` uses Tantivy's query syntax (e.g. `"hello world"`, `hello AND world`).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Uuid>> {
        Ok(self
            .search_with_scores(query, limit)?
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    }

    /// Search the index and return up to `limit` `(UUID, BM25-score)` pairs,
    /// ordered by descending relevance score.
    pub fn search_with_scores(&self, query: &str, limit: usize) -> Result<Vec<(Uuid, f32)>> {
        // Acquire the lock only long enough to extract owned/Copy handles:
        //   `Index` is Arc-backed (cheap clone), `Field` is u32 (Copy),
        //   `Searcher` is Arc-backed (cheap clone). All usable after the lock releases.
        let (index, id_field, body_field, searcher) = {
            let mut guard = self.state.lock();
            let s = Self::ensure(&mut guard, &self.path)?;
            (s.index.clone(), s.id_field, s.body_field, s.reader.searcher())
        };

        let parser = QueryParser::for_index(&index, vec![body_field]);
        let parsed = parser
            .parse_query(query)
            .map_err(|e| err_msg(format!("Invalid query \"{query}\": {e}")))?;

        let hits = searcher
            .search(&parsed, &TopDocs::with_limit(limit))
            .map_err(|e| err_msg(format!("Search failed: {e}")))?;

        let mut results = Vec::with_capacity(hits.len());
        for (score, addr) in hits {
            let doc: TantivyDocument = searcher
                .doc(addr)
                .map_err(|e| err_msg(format!("Failed to retrieve document: {e}")))?;

            if let Some(raw) = doc.get_first(id_field) {
                if let OwnedValue::Str(id_str) = OwnedValue::from(raw) {
                    if let Ok(uuid) = Uuid::parse_str(&id_str) {
                        results.push((uuid, score));
                    }
                }
            }
        }

        Ok(results)
    }
}
