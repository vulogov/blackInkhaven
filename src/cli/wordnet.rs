//! `inkhaven wordnet` — a sense-based multilingual thesaurus for manuscript
//! prose. `fetch` downloads open WordNet data and builds a local index; `lookup`
//! shows a word's senses with synonyms/antonyms/hypernyms/hyponyms; `list` shows
//! the available sources and which are installed. Data lives in the user data
//! dir, shared across projects.

use crate::error::{Error, Result};
use crate::wordnet::{self, fetch, WordNet};

/// `wordnet fetch <lang>…` — download + build each language's index.
pub fn fetch_langs(languages: &[String]) -> Result<()> {
    if languages.is_empty() {
        return Err(Error::Config("usage: inkhaven wordnet fetch <lang>… (e.g. `en`)".into()));
    }
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| Error::Config(format!("could not start async runtime: {e}")))?;
    for lang in languages {
        eprint!("fetching {lang} wordnet … ");
        let wn = rt.block_on(fetch::fetch(lang)).map_err(Error::Config)?;
        let path = wordnet::index_path(lang)
            .ok_or_else(|| Error::Config("could not resolve the user data directory".into()))?;
        wn.save(&path).map_err(Error::Store)?;
        eprintln!(
            "{} lemmas · {} synsets → {}",
            wn.lemma_count(),
            wn.synset_count(),
            path.display()
        );
    }
    Ok(())
}

/// `wordnet lookup <word> [--lang]` — print the word's senses and relations.
pub fn lookup(word: &str, language: Option<&str>) -> Result<()> {
    let lang = language.unwrap_or("en");
    let path = wordnet::index_path(lang)
        .ok_or_else(|| Error::Config("could not resolve the user data directory".into()))?;
    if !path.exists() {
        return Err(Error::Config(format!(
            "no `{lang}` wordnet installed — run `inkhaven wordnet fetch {lang}` first"
        )));
    }
    let wn = WordNet::load(&path).map_err(Error::Store)?;
    let result = wn.lookup(word);
    if result.is_empty() {
        println!("`{word}` — not found in the {lang} wordnet");
        return Ok(());
    }

    println!("{word} · {lang}");
    for (i, s) in result.senses.iter().enumerate() {
        println!("\n  {}. {}", i + 1, s.pos);
        if let Some(def) = &s.definition {
            if !def.is_empty() {
                println!("     {def}");
            }
        }
        let row = |label: &str, items: &[String]| {
            if !items.is_empty() {
                println!("     {label:<10} {}", items.join(", "));
            }
        };
        row("synonyms", &s.synonyms);
        row("antonyms", &s.antonyms);
        row("hypernyms", &s.hypernyms);
        row("hyponyms", &s.hyponyms);
    }
    Ok(())
}

/// `wordnet list` — the available sources and which are installed.
pub fn list() -> Result<()> {
    println!("WordNet sources (fetch with `inkhaven wordnet fetch <lang>`):\n");
    for src in fetch::SOURCES {
        let installed = wordnet::index_path(src.lang)
            .map(|p| p.exists())
            .unwrap_or(false);
        let status = if installed { "installed" } else { "—" };
        let ready = if src.format == fetch::Format::XmlGz { "" } else { "  (arrives in a later release)" };
        println!("  {:<3} {:<12}  {:<34}  {}{}", src.lang, status, src.name, src.license, ready);
    }
    if let Some(dir) = wordnet::data_dir() {
        println!("\ndata directory: {}", dir.display());
    }
    Ok(())
}
