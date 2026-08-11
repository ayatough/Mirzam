//! Typst-flavoured math: source text to LaTeX.
//!
//! LaTeX is hard to write from memory; Typst's math syntax is not. This crate
//! parses the useful subset of that syntax and lowers it to LaTeX, which then
//! goes through the same `math-core` path every formula already takes — so
//! spacing, stretchy delimiters and font handling are shared with the LaTeX
//! front end rather than reimplemented. The AST in the middle is the seam if
//! that ever changes.
//!
//! This is a subset parser, deliberately: depending on Typst itself would pull
//! its whole layout engine into crates that must compile to `wasm32`, and its
//! output is SVG, not MathML. The v1 surface:
//!
//! - `a/b` fractions, `^` and `_` scripts, with `(...)` grouping
//! - `sqrt(x)`, `root(3, x)`, `abs(x)`, `norm(x)`
//! - `mat(1, 2; 3, 4)` and `cases(x &"if" y, ...)`
//! - named symbols: Greek letters, `sum`, `product`, `integral`, `infinity`, …
//! - operators `->` `=>` `!=` `<=` `>=` and word relations `in`, `subset`, …
//! - `"literal text"`, `&` alignment with `\` line breaks, `#` escapes

/// Why a formula failed to parse. The renderer shows the source with this
/// message in the tooltip, the same way a broken LaTeX formula is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "typst math: {}", self.message)
    }
}

impl std::error::Error for Error {}

fn err<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error {
        message: message.into(),
    })
}

/// Converts one Typst math expression to LaTeX.
///
/// Alignment (`&`) and line breaks (`\`) at the top level wrap the result in
/// an `aligned` environment, which works in both display and inline math.
pub fn to_latex(src: &str) -> Result<String, Error> {
    let tokens = lex(src)?;
    let mut p = Parser { tokens, pos: 0 };
    let nodes = p.sequence(&[])?;
    if p.pos < p.tokens.len() {
        return err(format!("unmatched `{}`", p.tokens[p.pos].describe()));
    }
    Ok(emit_root(&nodes))
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// A number, possibly with a decimal point.
    Num(String),
    /// A run of letters: a variable, a symbol name, or a function name.
    Word(String),
    /// `"literal text"`, without the quotes.
    Str(String),
    /// `#c`: the character `c`, stripped of any meaning it had.
    Esc(char),
    /// A multi-character operator, already mapped to its LaTeX command.
    Sym(&'static str),
    /// Any other single character: `+`, `(`, `/`, `^`, …
    Ch(char),
}

impl Tok {
    fn describe(&self) -> String {
        match self {
            Tok::Num(n) => n.clone(),
            Tok::Word(w) => w.clone(),
            Tok::Str(_) => "\"…\"".into(),
            Tok::Esc(c) => format!("#{c}"),
            Tok::Sym(s) => s.to_string(),
            Tok::Ch(c) => c.to_string(),
        }
    }
}

fn lex(src: &str) -> Result<Vec<Tok>, Error> {
    let mut tokens = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            _ if c.is_whitespace() => {
                chars.next();
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some(c) => s.push(c),
                        None => return err("unterminated string literal"),
                    }
                }
                tokens.push(Tok::Str(s));
            }
            '#' => {
                chars.next();
                match chars.next() {
                    Some(c) => tokens.push(Tok::Esc(c)),
                    None => return err("`#` needs a character to escape"),
                }
            }
            _ if c.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        n.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                // A decimal point, but not a trailing dot (`1.` is `1` `.`).
                if chars.peek() == Some(&'.') {
                    let mut ahead = chars.clone();
                    ahead.next();
                    if ahead.peek().is_some_and(|d| d.is_ascii_digit()) {
                        n.push('.');
                        chars.next();
                        while let Some(&d) = chars.peek() {
                            if d.is_ascii_digit() {
                                n.push(d);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                }
                tokens.push(Tok::Num(n));
            }
            _ if c.is_alphabetic() => {
                let mut w = String::new();
                while let Some(&l) = chars.peek() {
                    if l.is_alphabetic() {
                        w.push(l);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Tok::Word(w));
            }
            '-' | '=' | '<' | '>' | '!' | '.' => {
                // Multi-character operators, longest first.
                let rest: String = chars.clone().take(3).collect();
                let (tok, len) = if rest.starts_with("...") {
                    (Tok::Sym("\\dots"), 3)
                } else if rest.starts_with("->") {
                    (Tok::Sym("\\to"), 2)
                } else if rest.starts_with("=>") {
                    (Tok::Sym("\\Rightarrow"), 2)
                } else if rest.starts_with("!=") {
                    (Tok::Sym("\\neq"), 2)
                } else if rest.starts_with("<=") {
                    (Tok::Sym("\\leq"), 2)
                } else if rest.starts_with(">=") {
                    (Tok::Sym("\\geq"), 2)
                } else if rest.starts_with("<-") {
                    (Tok::Sym("\\leftarrow"), 2)
                } else {
                    (Tok::Ch(c), 1)
                };
                for _ in 0..len {
                    chars.next();
                }
                tokens.push(tok);
            }
            '+' | '*' | '/' | '^' | '_' | '&' | '\\' | '(' | ')' | '[' | ']' | ',' | ';' | '|'
            | '\'' | ':' | '?' | '%' | '@' | '~' => {
                chars.next();
                tokens.push(Tok::Ch(c));
            }
            _ => return err(format!("unexpected character `{c}`")),
        }
    }
    Ok(tokens)
}

