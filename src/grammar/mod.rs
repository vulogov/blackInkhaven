//! In-tree Typst grammar for tree-sitter.
//!
//! Replaces the previous `tree-sitter-typst` path-dependency. The C
//! sources (`parser.c`, `scanner.c`, `unicode.h`, and the internal
//! `tree_sitter/*.h` headers) are compiled into the binary by
//! `build.rs` and exposed here as a single Rust function.
//!
//! Upstream: `github.com/uben0/tree-sitter-typst` — MIT licensed
//! (© 2023 Gerbais-Nief Eddie). See `LICENSES/tree-sitter-typst-LICENSE`
//! for the unmodified license text.
//!
//! ## Refreshing the grammar
//!
//! To pull a newer parser from upstream, copy these files verbatim
//! from `github.com/uben0/tree-sitter-typst` into this directory:
//!
//! - `src/parser.c` → `src/grammar/parser.c`
//! - `src/scanner.c` → `src/grammar/scanner.c`
//! - `src/unicode.h` → `src/grammar/unicode.h`
//! - `src/tree_sitter/{alloc,array,parser}.h`
//!   → `src/grammar/tree_sitter/`
//!
//! Update `LICENSES/tree-sitter-typst-LICENSE` if the upstream
//! copyright year or holder changes. Rebuild + verify
//! `tui::highlight::TypstHighlighter::new()` still constructs
//! without error, then re-test syntax colouring in the TUI editor
//! pane.

use tree_sitter::Language;

unsafe extern "C" {
    fn tree_sitter_typst() -> Language;
}

/// Get the tree-sitter `Language` for Typst. Wraps the in-tree
/// grammar's C entry point.
pub fn language() -> Language {
    unsafe { tree_sitter_typst() }
}
