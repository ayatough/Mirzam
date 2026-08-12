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
    /// Fills the destination, which must be a placeholder — what dropping
    /// onto a hole means.
    Into,
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
            // Taking the only thing out of a container leaves a hole, not an
            // empty `sqrt()` with nothing left to aim an edit at.
            if v.len() == 1 {
                return Some(std::mem::replace(&mut v[0], placeholder()));
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

/// Puts `node` at the destination: the shared back half of a move and of
/// dropping fresh material. Every slot means something everywhere:
///
/// - `Sup`/`Sub`: the destination gains the node as that script.
/// - `Before`/`After`: a sibling where the destination sits in a list; where
///   it sits in a fixed slot — a numerator, a script base — the two become a
///   run, because "beside" inside a slot can only mean juxtaposition.
/// - `Into`: a hole is filled; a container gains the node at the end of its
///   contents; anything else becomes a run with the node appended.
fn place(root: &mut Vec<Node>, to: &[usize], slot: MoveSlot, node: Node) -> bool {
    match slot {
        MoveSlot::Sup | MoveSlot::Sub => {
            let script = match slot {
                MoveSlot::Sup => ScriptSlot::Sup,
                _ => ScriptSlot::Sub,
            };
            if !attach_script(root, to, script) {
                return false;
            }
            let target = node_at(root, to).expect("just attached");
            let hole = match (slot, &target.kind) {
                (MoveSlot::Sub, _) => 1,
                (_, NodeKind::Script { sub: Some(_), .. }) => 2,
                _ => 1,
            };
            let mut path = to.to_vec();
            path.push(hole);
            replace(root, &path, node.kind)
        }
        MoveSlot::Before | MoveSlot::After => {
            let (&idx, parent_path) = to.split_last().expect("callers check non-empty");
            let listed = parent_path.is_empty()
                || node_at(root, parent_path).is_some_and(|p| is_vec_parent(&p.kind));
            if listed {
                let at = if slot == MoveSlot::After {
                    idx + 1
                } else {
                    idx
                };
                return insert(root, parent_path, at, node);
            }
            let Some(slot_node) = node_at_mut(root, to) else {
                return false;
            };
            let old = std::mem::replace(slot_node, placeholder());
            let pair = if slot == MoveSlot::After {
                vec![old, node]
            } else {
                vec![node, old]
            };
            *slot_node = Node::synthetic(NodeKind::Seq(pair));
            true
        }
        MoveSlot::Into => {
            let Some(target) = node_at_mut(root, to) else {
                return false;
            };
            if is_placeholder(target) {
                *target = Node::synthetic(node.kind);
                return true;
            }
            match &mut target.kind {
                NodeKind::Seq(v)
                | NodeKind::Paren(v)
                | NodeKind::Sqrt(v)
                | NodeKind::Abs(v)
                | NodeKind::Norm(v)
                | NodeKind::Call { arg: v, .. } => v.push(node),
                _ => {
                    let old = std::mem::replace(target, placeholder());
                    *target = Node::synthetic(NodeKind::Seq(vec![old, node]));
                }
            }
            true
        }
    }
}

/// [`place`] for material that is not in the tree yet — what dropping a
/// symbol from a palette means.
pub fn place_node(root: &mut Vec<Node>, to: &[usize], slot: MoveSlot, node: Node) -> bool {
    if to.is_empty() {
        return false;
    }
    if node_at(root, to).is_none() {
        return false;
    }
    if script_slot_occupied(root, to, slot) {
        return false;
    }
    place(root, to, slot, node)
}

fn script_slot_occupied(root: &[Node], to: &[usize], slot: MoveSlot) -> bool {
    let Some(target) = node_at(root, to) else {
        return false;
    };
    if let NodeKind::Script { sub, sup, .. } = &target.kind {
        return match slot {
            MoveSlot::Sup => sup.is_some(),
            MoveSlot::Sub => sub.is_some(),
            _ => false,
        };
    }
    false
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
    if node_at(root, to).is_none() || script_slot_occupied(root, to, slot) {
        return false;
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
            match node_at(root, parent_path) {
                Some(p) => match &p.kind {
                    NodeKind::Script { .. } => idx >= 1,
                    // A container's only child is swapped for a hole rather
                    // than removed, so nothing shifts.
                    k => is_vec_parent(k) && p.children().len() > 1,
                },
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

    place(root, &to, slot, node)
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
    fn deleting_a_containers_only_child_leaves_a_hole() {
        let mut ast = parse("sqrt(2) y").unwrap();
        let taken = delete(&mut ast, &[0, 0]).unwrap();
        assert_eq!(print(&[taken]), "2");
        assert_eq!(print(&ast), "sqrt(()) y");
        assert!(is_placeholder(node_at(&ast, &[0, 0]).unwrap()));
        // A root-level node still just leaves.
        let mut ast = parse("x").unwrap();
        delete(&mut ast, &[0]).unwrap();
        assert!(ast.is_empty());
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
    }

    /// "Beside" a node in a fixed slot joins it there, because a numerator
    /// has no sibling list to insert into.
    #[test]
    fn move_beside_a_slot_joins_it() {
        let mut ast = parse("a/b c").unwrap();
        assert!(move_node(&mut ast, &[1], &[0, 0], MoveSlot::Before));
        assert_eq!(print(&ast), "ca/b");
    }

    /// `Into` everywhere it can mean something: a hole is filled, a
    /// container's contents grow, anything else becomes a run.
    #[test]
    fn move_into_fills_grows_or_joins() {
        let mut ast = parse("x^() c").unwrap();
        assert!(move_node(&mut ast, &[1], &[0, 1], MoveSlot::Into));
        assert_eq!(print(&ast), "x^c");
        // A root with something in it already: the drop joins the contents.
        let mut ast = parse("sqrt(2) y").unwrap();
        assert!(move_node(&mut ast, &[1], &[0], MoveSlot::Into));
        assert_eq!(print(&ast), "sqrt(2 y)");
        // A leaf in a fixed slot: the two become a run.
        let mut ast = parse("x^2 c").unwrap();
        assert!(move_node(&mut ast, &[1], &[0, 1], MoveSlot::Into));
        assert_eq!(print(&ast), "x^(2 c)");
    }

    /// Fresh material lands the same way a moved node does.
    #[test]
    fn place_node_covers_every_slot() {
        let mut ast = parse("a/b").unwrap();
        // Into the numerator, which is a single leaf: they become a run.
        assert!(place_node(
            &mut ast,
            &[0, 0],
            MoveSlot::Into,
            parse("x").unwrap().remove(0)
        ));
        assert_eq!(print(&ast), "ax/b");
        // Before the denominator — a fixed slot, so "beside" joins.
        assert!(place_node(
            &mut ast,
            &[0, 1],
            MoveSlot::Before,
            parse("2").unwrap().remove(0)
        ));
        assert_eq!(print(&ast), "ax/(2 b)");
        // A shoulder in one step.
        let mut ast = parse("x + 1").unwrap();
        assert!(place_node(
            &mut ast,
            &[0],
            MoveSlot::Sup,
            parse("2").unwrap().remove(0)
        ));
        assert_eq!(print(&ast), "x^2 + 1");
        // An occupied shoulder still refuses.
        assert!(!place_node(
            &mut ast,
            &[0],
            MoveSlot::Sup,
            parse("3").unwrap().remove(0)
        ));
        assert_eq!(print(&ast), "x^2 + 1");
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