// ---------------------------------------------------------------------------
// AST and parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(String),
    /// A single italic letter.
    Ident(char),
    /// A LaTeX command or replacement, emitted verbatim.
    Sym(&'static str),
    /// A character passed through unchanged.
    Ch(char),
    Text(String),
    Esc(char),
    /// Juxtaposed nodes with no delimiter of their own.
    Seq(Vec<Node>),
    /// `(...)` that the author wants printed, stretchy.
    Paren(Vec<Node>),
    Frac(Box<Node>, Box<Node>),
    Script {
        base: Box<Node>,
        sub: Option<Box<Node>>,
        sup: Option<Box<Node>>,
    },
    Sqrt(Vec<Node>),
    Root(Vec<Node>, Vec<Node>),
    Abs(Vec<Node>),
    Norm(Vec<Node>),
    /// Rows of cells: `mat(1, 2; 3, 4)`.
    Matrix(Vec<Vec<Vec<Node>>>),
    /// Rows, each split on `&`: `cases(x &"if" y, ...)`.
    Cases(Vec<Vec<Vec<Node>>>),
    /// `&`, meaningful at the top level and inside `cases`.
    Align,
    /// `\`, a line break at the top level.
    Break,
}

/// Symbol and function names the parser knows. Greek follows Typst's glyphs
/// (`epsilon` is ε, `phi` is φ), so a deck reads the same in both tools.
/// Capital Greek letters without a LaTeX command are the Latin letters they
/// look like.
fn word_symbol(w: &str) -> Option<&'static str> {
    Some(match w {
        // Greek, lowercase.
        "alpha" => "\\alpha",
        "beta" => "\\beta",
        "gamma" => "\\gamma",
        "delta" => "\\delta",
        "epsilon" => "\\varepsilon",
        "zeta" => "\\zeta",
        "eta" => "\\eta",
        "theta" => "\\theta",
        "iota" => "\\iota",
        "kappa" => "\\kappa",
        "lambda" => "\\lambda",
        "mu" => "\\mu",
        "nu" => "\\nu",
        "xi" => "\\xi",
        "omicron" => "o",
        "pi" => "\\pi",
        "rho" => "\\rho",
        "sigma" => "\\sigma",
        "tau" => "\\tau",
        "upsilon" => "\\upsilon",
        "phi" => "\\varphi",
        "chi" => "\\chi",
        "psi" => "\\psi",
        "omega" => "\\omega",
        // Greek, uppercase.
        "Alpha" => "A",
        "Beta" => "B",
        "Gamma" => "\\Gamma",
        "Delta" => "\\Delta",
        "Epsilon" => "E",
        "Zeta" => "Z",
        "Eta" => "H",
        "Theta" => "\\Theta",
        "Iota" => "I",
        "Kappa" => "K",
        "Lambda" => "\\Lambda",
        "Mu" => "M",
        "Nu" => "N",
        "Xi" => "\\Xi",
        "Omicron" => "O",
        "Pi" => "\\Pi",
        "Rho" => "P",
        "Sigma" => "\\Sigma",
        "Tau" => "T",
        "Upsilon" => "\\Upsilon",
        "Phi" => "\\Phi",
        "Chi" => "X",
        "Psi" => "\\Psi",
        "Omega" => "\\Omega",
        // Big operators.
        "sum" => "\\sum",
        "product" => "\\prod",
        "integral" => "\\int",
        // Symbols.
        "infinity" | "oo" => "\\infty",
        "partial" | "diff" => "\\partial",
        "nabla" => "\\nabla",
        "hbar" => "\\hbar",
        "dots" => "\\dots",
        "times" => "\\times",
        "div" => "\\div",
        "emptyset" | "nothing" => "\\emptyset",
        "forall" => "\\forall",
        "exists" => "\\exists",
        // Relations.
        "in" => "\\in",
        "subset" => "\\subset",
        "supset" => "\\supset",
        "union" => "\\cup",
        "sect" | "inter" => "\\cap",
        "approx" => "\\approx",
        "equiv" => "\\equiv",
        "prop" => "\\propto",
        "pm" => "\\pm",
        "mp" => "\\mp",
        // Upright function names.
        "sin" => "\\sin",
        "cos" => "\\cos",
        "tan" => "\\tan",
        "cot" => "\\cot",
        "sec" => "\\sec",
        "csc" => "\\csc",
        "sinh" => "\\sinh",
        "cosh" => "\\cosh",
        "tanh" => "\\tanh",
        "arcsin" => "\\arcsin",
        "arccos" => "\\arccos",
        "arctan" => "\\arctan",
        "log" => "\\log",
        "ln" => "\\ln",
        "exp" => "\\exp",
        "lim" => "\\lim",
        "min" => "\\min",
        "max" => "\\max",
        "det" => "\\det",
        "gcd" => "\\gcd",
        "arg" => "\\arg",
        _ => return None,
    })
}

