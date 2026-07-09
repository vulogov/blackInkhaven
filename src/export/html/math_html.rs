//! TDOC-4 — a small Typst-math → MathML converter for the HTML exporter. MathML is
//! native to browsers (no JS, no font download), so the site stays self-contained.
//!
//! It covers the common subset — identifiers and Greek letters, numbers, operators,
//! super/subscripts (`^` `_`), fractions (`a/b`), roots (`sqrt(...)`), parentheses,
//! and the big operators (`sum` `integral` `product` `infinity`). Anything it does
//! not recognise degrades to plain symbols rather than failing.

/// Render Typst math as a block MathML equation.
pub fn render_block(src: &str) -> String {
    format!(
        "<math display=\"block\" class=\"math\"><mrow>{}</mrow></math>",
        to_mathml(src)
    )
}

/// Render Typst math inline.
pub fn render_inline(src: &str) -> String {
    format!("<math><mrow>{}</mrow></math>", to_mathml(src))
}

/// Convert a Typst-math string to MathML rows (without the enclosing `<math>`).
pub fn to_mathml(src: &str) -> String {
    let toks = tokenize(src);
    let mut p = Parser { toks, pos: 0 };
    p.parse_seq()
}

enum Tok {
    Ident(String),
    Num(String),
    Op(String),
    Caret,
    Under,
    LParen,
    RParen,
    Comma,
    Slash,
}

fn tokenize(s: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            out.push(Tok::Num(chars[start..i].iter().collect()));
        } else {
            // Multi-char operators first.
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            match two.as_str() {
                "<=" | ">=" | "!=" | "->" | "=>" | ":=" | "==" => {
                    out.push(Tok::Op(two));
                    i += 2;
                    continue;
                }
                _ => {}
            }
            match c {
                '^' => out.push(Tok::Caret),
                '_' => out.push(Tok::Under),
                '(' => out.push(Tok::LParen),
                ')' => out.push(Tok::RParen),
                ',' => out.push(Tok::Comma),
                '/' => out.push(Tok::Slash),
                _ => out.push(Tok::Op(c.to_string())),
            }
            i += 1;
        }
    }
    out
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    /// A run of factors until end / `)` / `,`.
    fn parse_seq(&mut self) -> String {
        let mut out = String::new();
        while !matches!(self.peek(), None | Some(Tok::RParen) | Some(Tok::Comma)) {
            out.push_str(&self.parse_factor());
        }
        out
    }

    /// An atom, its scripts, then any `/` fractions (looser than scripts).
    fn parse_factor(&mut self) -> String {
        let mut base = self.parse_scripted();
        while matches!(self.peek(), Some(Tok::Slash)) {
            self.pos += 1;
            let denom = self.parse_scripted();
            base = format!("<mfrac>{}{}</mfrac>", row(&base), row(&denom));
        }
        base
    }

    /// An atom with optional `^` / `_`.
    fn parse_scripted(&mut self) -> String {
        let mut base = self.parse_atom();
        loop {
            match self.peek() {
                Some(Tok::Caret) => {
                    self.pos += 1;
                    let sup = self.parse_atom();
                    base = format!("<msup>{}{}</msup>", row(&base), row(&sup));
                }
                Some(Tok::Under) => {
                    self.pos += 1;
                    let sub = self.parse_atom();
                    base = format!("<msub>{}{}</msub>", row(&base), row(&sub));
                }
                _ => break,
            }
        }
        base
    }

    fn parse_atom(&mut self) -> String {
        match self.toks.get(self.pos) {
            Some(Tok::Num(n)) => {
                let n = n.clone();
                self.pos += 1;
                format!("<mn>{}</mn>", esc(&n))
            }
            Some(Tok::Op(o)) => {
                let o = o.clone();
                self.pos += 1;
                format!("<mo>{}</mo>", op_symbol(&o))
            }
            Some(Tok::LParen) => {
                self.pos += 1;
                let inner = self.parse_seq();
                if matches!(self.peek(), Some(Tok::RParen)) {
                    self.pos += 1;
                }
                format!("<mrow><mo>(</mo>{inner}<mo>)</mo></mrow>")
            }
            Some(Tok::Ident(name)) => {
                let name = name.clone();
                self.pos += 1;
                self.ident_atom(&name)
            }
            // A stray script/paren/comma/slash — emit nothing and advance.
            Some(_) => {
                self.pos += 1;
                String::new()
            }
            None => String::new(),
        }
    }

    fn ident_atom(&mut self, name: &str) -> String {
        // Function-like: sqrt(...), root(n, x).
        if name == "sqrt" && matches!(self.peek(), Some(Tok::LParen)) {
            self.pos += 1;
            let inner = self.parse_seq();
            if matches!(self.peek(), Some(Tok::RParen)) {
                self.pos += 1;
            }
            return format!("<msqrt>{inner}</msqrt>");
        }
        if name == "frac" && matches!(self.peek(), Some(Tok::LParen)) {
            self.pos += 1;
            let a = self.parse_seq();
            if matches!(self.peek(), Some(Tok::Comma)) {
                self.pos += 1;
            }
            let b = self.parse_seq();
            if matches!(self.peek(), Some(Tok::RParen)) {
                self.pos += 1;
            }
            return format!("<mfrac>{}{}</mfrac>", row(&a), row(&b));
        }
        if let Some(sym) = big_operator(name) {
            return format!("<mo>{sym}</mo>");
        }
        if let Some(sym) = greek(name) {
            return format!("<mi>{sym}</mi>");
        }
        if is_function(name) {
            return format!("<mi mathvariant=\"normal\">{}</mi>", esc(name));
        }
        // A bare identifier (single or multi letter).
        format!("<mi>{}</mi>", esc(name))
    }
}

