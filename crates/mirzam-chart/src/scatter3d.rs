//! A point cloud, projected to two dimensions at build time.
//!
//! Three-dimensional data otherwise has to leave the deck as a screenshot, and
//! a screenshot cannot be pointed at: the whole reason every chart mark here
//! carries an id is so a `connect` arrow can name one of them, and "the
//! outlier in the top cluster" is exactly the thing a slide wants to name.
//! A projected cloud also stays vector, so it prints at any resolution.
//!
//! Everything below the projection is the SVG the rest of this crate already
//! emits - circles with ids - so `connect`, `annotate`, the theme tokens and
//! PDF export need to know nothing about it.
//!
//! **Points only, and orthographic only.** For a cloud of points the painter's
//! algorithm is exact rather than approximate: discs facing the viewer cannot
//! interpenetrate, so back-to-front is always a correct order. Surfaces do not
//! have that property and would need face splitting. A perspective camera
//! would bend the axis lines and make tick spacing depend on where a tick sat,
//! which turns label placement into a per-tick problem; axonometric keeps all
//! three axes straight, and is what mplot3d, Excel and Keynote draw.

use crate::{color_for, esc, parse_num, split_row, value_ticks, ChartSpec};
use std::cmp::Ordering;
use std::fmt::Write as _;

/// One named group of points. A cloud with no series column has exactly one,
/// unnamed.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub points: Vec<[f64; 3]>,
}

/// A parsed point cloud: the three axis names, and the groups of points.
#[derive(Debug, Clone, PartialEq)]
pub struct Cloud {
    pub axes: [String; 3],
    pub groups: Vec<Group>,
}

impl Cloud {
    /// The low and high end of the data along one axis.
    fn range(&self, axis: usize) -> (f64, f64) {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for p in self.groups.iter().flat_map(|g| g.points.iter()) {
            lo = lo.min(p[axis]);
            hi = hi.max(p[axis]);
        }
        if lo > hi {
            (0.0, 1.0)
        } else {
            (lo, hi)
        }
    }
}

/// Parses a cloud from CSV. The columns are positional - the first three are
/// x, y and z - because there is no first column standing for a category here
/// the way there is in a table. A fourth column, if there is one, names the
/// series each row belongs to, matching how a wide table's header does.
pub fn parse_cloud(src: &str) -> Result<Cloud, String> {
    let rows: Vec<Vec<String>> = src
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(split_row)
        .collect();

    let Some((header, body)) = rows.split_first() else {
        return Err("empty data".into());
    };
    if header.len() < 3 || header.len() > 4 {
        return Err(format!(
            "scatter3d expects three columns for x, y and z, and optionally a fourth naming the series; got {}",
            header.len()
        ));
    }
    let named = header.len() == 4;

    let mut groups: Vec<Group> = Vec::new();
    for (i, row) in body.iter().enumerate() {
        if row.len() != header.len() {
            return Err(format!(
                "row {} has {} columns, expected {}",
                i + 2,
                row.len(),
                header.len()
            ));
        }
        let mut p = [0.0f64; 3];
        for (axis, cell) in row.iter().take(3).enumerate() {
            p[axis] = parse_num(cell)
                .ok_or_else(|| format!("row {}: `{cell}` is not a number", i + 2))?;
        }
        let name = if named { row[3].clone() } else { String::new() };
        // First appearance orders the series, which is also what picks their
        // colours - the same rule a wide table's header columns follow.
        match groups.iter_mut().find(|g| g.name == name) {
            Some(g) => g.points.push(p),
            None => groups.push(Group {
                name,
                points: vec![p],
            }),
        }
    }

    Ok(Cloud {
        axes: [header[0].clone(), header[1].clone(), header[2].clone()],
        groups,
    })
}

