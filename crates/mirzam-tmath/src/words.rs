//! The vocabulary: which words are symbols, which are functions, which take
//! arguments. Shared by the parser (to classify) and the LaTeX emitter (to
//! lower), so a name cannot be parseable but unloweraable or vice versa.

/// Symbol and function names the parser knows. Greek follows Typst's glyphs
/// (`epsilon` is ε, `phi` is φ), so a deck reads the same in both tools.
/// Capital Greek letters without a LaTeX command are the Latin letters they
/// look like.
pub(crate) fn word_symbol(w: &str) -> Option<&'static str> {
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
        // Spacing.
        "quad" => "\\quad",
        "wide" => "\\qquad",
        "thin" => "\\,",
        "med" => "\\:",
        "thick" => "\\;",
        // Symbols.
        "infinity" | "oo" => "\\infty",
        "ell" => "\\ell",
        "Re" => "\\Re",
        "Im" => "\\Im",
        "aleph" => "\\aleph",
        "angle" => "\\angle",
        "angle.l" => "\\langle",
        "angle.r" => "\\rangle",
        "brace.l" => "\\{",
        "brace.r" => "\\}",
        "degree" => "\\degree",
        "star" | "star.op" => "\\star",
        "dagger" => "\\dagger",
        "dagger.double" => "\\ddagger",
        "compose" => "\\circ",
        "convolve" => "\\ast",
        "without" => "\\setminus",
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
        "arrow.r.bar" => "\\mapsto",
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
        "lt.double" => "\\ll",
        "gt.double" => "\\gg",
        "perp" => "\\perp",
        "parallel" => "\\parallel",
        "divides" => "\\mid",
        "divides.not" => "\\nmid",
        "therefore" => "\\therefore",
        "because" => "\\because",
        "top" => "\\top",
        "bot" => "\\bot",
        "models" => "\\models",
        "tack.r" => "\\vdash",
        "tack.l" => "\\dashv",
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
pub(crate) fn is_call_word(w: &str) -> bool {
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
            | "op"
            | "underbrace"
            | "overbrace"
            | "floor"
            | "ceil"
    ) || wrap_command(w).is_some()
}

/// One-argument wrappers lowered to a single LaTeX command: accents, letter
/// styles, `cancel`. `arrow(v)` is the vector accent; bare `arrow` the symbol.
pub(crate) fn wrap_command(w: &str) -> Option<&'static str> {
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
        // Struck through.
        "cancel" => "\\cancel",
        _ => return None,
    })
}

/// The delimiter pair for the fenced one-argument calls.
pub(crate) fn fence_pair(w: &str) -> Option<(&'static str, &'static str)> {
    Some(match w {
        "floor" => ("\\left\\lfloor", "\\right\\rfloor"),
        "ceil" => ("\\left\\lceil", "\\right\\rceil"),
        _ => return None,
    })
}

/// Upright function names, which glue to a following `(...)` argument list.
pub(crate) fn is_function_word(w: &str) -> bool {
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

/// The environment `mat`'s `delim:` names, or `None` for one it does not.
pub(crate) fn matrix_env(delim: &str) -> Option<&'static str> {
    Some(match delim {
        "(" => "pmatrix",
        "[" => "bmatrix",
        "{" => "Bmatrix",
        "|" => "vmatrix",
        "||" => "Vmatrix",
        _ => return None,
    })
}
