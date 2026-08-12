//! The tree between the parser and everything else.
//!
//! Every node records the byte range of source it came from, because an
//! editor's selection is a node, a node is a range, and a range is something
//! that can be replaced in text. Equality deliberately ignores spans: where a
//! node came from is not part of what it is, and the printer's round-trip
//! property (`parse(print(tree)) == tree`) would be unstatable otherwise.

/// A byte range in the source a node came from. Nodes made by edit
/// operations rather than the parser carry [`Span::EMPTY`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const EMPTY: Span = Span { start: 0, end: 0 };

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// The smallest span containing both, ignoring empty (synthetic) spans.
    pub fn cover(a: Span, b: Span) -> Span {
        match (a.is_empty(), b.is_empty()) {
            (true, _) => b,
            (_, true) => a,
            _ => Span {
                start: a.start.min(b.start),
                end: a.end.max(b.end),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
}

/// Structural equality only; see the module comment.
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Node {
    pub fn new(kind: NodeKind, span: Span) -> Node {
        Node { kind, span }
    }

    /// A node with no source of its own — the product of an edit operation.
    pub fn synthetic(kind: NodeKind) -> Node {
        Node {
            kind,
            span: Span::EMPTY,
        }
    }

    /// Direct children, in a stable order that edit paths index into.
    /// Matrix and cases cells are flattened in reading order.
    pub fn children(&self) -> Vec<&Node> {
        match &self.kind {
            NodeKind::Frac(a, b) => vec![a, b],
            NodeKind::Binom(n, k) => n.iter().chain(k.iter()).collect(),
            NodeKind::Script { base, sub, sup } => {
                let mut v = vec![&**base];
                v.extend(sub.iter().map(|b| &**b));
                v.extend(sup.iter().map(|b| &**b));
                v
            }
            NodeKind::Root(n, x) => n.iter().chain(x.iter()).collect(),
            NodeKind::Brace { content, label, .. } => {
                content.iter().chain(label.iter().flatten()).collect()
            }
            NodeKind::Matrix { rows, .. } | NodeKind::Cases(rows) => {
                rows.iter().flatten().flatten().collect()
            }
            NodeKind::Seq(inner)
            | NodeKind::Paren(inner)
            | NodeKind::Fence { inner, .. }
            | NodeKind::Sqrt(inner)
            | NodeKind::Abs(inner)
            | NodeKind::Norm(inner)
            | NodeKind::Call { arg: inner, .. } => inner.iter().collect(),
            _ => Vec::new(),
        }
    }

    /// [`Node::children`], mutably, in the same order.
    pub fn children_mut(&mut self) -> Vec<&mut Node> {
        match &mut self.kind {
            NodeKind::Frac(a, b) => vec![a, b],
            NodeKind::Binom(n, k) => n.iter_mut().chain(k.iter_mut()).collect(),
            NodeKind::Script { base, sub, sup } => {
                let mut v = vec![&mut **base];
                v.extend(sub.iter_mut().map(|b| &mut **b));
                v.extend(sup.iter_mut().map(|b| &mut **b));
                v
            }
            NodeKind::Root(n, x) => n.iter_mut().chain(x.iter_mut()).collect(),
            NodeKind::Brace { content, label, .. } => content
                .iter_mut()
                .chain(label.iter_mut().flatten())
                .collect(),
            NodeKind::Matrix { rows, .. } | NodeKind::Cases(rows) => {
                rows.iter_mut().flatten().flatten().collect()
            }
            NodeKind::Seq(inner)
            | NodeKind::Paren(inner)
            | NodeKind::Fence { inner, .. }
            | NodeKind::Sqrt(inner)
            | NodeKind::Abs(inner)
            | NodeKind::Norm(inner)
            | NodeKind::Call { arg: inner, .. } => inner.iter_mut().collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// A number, possibly with a decimal point.
    Num(String),
    /// A single italic letter.
    Ident(char),
    /// A named symbol: how the source spelt it, and the LaTeX it lowers to.
    /// The spelling is kept so the printer writes `oo` back as `oo`, not as
    /// some canonical alias.
    Sym {
        src: String,
        latex: &'static str,
    },
    /// A character passed through unchanged.
    Ch(char),
    /// `"literal text"`, without the quotes.
    Text(String),
    /// `#c`: the character `c`, stripped of any meaning it had.
    Esc(char),
    /// Juxtaposed nodes with no delimiter of their own.
    Seq(Vec<Node>),
    /// `(...)` that the author wants printed, stretchy.
    Paren(Vec<Node>),
    /// A delimited pair other than plain parens: `[a, b]`, and the mixed
    /// pairs intervals need — `[0, oo)`, `(0, 1]`. Stretchy, never grouping.
    Fence {
        open: char,
        close: char,
        inner: Vec<Node>,
    },
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
    /// One argument in one named wrapper: accents, letter styles, `cancel`,
    /// `floor`, `ceil`. The Typst name is kept; LaTeX is derived at emission.
    Call {
        name: String,
        arg: Vec<Node>,
    },
    Binom(Vec<Node>, Vec<Node>),
    /// `op("argmax")`: an upright operator the tables do not know.
    Op(String),
    /// `underbrace(x, "label")` and `overbrace`.
    Brace {
        name: String,
        content: Vec<Node>,
        label: Option<Vec<Node>>,
    },
    /// Rows of cells with a delimiter choice: `mat(1, 2; 3, 4)`,
    /// `mat(delim: "[", …)`. `vec(a, b)` parses to single-cell rows.
    Matrix {
        delim: String,
        rows: Vec<Vec<Vec<Node>>>,
    },
    /// Rows, each split on `&`: `cases(x &"if" y, ...)`.
    Cases(Vec<Vec<Vec<Node>>>),
    /// `&`, meaningful at the top level and inside `cases`.
    Align,
    /// `\`, a line break at the top level.
    Break,
}
