//! Tokens to the spanned tree.

use crate::ast::{Node, NodeKind, Span};
use crate::lex::{Tok, TokKind};
use crate::words::{is_call_word, is_function_word, matrix_env, word_symbol, wrap_command};
use crate::{err, Error};

pub(crate) struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Tok>) -> Parser {
        Parser { tokens, pos: 0 }
    }

    pub fn root(&mut self) -> Result<Vec<Node>, Error> {
        let nodes = self.sequence(&[])?;
        if self.pos < self.tokens.len() {
            return err(format!(
                "unmatched `{}`",
                self.tokens[self.pos].kind.describe()
            ));
        }
        Ok(nodes)
    }

    fn peek(&self) -> Option<&TokKind> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }

    fn next(&mut self) -> Option<TokKind> {
        let t = self.tokens.get(self.pos).map(|t| t.kind.clone());
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(&TokKind::Ch(c)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// The span of everything consumed since the token at `from`.
    fn span_from(&self, from: usize) -> Span {
        let start = self.tokens.get(from).map(|t| t.span.start).unwrap_or(0);
        let end = if self.pos > from {
            self.tokens[self.pos - 1].span.end
        } else {
            start
        };
        Span { start, end }
    }

    /// Parses items until one of `stops` (or the end of input), leaving the
    /// stop token unconsumed.
    fn sequence(&mut self, stops: &[char]) -> Result<Vec<Node>, Error> {
        let mut nodes = Vec::new();
        while let Some(t) = self.peek() {
            if let TokKind::Ch(c) = t {
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
        let from = self.pos;
        let mut lhs = self.postfix()?;
        while self.eat('/') {
            let rhs = self.postfix().map_err(|_| Error {
                message: "`/` needs a right-hand side".into(),
                at: None,
            })?;
            lhs = Node::new(
                NodeKind::Frac(Box::new(ungroup(lhs)), Box::new(ungroup(rhs))),
                self.span_from(from),
            );
        }
        Ok(lhs)
    }

    /// A primary with its primes, factorials and `^`/`_` scripts attached.
    fn postfix(&mut self) -> Result<Node, Error> {
        let from = self.pos;
        let mut base = self.primary()?;
        let mut sub: Option<Box<Node>> = None;
        let mut sup: Option<Box<Node>> = None;
        loop {
            match self.peek() {
                // Primes and factorials belong to their base, so `n!/2` is a
                // fraction of n-factorial. (`!=` already lexed as one token.)
                Some(TokKind::Ch(p @ ('\'' | '!'))) if sub.is_none() && sup.is_none() => {
                    let p = *p;
                    let mark = Node::new(NodeKind::Ch(p), self.tokens[self.pos].span);
                    self.pos += 1;
                    let span = Span::cover(base.span, mark.span);
                    base = Node::new(NodeKind::Seq(vec![base, mark]), span);
                }
                Some(TokKind::Ch('^')) => {
                    self.pos += 1;
                    if sup.is_some() {
                        return err("double superscript");
                    }
                    sup = Some(Box::new(self.script_operand()?));
                }
                Some(TokKind::Ch('_')) => {
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
            base = Node::new(
                NodeKind::Script {
                    base: Box::new(base),
                    sub,
                    sup,
                },
                self.span_from(from),
            );
        }
        Ok(base)
    }

    /// The operand of `^` or `_`: one primary, with `(...)` grouping unwrapped
    /// and a leading sign allowed, so `x^-1` means what it looks like.
    fn script_operand(&mut self) -> Result<Node, Error> {
        let from = self.pos;
        let sign = match self.peek() {
            Some(TokKind::Ch(c @ ('-' | '+'))) => {
                let c = *c;
                let span = self.tokens[self.pos].span;
                self.pos += 1;
                Some(Node::new(NodeKind::Ch(c), span))
            }
            _ => None,
        };
        let node = ungroup(self.primary().map_err(|_| Error {
            message: "`^` and `_` need an operand".into(),
            at: None,
        })?);
        Ok(match sign {
            Some(s) => Node::new(NodeKind::Seq(vec![s, node]), self.span_from(from)),
            None => node,
        })
    }

    fn primary(&mut self) -> Result<Node, Error> {
        let from = self.pos;
        let Some(t) = self.next() else {
            return err("unexpected end of formula");
        };
        let kind = match t {
            TokKind::Num(n) => NodeKind::Num(n),
            TokKind::Str(s) => NodeKind::Text(s),
            TokKind::Esc(c) => NodeKind::Esc(c),
            TokKind::Sym { src, latex } => NodeKind::Sym {
                src: src.to_string(),
                latex,
            },
            TokKind::Word(w) => return self.word(w, from),
            TokKind::Ch('(') => {
                let inner = self.sequence(&[')'])?;
                if !self.eat(')') {
                    return err("unmatched `(`");
                }
                NodeKind::Paren(inner)
            }
            TokKind::Ch('&') => NodeKind::Align,
            TokKind::Ch('\\') => NodeKind::Break,
            TokKind::Ch(c @ ('/' | '^' | '_')) => {
                return err(format!("`{c}` needs something before it"))
            }
            TokKind::Ch(c @ (')' | ']')) => return err(format!("unmatched `{c}`")),
            TokKind::Ch(c) => NodeKind::Ch(c),
        };
        Ok(Node::new(kind, self.span_from(from)))
    }

    /// A word is a call, a known symbol, a variable, or — unknown and longer
    /// than one letter — a run of variables, which is what LaTeX would have
    /// made of the same letters. A name that would render as something other
    /// than what the author meant — a dotted variant this parser does not
    /// know, or an unknown word used like a function — is an error instead:
    /// a red span is honest, quietly wrong glyphs are not.
    fn word(&mut self, w: String, from: usize) -> Result<Node, Error> {
        let called = self.peek() == Some(&TokKind::Ch('('));
        // `dot` and `arrow` are both a symbol and an accent; the argument
        // list is what distinguishes `a dot b` from `dot(x)`.
        if called && is_call_word(&w) {
            self.pos += 1;
            let kind = self.call(&w)?;
            return Ok(Node::new(kind, self.span_from(from)));
        }
        if let Some(sym) = word_symbol(&w) {
            let sym_node = Node::new(
                NodeKind::Sym {
                    src: w.clone(),
                    latex: sym,
                },
                self.span_from(from),
            );
            // `sin(x)/x` is the fraction of sin(x) by x: a function name glues
            // to its argument list, so `/`, `^` and `_` treat them as one.
            if called && is_function_word(&w) {
                self.pos += 1;
                let paren_from = self.pos - 1;
                let inner = self.sequence(&[')'])?;
                if !self.eat(')') {
                    return err("unmatched `(`");
                }
                let paren = Node::new(NodeKind::Paren(inner), self.span_from(paren_from));
                return Ok(Node::new(
                    NodeKind::Seq(vec![sym_node, paren]),
                    self.span_from(from),
                ));
            }
            return Ok(sym_node);
        }
        if is_call_word(&w) {
            return err(format!("`{w}` needs parenthesised arguments"));
        }
        let span = self.span_from(from);
        let mut letters = w.chars();
        if let (Some(first), None) = (letters.next(), letters.next()) {
            return Ok(Node::new(NodeKind::Ident(first), span));
        }
        if w.contains('.') {
            return err(format!("unknown symbol `{w}`"));
        }
        if called {
            return err(format!("unknown function `{w}`"));
        }
        Ok(Node::new(
            NodeKind::Seq(
                w.chars()
                    .map(|c| Node::new(NodeKind::Ident(c), span))
                    .collect(),
            ),
            span,
        ))
    }

    /// `mat`'s `delim:` named argument, when the next tokens are one. Only
    /// recognised at the head of the argument list, so a stray ratio like
    /// `mat(a : b)` still parses as content.
    fn mat_delim(&mut self) -> Result<String, Error> {
        let head = matches!(self.peek(), Some(TokKind::Word(w)) if w == "delim")
            && self.tokens.get(self.pos + 1).map(|t| &t.kind) == Some(&TokKind::Ch(':'));
        if !head {
            return Ok("(".to_string());
        }
        self.pos += 2;
        let Some(TokKind::Str(s)) = self.next() else {
            return err("`delim:` takes a quoted delimiter, e.g. `delim: \"[\"`");
        };
        if matrix_env(&s).is_none() {
            return err(format!("`delim:` does not know `{s}`"));
        }
        // The delimiter may be the only argument, or be followed by the cells.
        if !self.eat(',') && self.peek() != Some(&TokKind::Ch(')')) {
            return err("`delim:` must be followed by `,` and the matrix cells");
        }
        Ok(s)
    }

    /// Arguments of `name(...)`, split on `,` and `;`, the opening paren
    /// already consumed.
    fn call(&mut self, name: &str) -> Result<NodeKind, Error> {
        let delim = if name == "mat" {
            self.mat_delim()?
        } else {
            "(".to_string()
        };
        let mut rows: Vec<Vec<Vec<Node>>> = vec![Vec::new()];
        loop {
            let arg = self.sequence(&[',', ';', ')'])?;
            rows.last_mut().expect("rows are non-empty").push(arg);
            match self.next() {
                Some(TokKind::Ch(',')) => {}
                Some(TokKind::Ch(';')) => rows.push(Vec::new()),
                Some(TokKind::Ch(')')) => break,
                _ => return err(format!("unmatched `(` after `{name}`")),
            }
        }
        let mut args: Vec<Vec<Node>> = rows.concat();
        let argc = args.len();
        let one = |args: &mut Vec<Vec<Node>>| args.remove(0);
        if wrap_command(name).is_some() || matches!(name, "floor" | "ceil") {
            if argc != 1 {
                return err(format!("`{name}` takes one argument"));
            }
            return Ok(NodeKind::Call {
                name: name.to_string(),
                arg: one(&mut args),
            });
        }
        Ok(match name {
            "sqrt" if argc == 1 => NodeKind::Sqrt(one(&mut args)),
            "root" if argc == 2 => {
                let idx = one(&mut args);
                NodeKind::Root(idx, one(&mut args))
            }
            "abs" if argc == 1 => NodeKind::Abs(one(&mut args)),
            "norm" if argc == 1 => NodeKind::Norm(one(&mut args)),
            "binom" if argc == 2 => {
                let n = one(&mut args);
                NodeKind::Binom(n, one(&mut args))
            }
            // An upright word the tables do not know: `op("argmax")`.
            "op" => match (argc, args.pop()) {
                (1, Some(arg)) => match arg.as_slice() {
                    [node] => match &node.kind {
                        NodeKind::Text(s) => NodeKind::Op(s.clone()),
                        _ => return err("`op` takes a quoted name, e.g. `op(\"argmax\")`"),
                    },
                    _ => return err("`op` takes a quoted name, e.g. `op(\"argmax\")`"),
                },
                _ => return err("`op` takes a quoted name, e.g. `op(\"argmax\")`"),
            },
            "underbrace" | "overbrace" if argc == 1 || argc == 2 => {
                let content = one(&mut args);
                NodeKind::Brace {
                    name: name.to_string(),
                    content,
                    label: args.pop(),
                }
            }
            // A column vector; `mat` covers anything wider.
            "vec" => NodeKind::Matrix {
                delim,
                rows: args.into_iter().map(|e| vec![e]).collect(),
            },
            "mat" => NodeKind::Matrix { delim, rows },
            // Each `cases` argument is one row, its cells split on `&`.
            "cases" => NodeKind::Cases(args.into_iter().map(split_on_align).collect()),
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
/// A group of one thing *is* that thing — normalising here is what lets a
/// reparse of printed output reproduce an editor-built tree exactly.
fn ungroup(node: Node) -> Node {
    match node.kind {
        NodeKind::Paren(mut inner) => match inner.len() {
            1 => inner.pop().expect("length checked"),
            _ => Node::new(NodeKind::Seq(inner), node.span),
        },
        _ => node,
    }
}

fn split_on_align(nodes: Vec<Node>) -> Vec<Vec<Node>> {
    let mut cells = vec![Vec::new()];
    for n in nodes {
        if n.kind == NodeKind::Align {
            cells.push(Vec::new());
        } else {
            cells.last_mut().expect("cells are non-empty").push(n);
        }
    }
    cells
}
