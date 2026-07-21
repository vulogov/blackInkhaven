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

/// `wordnet import <lang> <path>` — build a language's index from a local
/// WN-LMF file (the escape hatch for data that can't be openly redistributed,
/// e.g. Russian / RuWordNet: obtain it under its licence, then import here).
pub fn import(lang: &str, path: &std::path::Path) -> Result<()> {
    let wn = fetch::import_file(path).map_err(Error::Config)?;
    let out = wordnet::index_path(lang)
        .ok_or_else(|| Error::Config("could not resolve the user data directory".into()))?;
    wn.save(&out).map_err(Error::Store)?;
    eprintln!(
        "imported {lang} wordnet · {} lemmas · {} synsets → {}",
        wn.lemma_count(),
        wn.synset_count(),
        out.display()
    );
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
    // For a non-English language, load English as a pivot (if installed) so
    // taxonomy relations expand through the interlingual index.
    let pivot = if lang.eq_ignore_ascii_case("en") {
        None
    } else {
        match wordnet::index_path("en").filter(|p| p.exists()) {
            Some(p) => match WordNet::load(&p) {
                Ok(wn) => Some(wn),
                // Don't hide a stale/incompatible pivot behind missing hypernyms.
                Err(e) => {
                    eprintln!("note: English pivot unavailable ({e}); hypernyms/hyponyms won't expand");
                    None
                }
            },
            None => None,
        }
    };
    let result = match pivot.as_ref() {
        Some(p) => wn.lookup_with_pivot(word, Some(p)),
        None => wn.lookup(word),
    };
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
    let installed = |lang: &str| -> &'static str {
        if wordnet::index_path(lang).map(|p| p.exists()).unwrap_or(false) {
            "installed"
        } else {
            "—"
        }
    };
    println!("WordNet sources (fetch with `inkhaven wordnet fetch <lang>`):\n");
    for src in fetch::SOURCES {
        println!("  {:<3} {:<11}  {:<34}  {}", src.lang, installed(src.lang), src.name, src.license);
    }
    // Russian is not openly redistributable, so it is imported, not fetched.
    println!(
        "  {:<3} {:<11}  {:<34}  {}",
        "ru", installed("ru"), "RuWordNet — import your own", "research licence"
    );
    println!(
        "\n  Russian has no open distribution — obtain a WN-LMF file under its licence and run\n  \
         `inkhaven wordnet import ru <file.xml>`. A non-English lookup expands its\n  \
         hypernyms/hyponyms through English, so `fetch en` too.",
    );
    if let Some(dir) = wordnet::data_dir() {
        println!("\ndata directory: {}", dir.display());
    }
    Ok(())
}
