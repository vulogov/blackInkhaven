//! `inkhaven poetry` — the poetry toolset CLI (POEM-1 onward). This slice covers
//! `forms`: list the built-in forms, print a form's `poem:` block for a language,
//! or scaffold a custom form. Later phases add scan / syllabify / rhyme / metre /
//! status / translation subcommands.

use crate::error::{Error, Result};
use crate::poetry::form::{FormsLibrary, PoemForm};

/// `poetry forms [--form N] [--language L] [--new --name M]`.
pub fn forms(form: Option<&str>, language: Option<&str>, new: bool, name: Option<&str>) -> Result<()> {
    let lib = FormsLibrary::builtin();

    if new {
        // Scaffold a `form: custom` block to paste into a poem: sidecar or into
        // .inkhaven/custom-forms.hjson. (An interactive editor + auto-save is a
        // later refinement.)
        let scaffold = PoemForm {
            form: "custom".into(),
            title: name.unwrap_or("my-form").to_string(),
            metre: "iambic".into(),
            feet: 5,
            metre_tradition: "accentual_syllabic".into(),
            rhyme_scheme: "ABAB".into(),
            language: language.unwrap_or("en").to_string(),
            ..Default::default()
        };
        println!("// A custom-form scaffold — edit the fields, then paste this into a");
        println!("// `poem:` sidecar, or into .inkhaven/custom-forms.hjson to reuse it.");
        print!("{}", scaffold.to_poem_block());
        return Ok(());
    }

    if let Some(f) = form {
        let lang = language.unwrap_or("en");
        let pf = lib.localized(f, lang).ok_or_else(|| {
            Error::Config(format!(
                "unknown form `{f}` — run `inkhaven poetry forms` to list the {} available",
                lib.all().len()
            ))
        })?;
        print!("{}", pf.to_poem_block());
        return Ok(());
    }

    println!(
        "poetry forms — `--form <name> [--language en|ru|fr|de|es]` prints a poem: block:\n"
    );
    for pf in lib.all() {
        println!("  {:<22}  {}", pf.form, pf.desc);
    }
    Ok(())
}
