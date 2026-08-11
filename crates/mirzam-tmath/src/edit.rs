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

/// Where [`move_node`] puts the node relative to its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveSlot {
    /// Becomes the destination's superscript.
    Sup,
    /// Becomes the destination's subscript.
    Sub,
    /// A sibling before the destination.
    Before,
    /// A sibling after the destination.
    After,
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

/// Whether a node's children live in a list that closes up when one leaves —
/// as opposed to fixed slots, where a hole stays behind.
fn is_vec_parent(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Seq(_)
            | NodeKind::Paren(_)
            | NodeKind::Sqrt(_)
            | NodeKind::Abs(_)
            | NodeKind::Norm(_)
            | NodeKind::Call { .. }
    )
}

/// Moves the node at `from` next to (or onto) the node at `to`: the gesture
/// "take b, put it on a's shoulder", as one operation — because doing it as
/// delete-then-place from outside would leave the caller adjusting paths the
/// deletion just shifted. Feasibility is checked before anything is taken,
/// so `false` always means an unchanged tree.
pub fn move_node(root: &mut Vec<Node>, from: &[usize], to: &[usize], slot: MoveSlot) -> bool {
    if from.is_empty() || to.is_empty() || to.starts_with(from) {
        return false;
    }
    // The destination must exist and accept the slot.
    let Some(target) = node_at(root, to) else {
        return false;
    };
    match slot {
        MoveSlot::Sup | MoveSlot::Sub => {
            if let NodeKind::Script { sub, sup, .. } = &target.kind {
                let occupied = match slot {
                    MoveSlot::Sup => sup.is_some(),
                    _ => sub.is_some(),
                };
                if occupied {
                    return false;
                }
            }
        }
        MoveSlot::Before | MoveSlot::After => {
            let (last, parent_path) = to.split_last().expect("checked non-empty");
            let _ = last;
            if !parent_path.is_empty() {
                let Some(parent) = node_at(root, parent_path) else {
                    return false;
                };
                if !is_vec_parent(&parent.kind) {
                    return false;
                }
            }
        }
    }

    // Whether taking `from` out shifts later sibling indices: it does when
    // the children close up (a list, or a script slot that disappears), and
    // does not when a placeholder stays behind (a fraction operand, a
    // script base).
    let closes_up = {
        let (&idx, parent_path) = from.split_last().expect("checked non-empty");
        if parent_path.is_empty() {
            true
        } else {
            match node_at(root, parent_path).map(|p| &p.kind) {
                Some(NodeKind::Script { .. }) => idx >= 1,
                Some(k) => is_vec_parent(k),
                None => return false,
            }
        }
    };

    let Some(node) = delete(root, from) else {
        return false;
    };

    // Re-aim `to` past the hole `from` left.
    let mut to = to.to_vec();
    let k = from.len() - 1;
    if closes_up && to.len() > k && to[..k] == from[..k] && to[k] > from[k] {
        to[k] -= 1;
    }

    match slot {
        MoveSlot::Sup | MoveSlot::Sub => {
            let script = match slot {
                MoveSlot::Sup => ScriptSlot::Sup,
                _ => ScriptSlot::Sub,
            };
            if !attach_script(root, &to, script) {
                return false;
            }
            let target = node_at(root, &to).expect("just attached");
            let hole = match (slot, &target.kind) {
                (MoveSlot::Sub, _) => 1,
                (_, NodeKind::Script { sub: Some(_), .. }) => 2,
                _ => 1,
            };
            let mut path = to;
            path.push(hole);
            replace(root, &path, node.kind)
        }
        MoveSlot::Before | MoveSlot::After => {
            let (&idx, parent_path) = to.split_last().expect("checked non-empty");
            let at = if slot == MoveSlot::After {
                idx + 1
            } else {
                idx
            };
            insert(root, parent_path, at, node)
        }
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

    /// The gesture the workstream was named for: a b c, then b onto a's
    /// shoulder.
    #[test]
    fn move_b_onto_a() {
        let mut ast = parse("a b c").unwrap();
        assert!(move_node(&mut ast, &[1], &[0], MoveSlot::Sup));
        assert_eq!(print(&ast), "a^b c");
        // And c under it, filling the other slot.
        assert!(move_node(&mut ast, &[1], &[0], MoveSlot::Sub));
        assert_eq!(print(&ast), "a_c^b");
    }

    #[test]
    fn move_before_and_after_adjust_for_the_hole() {
        let mut ast = parse("a b c").unwrap();
        // Moving a after c: deleting a shifts c from [2] to [1].
        assert!(move_node(&mut ast, &[0], &[2], MoveSlot::After));
        assert_eq!(print(&ast), "b c a");
        let mut ast = parse("a b c").unwrap();
        assert!(move_node(&mut ast, &[2], &[0], MoveSlot::Before));
        assert_eq!(print(&ast), "c a b");
    }

    #[test]
    fn move_refuses_what_would_lose_content() {
        // Into its own subtree.
        let mut ast = parse("sqrt(x) y").unwrap();
        let before = print(&ast);
        assert!(!move_node(&mut ast, &[0], &[0, 0], MoveSlot::Sup));
        assert_eq!(print(&ast), before);
        // Onto an occupied slot.
        let mut ast = parse("x^2 y").unwrap();
        let before = print(&ast);
        assert!(!move_node(&mut ast, &[1], &[0], MoveSlot::Sup));
        assert_eq!(print(&ast), before);
        // Before a fraction operand, which has no sibling list to join.
        let mut ast = parse("a/b c").unwrap();
        let before = print(&ast);
        assert!(!move_node(&mut ast, &[1], &[0, 0], MoveSlot::Before));
        assert_eq!(print(&ast), before);
    }

    #[test]
    fn move_out_of_a_fraction_leaves_the_hole() {
        let mut ast = parse("a/b + c").unwrap();
        // The numerator moves out to the end; a hole stays behind.
        assert!(move_node(&mut ast, &[0, 0], &[2], MoveSlot::After));
        assert_eq!(print(&ast), "()/b + c a");
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
