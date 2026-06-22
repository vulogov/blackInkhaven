//! LANG-3 P3 — export a language's translation system as a portable `.itm`
//! bundle (RFC §8.9, recast by Amendment A1).
//!
//! Under the retrieval architecture the "trained model" *is* the translation
//! memory, so the shippable artifact is the memory plus the lexicon: a
//! self-describing, browsable, re-importable language pack. The bundle is a
//! single `.itm` file (a zip), so an author can publish it alongside a book and
//! a reader can open it without Inkhaven:
//!
//! ```text
//! <lang>.itm
//!   manifest.hjson   — language, counts, format, provenance
//!   memory.tsv       — the confirmed (English → conlang) pairs (the memory)
//!   lexicon.tsv      — headword / part-of-speech / gloss
//!   README.md        — what it is and how to use it
//! ```
//!
//! Embeddings are intentionally left out (large and regenerable); they recompute
//! on first use after a re-import. Pure: returns the bundle bytes; the caller
//! writes them atomically.

use std::io::{self, Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Bundle metadata for the manifest.
pub struct BundleMeta {
    pub language: String,
    /// `YYYY-MM-DD` (the caller stamps it).
    pub exported: String,
    pub inkhaven_version: String,
    pub memory_pairs: usize,
    pub lexicon_entries: usize,
}

/// TSV-safe a field (tabs / newlines would corrupt the row).
fn tsv(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn map_zip(e: zip::result::ZipError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

/// Build the `.itm` bundle bytes for a language's translation system.
pub fn bundle(
    meta: &BundleMeta,
    memory: &[(String, String)],
    lexicon: &[(String, String, String)],
) -> io::Result<Vec<u8>> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));

    let manifest = format!(
        "{{\n  \
         format: \"inkhaven-translation-memory/v1\"\n  \
         language: {:?}\n  \
         exported: {:?}\n  \
         inkhaven_version: {:?}\n  \
         memory_pairs: {}\n  \
         lexicon_entries: {}\n\
         }}\n",
        meta.language, meta.exported, meta.inkhaven_version, meta.memory_pairs, meta.lexicon_entries,
    );
    zw.start_file("manifest.hjson", SimpleFileOptions::default()).map_err(map_zip)?;
    zw.write_all(manifest.as_bytes())?;

    zw.start_file("memory.tsv", SimpleFileOptions::default()).map_err(map_zip)?;
    zw.write_all(b"english\tconlang\n")?;
    for (en, con) in memory {
        writeln!(zw, "{}\t{}", tsv(en), tsv(con))?;
    }

    zw.start_file("lexicon.tsv", SimpleFileOptions::default()).map_err(map_zip)?;
    zw.write_all(b"headword\tpos\tgloss\n")?;
    for (w, pos, gloss) in lexicon {
        writeln!(zw, "{}\t{}\t{}", tsv(w), tsv(pos), tsv(gloss))?;
    }

    zw.start_file("README.md", SimpleFileOptions::default()).map_err(map_zip)?;
    zw.write_all(readme(meta).as_bytes())?;

    let cursor = zw.finish().map_err(map_zip)?;
    Ok(cursor.into_inner())
}

fn readme(meta: &BundleMeta) -> String {
    format!(
        "# {lang} — translation pack\n\
         \n\
         A portable translation system for the constructed language **{lang}**,\n\
         exported from Inkhaven {ver} on {date}.\n\
         \n\
         This is a *retrieval*-based translation system: the knowledge is a\n\
         **translation memory** of confirmed `English → {lang}` sentence pairs,\n\
         not a trained neural model. There is nothing to install and no GPU.\n\
         \n\
         ## Contents\n\
         \n\
         - `memory.tsv` — {pairs} confirmed translations (tab-separated\n\
         `english`, `{lang_l}`). Browse it as a phrasebook, or look a sentence up\n\
         directly.\n\
         - `lexicon.tsv` — {lex} dictionary entries (`headword`, `pos`, `gloss`).\n\
         - `manifest.hjson` — metadata.\n\
         \n\
         ## Using it with Inkhaven\n\
         \n\
         Re-seed another project's memory by replaying the pairs:\n\
         \n\
         ```sh\n\
         # for each row of memory.tsv:\n\
         inkhaven language remember {lang} --english \"<english>\" --conlang \"<{lang_l}>\"\n\
         ```\n\
         \n\
         The embeddings that power semantic recall are not shipped (they are\n\
         large and regenerate automatically the next time the pairs are used).\n",
        lang = meta.language,
        lang_l = meta.language.to_lowercase(),
        ver = meta.inkhaven_version,
        date = meta.exported,
        pairs = meta.memory_pairs,
        lex = meta.lexicon_entries,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn bundle_is_a_readable_zip_with_the_expected_entries() {
        let meta = BundleMeta {
            language: "Eldar".into(),
            exported: "2026-06-22".into(),
            inkhaven_version: "1.3.23".into(),
            memory_pairs: 1,
            lexicon_entries: 1,
        };
        let memory = vec![("the bird sees the stone".to_string(), "kira nami pata".to_string())];
        let lexicon = vec![("kira".to_string(), "noun".to_string(), "bird".to_string())];
        let bytes = bundle(&meta, &memory, &lexicon).unwrap();

        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().to_string()).collect();
        assert!(names.contains(&"manifest.hjson".to_string()));
        assert!(names.contains(&"memory.tsv".to_string()));
        assert!(names.contains(&"lexicon.tsv".to_string()));
        assert!(names.contains(&"README.md".to_string()));

        let mut mem = String::new();
        zip.by_name("memory.tsv").unwrap().read_to_string(&mut mem).unwrap();
        assert!(mem.contains("the bird sees the stone\tkira nami pata"));
    }
}
