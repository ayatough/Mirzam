//! The golden corpus: for each Typst expression, the MathML produced through
//! the lowering must equal the MathML of the LaTeX a person would have written
//! by hand. Comparing at the MathML level rather than the LaTeX string keeps
//! the corpus honest about what the reader sees, while staying immune to
//! harmless differences in emitted spacing or brace placement.

fn mathml(tex: &str) -> String {
    let conv = math_core::LatexToMathML::new(math_core::MathCoreConfig::default())
        .expect("default math config");
    conv.convert_with_local_state(tex, math_core::MathDisplay::Block)
        .unwrap_or_else(|e| panic!("LaTeX `{tex}` did not convert: {e}"))
        .mathml
}

/// (Typst source, hand-written LaTeX that must render identically).
const CORPUS: &[(&str, &str)] = &[
    // Fractions.
    ("a/b", r"\frac{a}{b}"),
    ("1 + x/2", r"1+\frac{x}{2}"),
    ("(a + b)/(c - d)", r"\frac{a+b}{c-d}"),
    ("a/b/c", r"\frac{\frac{a}{b}}{c}"),
    // Scripts.
    ("x^2", r"x^{2}"),
    ("x_(i+1)", r"x_{i+1}"),
    ("x_i^2", r"x_{i}^{2}"),
    ("x^-1", r"x^{-1}"),
    ("e^(i pi) + 1 = 0", r"e^{i\pi}+1=0"),
    // Big operators with limits.
    (
        "sum_(i=1)^n i = (n(n+1))/2",
        r"\sum_{i=1}^{n}i=\frac{n\left(n+1\right)}{2}",
    ),
    ("integral_0^oo e^(-x) = 1", r"\int_{0}^{\infty}e^{-x}=1"),
    ("product_(k=1)^n k", r"\prod_{k=1}^{n}k"),
    (
        "lim_(x -> 0) sin(x)/x = 1",
        r"\lim_{x\to 0}\frac{\sin\left(x\right)}{x}=1",
    ),
    // Roots and bars.
    ("sqrt(x + 1)", r"\sqrt{x+1}"),
    ("root(3, x)", r"\sqrt[3]{x}"),
    ("abs(x - y)", r"\left|x-y\right|"),
    ("norm(v)", r"\left\|v\right\|"),
    // Greek by name, following Typst's glyph choices.
    ("alpha beta gamma", r"\alpha \beta \gamma"),
    ("epsilon phi", r"\varepsilon \varphi"),
    ("Gamma Delta Omega", r"\Gamma \Delta \Omega"),
    ("Alpha", r"A"),
    // Operators and relations.
    ("a -> b => c", r"a\to b\Rightarrow c"),
    ("a != b", r"a\neq b"),
    ("a <= b >= c", r"a\leq b\geq c"),
    ("x in A subset B", r"x\in A\subset B"),
    ("A union B sect C", r"A\cup B\cap C"),
    // Matrices and cases.
    (
        "mat(1, 2; 3, 4)",
        r"\begin{pmatrix}1 & 2\\ 3 & 4\end{pmatrix}",
    ),
    // The space before `x` is one the author typed beside a quoted run, so
    // Typst keeps it — writing `"if "` to buy it back is the workaround this
    // no longer needs.
    (
        "cases(x^2 &\"if\" x > 0, 0 &\"otherwise\")",
        r"\begin{cases}x^{2} & \text{if}\ x>0\\ 0 & \text{otherwise}\end{cases}",
    ),
    // Text, escapes, alignment.
    ("v = 60 \"km/h\"", r"v=60\ \text{km/h}"),
    ("a #& b", r"a\&b"),
    (
        "f(x) &= x^2 \\ g(x) &= 2x",
        r"\begin{aligned}f\left(x\right)&=x^{2}\\ g\left(x\right)&=2x\end{aligned}",
    ),
    // Physics-flavoured everyday expressions.
    ("E = m c^2", r"E=mc^{2}"),
    ("i hbar partial_t psi", r"i\hbar \partial_{t}\psi"),
    (
        "nabla times E = - partial_t B",
        r"\nabla \times E=-\partial_{t}B",
    ),
    ("x approx 3.14", r"x\approx 3.14"),
    ("p equiv q", r"p\equiv q"),
    ("a pm b", r"a\pm b"),
    // Dotted variants.
    ("A subset.eq B", r"A\subseteq B"),
    ("x in.not A", r"x\notin A"),
    ("arrow(v) dot.op arrow(w)", r"\vec{v}\cdot \vec{w}"),
    ("x_1, dots.h, x_n", r"x_{1},\dots,x_{n}"),
    (
        "integral.cont E dot.op d arrow(l)",
        r"\oint E\cdot d\vec{l}",
    ),
    // Accents and letter styles.
    ("m ddot(x) = - k x", r"m\ddot{x}=-kx"),
    ("hat(H) psi = E psi", r"\hat{H}\psi =E\psi"),
    ("x in bb(R)^n", r"x\in \mathbb{R}^{n}"),
    ("cal(L) = T - V", r"\mathcal{L}=T-V"),
    ("macron(z) z = abs(z)^2", r"\bar{z}z=\left|z\right|^{2}"),
    // Vectors, binomials, floors.
    ("vec(1, 2)", r"\begin{pmatrix}1\\ 2\end{pmatrix}"),
    (
        "binom(n, k) = n!/(k! (n-k)!)",
        r"\binom{n}{k}=\frac{n!}{k!\left(n-k\right)!}",
    ),
    (
        "floor(x) <= x <= ceil(x)",
        r"\left\lfloor x\right\rfloor \leq x\leq \left\lceil x\right\rceil",
    ),
    // Delimited matrices, braces, custom operators.
    (
        "mat(delim: \"[\", 1, 2; 3, 4)",
        r"\begin{bmatrix}1 & 2\\ 3 & 4\end{bmatrix}",
    ),
    (
        "underbrace(a + b, \"total\")",
        r"\underbrace{a+b}_{\text{total}}",
    ),
    (
        "op(\"argmax\")_theta f(theta)",
        r"\operatorname{argmax}_{\theta }f\left(\theta \right)",
    ),
    // Brackets and mixed fences stretch like parens.
    ("[a/b]", r"\left[\frac{a}{b}\right]"),
    ("x in [0, oo)", r"x\in \left[0,\infty\right)"),
    ("(0, 1]", r"\left(0,1\right]"),
    // Spacing words.
    ("a quad b", r"a\quad b"),
    ("integral f(x) thin d x", r"\int f\left(x\right)\,dx"),
    // Accents widen over more than one glyph.
    ("hat(A B)", r"\widehat{AB}"),
    ("arrow(A B)", r"\overrightarrow{AB}"),
    ("hat(x) + arrow(v)", r"\hat{x}+\vec{v}"),
    // More operators and symbols.
    ("f: x |-> x^2", r"f:x\mapsto x^{2}"),
    ("a << b >> c", r"a\ll b\gg c"),
    ("a perp b parallel c", r"a\perp b\parallel c"),
    ("A without B", r"A\setminus B"),
    ("p divides q", r"p\mid q"),
    ("therefore ell in bb(R)", r"\therefore \ell \in \mathbb{R}"),
    ("Re z + Im z", r"\Re z+\Im z"),
    ("f compose g", r"f\circ g"),
    ("f convolve g", r"f\ast g"),
    ("x tack.r y", r"x\vdash y"),
    ("angle.l u, v angle.r", r"\langle u,v\rangle"),
];