/// Wrap a fragment as a single MathML element (scripts/fracs need one child each).
fn row(s: &str) -> String {
    format!("<mrow>{s}</mrow>")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn op_symbol(o: &str) -> String {
    let s = match o {
        "<=" => "≤",
        ">=" => "≥",
        "!=" => "≠",
        "->" => "→",
        "=>" => "⇒",
        ":=" => "≔",
        "*" => "·",
        "<" => "&lt;",
        ">" => "&gt;",
        "&" => "&amp;",
        "-" => "−",
        other => return esc(other),
    };
    s.to_string()
}

fn big_operator(name: &str) -> Option<&'static str> {
    Some(match name {
        "sum" => "∑",
        "product" | "prod" => "∏",
        "integral" | "int" => "∫",
        "infinity" | "infty" | "oo" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "dot" => "⋅",
        "times" => "×",
        "div" => "÷",
        "plus" => "+",
        "minus" => "−",
        "approx" => "≈",
        "eq" => "=",
        "in" => "∈",
        "forall" => "∀",
        "exists" => "∃",
        _ => return None,
    })
}

fn greek(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α", "beta" => "β", "gamma" => "γ", "delta" => "δ",
        "epsilon" => "ε", "zeta" => "ζ", "eta" => "η", "theta" => "θ",
        "iota" => "ι", "kappa" => "κ", "lambda" => "λ", "mu" => "μ",
        "nu" => "ν", "xi" => "ξ", "pi" => "π", "rho" => "ρ",
        "sigma" => "σ", "tau" => "τ", "upsilon" => "υ", "phi" => "φ",
        "chi" => "χ", "psi" => "ψ", "omega" => "ω",
        "Gamma" => "Γ", "Delta" => "Δ", "Theta" => "Θ", "Lambda" => "Λ",
        "Xi" => "Ξ", "Pi" => "Π", "Sigma" => "Σ", "Phi" => "Φ",
        "Psi" => "Ψ", "Omega" => "Ω",
        _ => return None,
    })
}

fn is_function(name: &str) -> bool {
    matches!(
        name,
        "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "log" | "ln" | "exp"
            | "lim" | "max" | "min" | "det" | "gcd" | "arg" | "sinh" | "cosh" | "tanh"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superscript_and_function() {
        let m = to_mathml("f(x) = x^2");
        assert!(m.contains("<mi>f</mi>"));
        assert!(m.contains("<msup><mrow><mi>x</mi></mrow><mrow><mn>2</mn></mrow></msup>"), "got: {m}");
        assert!(m.contains("<mo>=</mo>"));
    }

    #[test]
    fn fraction_root_greek() {
        assert!(to_mathml("a/b").contains("<mfrac>"));
        assert!(to_mathml("sqrt(x)").contains("<msqrt><mi>x</mi></msqrt>"));
        assert!(to_mathml("alpha + pi").contains("<mi>α</mi>"));
        assert!(to_mathml("alpha + pi").contains("<mi>π</mi>"));
    }

    #[test]
    fn operators_and_subscript() {
        assert!(to_mathml("x <= y").contains("<mo>≤</mo>"));
        assert!(to_mathml("x_i").contains("<msub>"));
        assert!(to_mathml("sum").contains("<mo>∑</mo>"));
    }

    #[test]
    fn block_wraps_in_math() {
        assert!(render_block("x").starts_with("<math display=\"block\""));
    }
}
