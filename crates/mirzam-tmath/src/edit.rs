//! Structural edit operations: the verbs a formula editor needs, as pure
//! tree transformations. No operation writes text — after editing,
//! [`crate::print`] turns the tree back into Typst-math source, so there is
//! exactly one writer.
//!
//! A node is addressed by a **path**: indices into [`crate::Node::children`]
//! starting from the root sequence. Paths are ephemeral — an edit may
//! invalidate every path into the tree, and a UI is expected to re-derive
//! them from the tree it just received.
//!
//! Empty slots an edit leaves behind — the denominator of a fraction that
//! was just wrapped, the exponent that was just attached — are
//! [`placeholder`] nodes, printed as `()`, for the UI to render as a hole to
//! drop something into.
//!
//! Operations return `false` (or `None`) when the path does not resolve or
//! the operation does not apply there; the tree is left unchanged.

use crate::ast::{Node, NodeKind};

/// Which script slot [`attach_script`] opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptSlot {
    Sub,
    Sup,
}

/// An empty slot: prints as `()`, renders as a hole.
pub fn placeholder() -> Node {
    Node::synthetic(NodeKind::Seq(Vec::new()))
}

/// Whether a node is such a hole. `()` parses as an empty `Paren`, so both
/// empty shapes count.
pub fn is_placeholder(node: &Node) -> bool {
    matches!(&node.kind, NodeKind::Seq(v) | NodeKind::Paren(v) if v.is_empty())
}

/// The node at `path`, if the path resolves.
pub fn node_at<'a>(root: &'a [Node], path: &[usize]) -> Option<&'a Node> {
    let (&first, rest) = path.split_first()?;
    let mut node = root.get(first)?;
    for &i in rest {
        let mut kids = node.children();
        if i >= kids.len() {
            return None;
        }
        node = kids.swap_remove(i);
    }
    Some(node)
}

fn node_at_mut<'a>(root: &'a mut [Node], path: &[usize]) -> Option<&'a mut Node> {
    let (&first, rest) = path.split_first()?;
    let mut node = root.get_mut(first)?;
    for &i in rest {
        let mut kids = node.children_mut();
        if i >= kids.len() {
            return None;
        }
        node = kids.swap_remove(i);
    }
    Some(node)
}

/// The node becomes the numerator of a fraction with a hole underneath.
pub fn wrap_in_fraction(root: &mut [Node], path: &[usize]) -> bool {
    let Some(node) = node_at_mut(root, path) else {
        return false;
    };
    let old = std::mem::replace(node, placeholder());
    *node = Node::synthetic(NodeKind::Frac(Box::new(old), Box::new(placeholder())));
    true
}

/// The node gains an empty subscript or superscript slot. If it already
/// carries scripts, the missing slot is opened; a slot that is already
/// there is left alone and the call reports `false`.
pub fn attach_script(root: &mut [Node], path: &[usize], slot: ScriptSlot) -> bool {
    let Some(node) = node_at_mut(root, path) else {
        return false;
    };
    if let NodeKind::Script { sub, sup, .. } = &mut node.kind {
        let target = match slot {
            ScriptSlot::Sub => sub,
            ScriptSlot::Sup => sup,
        };
        if target.is_some() {
            return false;
        }
        *target = Some(Box::new(placeholder()));
        return true;
    }
    let old = std::mem::replace(node, placeholder());
    // The source syntax cannot script a bare fraction — `a/b^2` binds the
    // script to `b` — so the base gets its parens here, as an author would
    // write them.
    let base = match old.kind {
        NodeKind::Frac(..) => Node::synthetic(NodeKind::Paren(vec![old])),
        _ => old,
    };
    let (sub, sup) = match slot {
        ScriptSlot::Sub => (Some(Box::new(placeholder())), None),
        ScriptSlot::Sup => (None, Some(Box::new(placeholder()))),
    };
    *node = Node::synthetic(NodeKind::Script {
        base: Box::new(base),
        sub,
        sup,
    });
    true
}

/// The node becomes the argument of a one-argument call: `sqrt`, `abs`,
/// `norm`, `floor`, `ceil`, an accent or a letter style.
pub fn wrap_call(root: &mut [Node], path: &[usize], name: &str) -> bool {
    let known = matches!(name, "sqrt" | "abs" | "norm" | "floor" | "ceil")
        || crate::words::wrap_command(name).is_some();
    if !known {
        return false;
    }
    let Some(node) = node_at_mut(root, path) else {
        return false;
    };
    let old = std::mem::replace(node, placeholder());
    *node = Node::synthetic(match name {
        "sqrt" => NodeKind::Sqrt(vec![old]),
        "abs" => NodeKind::Abs(vec![old]),
        "norm" => NodeKind::Norm(vec![old]),
        _ => NodeKind::Call {
            name: name.to_string(),
            arg: vec![old],
        },
    });
    true
}

/// The node at `path` becomes `kind` — how a symbol lands in a hole.
pub fn replace(root: &mut [Node], path: &[usize], kind: NodeKind) -> bool {
    let Some(node) = node_at_mut(root, path) else {
        return false;
    };
    *node = Node::synthetic(kind);
    true
}