/// Names that take parenthesised arguments.
const CALL_FUNCS: [&str; 6] = ["sqrt", "root", "mat", "cases", "abs", "norm"];

/// Upright function names, which glue to a following `(...)` argument list.
fn is_function_word(w: &str) -> bool {
    matches!(
        w,
        "sin"
            | "cos"
            | "tan"
            | "cot"
            | "sec"
            | "csc"
            | "sinh"
            | "cosh"
            | "tanh"
            | "arcsin"
            | "arccos"
            | "arctan"
            | "log"
            | "ln"
            | "exp"
            | "min"
            | "max"
            | "det"
            | "gcd"
            | "arg"
    )
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(&Tok::Ch(c)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Parses items until one of `stops` (or the end of input), leaving the
    /// stop token unconsumed.
    fn sequence(&mut self, stops: &[char]) -> Result<Vec<Node>, Error> {
        let mut nodes = Vec::new();
        while let Some(t) = self.peek() {
            if let Tok::Ch(c) = t {
                if stops.contains(c) {
                    break;
                }
            }
            nodes.push(self.item()?);
        }
        Ok(nodes)
    }

    /// One item: a fraction chain of script-carrying primaries.
    fn item(&mut self) -> Result<Node, Error> {
        let mut lhs = self.postfix()?;
        while self.eat('/') {
            let rhs = self.postfix().map_err(|_| Error {
                message: "`/` needs a right-hand side".into(),
            })?;
            lhs = Node::Frac(Box::new(ungroup(lhs)), Box::new(ungroup(rhs)));
        }
        Ok(lhs)
    }

    /// A primary with its primes and `^`/`_` scripts attached.
    fn postfix(&mut self) -> Result<Node, Error> {
        let mut base = self.primary()?;
        let mut sub: Option<Box<Node>> = None;
        let mut sup: Option<Box<Node>> = None;
        loop {
            match self.peek() {
                Some(Tok::Ch('\'')) if sub.is_none() && sup.is_none() => {
                    self.pos += 1;
                    base = Node::Seq(vec![base, Node::Ch('\'')]);
                }
                Some(Tok::Ch('^')) => {
                    self.pos += 1;
                    if sup.is_some() {
                        return err("double superscript");
                    }
                    sup = Some(Box::new(self.script_operand()?));
                }
                Some(Tok::Ch('_')) => {
                    self.pos += 1;
                    if sub.is_some() {
                        return err("double subscript");
                    }
                    sub = Some(Box::new(self.script_operand()?));
                }
                _ => break,
            }
        }
        if sub.is_some() || sup.is_some() {
            base = Node::Script {
                base: Box::new(base),
                sub,
                sup,
            };
        }
        Ok(base)
    }

    /// The operand of `^` or `_`: one primary, with `(...)` grouping unwrapped
    /// and a leading sign allowed, so `x^-1` means what it looks like.
    fn script_operand(&mut self) -> Result<Node, Error> {
        let sign = match self.peek() {
            Some(Tok::Ch(c @ ('-' | '+'))) => {
                let c = *c;
                self.pos += 1;
                Some(c)
            }
            _ => None,
        };
        let node = ungroup(self.primary().map_err(|_| Error {
            message: "`^` and `_` need an operand".into(),
        })?);
        Ok(match sign {
            Some(s) => Node::Seq(vec![Node::Ch(s), node]),
            None => node,
        })
    }

    fn primary(&mut self) -> Result<Node, Error> {
        let Some(t) = self.next() else {
            return err("unexpected end of formula");
        };
        Ok(match t {
            Tok::Num(n) => Node::Num(n),
            Tok::Str(s) => Node::Text(s),
            Tok::Esc(c) => Node::Esc(c),
            Tok::Sym(s) => Node::Sym(s),
            Tok::Word(w) => self.word(w)?,
            Tok::Ch('(') => {
                let inner = self.sequence(&[')'])?;
                if !self.eat(')') {
                    return err("unmatched `(`");
                }
                Node::Paren(inner)
            }
            Tok::Ch('&') => Node::Align,
            Tok::Ch('\\') => Node::Break,
            Tok::Ch(c @ ('/' | '^' | '_')) => {
                return err(format!("`{c}` needs something before it"))
            }
            Tok::Ch(c @ (')' | ']')) => return err(format!("unmatched `{c}`")),
            Tok::Ch(c) => Node::Ch(c),
        })
    }

    /// A word is a call, a known symbol, a variable, or — unknown and longer
    /// than one letter — a run of variables, which is what LaTeX would have
    /// made of the same letters.
    fn word(&mut self, w: String) -> Result<Node, Error> {
        if CALL_FUNCS.contains(&w.as_str()) {
            if self.peek() != Some(&Tok::Ch('(')) {
                return err(format!("`{w}` needs parenthesised arguments"));
            }
            self.pos += 1;
            return self.call(&w);
        }
        if let Some(sym) = word_symbol(&w) {
            // `sin(x)/x` is the fraction of sin(x) by x: a function name glues
            // to its argument list, so `/`, `^` and `_` treat them as one.
            if is_function_word(&w) && self.peek() == Some(&Tok::Ch('(')) {
                self.pos += 1;
                let inner = self.sequence(&[')'])?;
                if !self.eat(')') {
                    return err("unmatched `(`");
                }
                return Ok(Node::Seq(vec![Node::Sym(sym), Node::Paren(inner)]));
            }
            return Ok(Node::Sym(sym));
        }
        let mut letters = w.chars();
        if let (Some(first), None) = (letters.next(), letters.next()) {
            return Ok(Node::Ident(first));
        }
        Ok(Node::Seq(w.chars().map(Node::Ident).collect()))
    }

    /// Arguments of `name(...)`, split on `,` and `;`, the opening paren
    /// already consumed.
    fn call(&mut self, name: &str) -> Result<Node, Error> {
        let mut rows: Vec<Vec<Vec<Node>>> = vec![Vec::new()];
        loop {
            let arg = self.sequence(&[',', ';', ')'])?;
            rows.last_mut().expect("rows are non-empty").push(arg);
            match self.next() {
                Some(Tok::Ch(',')) => {}
                Some(Tok::Ch(';')) => rows.push(Vec::new()),
                Some(Tok::Ch(')')) => break,
                _ => return err(format!("unmatched `(` after `{name}`")),
            }
        }
        let mut args: Vec<Vec<Node>> = rows.concat();
        let argc = args.len();
        let one = |args: &mut Vec<Vec<Node>>| args.remove(0);
        Ok(match name {
            "sqrt" if argc == 1 => Node::Sqrt(one(&mut args)),
            "root" if argc == 2 => {
                let idx = one(&mut args);
                Node::Root(idx, one(&mut args))
            }
            "abs" if argc == 1 => Node::Abs(one(&mut args)),
            "norm" if argc == 1 => Node::Norm(one(&mut args)),
            "mat" => Node::Matrix(rows),
            // Each `cases` argument is one row, its cells split on `&`.
            "cases" => Node::Cases(args.into_iter().map(split_on_align).collect()),
            "sqrt" => return err("`sqrt` takes one argument; `root(n, x)` takes the index first"),
            "root" => return err("`root` takes two arguments: `root(n, x)`"),
            "abs" | "norm" => return err(format!("`{name}` takes one argument")),
            _ => unreachable!("only CALL_FUNCS reach here"),
        })
    }
}

/// `(...)` used as a fraction or script operand groups without printing.
fn ungroup(node: Node) -> Node {
    match node {
        Node::Paren(inner) => Node::Seq(inner),
        other => other,
    }
}

fn split_on_align(nodes: Vec<Node>) -> Vec<Vec<Node>> {
    let mut cells = vec![Vec::new()];
    for n in nodes {
        if n == Node::Align {
            cells.push(Vec::new());
        } else {
            cells.last_mut().expect("cells are non-empty").push(n);
        }
    }
    cells
}

// ---------------------------------------------------------------------------
// LaTeX emission
// ---------------------------------------------------------------------------

/// Emits the whole formula. Top-level `&` or `\` means the author is aligning
/// equations, which LaTeX only allows inside an environment — `aligned` works
/// in both display and inline math.
fn emit_root(nodes: &[Node]) -> String {
    let aligned = nodes.iter().any(|n| matches!(n, Node::Align | Node::Break));
    if !aligned {
        return emit_seq(nodes);
    }
    let rows: Vec<String> = nodes
        .split(|n| *n == Node::Break)
        .map(|row| {
            row.split(|n| *n == Node::Align)
                .map(emit_seq)
                .collect::<Vec<_>>()
                .join(" & ")
        })
        .collect();
    format!("\\begin{{aligned}}{}\\end{{aligned}}", rows.join(" \\\\ "))
}

fn emit_seq(nodes: &[Node]) -> String {
    nodes.iter().map(emit).collect::<Vec<_>>().join(" ")
}

fn emit(node: &Node) -> String {
    match node {
        Node::Num(n) => n.clone(),
        Node::Ident(c) => c.to_string(),
        Node::Sym(s) => s.to_string(),
        Node::Ch(c) => c.to_string(),
        Node::Text(s) => format!("\\text{{{}}}", escape_text(s)),
        Node::Esc(c) => escape_char(*c),
        Node::Seq(inner) => emit_seq(inner),
        Node::Paren(inner) => format!("\\left({}\\right)", emit_seq(inner)),
        Node::Frac(a, b) => format!("\\frac{{{}}}{{{}}}", emit(a), emit(b)),
        Node::Script { base, sub, sup } => {
            // Braces around a one-token base change more than grouping:
            // `{\sum}` is an ordinary atom that has lost its movable limits,
            // and even `{e}` shifts the spacing of the operator before it.
            // Brace only what actually needs holding together.
            let mut s = match &**base {
                Node::Sym(cmd) => cmd.to_string(),
                Node::Ident(c) | Node::Ch(c) => c.to_string(),
                Node::Num(n) if n.chars().count() == 1 => n.clone(),
                other => format!("{{{}}}", emit(other)),
            };
            if let Some(b) = sub {
                s.push_str(&format!("_{{{}}}", emit(b)));
            }
            if let Some(p) = sup {
                s.push_str(&format!("^{{{}}}", emit(p)));
            }
            s
        }
        Node::Sqrt(x) => format!("\\sqrt{{{}}}", emit_seq(x)),
        Node::Root(n, x) => format!("\\sqrt[{}]{{{}}}", emit_seq(n), emit_seq(x)),
        Node::Abs(x) => format!("\\left|{}\\right|", emit_seq(x)),
        Node::Norm(x) => format!("\\left\\|{}\\right\\|", emit_seq(x)),
        Node::Matrix(rows) => {
            let body = rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|c| emit_seq(c))
                        .collect::<Vec<_>>()
                        .join(" & ")
                })
                .collect::<Vec<_>>()
                .join(" \\\\ ");
            format!("\\begin{{pmatrix}}{body}\\end{{pmatrix}}")
        }
        Node::Cases(rows) => {
            let body = rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|c| emit_seq(c))
                        .collect::<Vec<_>>()
                        .join(" & ")
                })
                .collect::<Vec<_>>()
                .join(" \\\\ ");
            format!("\\begin{{cases}}{body}\\end{{cases}}")
        }
        // Alignment that survives to here sits somewhere LaTeX gives it no
        // meaning; a literal ampersand keeps the output well-formed.
        Node::Align => "\\&".into(),
        Node::Break => "\\\\".into(),
    }
}

