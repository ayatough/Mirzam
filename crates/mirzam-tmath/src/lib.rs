//! Typst-flavoured math: source text to LaTeX, and a tree an editor can hold.
//!
//! LaTeX is hard to write from memory; Typst's math syntax is not. This crate
//! parses the useful subset of that syntax and lowers it to LaTeX, which then
//! goes through the same `math-core` path every formula already takes — so
//! spacing, stretchy delimiters and font handling are shared with the LaTeX
//! front end rather than reimplemented. The AST in the middle is the seam:
//! [`parse`] exposes it with source spans, [`print`] writes it back as
//! Typst-math source, and [`edit`] transforms it — which together are what a
//! structural formula editor builds on.
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

mod ast;
pub mod edit;
mod latex;
mod lex;
mod parse;
mod print;
mod words;

pub use ast::{Node, NodeKind, Span};

/// Why a formula failed to parse. The renderer shows the source with this
/// message in the tooltip, the same way a broken LaTeX formula is shown.
/// `at` is the byte offset of the problem, when the failing stage knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
    pub at: Option<usize>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "typst math: {}", self.message)
    }
}

impl std::error::Error for Error {}

pub(crate) fn err<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error {
        message: message.into(),
        at: None,
    })
}

pub(crate) fn err_at<T>(message: impl Into<String>, at: usize) -> Result<T, Error> {
    Err(Error {
        message: message.into(),
        at: Some(at),
    })
}

/// Parses one Typst math expression into its tree, spans included.
pub fn parse(src: &str) -> Result<Vec<Node>, Error> {
    parse::Parser::new(lex::lex(src)?).root()
}

/// Writes a tree back as Typst-math source. The one writer the crate has:
/// `parse(&print(&tree))` reproduces `tree`, a property the corpus holds it
/// to, so edit operations compose with parsing instead of inventing text.
pub fn print(nodes: &[Node]) -> String {
    print::print_root(nodes)
}

/// Converts one Typst math expression to LaTeX.
///
/// Alignment (`&`) and line breaks (`\`) at the top level wrap the result in
/// an `aligned` environment, which works in both display and inline math.
pub fn to_latex(src: &str) -> Result<String, Error> {
    Ok(latex::emit_root(&parse(src)?))
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
    fn brackets_and_intervals_are_fences() {
        assert_eq!(latex("[a/b]"), "\\left[\\frac{a}{b}\\right]");
        assert_eq!(latex("[0, oo)"), "\\left[0 , \\infty\\right)");
        assert_eq!(latex("(0, 1]"), "\\left(0 , 1\\right]");
        assert!(to_latex("[a").is_err());
        assert!(to_latex("a]").is_err());
    }

    #[test]
    fn spacing_words() {
        assert_eq!(latex("a quad b"), "a \\quad b");
        assert_eq!(latex("a wide b"), "a \\qquad b");
        assert_eq!(latex("d thin x"), "d \\, x");
    }

    #[test]
    fn accents_widen_over_words() {
        assert_eq!(latex("hat(x)"), "\\hat{x}");
        assert_eq!(latex("hat(A B)"), "\\widehat{A B}");
        assert_eq!(latex("arrow(A B)"), "\\overrightarrow{A B}");
        // No wide tilde in the renderer; the narrow one is still right for
        // one glyph and least wrong for more.
        assert_eq!(latex("tilde(x y)"), "\\tilde{x y}");
    }

    #[test]
    fn more_operators_map() {
        assert_eq!(latex("x |-> x^2"), "x \\mapsto x^{2}");
        assert_eq!(latex("a << b >> c"), "a \\ll b \\gg c");
        assert_eq!(latex("a | b"), "a | b");
        assert_eq!(latex("A without B"), "A \\setminus B");
        assert_eq!(latex("a perp b"), "a \\perp b");
        assert_eq!(latex("p divides.not q"), "p \\nmid q");
        assert_eq!(latex("theta degree"), "\\theta \\degree");
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

    #[test]
    fn nothing_empty_reaches_a_fraction_or_a_script() {
        // `a \/ b` is a Typst author's escaped slash; here `\` is a line
        // break, so it used to parse as a fraction with an empty numerator.
        // That renders as a gap on its own and panics `math-core` inside a
        // matrix cell, which loses the whole deck — so all three matrix-like
        // constructs are held to it.
        for src in [
            "mat(a \\/ b, c; d, e)",
            "cases(a \\/ b, c)",
            "vec(a \\/ b, c)",
            "a \\/ b",
        ] {
            let e = to_latex(src).unwrap_err();
            assert!(e.message.contains("line break"), "{src}: {e}");
        }
        // `&` is structure too, and a matrix cell is where it turns up.
        assert!(to_latex("mat(&/b, c)").is_err());
        assert!(to_latex("x^\\").is_err());
        // An empty group is a hole the formula editor puts there on purpose,
        // so it stays parseable — it lowers to an empty LaTeX group, which
        // `math-core` renders as a gap rather than refusing.
        assert_eq!(latex("x^()"), "x^{}");
        // The escape this subset does have still divides nothing.
        assert_eq!(
            latex("mat(a #/ b, c)"),
            "\\begin{pmatrix}a / b & c\\end{pmatrix}"
        );
    }

    #[test]
    fn percent_survives_into_the_output() {
        // A bare `%` opens a LaTeX comment, which used to swallow the rest of
        // the formula with no error and no warning: `99% "of the mass"` came
        // out as `99`.
        assert_eq!(latex("99% \"of it\""), "99 \\% \\text{of it}");
        assert_eq!(latex("p = 5%"), "p = 5 \\%");
    }

    #[test]
    fn typst_names_a_deck_reaches_for() {
        assert_eq!(
            latex("integral f(s) dif s"),
            "\\int f \\left(s\\right) \\mathrm{d} s"
        );
        assert_eq!(latex("EE[x]"), "\\mathbb{E} \\left[x\\right]");
        assert_eq!(latex("RR^n"), "\\mathbb{R}^{n}");
        assert_eq!(latex("A space B"), "A \\  B");
    }

    #[test]
    fn a_longer_unknown_name_is_an_error_not_italic_letters() {
        // `dif s` as four italic letters reads as a typo the author made,
        // not as a tool that does not know the word.
        let e = to_latex("f(s) dfi s").unwrap_err();
        assert!(e.message.contains("dfi"), "{e}");
        assert!(e.message.contains("d f i"), "{e}");
        // Two letters are the product a LaTeX author writes the same way.
        assert_eq!(latex("dx"), "d x");
    }

    #[test]
    fn spans_point_back_into_the_source() {
        let src = "a + sqrt(x)";
        let ast = parse(src).unwrap();
        assert_eq!(&src[ast[0].span.start..ast[0].span.end], "a");
        assert_eq!(&src[ast[2].span.start..ast[2].span.end], "sqrt(x)");
        // Equality ignores spans: the same formula from different offsets
        // is the same tree.
        assert_eq!(parse("  a +   sqrt(x)").unwrap(), ast);
    }

    #[test]
    fn errors_carry_a_position_when_the_lexer_finds_them() {
        let e = parse("a + \"open").unwrap_err();
        assert_eq!(e.at, Some(4));
    }
}
