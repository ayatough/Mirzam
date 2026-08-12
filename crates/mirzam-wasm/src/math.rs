//! The math panel's back end: the browser holds a formula as Typst-math
//! source text, and every tap comes here as one operation. Each call parses
//! the source, applies the operation through `mirzam_tmath::edit`, prints the
//! result, and returns the new source with a tree for the boxes and MathML
//! for the preview — so the JS side keeps no model of its own, and the text
//! in the deck stays the single source of truth.

use mirzam_tmath::{edit, parse, print, Node, NodeKind};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

/// The full panel state for a formula source: `{ok, src, tree, mathml}`, or
/// `{ok: false, error}` when the source does not parse.
#[wasm_bindgen]
pub fn math_state(src: &str) -> String {
    state_json(src).to_string()
}

/// Applies one edit operation to a formula and returns the new state.
/// `op` is JSON: `{op, path?, name?, src?, parent?, index?}`. On failure the
/// error is returned with `ok: false` and the caller keeps its old state.
#[wasm_bindgen]
pub fn math_apply(src: &str, op: &str) -> String {
    match apply(src, op) {
        Ok(new_src) => state_json(&new_src).to_string(),
        Err(e) => json!({ "ok": false, "error": e }).to_string(),
    }
}

fn state_json(src: &str) -> Value {
    match parse(src) {
        Ok(tree) => json!({
            "ok": true,
            "src": src,
            "tree": tree.iter().map(node_json).collect::<Vec<_>>(),
            "mathml": mirzam_render::render_math(src, mirzam_render::MathDialect::Typst, true),
            // The tap surface: the same formula typeset, with the path of
            // its node on every element a finger could land on.
            "emathml": editor_mathml(&tree),
            // The same formula as LaTeX, so the panel can land it in a deck
            // whose `math:` dialect is LaTeX without flipping the deck.
            "latex": mirzam_tmath::to_latex(src).unwrap_or_default(),
        }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

// ---------------------------------------------------------------------------
// The tap surface: MathML with node paths
// ---------------------------------------------------------------------------
//
// The deck's own rendering goes through LaTeX into `math-core`, which erases
// the correspondence between MathML elements and tree nodes — fine for a
// reader, useless for a finger. This emitter draws the structure itself
// (`mfrac`, `msup`, `msqrt`, …) stamping `data-path` as it goes, and hands
// the *leaves* to the real converter so every glyph — α, ∞, ⊆ — is exactly
// the one the deck will show. Nodes whose insides the panel does not edit in
// place (matrices, braces, letter styles) render whole, as one tap target.

fn editor_mathml(nodes: &[Node]) -> String {
    let mut out = String::from("<math display=\"block\"><mrow>");
    for (i, n) in nodes.iter().enumerate() {
        emit_mathml(n, &mut vec![i], &mut out);
    }
    out.push_str("</mrow></math>");
    out
}

fn path_attr(path: &[usize]) -> String {
    let joined = path
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!(" data-path=\"{joined}\"")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Emits exactly one element for `n`, so slot-counting parents (`mfrac`,
/// `msubsup`) can rely on child positions.
fn emit_mathml(n: &Node, path: &mut Vec<usize>, out: &mut String) {
    let p = path_attr(path);
    if edit::is_placeholder(n) {
        out.push_str(&format!("<mrow{p} class=\"mph\"><mtext>□</mtext></mrow>"));
        return;
    }
    let kids = |inner: &[Node], path: &mut Vec<usize>, out: &mut String, from: usize| {
        for (i, c) in inner.iter().enumerate() {
            path.push(from + i);
            emit_mathml(c, path, out);
            path.pop();
        }
    };
    match &n.kind {
        NodeKind::Num(s) => out.push_str(&format!("<mn{p}>{}</mn>", xml_escape(s))),
        NodeKind::Ident(c) => out.push_str(&format!("<mi{p}>{}</mi>", xml_escape(&c.to_string()))),
        NodeKind::Ch(c) => out.push_str(&format!("<mo{p}>{}</mo>", xml_escape(&c.to_string()))),
        NodeKind::Text(s) => out.push_str(&format!("<mtext{p}>{}</mtext>", xml_escape(s))),
        NodeKind::Esc(c) => {
            out.push_str(&format!("<mtext{p}>{}</mtext>", xml_escape(&c.to_string())))
        }
        NodeKind::Seq(inner) => {
            out.push_str(&format!("<mrow{p}>"));
            kids(inner, path, out, 0);
            out.push_str("</mrow>");
        }
        NodeKind::Paren(inner) => {
            out.push_str(&format!("<mrow{p}><mo>(</mo>"));
            kids(inner, path, out, 0);
            out.push_str("<mo>)</mo></mrow>");
        }
        NodeKind::Frac(a, b) => {
            out.push_str(&format!("<mfrac{p}>"));
            path.push(0);
            emit_mathml(a, path, out);
            path.pop();
            path.push(1);
            emit_mathml(b, path, out);
            path.pop();
            out.push_str("</mfrac>");
        }
        NodeKind::Script { base, sub, sup } => {
            let tag = match (sub, sup) {
                (Some(_), Some(_)) => "msubsup",
                (Some(_), None) => "msub",
                _ => "msup",
            };
            out.push_str(&format!("<{tag}{p}>"));
            path.push(0);
            emit_mathml(base, path, out);
            path.pop();
            for (at, slot) in (1..).zip([sub, sup].into_iter().flatten()) {
                path.push(at);
                emit_mathml(slot, path, out);
                path.pop();
            }
            out.push_str(&format!("</{tag}>"));
        }
        NodeKind::Sqrt(inner) => {
            out.push_str(&format!("<msqrt{p}><mrow>"));
            kids(inner, path, out, 0);
            out.push_str("</mrow></msqrt>");
        }
        NodeKind::Abs(inner) => {
            out.push_str(&format!("<mrow{p}><mo>|</mo>"));
            kids(inner, path, out, 0);
            out.push_str("<mo>|</mo></mrow>");
        }
        NodeKind::Norm(inner) => {
            out.push_str(&format!("<mrow{p}><mo>‖</mo>"));
            kids(inner, path, out, 0);
            out.push_str("<mo>‖</mo></mrow>");
        }
        // Everything else — symbols with their glyphs, matrices, braces,
        // accents, styles — renders whole through the deck's own converter
        // and is one tap target.
        _ => {
            let src = print(std::slice::from_ref(n));
            let inner = converted_inner(&src);
            out.push_str(&format!("<mrow{p}>{inner}</mrow>"));
        }
    }
}

/// The children of the `<math>` element the real converter produces for one
/// node's source — the glyphs without the wrapper.
fn converted_inner(src: &str) -> String {
    let rendered = mirzam_render::render_math(src, mirzam_render::MathDialect::Typst, false);
    let inner = rendered
        .strip_suffix("</math>")
        .and_then(|r| r.split_once('>').map(|(_, body)| body));
    match (rendered.starts_with("<math"), inner) {
        (true, Some(body)) => body.to_string(),
        // A node that fails to convert alone should not exist; show a mark
        // rather than injecting the error span into MathML.
        _ => "<mtext>?</mtext>".into(),
    }
}

fn apply(src: &str, op: &str) -> Result<String, String> {
    let op: Value = serde_json::from_str(op).map_err(|e| format!("bad operation: {e}"))?;
    let kind = op["op"].as_str().ok_or("operation has no `op`")?;
    let path_of = |key: &str| -> Result<Vec<usize>, String> {
        op[key]
            .as_array()
            .ok_or(format!("operation has no `{key}`"))?
            .iter()
            .map(|v| v.as_u64().map(|n| n as usize).ok_or("bad path".into()))
            .collect()
    };
    let path = || path_of("path");
    // A snippet the user is placing — a letter, a number, a symbol name —
    // goes through the same parser as everything else, so what can be placed
    // is exactly what can be written.
    let snippet = |key: &str| -> Result<Node, String> {
        let text = op[key].as_str().unwrap_or_default();
        let mut nodes = parse(text).map_err(|e| e.to_string())?;
        Ok(match nodes.len() {
            0 => edit::placeholder(),
            1 => nodes.remove(0),
            _ => mirzam_tmath::Node::synthetic(NodeKind::Seq(nodes)),
        })
    };

    let mut tree = parse(src).map_err(|e| e.to_string())?;
    let applied = match kind {
        "frac" => edit::wrap_in_fraction(&mut tree, &path()?),
        "sub" => edit::attach_script(&mut tree, &path()?, edit::ScriptSlot::Sub),
        "sup" => edit::attach_script(&mut tree, &path()?, edit::ScriptSlot::Sup),
        "call" => {
            let name = op["name"].as_str().ok_or("`call` has no `name`")?;
            edit::wrap_call(&mut tree, &path()?, name)
        }
        "replace" => {
            let node = snippet("src")?;
            edit::replace(&mut tree, &path()?, node.kind)
        }
        "delete" => edit::delete(&mut tree, &path()?).is_some(),
        // "Take this, put it there": one operation, because the deletion
        // shifts the very paths a two-step caller would aim with.
        "move" => {
            let slot = move_slot(&op)?;
            edit::move_node(&mut tree, &path_of("from")?, &path_of("to")?, slot)
        }
        // The same landing rules for material that is not in the tree yet —
        // what dropping a palette symbol means.
        "place" => {
            let slot = move_slot(&op)?;
            let node = snippet("src")?;
            edit::place_node(&mut tree, &path_of("to")?, slot, node)
        }
        "append" => {
            let node = snippet("src")?;
            let at = tree.len();
            edit::insert(&mut tree, &[], at, node)
        }
        "insert" => {
            let parent: Vec<usize> = op["parent"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default();
            let index = op["index"].as_u64().unwrap_or(0) as usize;
            let node = snippet("src")?;
            edit::insert(&mut tree, &parent, index, node)
        }
        other => return Err(format!("unknown operation `{other}`")),
    };
    if !applied {
        return Err("the operation does not apply there".into());
    }
    Ok(print(&tree))
}

fn move_slot(op: &Value) -> Result<edit::MoveSlot, String> {
    match op["slot"].as_str() {
        Some("sup") => Ok(edit::MoveSlot::Sup),
        Some("sub") => Ok(edit::MoveSlot::Sub),
        Some("before") => Ok(edit::MoveSlot::Before),
        Some("after") => Ok(edit::MoveSlot::After),
        Some("into") => Ok(edit::MoveSlot::Into),
        _ => Err("a drop needs a `slot`: sup, sub, before, after or into".into()),
    }
}

/// One node for the box view: `{k, t?, c, ph?, sub?, sup?}`. The children
/// array is in exactly the order `mirzam_tmath`'s paths index, so the box a
/// finger lands on names the node an operation edits.
fn node_json(n: &Node) -> Value {
    let mut o = json!({ "k": kind_tag(n), "c": n.children().iter().map(|c| node_json(c)).collect::<Vec<_>>() });
    if let Some(label) = label(n) {
        o["t"] = json!(label);
    }
    if edit::is_placeholder(n) {
        o["ph"] = json!(true);
    }
    if let NodeKind::Script { sub, sup, .. } = &n.kind {
        o["sub"] = json!(sub.is_some());
        o["sup"] = json!(sup.is_some());
    }
    o
}

fn kind_tag(n: &Node) -> &'static str {
    match &n.kind {
        NodeKind::Num(_) | NodeKind::Ident(_) | NodeKind::Sym { .. } | NodeKind::Ch(_) => "leaf",
        NodeKind::Text(_) | NodeKind::Esc(_) | NodeKind::Op(_) => "leaf",
        NodeKind::Seq(_) => "seq",
        NodeKind::Paren(_) => "paren",
        NodeKind::Frac(..) => "frac",
        NodeKind::Script { .. } => "script",
        NodeKind::Sqrt(_)
        | NodeKind::Root(..)
        | NodeKind::Abs(_)
        | NodeKind::Norm(_)
        | NodeKind::Call { .. }
        | NodeKind::Binom(..)
        | NodeKind::Brace { .. }
        | NodeKind::Matrix { .. }
        | NodeKind::Cases(_) => "call",
        NodeKind::Align | NodeKind::Break => "leaf",
    }
}

/// What a leaf box shows, or a call box's name. The spellings are the
/// source's own; the JS side maps the common ones to glyphs.
fn label(n: &Node) -> Option<String> {
    Some(match &n.kind {
        NodeKind::Num(s) => s.clone(),
        NodeKind::Ident(c) | NodeKind::Ch(c) | NodeKind::Esc(c) => c.to_string(),
        NodeKind::Sym { src, .. } => src.clone(),
        NodeKind::Text(s) => format!("\"{s}\""),
        NodeKind::Op(s) => s.clone(),
        NodeKind::Sqrt(_) => "sqrt".into(),
        NodeKind::Root(..) => "root".into(),
        NodeKind::Abs(_) => "abs".into(),
        NodeKind::Norm(_) => "norm".into(),
        NodeKind::Call { name, .. } | NodeKind::Brace { name, .. } => name.clone(),
        NodeKind::Binom(..) => "binom".into(),
        NodeKind::Matrix { .. } => "mat".into(),
        NodeKind::Cases(_) => "cases".into(),
        NodeKind::Align => "&".into(),
        NodeKind::Break => "\\".into(),
        NodeKind::Seq(_) | NodeKind::Paren(_) | NodeKind::Frac(..) | NodeKind::Script { .. } => {
            return None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_ok(src: &str, op: &str) -> String {
        let v: Value = serde_json::from_str(&math_apply(src, op)).unwrap();
        assert_eq!(v["ok"], json!(true), "{v}");
        v["src"].as_str().unwrap().to_string()
    }

    /// The whole panel flow, as the JS side drives it: place a letter, give
    /// it a superscript, fill the hole, wrap it in a fraction.
    #[test]
    fn the_tap_flow_builds_a_formula() {
        let s = apply_ok("", r#"{"op":"append","src":"x"}"#);
        assert_eq!(s, "x");
        let s = apply_ok(&s, r#"{"op":"sup","path":[0]}"#);
        assert_eq!(s, "x^()");
        let s = apply_ok(&s, r#"{"op":"replace","path":[0,1],"src":"2"}"#);
        assert_eq!(s, "x^2");
        let s = apply_ok(&s, r#"{"op":"frac","path":[0]}"#);
        assert_eq!(s, "x^2/()");
        let s = apply_ok(&s, r#"{"op":"replace","path":[0,1],"src":"2"}"#);
        assert_eq!(s, "x^2/2");
    }

    #[test]
    fn the_state_carries_tree_and_mathml() {
        let v: Value = serde_json::from_str(&math_state("alpha/2")).unwrap();
        assert_eq!(v["ok"], json!(true));
        assert_eq!(v["tree"][0]["k"], json!("frac"));
        assert_eq!(v["tree"][0]["c"][0]["t"], json!("alpha"));
        assert!(v["mathml"].as_str().unwrap().contains("mfrac"), "{v}");
        // The LaTeX twin, for decks whose `math:` dialect is LaTeX.
        assert_eq!(v["latex"], json!("\\frac{\\alpha}{2}"));
    }

    #[test]
    fn a_bad_operation_reports_and_leaves_no_trace() {
        let v: Value =
            serde_json::from_str(&math_apply("x", r#"{"op":"sub","path":[7]}"#)).unwrap();
        assert_eq!(v["ok"], json!(false));
        // A snippet outside the subset is the parser's error, verbatim.
        let v: Value = serde_json::from_str(&math_apply(
            "x",
            r#"{"op":"replace","path":[0],"src":"spam(1)"}"#,
        ))
        .unwrap();
        assert_eq!(v["ok"], json!(false));
        assert!(v["error"].as_str().unwrap().contains("spam"), "{v}");
    }

    /// The gesture, end to end through the JSON layer: a b c, then b onto
    /// a's shoulder.
    #[test]
    fn move_through_the_json_layer() {
        let s = apply_ok("a b c", r#"{"op":"move","from":[1],"to":[0],"slot":"sup"}"#);
        assert_eq!(s, "a^b c");
        let v: Value = serde_json::from_str(&math_apply(
            "x^2 y",
            r#"{"op":"move","from":[1],"to":[0],"slot":"sup"}"#,
        ))
        .unwrap();
        assert_eq!(v["ok"], json!(false), "occupied slot must refuse");
    }

    /// One drop, one operation: `place` lands fresh material by slot.
    #[test]
    fn place_through_the_json_layer() {
        let s = apply_ok(
            "sqrt(2)",
            r#"{"op":"place","to":[0],"slot":"into","src":"y"}"#,
        );
        assert_eq!(s, "sqrt(2 y)");
        let s = apply_ok(
            "a/b",
            r#"{"op":"place","to":[0,0],"slot":"after","src":"x"}"#,
        );
        assert_eq!(s, "ax/b");
        let s = apply_ok("x", r#"{"op":"place","to":[0],"slot":"sup","src":"2"}"#);
        assert_eq!(s, "x^2");
    }

    /// The tap surface: structure drawn with paths, leaves with real glyphs.
    #[test]
    fn editor_mathml_carries_paths_and_glyphs() {
        let v: Value = serde_json::from_str(&math_state("alpha/2 + x^2")).unwrap();
        let e = v["emathml"].as_str().unwrap();
        assert!(e.contains("<mfrac data-path=\"0\">"), "{e}");
        assert!(e.contains("data-path=\"0,0\""), "{e}");
        assert!(e.contains('α'), "the symbol renders as its glyph: {e}");
        assert!(e.contains("<msup data-path=\"2\">"), "{e}");
        // A hole is visible and tappable.
        let v: Value = serde_json::from_str(&math_state("x^()")).unwrap();
        assert!(v["emathml"].as_str().unwrap().contains("mph"), "{v}");
    }

    #[test]
    fn placeholders_are_marked_for_the_boxes() {
        let v: Value = serde_json::from_str(&math_state("x^()")).unwrap();
        let script = &v["tree"][0];
        assert_eq!(script["k"], json!("script"));
        assert_eq!(script["sup"], json!(true));
        assert_eq!(script["c"][1]["ph"], json!(true));
    }
}
