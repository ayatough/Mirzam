//! The spanned tree, written back as Typst-math source — the inverse of the
//! parser. Every edit operation goes through here, so there is exactly one
//! way the crate writes text.
//!
//! The property that keeps this honest, tested over the whole corpus:
//! `parse(print(tree)) == tree`. The printed spelling is canonical rather
//! than byte-identical to the original — `_` before `^`, one space between
//! items — but symbol spellings the author chose (`oo` vs `infinity`) are
//! kept, because the node remembers them.

use crate::ast::{Node, NodeKind};

pub(crate) fn print_root(nodes: &[Node]) -> String {
    print_seq(nodes)
}

fn print_seq(nodes: &[Node]) -> String {
    nodes.iter().map(print).collect::<Vec<_>>().join(" ")
}

/// An inner sequence: a run of single letters came from one word (`dx`) and
/// must go back as one, or it would reparse as separate items of the parent
/// sequence. An empty one is an editor placeholder and prints as `()`.
fn print_inner_seq(nodes: &[Node]) -> String {
    if nodes.is_empty() {
        return "()".into();
    }
    if nodes.len() > 1
        && nodes
            .iter()
            .all(|n| matches!(n.kind, NodeKind::Ident(_) | NodeKind::Ch('\'' | '!')))
    {
        return nodes
            .iter()
            .map(|n| match n.kind {
                NodeKind::Ident(c) | NodeKind::Ch(c) => c.to_string(),
                _ => unreachable!("filtered above"),
            })
            .collect();
    }
    print_seq(nodes)
}

/// A fraction operand: already a single postfix unit unless it is a grouped
/// sequence, which needs its parens back. A fraction on the right needs them
/// too, or `a/(b/c)` would flatten into left association.
fn print_frac_operand(node: &Node, rhs: bool) -> String {
    match &node.kind {
        NodeKind::Seq(inner) => grouped(inner),
        NodeKind::Frac(..) if rhs => format!("({})", print(node)),
        _ => print(node),
    }
}

/// A grouped sequence in an operand position: back into its parens — unless
/// it is a letter run, which stands alone, or a placeholder, whose parens
/// are the whole of it.
fn grouped(inner: &[Node]) -> String {
    if inner.is_empty() {
        return "()".into();
    }
    match only_letters(inner) {
        Some(word) => word,
        None => format!("({})", print_inner_seq(inner)),
    }
}

/// A script operand: the parser takes exactly one primary here, so anything
/// composite needs parens to survive the round trip.
fn print_script_operand(node: &Node) -> String {
    match &node.kind {
        NodeKind::Seq(inner) => grouped(inner),
        NodeKind::Frac(..) | NodeKind::Script { .. } => format!("({})", print(node)),
        _ => print(node),
    }
}

/// The word a run of letters came from, when the sequence is only that run —
/// and not a run that would reparse as a known name (`p` `i` must not print
/// as `pi`).
fn only_letters(nodes: &[Node]) -> Option<String> {
    if nodes.len() < 2 {
        return None;
    }
    let word: String = nodes
        .iter()
        .map(|n| match n.kind {
            NodeKind::Ident(c) => Some(c),
            _ => None,
        })
        .collect::<Option<String>>()?;
    if crate::words::word_symbol(&word).is_some() || crate::words::is_call_word(&word) {
        return None;
    }
    Some(word)
}

fn print(node: &Node) -> String {
    match &node.kind {
        NodeKind::Num(n) => n.clone(),
        NodeKind::Ident(c) => c.to_string(),
        NodeKind::Sym { src, .. } => src.clone(),
        NodeKind::Ch(c) => c.to_string(),
        NodeKind::Text(s) => format!("\"{s}\""),
        NodeKind::Esc(c) => format!("#{c}"),
        NodeKind::Seq(inner) => print_inner_seq(inner),
        NodeKind::Paren(inner) => format!("({})", print_seq(inner)),
        NodeKind::Fence { open, close, inner } => {
            format!("{open}{}{close}", print_seq(inner))
        }
        NodeKind::Frac(a, b) => format!(
            "{}/{}",
            print_frac_operand(a, false),
            print_frac_operand(b, true)
        ),
        NodeKind::Script { base, sub, sup } => {
            let mut s = match &base.kind {
                // A grouped base needs parens back — except the glue shapes
                // postfix builds itself: primes, factorials, a function with
                // its arguments, a letter run.
                NodeKind::Seq(inner) => print_inner_seq(inner),
                _ => print(base),
            };
            if let Some(b) = sub {
                s.push('_');
                s.push_str(&print_script_operand(b));
            }
            if let Some(p) = sup {
                s.push('^');
                s.push_str(&print_script_operand(p));
            }
            s
        }
        NodeKind::Sqrt(x) => format!("sqrt({})", print_seq(x)),
        NodeKind::Root(n, x) => format!("root({}, {})", print_seq(n), print_seq(x)),
        NodeKind::Abs(x) => format!("abs({})", print_seq(x)),
        NodeKind::Norm(x) => format!("norm({})", print_seq(x)),
        NodeKind::Call { name, arg } => format!("{name}({})", print_seq(arg)),
        NodeKind::Binom(n, k) => format!("binom({}, {})", print_seq(n), print_seq(k)),
        NodeKind::Op(name) => format!("op(\"{name}\")"),
        NodeKind::Brace {
            name,
            content,
            label,
        } => match label {
            Some(l) => format!("{name}({}, {})", print_seq(content), print_seq(l)),
            None => format!("{name}({})", print_seq(content)),
        },
        NodeKind::Matrix { delim, rows } => {
            let body = rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|c| print_seq(c))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .collect::<Vec<_>>()
                .join("; ");
            if delim == "(" {
                format!("mat({body})")
            } else {
                format!("mat(delim: \"{delim}\", {body})")
            }
        }
        NodeKind::Cases(rows) => {
            let body = rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|c| print_seq(c))
                        .collect::<Vec<_>>()
                        .join(" & ")
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("cases({body})")
        }
        NodeKind::Align => "&".into(),
        NodeKind::Break => "\\".into(),
    }
}