#[test]
fn corpus_matches_handwritten_latex() {
    let mut failures = Vec::new();
    for (typst, tex) in CORPUS {
        let lowered = match mirzam_tmath::to_latex(typst) {
            Ok(l) => l,
            Err(e) => {
                failures.push(format!("`{typst}` failed to parse: {e}"));
                continue;
            }
        };
        let got = mathml(&lowered);
        let want = mathml(tex);
        if got != want {
            failures.push(format!(
                "`{typst}`\n  lowered:  {lowered}\n  got:      {got}\n  expected: {want}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The printer's round-trip property, over every expression the corpus
/// knows: printing a tree and parsing it back reproduces the tree, and the
/// printed source lowers to the same LaTeX. This is what lets every edit
/// operation write text through one printer instead of inventing its own.
#[test]
fn print_round_trips_the_whole_corpus() {
    let mut failures = Vec::new();
    for (typst, _) in CORPUS {
        let tree = mirzam_tmath::parse(typst).expect("corpus entries parse");
        let printed = mirzam_tmath::print(&tree);
        match mirzam_tmath::parse(&printed) {
            Ok(reparsed) if reparsed == tree => {}
            Ok(_) => failures.push(format!(
                "`{typst}` printed as `{printed}`, which parses to a different tree"
            )),
            Err(e) => failures.push(format!(
                "`{typst}` printed as `{printed}`, which does not parse: {e}"
            )),
        }
        let direct = mirzam_tmath::to_latex(typst).expect("corpus entries lower");
        let via_print = mirzam_tmath::to_latex(&printed).unwrap_or_default();
        if direct != via_print {
            failures.push(format!(
                "`{typst}` changes meaning through the printer:\n  direct:    {direct}\n  via print: {via_print}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A parse error must never panic and must carry a message the error span can
/// show; a lowering that parses must never produce LaTeX that fails to
/// convert.
#[test]
fn everything_that_parses_converts() {
    for src in [
        "x",
        "",
        "a + b",
        "f'(x)",
        "(a, b)",
        "[a]",
        "a | b",
        "90 %",
        "a : b",
        "x!",
        "mat(1)",
        "cases(1)",
        "sqrt(2)/2",
        "x^(a/b)",
        "alpha_1",
        "\"if\" x",
    ] {
        if let Ok(lowered) = mirzam_tmath::to_latex(src) {
            let conv = math_core::LatexToMathML::new(math_core::MathCoreConfig::default())
                .expect("default math config");
            conv.convert_with_local_state(&lowered, math_core::MathDisplay::Inline)
                .unwrap_or_else(|e| panic!("`{src}` lowered to `{lowered}` which failed: {e}"));
            // The oddballs must round-trip through the printer too.
            let tree = mirzam_tmath::parse(src).expect("lowering implies parsing");
            let printed = mirzam_tmath::print(&tree);
            let reparsed = mirzam_tmath::parse(&printed)
                .unwrap_or_else(|e| panic!("`{src}` printed as `{printed}` which fails: {e}"));
            assert_eq!(reparsed, tree, "`{src}` printed as `{printed}`");
        }
    }
}
