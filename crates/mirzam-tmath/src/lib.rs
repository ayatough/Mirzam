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
//! output is SVG, not MathML. The surface:
//!
//! - `a/b` fractions, `^` and `_` scripts, with `(...)` grouping
//! - `sqrt(x)`, `root(3, x)`, `abs(x)`, `norm(x)`, `floor(x)`, `ceil(x)`,
//!   `binom(n, k)`, `cancel(x)`
//! - `mat(1, 2; 3, 4)` with `delim:`, `vec(1, 2)`, `cases(x &"if" y, ...)`
//! - accents `hat` `tilde` `dot` `ddot` `macron` `overline` `underline`
//!   `arrow`, letter styles `bb` `cal` `frak` `bold` `upright` `sans` `mono`
//! - named symbols: Greek letters with `.alt` variants, `sum`, `product`,
//!   `integral` and its `.double`/`.triple`/`.cont` variants, `infinity`, …
//! - operators `->` `=>` `!=` `<=` `>=`, word relations `in`, `subset` and
//!   dotted variants `subset.eq`, `in.not`, `dot.op`, `arrow.l.r`, …
//! - `underbrace(x, "label")`, `overbrace`, `op("argmax")`
//! - `"literal text"`, `&` alignment with `\` line breaks, `#` escapes
//!
//! Anything outside the subset is a parse error, shown as a red span with
//! this parser's message — never a silently different formula. That is why
//! unknown dotted names and unknown words used like functions refuse to
//! render as a run of letters.

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
                loop {
                    while let Some(&l) = chars.peek() {
                        if l.is_alphabetic() {
                            w.push(l);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    // A dotted variant name — `subset.eq`, `dots.c` — is one
                    // word. Read as anything else it would silently render as
                    // the base symbol, a stray dot and some letters.
                    if chars.peek() == Some(&'.') {
                        let mut ahead = chars.clone();
                        ahead.next();
                        if ahead.peek().is_some_and(|l| l.is_alphabetic()) {
                            w.push('.');
                            chars.next();
                            continue;
                        }
                    }
                    break;
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
    /// One argument inside one wrapping command: accents, letter styles,
    /// `cancel`.
    Wrap(&'static str, Vec<Node>),
    /// Content between a fixed delimiter pair: `floor`, `ceil`.
    Fenced(&'static str, &'static str, Vec<Node>),
    Binom(Vec<Node>, Vec<Node>),
    /// `op("argmax")`: an upright operator the tables do not know.
    Op(String),
    /// `underbrace(x, "label")` and `overbrace`.
    Brace {
        cmd: &'static str,
        /// Which side the label attaches to: `_` below, `^` above.
        attach: &'static str,
        content: Vec<Node>,
        label: Option<Vec<Node>>,
    },
    /// Rows of cells in a delimited environment: `mat(1, 2; 3, 4)`,
    /// `vec(1, 2)`.
    Matrix(&'static str, Vec<Vec<Vec<Node>>>),
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
        // Greek, dotted variants: the glyph the other name does not give.
        "epsilon.alt" => "\\epsilon",
        "phi.alt" => "\\phi",
        "theta.alt" => "\\vartheta",
        "rho.alt" => "\\varrho",
        "pi.alt" => "\\varpi",
        "sigma.alt" => "\\varsigma",
        // Big operators.
        "sum" => "\\sum",
        "product" => "\\prod",
        "integral" => "\\int",
        "integral.double" => "\\iint",
        "integral.triple" => "\\iiint",
        "integral.cont" => "\\oint",
        "union.big" => "\\bigcup",
        "sect.big" | "inter.big" => "\\bigcap",
        // Symbols.
        "infinity" | "oo" => "\\infty",
        "partial" | "diff" => "\\partial",
        "nabla" => "\\nabla",
        "hbar" | "planck.reduce" => "\\hbar",
        "dots" | "dots.h" => "\\dots",
        "dots.c" => "\\cdots",
        "dots.v" => "\\vdots",
        "dots.down" => "\\ddots",
        "times" => "\\times",
        "times.circle" => "\\otimes",
        "plus.circle" => "\\oplus",
        "plus.minus" | "pm" => "\\pm",
        "minus.plus" | "mp" => "\\mp",
        "div" => "\\div",
        // Bare `dot` is the multiplication dot; `dot(x)` is the accent.
        "dot" | "dot.op" => "\\cdot",
        "emptyset" | "nothing" => "\\emptyset",
        "forall" => "\\forall",
        "exists" => "\\exists",
        "and" => "\\wedge",
        "or" => "\\vee",
        "not" => "\\neg",
        // Arrows. `->` and `=>` also lex directly.
        "arrow" | "arrow.r" => "\\rightarrow",
        "arrow.l" => "\\leftarrow",
        "arrow.r.double" => "\\Rightarrow",
        "arrow.l.double" => "\\Leftarrow",
        "arrow.l.r" => "\\leftrightarrow",
        "arrow.l.r.double" => "\\Leftrightarrow",
        "arrow.r.long" => "\\longrightarrow",
        // Relations.
        "in" => "\\in",
        "in.not" => "\\notin",
        "subset" => "\\subset",
        "subset.eq" => "\\subseteq",
        "supset" => "\\supset",
        "supset.eq" => "\\supseteq",
        "union" => "\\cup",
        "sect" | "inter" => "\\cap",
        "approx" => "\\approx",
        "equiv" => "\\equiv",
        "prop" => "\\propto",
        "tilde.op" => "\\sim",
        "lt.eq" => "\\leq",
        "gt.eq" => "\\geq",
        "eq.not" => "\\neq",
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

/// Names that take parenthesised arguments when followed by `(`.
fn is_call_word(w: &str) -> bool {
    matches!(
        w,
        "sqrt"
            | "root"
            | "mat"
            | "cases"
            | "abs"
            | "norm"
            | "vec"
            | "binom"
            | "floor"
            | "ceil"
            | "op"
            | "underbrace"
            | "overbrace"
            | "cancel"
    ) || wrap_command(w).is_some()
}

/// Accents and letter styles: one argument, one wrapping LaTeX command.
/// `arrow(v)` is the vector accent; `vec()` is Typst's column vector.
fn wrap_command(w: &str) -> Option<&'static str> {
    Some(match w {
        // Accents.
        "hat" => "\\hat",
        "tilde" => "\\tilde",
        "dot" => "\\dot",
        "ddot" => "\\ddot",
        "macron" => "\\bar",
        "overline" => "\\overline",
        "underline" => "\\underline",
        "arrow" => "\\vec",
        // Letter styles.
        "bb" => "\\mathbb",
        "cal" => "\\mathcal",
        "frak" => "\\mathfrak",
        "bold" => "\\mathbf",
        "upright" => "\\mathrm",
        "sans" => "\\mathsf",
        "mono" => "\\mathtt",
        _ => return None,
    })
}

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
                // Primes and factorials belong to their base, so `n!/2` is a
                // fraction of n-factorial. (`!=` already lexed as one token.)
                Some(Tok::Ch(p @ ('\'' | '!'))) if sub.is_none() && sup.is_none() => {
                    let p = *p;
                    self.pos += 1;
                    base = Node::Seq(vec![base, Node::Ch(p)]);
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
    /// made of the same letters. A name that would render as something other
    /// than what the author meant — a dotted variant this parser does not
    /// know, or an unknown word used like a function — is an error instead:
    /// a red span is honest, quietly wrong glyphs are not.
    fn word(&mut self, w: String) -> Result<Node, Error> {
        let called = self.peek() == Some(&Tok::Ch('('));
        // `dot` and `arrow` are both a symbol and an accent; the argument
        // list is what distinguishes `a dot b` from `dot(x)`.
        if called && is_call_word(&w) {
            self.pos += 1;
            return self.call(&w);
        }
        if let Some(sym) = word_symbol(&w) {
            // `sin(x)/x` is the fraction of sin(x) by x: a function name glues
            // to its argument list, so `/`, `^` and `_` treat them as one.
            if called && is_function_word(&w) {
                self.pos += 1;
                let inner = self.sequence(&[')'])?;
                if !self.eat(')') {
                    return err("unmatched `(`");
                }
                return Ok(Node::Seq(vec![Node::Sym(sym), Node::Paren(inner)]));
            }
            return Ok(Node::Sym(sym));
        }
        if is_call_word(&w) {
            return err(format!("`{w}` needs parenthesised arguments"));
        }
        let mut letters = w.chars();
        if let (Some(first), None) = (letters.next(), letters.next()) {
            return Ok(Node::Ident(first));
        }
        if w.contains('.') {
            return err(format!("unknown symbol `{w}`"));
        }
        if called {
            return err(format!("unknown function `{w}`"));
        }
        Ok(Node::Seq(w.chars().map(Node::Ident).collect()))
    }

    /// `mat`'s `delim:` named argument, when the next tokens are one. Only
    /// recognised at the head of the argument list, so a stray ratio like
    /// `mat(a : b)` still parses as content.
    fn mat_delim(&mut self) -> Result<&'static str, Error> {
        if !(self.peek() == Some(&Tok::Word("delim".into()))
            && self.tokens.get(self.pos + 1) == Some(&Tok::Ch(':')))
        {
            return Ok("pmatrix");
        }
        self.pos += 2;
        let Some(Tok::Str(s)) = self.next() else {
            return err("`delim:` takes a quoted delimiter, e.g. `delim: \"[\"`");
        };
        let env = match s.as_str() {
            "(" => "pmatrix",
            "[" => "bmatrix",
            "{" => "Bmatrix",
            "|" => "vmatrix",
            "||" => "Vmatrix",
            other => return err(format!("`delim:` does not know `{other}`")),
        };
        // The delimiter may be the only argument, or be followed by the cells.
        if !self.eat(',') && self.peek() != Some(&Tok::Ch(')')) {
            return err("`delim:` must be followed by `,` and the matrix cells");
        }
        Ok(env)
    }

    /// Arguments of `name(...)`, split on `,` and `;`, the opening paren
    /// already consumed.
    fn call(&mut self, name: &str) -> Result<Node, Error> {
        let delim = if name == "mat" {
            self.mat_delim()?
        } else {
            "pmatrix"
        };
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
        if let Some(cmd) = wrap_command(name) {
            if argc != 1 {
                return err(format!("`{name}` takes one argument"));
            }
            return Ok(Node::Wrap(cmd, one(&mut args)));
        }
        Ok(match name {
            "sqrt" if argc == 1 => Node::Sqrt(one(&mut args)),
            "root" if argc == 2 => {
                let idx = one(&mut args);
                Node::Root(idx, one(&mut args))
            }
            "abs" if argc == 1 => Node::Abs(one(&mut args)),
            "norm" if argc == 1 => Node::Norm(one(&mut args)),
            "floor" if argc == 1 => {
                Node::Fenced("\\left\\lfloor", "\\right\\rfloor", one(&mut args))
            }
            "ceil" if argc == 1 => Node::Fenced("\\left\\lceil", "\\right\\rceil", one(&mut args)),
            "cancel" if argc == 1 => Node::Wrap("\\cancel", one(&mut args)),
            "binom" if argc == 2 => {
                let n = one(&mut args);
                Node::Binom(n, one(&mut args))
            }
            // An upright word the tables do not know: `op("argmax")`.
            "op" => match (argc, args.pop()) {
                (1, Some(arg)) => match arg.as_slice() {
                    [Node::Text(s)] => Node::Op(s.clone()),
                    _ => return err("`op` takes a quoted name, e.g. `op(\"argmax\")`"),
                },
                _ => return err("`op` takes a quoted name, e.g. `op(\"argmax\")`"),
            },
            "underbrace" | "overbrace" if argc == 1 || argc == 2 => {
                let content = one(&mut args);
                let label = args.pop();
                let (cmd, attach) = if name == "underbrace" {
                    ("\\underbrace", "_")
                } else {
                    ("\\overbrace", "^")
                };
                Node::Brace {
                    cmd,
                    attach,
                    content,
                    label,
                }
            }
            // A column vector; `mat` covers anything wider.
            "vec" => Node::Matrix(delim, args.into_iter().map(|e| vec![e]).collect()),
            "mat" => Node::Matrix(delim, rows),
            // Each `cases` argument is one row, its cells split on `&`.
            "cases" => Node::Cases(args.into_iter().map(split_on_align).collect()),
            "sqrt" => return err("`sqrt` takes one argument; `root(n, x)` takes the index first"),
            "root" => return err("`root` takes two arguments: `root(n, x)`"),
            "binom" => return err("`binom` takes two arguments: `binom(n, k)`"),
            "underbrace" | "overbrace" => {
                return err(format!("`{name}` takes the content and an optional label"))
            }
            _ => return err(format!("`{name}` takes one argument")),
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
                one @ (Node::Op(_) | Node::Wrap(..)) => emit(one),
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
        Node::Wrap(cmd, x) => format!("{cmd}{{{}}}", emit_seq(x)),
        Node::Fenced(open, close, x) => format!("{open} {} {close}", emit_seq(x)),
        Node::Binom(n, k) => format!("\\binom{{{}}}{{{}}}", emit_seq(n), emit_seq(k)),
        Node::Op(name) => format!("\\operatorname{{{}}}", escape_text(name)),
        Node::Brace {
            cmd,
            attach,
            content,
            label,
        } => {
            let mut s = format!("{cmd}{{{}}}", emit_seq(content));
            if let Some(l) = label {
                s.push_str(&format!("{attach}{{{}}}", emit_seq(l)));
            }
            s
        }
        Node::Matrix(env, rows) => {
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
            format!("\\begin{{{env}}}{body}\\end{{{env}}}")
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

    #[test]
    fn dotted_variants_map() {
        assert_eq!(latex("A subset.eq B"), "A \\subseteq B");
        assert_eq!(latex("x in.not A"), "x \\notin A");
        assert_eq!(latex("a dot.op b"), "a \\cdot b");
        assert_eq!(latex("1, 2, dots.c"), "1 , 2 , \\cdots");
        assert_eq!(latex("integral.cont"), "\\oint");
    }

    #[test]
    fn accents_and_styles() {
        assert_eq!(latex("dot(x)"), "\\dot{x}");
        assert_eq!(latex("ddot(x) + hat(p)"), "\\ddot{x} + \\hat{p}");
        assert_eq!(latex("arrow(v)"), "\\vec{v}");
        assert_eq!(latex("bb(R)"), "\\mathbb{R}");
        assert_eq!(latex("cal(F)"), "\\mathcal{F}");
        // Bare `dot` is still the multiplication dot.
        assert_eq!(latex("a dot b"), "a \\cdot b");
        // And bare `arrow` the arrow.
        assert_eq!(latex("a arrow b"), "a \\rightarrow b");
    }

    #[test]
    fn vectors_binomials_floors() {
        assert_eq!(latex("vec(1, 2)"), "\\begin{pmatrix}1 \\\\ 2\\end{pmatrix}");
        assert_eq!(latex("binom(n, k)"), "\\binom{n}{k}");
        assert_eq!(latex("floor(x)"), "\\left\\lfloor x \\right\\rfloor");
        assert_eq!(latex("ceil(x)"), "\\left\\lceil x \\right\\rceil");
    }

    #[test]
    fn matrix_delimiters() {
        assert_eq!(
            latex("mat(delim: \"[\", 1, 2; 3, 4)"),
            "\\begin{bmatrix}1 & 2 \\\\ 3 & 4\\end{bmatrix}"
        );
        // A ratio at the head of a cell is content, not a named argument.
        assert_eq!(latex("mat(a : b)"), "\\begin{pmatrix}a : b\\end{pmatrix}");
        assert!(to_latex("mat(delim: \"<\", 1)").is_err());
    }

    #[test]
    fn braces_and_custom_operators() {
        assert_eq!(
            latex("underbrace(a + b, \"total\")"),
            "\\underbrace{a + b}_{\\text{total}}"
        );
        assert_eq!(latex("overbrace(x)"), "\\overbrace{x}");
        assert_eq!(latex("op(\"argmax\")"), "\\operatorname{argmax}");
        assert!(to_latex("op(x)").is_err());
    }

    #[test]
    fn out_of_subset_is_loud_not_silently_wrong() {
        // An unknown dotted name must not render as symbol-dot-letters.
        let e = to_latex("subset.q").unwrap_err();
        assert!(e.message.contains("subset.q"), "{e}");
        // An unknown word used like a function must not render as letters.
        let e = to_latex("spam(x)").unwrap_err();
        assert!(e.message.contains("spam"), "{e}");
        // An accent without its argument is told what it needs.
        assert!(to_latex("hat x").is_err());
        // But a bare unknown word is still a run of variables…
        assert_eq!(latex("dx"), "d x");
        // …and a single letter applied to parens is still juxtaposition.
        assert_eq!(latex("f(x)"), "f \\left(x\\right)");
    }
}
