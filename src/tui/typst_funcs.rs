//! Baked-in list of Typst built-in functions for the Ctrl+B F picker.
//!
//! Curated subset (~80 entries) — covers the most common markup,
//! layout, math, and structural calls. The picker inserts
//! `#<name>(|)` at the cursor (markup-mode default for Phase 1) so the
//! signature column is informational; only `name` is functionally
//! load-bearing.
//!
//! Maintaining: when the Typst stdlib gains commonly-used functions,
//! add a row here. Order doesn't matter — the picker sorts
//! alphabetically.

#[derive(Debug, Clone, Copy)]
pub struct TypstFn {
    pub name: &'static str,
    pub signature: &'static str,
    pub description: &'static str,
}

const fn f(name: &'static str, signature: &'static str, description: &'static str) -> TypstFn {
    TypstFn { name, signature, description }
}

pub fn all() -> Vec<TypstFn> {
    let mut out: Vec<TypstFn> = ENTRIES.to_vec();
    out.sort_by_key(|e| e.name);
    out
}

const ENTRIES: &[TypstFn] = &[
    // ── Markup ─────────────────────────────────────────────────
    f("text", "text(font: \"Garamond\", size: 11pt)[…]", "Set text properties for the body."),
    f("emph", "emph[…]", "Italic emphasis."),
    f("strong", "strong[…]", "Bold emphasis."),
    f("underline", "underline[…]", "Underlined text."),
    f("overline", "overline[…]", "Overlined text."),
    f("strike", "strike[…]", "Strikethrough text."),
    f("highlight", "highlight[…]", "Highlighted text."),
    f("sub", "sub[…]", "Subscript."),
    f("super", "super[…]", "Superscript."),
    f("smallcaps", "smallcaps[…]", "Small-caps text."),
    f("raw", "raw(\"code\", lang: \"rust\")", "Inline / block raw code with optional language."),
    f("link", "link(\"https://…\", […])", "Hyperlink."),

    // ── References / labels ───────────────────────────────────
    f("label", "label(\"name\")", "Attach a reference label to the preceding element."),
    f("ref", "ref(<label>)", "Reference a labelled element."),
    f("cite", "cite(<key>)", "Cite a bibliography entry."),
    f("bibliography", "bibliography(\"refs.bib\")", "Render the bibliography from a .bib / .yaml file."),
    f("footnote", "footnote[…]", "Insert a footnote."),

    // ── Headings / structure ──────────────────────────────────
    f("heading", "heading(level: 1)[Title]", "Section heading."),
    f("outline", "outline(title: \"Contents\", depth: 3)", "Generate a table of contents."),
    f("counter", "counter(\"page\")", "Numbering counter."),

    // ── Lists ─────────────────────────────────────────────────
    f("list", "list[Item 1][Item 2]", "Unordered list."),
    f("enum", "enum[First][Second]", "Numbered list."),
    f("terms", "terms((\"key\", [definition]))", "Definition list."),

    // ── Tables / grid ─────────────────────────────────────────
    f("table", "table(columns: 3, [a],[b],[c])", "Tabular layout."),
    f("grid", "grid(columns: (1fr, 2fr), [a], [b])", "Low-level grid layout."),

    // ── Blocks / boxes ────────────────────────────────────────
    f("block", "block(height: 4cm)[…]", "Block-level container."),
    f("box", "box(width: 3em)[…]", "Inline-level container."),
    f("rect", "rect(fill: blue, width: 2cm, height: 1cm)", "Filled rectangle."),
    f("circle", "circle(radius: 1cm, fill: red)", "Filled circle."),
    f("ellipse", "ellipse(width: 2cm, height: 1cm)", "Filled ellipse."),
    f("polygon", "polygon((0pt,0pt),(2cm,0pt),(1cm,2cm))", "Polygon by points."),

    // ── Page layout ───────────────────────────────────────────
    f("page", "page(paper: \"a4\", margin: 2.5cm)", "Document page configuration."),
    f("pagebreak", "pagebreak(weak: true)", "Force / soft page break."),
    f("columns", "columns(2)[…]", "Multi-column layout."),
    f("place", "place(top + right, dx: 1cm)[…]", "Absolute placement."),
    f("pad", "pad(left: 2em, right: 2em)[…]", "Pad content."),
    f("align", "align(center)[…]", "Align content."),
    f("h", "h(1em)", "Horizontal spacing."),
    f("v", "v(1em, weak: true)", "Vertical spacing."),
    f("hide", "hide[…]", "Reserve space without rendering."),
    f("repeat", "repeat[.]", "Repeat content to fill space."),
    f("rotate", "rotate(45deg, …)", "Rotate content."),
    f("scale", "scale(x: 200%, y: 50%, …)", "Scale content."),
    f("move", "move(dx: 1em, dy: -.5em, …)", "Translate content."),
    f("linebreak", "linebreak()", "Force a line break."),
    f("parbreak", "parbreak()", "Force a paragraph break."),
    f("par", "par(justify: true)[…]", "Paragraph properties."),

    // ── Images / figures ──────────────────────────────────────
    f("image", "image(\"path/to/file.png\", width: 80%)", "Embed an image."),
    f("figure", "figure(image(\"…\"), caption: [Caption.])", "Figure with caption."),

    // ── Math ──────────────────────────────────────────────────
    f("math.equation", "math.equation(numbering: \"(1)\")[…]", "Math equation."),
    f("math.frac", "math.frac(a, b)", "Fraction."),
    f("math.sqrt", "math.sqrt(x)", "Square root."),
    f("math.sum", "math.sum_(i=1)^n", "Summation."),
    f("math.integral", "math.integral_a^b f(x) d x", "Integral."),
    f("math.lim", "math.lim_(n -> oo)", "Limit."),
    f("math.vec", "math.vec(a, b, c)", "Vector."),
    f("math.mat", "math.mat(a, b; c, d)", "Matrix."),

    // ── Values / conversion ───────────────────────────────────
    f("let", "let x = 1", "Variable binding."),
    f("set", "set text(size: 12pt)", "Set default arguments for a function."),
    f("show", "show heading: it => block(it)", "Per-element show rule."),
    f("import", "import \"module.typ\": *", "Import another file."),
    f("include", "include \"chapter.typ\"", "Include another file as content."),
    f("type", "type(x)", "Type of a value."),
    f("repr", "repr(x)", "String representation."),
    f("str", "str(x)", "Convert to string."),
    f("int", "int(x)", "Convert to integer."),
    f("float", "float(x)", "Convert to float."),
    f("bool", "bool(x)", "Convert to boolean."),
    f("range", "range(0, 10)", "Numeric range."),
    f("calc.min", "calc.min(a, b, …)", "Minimum of values."),
    f("calc.max", "calc.max(a, b, …)", "Maximum of values."),
    f("calc.abs", "calc.abs(x)", "Absolute value."),

    // ── Color / fill ──────────────────────────────────────────
    f("rgb", "rgb(\"#1e1e2e\")", "RGB colour."),
    f("cmyk", "cmyk(0%, 80%, 100%, 0%)", "CMYK colour."),
    f("luma", "luma(50%)", "Greyscale colour."),
    f("gradient.linear", "gradient.linear(red, blue, angle: 45deg)", "Linear gradient."),

    // ── Date / time ───────────────────────────────────────────
    f("datetime", "datetime(year: 2026, month: 5, day: 19)", "Date/time literal."),
    f("duration", "duration(hours: 1, minutes: 30)", "Duration."),

    // ── Tables / fields helpers ───────────────────────────────
    f("table.header", "table.header(repeat: true, [Name],[Age])", "Table header row(s)."),
    f("table.cell", "table.cell(colspan: 2)[…]", "Table cell with span / alignment overrides."),

    // ── Misc utilities ────────────────────────────────────────
    f("query", "query(<label>)", "Query the document for matching elements."),
    f("locate", "locate(it => […])", "Locate an element to read its position."),
    f("style", "style(styles => …)", "Read the active style context."),
    f("state", "state(\"key\", 0)", "Counter / mutable state."),
];