/// Removes the node and returns it, so a UI can move it somewhere else.
/// From a sequence-shaped parent the node is taken out; from a fixed slot
/// (a fraction operand, a script base) a hole is left in its place; a
/// script's own sub or sup slot disappears entirely.
pub fn delete(root: &mut Vec<Node>, path: &[usize]) -> Option<Node> {
    let (&last, parent_path) = path.split_last()?;
    if parent_path.is_empty() {
        if last >= root.len() {
            return None;
        }
        return Some(root.remove(last));
    }
    let parent = node_at_mut(root, parent_path)?;
    match &mut parent.kind {
        NodeKind::Seq(v)
        | NodeKind::Paren(v)
        | NodeKind::Sqrt(v)
        | NodeKind::Abs(v)
        | NodeKind::Norm(v)
        | NodeKind::Call { arg: v, .. } => {
            if last >= v.len() {
                return None;
            }
            Some(v.remove(last))
        }
        NodeKind::Frac(a, b) => {
            let slot = match last {
                0 => a,
                1 => b,
                _ => return None,
            };
            Some(std::mem::replace(&mut **slot, placeholder()))
        }
        NodeKind::Script { base, sub, sup } => {
            // Child order is base, then sub if present, then sup.
            let mut index = last;
            if index == 0 {
                return Some(std::mem::replace(&mut **base, placeholder()));
            }
            index -= 1;
            if let Some(s) = sub {
                if index == 0 {
                    let taken = std::mem::replace(&mut **s, placeholder());
                    *sub = None;
                    return Some(taken);
                }
                index -= 1;
            }
            if index == 0 {
                if let Some(s) = sup {
                    let taken = std::mem::replace(&mut **s, placeholder());
                    *sup = None;
                    return Some(taken);
                }
            }
            None
        }
        _ => None,
    }
}

/// Inserts `node` at `index` of a sequence-shaped container: the root when
/// `parent_path` is empty, else the container at that path.
pub fn insert(root: &mut Vec<Node>, parent_path: &[usize], index: usize, node: Node) -> bool {
    if parent_path.is_empty() {
        if index > root.len() {
            return false;
        }
        root.insert(index, node);
        return true;
    }
    let Some(parent) = node_at_mut(root, parent_path) else {
        return false;
    };
    match &mut parent.kind {
        NodeKind::Seq(v)
        | NodeKind::Paren(v)
        | NodeKind::Sqrt(v)
        | NodeKind::Abs(v)
        | NodeKind::Norm(v)
        | NodeKind::Call { arg: v, .. } => {
            if index > v.len() {
                return false;
            }
            v.insert(index, node);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse, print};

    /// The flow the workstream brief opens with: type `a b c`, then place
    /// the structure — c becomes a superscript of b, b^c a subscript of a.
    #[test]
    fn a_b_c_becomes_a_sub_b_sup_c() {
        let mut ast = parse("a b c").unwrap();
        let c = delete(&mut ast, &[2]).unwrap();
        let b = delete(&mut ast, &[1]).unwrap();

        assert!(attach_script(&mut ast, &[0], ScriptSlot::Sub));
        assert_eq!(print(&ast), "a_()");
        // The sub slot is child 1 of the script node.
        assert!(replace(&mut ast, &[0, 1], b.kind));
        assert_eq!(print(&ast), "a_b");

        assert!(attach_script(&mut ast, &[0, 1], ScriptSlot::Sup));
        assert!(replace(&mut ast, &[0, 1, 1], c.kind));
        assert_eq!(print(&ast), "a_(b^c)");

        // And the result parses back to the same tree.
        assert_eq!(parse(&print(&ast)).unwrap(), ast);
    }

    #[test]
    fn wrapping_in_a_fraction_leaves_a_hole() {
        let mut ast = parse("x + 1").unwrap();
        assert!(wrap_in_fraction(&mut ast, &[0]));
        assert_eq!(print(&ast), "x/() + 1");
        assert!(is_placeholder(node_at(&ast, &[0, 1]).unwrap()));

        assert!(replace(
            &mut ast,
            &[0, 1],
            parse("2").unwrap().remove(0).kind
        ));
        assert_eq!(print(&ast), "x/2 + 1");
    }

    #[test]
    fn scripting_a_fraction_parenthesises_it() {
        let mut ast = parse("a/b").unwrap();
        assert!(attach_script(&mut ast, &[0], ScriptSlot::Sup));
        assert_eq!(print(&ast), "(a/b)^()");
        // What cannot be written cannot come back from a reparse; the
        // printed form must parse to the printed form's own tree.
        let reparsed = parse(&print(&ast)).unwrap();
        assert_eq!(print(&reparsed), "(a/b)^()");
    }

    #[test]
    fn wrap_call_covers_the_one_argument_names() {
        let mut ast = parse("x + 1").unwrap();
        assert!(wrap_call(&mut ast, &[0], "sqrt"));
        assert_eq!(print(&ast), "sqrt(x) + 1");
        assert!(wrap_call(&mut ast, &[0], "hat"));
        assert_eq!(print(&ast), "hat(sqrt(x)) + 1");
        assert!(!wrap_call(&mut ast, &[0], "spam"));
    }

    #[test]
    fn delete_from_a_script_slot_removes_the_slot() {
        let mut ast = parse("x_i^2").unwrap();
        // Children of the script: base x, sub i, sup 2.
        let sub = delete(&mut ast, &[0, 1]).unwrap();
        assert_eq!(print(&ast), "x^2");
        assert_eq!(print(&[sub]), "i");
    }

    #[test]
    fn insert_builds_a_sequence() {
        let mut ast = parse("a").unwrap();
        let plus = parse("+").unwrap().remove(0);
        let b = parse("b").unwrap().remove(0);
        assert!(insert(&mut ast, &[], 1, plus));
        assert!(insert(&mut ast, &[], 2, b));
        assert_eq!(print(&ast), "a + b");
    }

    #[test]
    fn a_bad_path_changes_nothing() {
        let mut ast = parse("a + b").unwrap();
        let before = print(&ast);
        assert!(!wrap_in_fraction(&mut ast, &[9]));
        assert!(!attach_script(&mut ast, &[0, 4], ScriptSlot::Sub));
        assert!(delete(&mut ast, &[7]).is_none());
        assert_eq!(print(&ast), before);
    }
}