/// An orthographic camera: where right, up and the eye point in world space.
struct Camera {
    right: [f64; 3],
    up: [f64; 3],
    eye: [f64; 3],
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Camera {
    /// `azim` turns the camera about the vertical axis, `elev` lifts it above
    /// the horizon; both in degrees, as an author writes them.
    fn new(azim: f64, elev: f64) -> Self {
        let (a, e) = (azim.to_radians(), elev.to_radians());
        Camera {
            right: [-a.sin(), a.cos(), 0.0],
            up: [-e.sin() * a.cos(), -e.sin() * a.sin(), e.cos()],
            eye: [e.cos() * a.cos(), e.cos() * a.sin(), e.sin()],
        }
    }

    /// A point in the unit cube, as `(screen x, screen y, depth)`. Screen y
    /// grows downward, the way SVG's does; depth grows toward the viewer, so
    /// drawing in ascending depth is back to front.
    fn project(&self, p: [f64; 3]) -> (f64, f64, f64) {
        // The cube is centred first, so its middle projects to the origin and
        // "away from the middle" is simply the direction of the projection.
        let c = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
        (dot(c, self.right), -dot(c, self.up), dot(c, self.eye))
    }
}

/// Renders the cloud. The pane, the viewBox and the legend are the caller's;
/// this draws the box, its grid, the axes and the points.
pub fn render(svg: &mut String, spec: &ChartSpec, cloud: &Cloud, id: &str, w: f64, h: f64) {
    let cam = Camera::new(spec.azim.unwrap_or(45.0), spec.elev.unwrap_or(30.0));
    let zoom = spec.zoom.unwrap_or(1.0).clamp(0.2, 3.0);

    let top = if spec.title.is_some() { 44.0 } else { 16.0 };
    let bottom = if cloud.groups.len() > 1 { 44.0 } else { 20.0 };
    // The margins hold the labels, which stand off the box on every side. A
    // cube projects to about seven by six and the pane is far wider than that,
    // so height is what limits the box and is the margin worth being mean
    // with; the width has room to spare whatever is done with it.
    let (mx, my) = (58.0, 12.0);
    let (pw, ph) = (w - 2.0 * mx, h - top - bottom - 2.0 * my);
    let (cx, cy) = (w / 2.0, top + my + ph / 2.0);

    let ranges = [cloud.range(0), cloud.range(1), cloud.range(2)];
    // Normalising each axis to the unit cube is what lets one camera serve
    // data of any magnitude: a cloud in millimetres and one in years project
    // to the same box.
    let unit = |p: [f64; 3]| -> [f64; 3] {
        let mut out = [0.5f64; 3];
        for (axis, o) in out.iter_mut().enumerate() {
            let (lo, hi) = ranges[axis];
            if hi > lo {
                *o = (p[axis] - lo) / (hi - lo);
            }
        }
        out
    };

    // The cube contains every point, so fitting the cube fits the cloud - and
    // it does not change as points are added, which keeps a deck's 3D charts
    // the same size as each other.
    let corners: Vec<(f64, f64, f64)> = (0..8)
        .map(|i| cam.project([(i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64]))
        .collect();
    let extent = |f: fn(&(f64, f64, f64)) -> f64| {
        let lo = corners.iter().map(f).fold(f64::INFINITY, f64::min);
        let hi = corners.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
        (hi - lo).max(1e-9)
    };
    let scale = (pw / extent(|c| c.0)).min(ph / extent(|c| c.1)) * zoom;
    let at = |p: [f64; 3]| {
        let (x, y, d) = cam.project(p);
        (cx + x * scale, cy + y * scale, d)
    };
    let xy = |p: [f64; 3]| {
        let (x, y, _) = at(p);
        (x, y)
    };

    // Which face of each pair faces away from the viewer. Drawing those three
    // and no others is a sign test on the projected normals, not a special
    // case per angle - and it is what keeps the box behind the data.
    let back = [
        if cam.eye[0] > 0.0 { 0.0 } else { 1.0 },
        if cam.eye[1] > 0.0 { 0.0 } else { 1.0 },
        if cam.eye[2] > 0.0 { 0.0 } else { 1.0 },
    ];

    let ticks: Vec<Vec<(f64, String)>> = (0..3)
        .map(|axis| {
            let (lo, hi) = ranges[axis];
            value_ticks(lo, hi)
                .into_iter()
                .map(|(v, text)| (if hi > lo { (v - lo) / (hi - lo) } else { 0.5 }, text))
                .collect()
        })
        .collect();

    // A point on the cube with one coordinate replaced.
    let corner = |mut p: [f64; 3], axis: usize, t: f64| {
        p[axis] = t;
        p
    };

    for axis in 0..3 {
        let (j, k) = ((axis + 1) % 3, (axis + 2) % 3);
        // The face perpendicular to `axis`, spanning the other two.
        let quad: Vec<String> = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
            .iter()
            .map(|&(u, v)| {
                let mut p = [0.0; 3];
                p[axis] = back[axis];
                p[j] = u;
                p[k] = v;
                let (x, y) = xy(p);
                format!("{x:.1},{y:.1}")
            })
            .collect();
        let _ = write!(
            svg,
            "<polygon class=\"mz-chart-face\" points=\"{}\"/>",
            quad.join(" ")
        );
        // Grid lines across that face, one per tick of each axis it spans.
        for (along, across) in [(j, k), (k, j)] {
            for (t, _) in &ticks[along] {
                let mut a = [0.0; 3];
                a[axis] = back[axis];
                a[along] = *t;
                a[across] = 0.0;
                let b = corner(a, across, 1.0);
                let (x1, y1) = xy(a);
                let (x2, y2) = xy(b);
                let _ = write!(
                    svg,
                    "<line class=\"mz-chart-grid\" x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\"/>"
                );
            }
        }
    }

    // Which of the four cube edges parallel to each axis carries its ticks.
    //
    // The obvious answer - the three edges meeting at the back corner - is
    // wrong for the vertical axis: at the usual camera that edge runs straight
    // down the middle of the projected box, and a label beside it lands on the
    // data. So take the edge whose middle is furthest from the middle of the
    // box, which is the one with room outside it, preferring the lower and
    // then the left of a tie. That is a front edge as often as a back one, but
    // an axis line is a hairline over a scatter and the *faces* - the things
    // that would hide a point - are still the three turned away.
    // One list for the whole box. Checking an axis's labels only against its
    // own is what lets two axes collide near a corner they share, which is
    // where they run closest - and where the sweep test caught it.
    let mut placed: Vec<(f64, f64, f64)> = Vec::new();
    let clear = |placed: &[(f64, f64, f64)], x: f64, y: f64, half: f64| {
        !placed
            .iter()
            .any(|(kx, ky, kh)| (x - kx).abs() < half + kh + 5.0 && (y - ky).abs() < 19.0)
    };

    for (axis, (axis_ticks, name)) in ticks.iter().zip(&cloud.axes).enumerate() {
        let (j, k) = ((axis + 1) % 3, (axis + 2) % 3);
        let edge = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
            .iter()
            .map(|&(u, v)| {
                let mut p = [0.0; 3];
                p[j] = u;
                p[k] = v;
                p
            })
            .max_by(|a, b| {
                let rank = |p: &[f64; 3]| {
                    let (x, y) = xy(corner(*p, axis, 0.5));
                    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                    // Rounded, so that a tie really is one and the two rules
                    // below get to decide it rather than the last decimal.
                    ((d * 10.0).round(), (y * 10.0).round(), -(x * 10.0).round())
                };
                rank(a).partial_cmp(&rank(b)).unwrap_or(Ordering::Equal)
            })
            .unwrap_or([0.0; 3]);

        let (x1, y1) = xy(corner(edge, axis, 0.0));
        let (x2, y2) = xy(corner(edge, axis, 1.0));
        let _ = write!(
            svg,
            "<line class=\"mz-chart-edge\" x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\"/>"
        );
        // Straight out from the middle of the box, which is where the room is
        // - and, the edge having been chosen for having some, is never the
        // nothing-direction it would be on an edge through the middle.
        let (dx, dy) = {
            let (x, y) = xy(corner(edge, axis, 0.5));
            let (dx, dy) = (x - cx, y - cy);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1.0 {
                (0.0, 1.0)
            } else {
                (dx / len, dy / len)
            }
        };
        let out = |p: [f64; 3], by: f64| {
            let (x, y) = xy(p);
            (x + dx * by, y + dy * by)
        };
        // An axis pointing near enough at the camera is a few pixels long on
        // screen however far apart its ticks are in the data, so a label is
        // kept only while it clears everything already placed - and then a
        // stronger rule: an axis left with a single tick has nothing to say,
        // so it says nothing. Looking straight down at a cloud, the height
        // axis is exactly that.
        let start = placed.len();
        let mut texts: Vec<&str> = Vec::new();
        for (t, text) in axis_ticks {
            let (x, y) = out(corner(edge, axis, *t), 16.0);
            let half = 4.5 * text.chars().count() as f64;
            if clear(&placed, x, y, half) {
                placed.push((x, y, half));
                texts.push(text);
            }
        }
        if texts.len() < 2 {
            placed.truncate(start);
            texts.clear();
        }
        for ((x, y, _), text) in placed[start..].iter().zip(&texts) {
            let _ = write!(
                svg,
                "<text class=\"mz-chart-tick\" x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
                y + 5.0,
                esc(text)
            );
        }
        // The name goes beyond whatever the ticks took, not through it. An
        // axis with nowhere left to write its name goes unnamed, which at the
        // angle that happens is an axis pointing at the camera anyway.
        let half = 4.5 * name.chars().count() as f64;
        let spot = [40.0, 58.0, 76.0, 94.0]
            .into_iter()
            .map(|by| out(corner(edge, axis, 0.5), by))
            .find(|(x, y)| clear(&placed, *x, *y, half));
        if let Some((x, y)) = spot {
            placed.push((x, y, half));
            let _ = write!(
                svg,
                "<text class=\"mz-chart-axis\" x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
                y + 5.0,
                esc(name)
            );
        }
    }

    // Back to front. The id keeps naming the row, so what this reorders is
    // only which disc is drawn over which - `#cloud-1-2` is the third point of
    // the second series from every angle.
    let mut marks: Vec<(f64, String)> = Vec::new();
    for (gi, group) in cloud.groups.iter().enumerate() {
        let c = color_for(spec, gi);
        let dim = match &spec.highlight {
            Some(name) if group.name != *name => " opacity=\"0.32\"",
            _ => "",
        };
        for (pi, p) in group.points.iter().enumerate() {
            let (x, y, d) = at(unit(*p));
            marks.push((
                d,
                format!(
                    "<circle class=\"mz-chart-point mz-chart-3d\" id=\"{id}-{gi}-{pi}\" cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"5\" fill=\"{c}\"{dim}/>"
                ),
            ));
        }
    }
    marks.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, mark) in marks {
        svg.push_str(&mark);
    }
}

/// The series names, for the caller's legend. Empty when the cloud has no
/// series column and there is nothing to tell apart.
pub fn legend(cloud: &Cloud) -> Vec<String> {
    if cloud.groups.len() < 2 {
        return Vec::new();
    }
    cloud.groups.iter().map(|g| g.name.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_three_columns_as_one_unnamed_series() {
        let c = parse_cloud("x, y, z\n1, 2, 3\n4, 5, 6\n").unwrap();
        assert_eq!(c.axes, ["x".to_string(), "y".to_string(), "z".to_string()]);
        assert_eq!(c.groups.len(), 1);
        assert_eq!(c.groups[0].points, vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    }

    #[test]
    fn a_fourth_column_names_the_series_in_order_of_appearance() {
        let c = parse_cloud("x, y, z, group\n1, 1, 1, b\n2, 2, 2, a\n3, 3, 3, b\n").unwrap();
        assert_eq!(c.groups.len(), 2);
        assert_eq!(c.groups[0].name, "b");
        assert_eq!(c.groups[0].points.len(), 2);
        assert_eq!(c.groups[1].name, "a");
    }

    #[test]
    fn too_few_or_too_many_columns_are_refused() {
        assert!(parse_cloud("x, y\n1, 2\n").is_err());
        assert!(parse_cloud("x, y, z, g, extra\n1, 2, 3, a, b\n").is_err());
    }

    #[test]
    fn a_cell_that_is_not_a_number_names_its_row() {
        let e = parse_cloud("x, y, z\n1, oops, 3\n").unwrap_err();
        assert!(e.contains("row 2"), "{e}");
        assert!(e.contains("oops"), "{e}");
    }

    /// The camera is what everything else is checked against, so check it
    /// against the view whose answer is obvious: straight on, from the front.
    #[test]
    fn a_camera_on_the_horizon_puts_up_on_the_screen_and_x_toward_the_eye() {
        let cam = Camera::new(0.0, 0.0);
        let (x, y, d) = cam.project([0.5, 1.0, 0.5]); // one step along +y
        assert!(x > 0.0 && y.abs() < 1e-9, "+y goes right: {x}, {y}");
        assert!(d.abs() < 1e-9, "and nowhere in depth: {d}");
        let (_, y, _) = cam.project([0.5, 0.5, 1.0]); // one step along +z
        assert!(y < 0.0, "+z goes up the screen, so down in SVG: {y}");
        let (_, _, d) = cam.project([1.0, 0.5, 0.5]); // one step along +x
        assert!(d > 0.0, "+x comes toward the eye: {d}");
    }

    fn spec(src: &str) -> crate::ChartDoc {
        crate::parse_chart(src, |_| None)
    }

    fn ids(svg: &str) -> Vec<String> {
        svg.split("mz-chart-3d\" id=\"")
            .skip(1)
            .filter_map(|s| s.split('"').next().map(str::to_string))
            .collect()
    }

    const CUBE: &str =
        "type: scatter3d\nid: p\ndata: |\n  x, y, z\n  0, 0, 0\n  1, 0, 0\n  0, 1, 0\n  1, 1, 1\n";

    #[test]
    fn every_point_becomes_a_mark_with_a_row_id() {
        let doc = spec(CUBE);
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let svg = crate::render_svg(&doc, "c");
        let mut got = ids(&svg);
        got.sort();
        assert_eq!(got, ["p-0-0", "p-0-1", "p-0-2", "p-0-3"]);
    }

    /// Depth sorting decides what is drawn over what. It must not decide what
    /// anything is called, or a `connect` arrow would move when the camera did.
    #[test]
    fn a_mark_id_is_the_same_from_every_angle() {
        let mut seen = Vec::new();
        for azim in [0, 45, 120, 200, 315] {
            for elev in [-40, 0, 30, 80] {
                let doc = spec(&format!("{CUBE}azim: {azim}\nelev: {elev}\n"));
                assert!(doc.errors.is_empty(), "{:?}", doc.errors);
                let svg = crate::render_svg(&doc, "c");
                // The row a mark names is fixed; only its position moves.
                let mut got = ids(&svg);
                got.sort();
                assert_eq!(got, ["p-0-0", "p-0-1", "p-0-2", "p-0-3"], "{azim}/{elev}");
                seen.push(svg);
            }
        }
        assert!(
            seen.windows(2).any(|w| w[0] != w[1]),
            "the camera should actually move the points"
        );
    }

    #[test]
    fn points_are_emitted_back_to_front() {
        // Two points differing only along the eye direction at azim 0, elev 0,
        // which is +x: the far one has to be written first.
        let doc = spec(
            "type: scatter3d\nid: p\nazim: 0\nelev: 0\ndata: |\n  x, y, z\n  1, 0, 0\n  0, 0, 0\n",
        );
        let svg = crate::render_svg(&doc, "c");
        assert_eq!(
            ids(&svg),
            ["p-0-1", "p-0-0"],
            "the near point is drawn last"
        );
    }

    #[test]
    fn the_box_shows_three_faces_and_three_axes() {
        let svg = crate::render_svg(&spec(CUBE), "c");
        assert_eq!(svg.matches("mz-chart-face").count(), 3);
        assert_eq!(svg.matches("mz-chart-edge").count(), 3);
    }

    /// The faces are chosen by a sign test, so turning the camera right round
    /// swaps every one of them - and never leaves a face in front of the data.
    #[test]
    fn the_faces_drawn_are_the_ones_facing_away() {
        let front = crate::render_svg(&spec(&format!("{CUBE}azim: 45\nelev: 30\n")), "c");
        let behind = crate::render_svg(&spec(&format!("{CUBE}azim: 225\nelev: -30\n")), "c");
        let faces = |svg: &str| -> Vec<String> {
            svg.split("mz-chart-face\" points=\"")
                .skip(1)
                .filter_map(|s| s.split('"').next().map(str::to_string))
                .collect()
        };
        assert_eq!(faces(&front).len(), 3);
        assert!(
            faces(&front).iter().all(|f| !faces(&behind).contains(f)),
            "every face should have swapped"
        );
    }

    #[test]
    fn the_axis_names_come_from_the_header() {
        let doc = spec("type: scatter3d\ndata: |\n  mass, drag, lift\n  1, 2, 3\n  2, 3, 4\n");
        let svg = crate::render_svg(&doc, "c");
        for name in ["mass", "drag", "lift"] {
            assert!(svg.contains(&format!(">{name}</text>")), "{name} missing");
        }
    }

    #[test]
    fn a_series_column_colours_and_names_the_groups() {
        let doc =
            spec("type: scatter3d\nid: p\ndata: |\n  x, y, z, group\n  1, 1, 1, a\n  2, 2, 2, b\n");
        let svg = crate::render_svg(&doc, "c");
        assert!(svg.contains("id=\"p-0-0\""));
        assert!(svg.contains("id=\"p-1-0\""));
        assert!(svg.contains(">a</text>"), "the legend names the series");
        assert!(svg.contains(">b</text>"));
    }

    /// Every text the box draws, as `(x, y, half width)`.
    fn labels(svg: &str) -> Vec<(f64, f64, f64)> {
        let mut out = Vec::new();
        for class in ["mz-chart-tick", "mz-chart-axis"] {
            for chunk in svg.split(&format!("class=\"{class}\"")).skip(1) {
                let attr = |name: &str| -> f64 {
                    chunk
                        .split(&format!("{name}=\""))
                        .nth(1)
                        .unwrap()
                        .split('"')
                        .next()
                        .unwrap()
                        .parse()
                        .unwrap()
                };
                let text = chunk.split('>').nth(1).unwrap().split('<').next().unwrap();
                out.push((attr("x"), attr("y"), 4.5 * text.chars().count() as f64));
            }
        }
        out
    }

    /// The risk a box of three labelled axes carries is that some camera angle
    /// puts two labels on top of each other. Sweep the angles and check.
    #[test]
    fn no_two_labels_collide_at_any_camera_angle() {
        for azim in (0..360).step_by(15) {
            for elev in [-89, -60, -30, -5, 0, 5, 30, 60, 89] {
                let doc = spec(&format!(
                    "type: scatter3d\nazim: {azim}\nelev: {elev}\ndata: |\n  width, depth, height\n  1, 2, 3\n  4, 9, 6\n  7, 5, 11\n"
                ));
                assert!(doc.errors.is_empty(), "{:?}", doc.errors);
                let svg = crate::render_svg(&doc, "c");
                let seen = labels(&svg);
                for (i, a) in seen.iter().enumerate() {
                    for b in &seen[i + 1..] {
                        assert!(
                            (a.0 - b.0).abs() >= a.2 + b.2 || (a.1 - b.1).abs() >= 15.0,
                            "labels overlap at azim {azim}, elev {elev}: {a:?} and {b:?}"
                        );
                    }
                }
            }
        }
    }

    /// A slide can honestly show a few thousand points; past that a scatter is
    /// a smear. The work is a matrix multiply each and one sort, so the size
    /// that matters is what comes out - and every point is one mark.
    #[test]
    fn ten_thousand_points_project_and_sort() {
        let rows: String = (0..10_000)
            .map(|i| {
                let f = i as f64;
                format!(
                    "  {}, {}, {}\n",
                    f % 97.0,
                    (f * 7.0) % 89.0,
                    (f * 13.0) % 83.0
                )
            })
            .collect();
        let doc = spec(&format!(
            "type: scatter3d\nid: p\ndata: |\n  x, y, z\n{rows}"
        ));
        let svg = crate::render_svg(&doc, "c");
        assert_eq!(ids(&svg).len(), 10_000);
        // Drawn, and said something about: this many marks is what the deck
        // then has to carry.
        assert!(
            doc.errors.iter().any(|e| e.contains("smear")),
            "{:?}",
            doc.errors
        );
    }

    #[test]
    fn a_cloud_a_slide_can_show_says_nothing() {
        let rows: String = (0..300).map(|i| format!("  {i}, {i}, {i}\n")).collect();
        let doc = spec(&format!("type: scatter3d\ndata: |\n  x, y, z\n{rows}"));
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
    }

    #[test]
    fn a_flat_cloud_still_draws() {
        // Every z the same: the axis has no range to normalise against.
        let doc = spec("type: scatter3d\nid: p\ndata: |\n  x, y, z\n  1, 1, 5\n  2, 3, 5\n");
        let svg = crate::render_svg(&doc, "c");
        assert_eq!(ids(&svg).len(), 2);
        assert!(!svg.contains("NaN"), "{svg}");
    }

    #[test]
    fn one_point_is_not_a_division_by_zero() {
        let doc = spec("type: scatter3d\nid: p\ndata: |\n  x, y, z\n  7, 7, 7\n");
        let svg = crate::render_svg(&doc, "c");
        assert_eq!(ids(&svg).len(), 1);
        assert!(!svg.contains("NaN"), "{svg}");
    }
}
