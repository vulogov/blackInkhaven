//! Build step that compiles the in-tree Typst tree-sitter grammar
//! (`src/grammar/parser.c` + `src/grammar/scanner.c`) into a static
//! archive linked into the final binary.
//!
//! Replaces the previous `vendor/tree-sitter-typst` path-dep — keeps
//! inkhaven a single, crates.io-publishable crate.
//!
//! Upstream source: github.com/uben0/tree-sitter-typst (MIT, © 2023
//! Gerbais-Nief Eddie). See `LICENSES/tree-sitter-typst-LICENSE`.

mod config_help_extract;

fn main() {
    let src_dir = std::path::Path::new("src/grammar");
    let parser_c = src_dir.join("parser.c");
    let scanner_c = src_dir.join("scanner.c");

    let mut build = cc::Build::new();
    build
        .include(src_dir)
        .file(&parser_c)
        .file(&scanner_c)
        // Tree-sitter's generated parser triggers a few benign
        // warnings on clang/gcc; mute them so a -Werror downstream
        // doesn't fail the build.
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    build.compile("tree_sitter_typst_grammar");

    println!("cargo:rerun-if-changed={}", parser_c.display());
    println!("cargo:rerun-if-changed={}", scanner_c.display());
    println!("cargo:rerun-if-changed=src/grammar/unicode.h");
    println!("cargo:rerun-if-changed=src/grammar/tree_sitter/alloc.h");
    println!("cargo:rerun-if-changed=src/grammar/tree_sitter/array.h");
    println!("cargo:rerun-if-changed=src/grammar/tree_sitter/parser.h");

    // 1.2.10+ — emit the config-TUI's in-process help
    // table from doc-comments on the Config struct
    // tree.  Generated into $OUT_DIR/config_help.rs
    // and `include!`-ed by `src/config_tui/help.rs`.
    config_help_extract::run();
}