/// Characters that need escaping inside `\text{...}`.
fn escape_text(s: &str) -> String {
    s.chars().map(escape_char).collect()
}

fn escape_char(c: char) -> String {
    match c {
        '&' | '_' | '#' | '%' | '$' | '{' | '}' => format!("\\{c}"),
        '\\' => "\\backslash ".into(),
        _ => c.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn latex(src: &str) -> String {
        to_latex(src).unwrap_or_else(|e| panic!("{src}: {e}"))
    }

    #[test]
    fn fractions_bind_the_adjacent_factor() {
        assert_eq!(latex("a/b"), "\\frac{a}{b}");
        assert_eq!(latex("1 + x/2"), "1 + \\frac{x}{2}");
        // The parens group without printing.
        assert_eq!(latex("(a + b)/c"), "\\frac{a + b}{c}");
        // Left associative.
        assert_eq!(latex("a/b/c"), "\\frac{\\frac{a}{b}}{c}");
    }

    #[test]
    fn scripts_and_grouping() {
        assert_eq!(latex("x^2"), "x^{2}");
        assert_eq!(latex("x_(i+1)"), "x_{i + 1}");
        assert_eq!(latex("x_i^2"), "x_{i}^{2}");
        assert_eq!(latex("x^-1"), "x^{- 1}");
    }

    #[test]
    fn big_operators_take_limits() {
        // No braces around the operator: `{\sum}` would freeze its limits
        // beside it instead of above and below.
        assert_eq!(latex("sum_(i=1)^n i"), "\\sum_{i = 1}^{n} i");
        assert_eq!(latex("integral_0^oo"), "\\int_{0}^{\\infty}");
        assert_eq!(latex("lim_(x -> 0)"), "\\lim_{x \\to 0}");
    }

    #[test]
    fn a_function_glues_to_its_arguments() {
        assert_eq!(latex("sin(x)/x"), "\\frac{\\sin \\left(x\\right)}{x}");
    }

    #[test]
    fn roots_and_bars() {
        assert_eq!(latex("sqrt(x + 1)"), "\\sqrt{x + 1}");
        assert_eq!(latex("root(3, x)"), "\\sqrt[3]{x}");
        assert_eq!(latex("abs(x - y)"), "\\left|x - y\\right|");
        assert_eq!(latex("norm(v)"), "\\left\\|v\\right\\|");
    }

    #[test]
    fn greek_and_words() {
        assert_eq!(latex("alpha + Omega"), "\\alpha + \\Omega");
        // Typst's epsilon is the curly one.
        assert_eq!(latex("epsilon"), "\\varepsilon");
        // Capitals with no LaTeX command are the Latin letters they look like.
        assert_eq!(latex("Alpha"), "A");
        // An unknown word is a run of variables, as LaTeX would read it.
        assert_eq!(latex("dx"), "d x");
    }

    #[test]
    fn operators_map() {
        assert_eq!(latex("a -> b"), "a \\to b");
        assert_eq!(latex("a => b"), "a \\Rightarrow b");
        assert_eq!(latex("a != b"), "a \\neq b");
        assert_eq!(latex("a <= b"), "a \\leq b");
        assert_eq!(latex("a >= b"), "a \\geq b");
        assert_eq!(latex("x in A subset B"), "x \\in A \\subset B");
    }

    #[test]
    fn matrices() {
        assert_eq!(
            latex("mat(1, 2; 3, 4)"),
            "\\begin{pmatrix}1 & 2 \\\\ 3 & 4\\end{pmatrix}"
        );
    }

    #[test]
    fn cases_split_cells_on_align() {
        assert_eq!(
            latex("cases(x^2 &\"if\" x > 0, 0 &\"otherwise\")"),
            "\\begin{cases}x^{2} & \\text{if} x > 0 \\\\ 0 & \\text{otherwise}\\end{cases}"
        );
    }

    #[test]
    fn text_and_escapes() {
        assert_eq!(latex("\"speed limit\""), "\\text{speed limit}");
        // `#` strips a character of its meaning.
        assert_eq!(latex("a #/ b"), "a / b");
        assert_eq!(latex("a #& b"), "a \\& b");
    }

    #[test]
    fn alignment_wraps_in_aligned() {
        assert_eq!(
            latex("a &= b \\ c &= d"),
            "\\begin{aligned}a & = b \\\\ c & = d\\end{aligned}"
        );
    }

    #[test]
    fn printed_parens_are_stretchy() {
        assert_eq!(latex("f(x)"), "f \\left(x\\right)");
    }

    #[test]
    fn errors_name_the_problem() {
        assert!(to_latex("sqrt(").is_err());
        assert!(to_latex("a/").is_err());
        assert!(to_latex("x^").is_err());
        assert!(to_latex("\"unterminated").is_err());
        assert!(to_latex("(a").is_err());
        assert!(to_latex("a)").is_err());
        assert!(to_latex("root(3)").is_err());
        assert!(to_latex("sqrt x").is_err());
    }
}
