//! The spanned tree, lowered to LaTeX.

use crate::ast::{Node, NodeKind};
use crate::words::{fence_pair, matrix_env, wrap_command};

/// Emits the whole formula. Top-level `&` or `\` means the author is aligning
/// equations, which LaTeX only allows inside an environment — `aligned` works
/// in both display and inline math.
pub(crate) fn emit_root(nodes: &[Node]) -> String {
    let aligned = nodes
        .iter()
        .any(|n| matches!(n.kind, NodeKind::Align | NodeKind::Break));
    if !aligned {
        return emit_seq(nodes);
    }
    let rows: Vec<String> = nodes
        .split(|n| n.kind == NodeKind::Break)
        .map(|row| {
            row.split(|n| n.kind == NodeKind::Align)
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
    match &node.kind {
        NodeKind::Num(n) => n.clone(),
        NodeKind::Ident(c) => c.to_string(),
        NodeKind::Sym { latex, .. } => latex.to_string(),
        NodeKind::Ch(c) => c.to_string(),
        NodeKind::Text(s) => format!("\\text{{{}}}", escape_text(s)),
        NodeKind::Esc(c) => escape_char(*c),
        NodeKind::Seq(inner) => emit_seq(inner),
        NodeKind::Paren(inner) => format!("\\left({}\\right)", emit_seq(inner)),
        NodeKind::Frac(a, b) => format!("\\frac{{{}}}{{{}}}", emit(a), emit(b)),
        NodeKind::Script { base, sub, sup } => {
            // Braces around a one-token base change more than grouping:
            // `{\sum}` is an ordinary atom that has lost its movable limits,
            // and even `{e}` shifts the spacing of the operator before it.
            // Brace only what actually needs holding together.
            let mut s = match &base.kind {
                NodeKind::Sym { latex, .. } => latex.to_string(),
                NodeKind::Ident(c) | NodeKind::Ch(c) => c.to_string(),
                NodeKind::Num(n) if n.chars().count() == 1 => n.clone(),
                NodeKind::Op(_) | NodeKind::Call { .. } => emit(base),
                _ => format!("{{{}}}", emit(base)),
            };
            if let Some(b) = sub {
                s.push_str(&format!("_{{{}}}", emit(b)));
            }
            if let Some(p) = sup {
                s.push_str(&format!("^{{{}}}", emit(p)));
            }
            s
        }
        NodeKind::Sqrt(x) => format!("\\sqrt{{{}}}", emit_seq(x)),
        NodeKind::Root(n, x) => format!("\\sqrt[{}]{{{}}}", emit_seq(n), emit_seq(x)),
        NodeKind::Abs(x) => format!("\\left|{}\\right|", emit_seq(x)),
        NodeKind::Norm(x) => format!("\\left\\|{}\\right\\|", emit_seq(x)),
        NodeKind::Call { name, arg } => match wrap_command(name) {
            Some(cmd) => format!("{cmd}{{{}}}", emit_seq(arg)),
            None => {
                let (open, close) = fence_pair(name).expect("call names are known");
                format!("{open} {} {close}", emit_seq(arg))
            }
        },
        NodeKind::Binom(n, k) => format!("\\binom{{{}}}{{{}}}", emit_seq(n), emit_seq(k)),
        NodeKind::Op(name) => format!("\\operatorname{{{}}}", escape_text(name)),
        NodeKind::Brace {
            name,
            content,
            label,
        } => {
            let (cmd, attach) = if name == "underbrace" {
                ("\\underbrace", "_")
            } else {
                ("\\overbrace", "^")
            };
            let mut s = format!("{cmd}{{{}}}", emit_seq(content));
            if let Some(l) = label {
                s.push_str(&format!("{attach}{{{}}}", emit_seq(l)));
            }
            s
        }
        NodeKind::Matrix { delim, rows } => {
            let env = matrix_env(delim).expect("delims are validated at parse");
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
        NodeKind::Cases(rows) => {
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
        NodeKind::Align => "\\&".into(),
        NodeKind::Break => "\\\\".into(),
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
