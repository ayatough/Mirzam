//! Data-driven charts rendered to SVG at build time.
//!
//! A `chart` block is YAML describing the chart and its data:
//!
//! ```text
//! type: bar
//! title: Quarterly revenue
//! data: |
//!   quarter, 2024, 2025
//!   Q1, 120, 180
//!   Q2, 140, 240
//! ```
//!
//! `data` may be inline CSV as above, or the name of a `.csv` file that the
//! host resolves and passes in. Every mark gets a stable id
//! (`#<chart-id>-<series>-<row>`), so `connect` blocks can point an arrow from
//! a sentence to an individual bar or point - something a hand-drawn image
//! cannot offer.

use serde::Deserialize;
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChartKind {
    #[default]
    Bar,
    Line,
    Area,
    Pie,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ChartSpec {
    #[serde(rename = "type")]
    pub kind: ChartKind,
    pub id: Option<String>,
    pub title: Option<String>,
    /// Inline CSV, or the path of a CSV file supplied by the host.
    pub data: String,
    /// The column holding the x values; the first column when unset.
    pub x: Option<String>,
    /// Axis label for the value axis.
    pub y_label: Option<String>,
    /// Format values as percentages of the column total.
    pub stacked: bool,
    /// Hide the legend even when several series are present.
    pub legend: Option<bool>,
    /// Series colors; defaults to the theme palette.
    pub colors: Vec<String>,
    /// Highlight one series by dimming the others.
    pub highlight: Option<String>,
}

/// Parsed tabular data: a category column plus one or more numeric series.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub category_name: String,
    pub categories: Vec<String>,
    /// The categories read as numbers, when every one of them is a number.
    /// `None` leaves the axis ordinal, which is all a text column can be.
    pub x_values: Option<Vec<f64>>,
    pub series: Vec<Series>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub name: String,
    pub values: Vec<f64>,
}

pub struct ChartDoc {
    pub spec: ChartSpec,
    pub table: Option<Table>,
    pub errors: Vec<String>,
    /// A `data:` value that names a file the host must supply.
    pub data_file: Option<String>,
}

/// Parses a chart block. `resolve` supplies the contents of a referenced CSV
/// file, letting the caller decide how files are read (filesystem, host table).
pub fn parse_chart(src: &str, resolve: impl Fn(&str) -> Option<String>) -> ChartDoc {
    let mut errors = Vec::new();
    let bail = |errors: Vec<String>| ChartDoc {
        spec: ChartSpec::default(),
        table: None,
        errors,
        data_file: None,
    };

    // Read the block once and take two things from the one tree: which keys
    // were written, and the spec itself. Parsing it twice would be the tidier
    // code and a second pass over every chart in the deck.
    let block: serde_yaml::Value = match serde_yaml::from_str(src) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("chart: cannot parse block: {e}"));
            return bail(errors);
        }
    };
    let unknown = unknown_keys(&block);
    let spec: ChartSpec = match serde_yaml::from_value(block) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("chart: cannot parse block: {e}"));
            return bail(errors);
        }
    };

    // A single-line `data` that ends in .csv names a file rather than inline data.
    let trimmed = spec.data.trim();
    let data_file =
        (!trimmed.contains('\n') && trimmed.ends_with(".csv")).then(|| trimmed.to_string());

    let csv = match &data_file {
        Some(path) => match resolve(path) {
            Some(content) => content,
            None => {
                errors.push(format!("chart: cannot read data file `{path}`"));
                String::new()
            }
        },
        None => spec.data.clone(),
    };

    // A key nobody reads is a key that silently did nothing: `y_label` typed
    // `ylabel`, `highlight` typed `hightlight`. `serde` drops it without a
    // word, so this is the only place it can be noticed.
    for key in unknown {
        errors.push(format!("chart: unknown key `{key}`, ignored"));
    }

    let table = match parse_csv(&csv, spec.x.as_deref()) {
        Ok(t) if !t.series.is_empty() && !t.categories.is_empty() => Some(t),
        Ok(_) => {
            errors.push("chart: no data rows".to_string());
            None
        }
        Err(e) => {
            errors.push(format!("chart: {e}"));
            None
        }
    };

    ChartDoc {
        spec,
        table,
        errors,
        data_file,
    }
}

/// Keys [`ChartSpec`] understands. It lives beside the struct because a field
/// added there and not here starts warning about itself.
const KNOWN_KEYS: &[&str] = &[
    "type",
    "id",
    "title",
    "data",
    "x",
    "y_label",
    "stacked",
    "legend",
    "colors",
    "highlight",
];

/// Keys in the block that [`ChartSpec`] does not read.
fn unknown_keys(block: &serde_yaml::Value) -> Vec<String> {
    let serde_yaml::Value::Mapping(map) = block else {
        return Vec::new();
    };
    map.keys()
        .filter_map(|k| k.as_str())
        .filter(|k| !KNOWN_KEYS.contains(k))
        .map(str::to_string)
        .collect()
}

/// Reads a number, ignoring the `%` and thousands separators a spreadsheet
/// leaves behind in a cell.
fn parse_num(cell: &str) -> Option<f64> {
    cell.replace(['%', ','], "").parse::<f64>().ok()
}

/// Parses CSV/TSV into a category column plus numeric series. `x` names the
/// column the categories come from; without it the first column does, which is
/// what every chart written before `x:` existed relies on.
pub fn parse_csv(src: &str, x: Option<&str>) -> Result<Table, String> {
    let rows: Vec<Vec<String>> = src
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(split_row)
        .collect();

    let Some((header, body)) = rows.split_first() else {
        return Err("empty data".into());
    };
    if header.len() < 2 {
        return Err("expected a category column and at least one series column".into());
    }

    let cat = match x {
        Some(name) => header
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| format!("no column named `{name}`"))?,
        None => 0,
    };

    let mut series: Vec<Series> = header
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != cat)
        .map(|(_, name)| Series {
            name: name.clone(),
            values: Vec::new(),
        })
        .collect();
    let mut categories = Vec::new();

    for (i, row) in body.iter().enumerate() {
        if row.len() != header.len() {
            return Err(format!(
                "row {} has {} columns, expected {}",
                i + 2,
                row.len(),
                header.len()
            ));
        }
        categories.push(row[cat].clone());
        let cells = row.iter().enumerate().filter(|(i, _)| *i != cat);
        for (s, (_, cell)) in series.iter_mut().zip(cells) {
            let v = parse_num(cell)
                .ok_or_else(|| format!("row {}: `{cell}` is not a number", i + 2))?;
            s.values.push(v);
        }
    }

    // A category column that is numbers all the way down is a quantity, not a
    // set of labels. The text is kept exactly as written even so: `00` is an
    // hour, not the number zero rendered back at whoever typed it.
    let x_values = categories.iter().map(|c| parse_num(c)).collect();

    Ok(Table {
        category_name: header[cat].clone(),
        categories,
        x_values,
        series,
    })
}

/// Splits one CSV/TSV row, honoring double-quoted fields so values such as
/// `"1,200"` survive as a single cell.
fn split_row(line: &str) -> Vec<String> {
    let sep = if line.contains('\t') { '\t' } else { ',' };
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c == sep && !quoted => cells.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    cells.push(cur);
    cells.into_iter().map(|c| c.trim().to_string()).collect()
}

const PALETTE: [&str; 6] = [
    "var(--mz-accent1)",
    "var(--mz-accent2)",
    "var(--mz-chart3)",
    "var(--mz-chart4)",
    "var(--mz-chart5)",
    "var(--mz-chart6)",
];

fn color_for(spec: &ChartSpec, i: usize) -> String {
    match spec.colors.get(i) {
        Some(c) if c.starts_with('@') => format!("var(--mz-{})", &c[1..]),
        Some(c) => c
            .chars()
            .filter(|c| c.is_alphanumeric() || "#(),.%- ".contains(*c))
            .collect(),
        None => PALETTE[i % PALETTE.len()].to_string(),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Formats a value without trailing zeros.
fn num(v: f64) -> String {
    if v.fract().abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        format!("{v:.1}")
    }
}

/// Renders the chart to SVG. The chart fills its pane, so the viewBox is fixed
/// and the surrounding CSS scales it.
pub fn render_svg(doc: &ChartDoc, chart_id: &str) -> String {
    let Some(table) = &doc.table else {
        return String::new();
    };
    let spec = &doc.spec;
    let id = spec.id.clone().unwrap_or_else(|| chart_id.to_string());

    let (w, h) = (720.0f64, 440.0f64);
    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg class=\"mz-chart\" id=\"{}\" viewBox=\"0 0 {w} {h}\" preserveAspectRatio=\"xMidYMid meet\" role=\"img\">",
        esc(&id)
    );

    if let Some(title) = &spec.title {
        let _ = write!(
            svg,
            "<text class=\"mz-chart-title\" x=\"{}\" y=\"26\" text-anchor=\"middle\">{}</text>",
            w / 2.0,
            esc(title)
        );
    }

    match spec.kind {
        ChartKind::Pie => render_pie(&mut svg, spec, table, &id, w, h),
        _ => render_cartesian(&mut svg, spec, table, &id, w, h),
    }

    // Legend, when there is more than one series.
    let show_legend = spec.legend.unwrap_or(table.series.len() > 1);
    if show_legend {
        let mut x = 70.0;
        for (si, s) in table.series.iter().enumerate() {
            let c = color_for(spec, si);
            let _ = write!(
                svg,
                "<rect x=\"{x}\" y=\"{}\" width=\"14\" height=\"14\" rx=\"3\" fill=\"{c}\"/>\
                 <text class=\"mz-chart-legend\" x=\"{}\" y=\"{}\">{}</text>",
                h - 18.0,
                x + 20.0,
                h - 6.0,
                esc(&s.name)
            );
            x += 34.0 + 9.0 * s.name.chars().count() as f64;
        }
    }

    svg.push_str("</svg>");
    svg
}

fn render_cartesian(svg: &mut String, spec: &ChartSpec, table: &Table, id: &str, w: f64, h: f64) {
    let (left, right, top) = (66.0, 20.0, if spec.title.is_some() { 46.0 } else { 20.0 });
    let bottom = if table.series.len() > 1 { 66.0 } else { 44.0 };
    let (pw, ph) = (w - left - right, h - top - bottom);

    let max = table
        .series
        .iter()
        .flat_map(|s| s.values.iter())
        .fold(0.0f64, |a, b| a.max(*b));
    let max = nice_ceil(if max <= 0.0 { 1.0 } else { max });

    // Grid lines and value axis.
    for i in 0..=4 {
        let v = max * i as f64 / 4.0;
        let y = top + ph - ph * (i as f64 / 4.0);
        let _ = write!(
            svg,
            "<line class=\"mz-chart-grid\" x1=\"{left}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\"/>\
             <text class=\"mz-chart-tick\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
            left + pw,
            left - 10.0,
            y + 4.0,
            num(v)
        );
    }
    if let Some(label) = &spec.y_label {
        let _ = write!(
            svg,
            "<text class=\"mz-chart-axis\" transform=\"rotate(-90 16 {:.1})\" x=\"16\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            top + ph / 2.0,
            top + ph / 2.0,
            esc(label)
        );
    }

    let n = table.categories.len().max(1) as f64;
    let slot = pw / n;

    // A category column that parsed as numbers is a quantity, so `line` and
    // `area` place their points along it by value: three years one apart and
    // then a gap of four stop drawing as four even steps. Bars stay ordinal
    // whatever the column holds, because a bar's width *is* its slot.
    let span = matches!(spec.kind, ChartKind::Line | ChartKind::Area)
        .then_some(table.x_values.as_ref())
        .flatten()
        .filter(|xs| xs.len() > 1)
        .and_then(|xs| {
            let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (hi > lo).then_some((xs, lo, hi))
        });

    // Keep the extremes off the edges of the plot area: a disc sitting on the
    // axis hangs over the value labels, and its tick lands underneath them.
    let inset = pw * 0.04;
    let x_at = |ci: usize| match span {
        Some((xs, lo, hi)) => left + inset + (xs[ci] - lo) / (hi - lo) * (pw - 2.0 * inset),
        None => left + slot * (ci as f64 + 0.5),
    };

    // The rows in left-to-right order. Equal slots are already in it; placing
    // by value is not, and a line drawn in row order would double back.
    let mut order: Vec<usize> = (0..table.categories.len()).collect();
    if span.is_some() {
        order.sort_by(|a, b| x_at(*a).total_cmp(&x_at(*b)));
    }

    // Category labels, dropping any that would run into the one before it. The
    // width is an estimate - the renderer has no font metrics - so it is
    // deliberately generous: a label too many is worse than a gap.
    let mut filled = f64::NEG_INFINITY;
    for &ci in &order {
        let cat = &table.categories[ci];
        let x = x_at(ci);
        let half = 4.5 * cat.chars().count() as f64;
        if x - half < filled {
            continue;
        }
        filled = x + half + 6.0;
        let _ = write!(
            svg,
            "<text class=\"mz-chart-tick\" x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            top + ph + 22.0,
            esc(cat)
        );
    }

    let dim = |si: usize| -> &'static str {
        match &spec.highlight {
            Some(name) if table.series[si].name != *name => " opacity=\"0.32\"",
            _ => "",
        }
    };

    match spec.kind {
        ChartKind::Bar => {
            let count = table.series.len().max(1) as f64;
            let group = slot * 0.68;
            let bw = group / count;
            for (si, s) in table.series.iter().enumerate() {
                let c = color_for(spec, si);
                for (ci, v) in s.values.iter().enumerate() {
                    let bh = (v / max) * ph;
                    let x = left + slot * ci as f64 + (slot - group) / 2.0 + bw * si as f64;
                    let y = top + ph - bh;
                    // The mark's id sits on a group holding the bar *and* its
                    // value label, so animating `#chart-0-1` moves the number
                    // with the bar instead of leaving it floating.
                    let _ = write!(
                        svg,
                        "<g id=\"{id}-{si}-{ci}\"><rect class=\"mz-chart-bar\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{:.1}\" height=\"{bh:.1}\" rx=\"3\" fill=\"{c}\"{}/>",
                        (bw - 3.0).max(1.0),
                        dim(si)
                    );
                    let _ = write!(
                        svg,
                        "<text class=\"mz-chart-value\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text></g>",
                        x + (bw - 3.0).max(1.0) / 2.0,
                        y - 6.0,
                        num(*v)
                    );
                }
            }
        }
        ChartKind::Line | ChartKind::Area => {
            for (si, s) in table.series.iter().enumerate() {
                let c = color_for(spec, si);
                let pts: Vec<(f64, f64)> = s
                    .values
                    .iter()
                    .enumerate()
                    .map(|(ci, v)| (x_at(ci), top + ph - (v / max) * ph))
                    .collect();
                // The path walks the axis; the discs below keep their row
                // order, so `#id-<series>-<row>` still names the row it named
                // before the axis had values on it.
                let along: Vec<(f64, f64)> = order
                    .iter()
                    .filter_map(|&ci| pts.get(ci).copied())
                    .collect();
                let path = along
                    .iter()
                    .map(|(x, y)| format!("{x:.1},{y:.1}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                if spec.kind == ChartKind::Area {
                    let _ = write!(
                        svg,
                        "<polygon class=\"mz-chart-area\" points=\"{:.1},{:.1} {path} {:.1},{:.1}\" fill=\"{c}\" opacity=\"0.18\"/>",
                        along.first().map(|p| p.0).unwrap_or(left),
                        top + ph,
                        along.last().map(|p| p.0).unwrap_or(left),
                        top + ph
                    );
                }
                let _ = write!(
                    svg,
                    "<polyline class=\"mz-chart-line\" points=\"{path}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"3\"{}/>",
                    dim(si)
                );
                for (ci, (x, y)) in pts.iter().enumerate() {
                    let _ = write!(
                        svg,
                        "<circle class=\"mz-chart-point\" id=\"{id}-{si}-{ci}\" cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"5\" fill=\"{c}\"{}/>",
                        dim(si)
                    );
                }
            }
        }
        ChartKind::Pie => unreachable!("handled by render_pie"),
    }
}

fn render_pie(svg: &mut String, spec: &ChartSpec, table: &Table, id: &str, w: f64, h: f64) {
    // A pie uses the first series only.
    let Some(series) = table.series.first() else {
        return;
    };
    let total: f64 = series.values.iter().sum();
    if total <= 0.0 {
        return;
    }
    // Leave room on both sides for the outside labels.
    let (cx, cy) = (w / 2.0, h / 2.0 + 8.0);
    let r = (h - 150.0).max(60.0) / 2.0;
    let mut angle = -std::f64::consts::FRAC_PI_2;

    for (ci, v) in series.values.iter().enumerate() {
        let sweep = v / total * std::f64::consts::TAU;
        let (x0, y0) = (cx + r * angle.cos(), cy + r * angle.sin());
        let end = angle + sweep;
        let (x1, y1) = (cx + r * end.cos(), cy + r * end.sin());
        let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
        let c = color_for(spec, ci);
        let _ = write!(
            svg,
            "<path class=\"mz-chart-slice\" id=\"{id}-0-{ci}\" d=\"M {cx:.1} {cy:.1} L {x0:.1} {y0:.1} A {r:.1} {r:.1} 0 {large} 1 {x1:.1} {y1:.1} Z\" fill=\"{c}\"/>"
        );
        // Label outside the slice, anchored away from the pie so it never
        // overlaps the wedge. Slivers are left unlabeled.
        let share = v / total * 100.0;
        if share >= 4.0 {
            let mid = angle + sweep / 2.0;
            let lr = r * 1.12;
            let lx = cx + lr * mid.cos();
            let anchor = if mid.cos() >= 0.0 { "start" } else { "end" };
            let pad = if mid.cos() >= 0.0 { 8.0 } else { -8.0 };
            let _ = write!(
                svg,
                "<text class=\"mz-chart-value\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"{anchor}\">{} {}%</text>",
                lx + pad,
                cy + lr * mid.sin() + 4.0,
                esc(table.categories.get(ci).map(String::as_str).unwrap_or("")),
                num(share.round())
            );
        }
        angle = end;
    }
}

/// Rounds an axis maximum up to a readable value.
fn nice_ceil(v: f64) -> f64 {
    let mag = 10f64.powf(v.log10().floor());
    let scaled = v / mag;
    let step = if scaled <= 1.0 {
        1.0
    } else if scaled <= 2.0 {
        2.0
    } else if scaled <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * mag
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn parses_inline_csv() {
        let doc = parse_chart(
            "type: bar\ntitle: T\ndata: |\n  quarter, 2024, 2025\n  Q1, 10, 20\n  Q2, 30, 40\n",
            none,
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let t = doc.table.unwrap();
        assert_eq!(t.category_name, "quarter");
        assert_eq!(t.categories, vec!["Q1", "Q2"]);
        assert_eq!(t.series.len(), 2);
        assert_eq!(t.series[0].name, "2024");
        assert_eq!(t.series[1].values, vec![20.0, 40.0]);
    }

    #[test]
    fn resolves_csv_file() {
        let doc = parse_chart("type: line\ndata: results.csv\n", |p| {
            (p == "results.csv").then(|| "x,y\na,1\nb,2\n".to_string())
        });
        assert!(doc.errors.is_empty());
        assert_eq!(doc.data_file.as_deref(), Some("results.csv"));
        assert_eq!(doc.table.unwrap().series[0].values, vec![1.0, 2.0]);
    }

    #[test]
    fn missing_file_reports_error() {
        let doc = parse_chart("type: bar\ndata: nope.csv\n", none);
        assert!(doc.errors.iter().any(|e| e.contains("nope.csv")));
    }

    #[test]
    fn ragged_rows_rejected() {
        let err = parse_csv("a,b,c\n1,2\n", None).unwrap_err();
        assert!(err.contains("columns"), "{err}");
    }

    #[test]
    fn strips_percent_and_thousands_separators() {
        let t = parse_csv("k,v\na,\"1,200\"\nb,45%\n", None).unwrap();
        assert_eq!(t.series[0].values, vec![1200.0, 45.0]);
    }

    #[test]
    fn bar_marks_get_stable_ids() {
        let doc = parse_chart("type: bar\ndata: |\n  k, s1\n  a, 3\n  b, 6\n", none);
        let svg = render_svg(&doc, "chart1");
        assert!(svg.contains("id=\"chart1-0-0\""));
        assert!(svg.contains("id=\"chart1-0-1\""));
        assert!(svg.contains("viewBox=\"0 0 720 440\""));
    }

    #[test]
    fn explicit_id_is_used_for_marks() {
        let doc = parse_chart("type: bar\nid: rev\ndata: |\n  k, s\n  a, 1\n", none);
        let svg = render_svg(&doc, "chart1");
        assert!(svg.contains("id=\"rev\""));
        assert!(svg.contains("id=\"rev-0-0\""));
    }

    #[test]
    fn bar_mark_id_groups_bar_with_its_value_label() {
        let doc = parse_chart("type: bar\nid: rev\ndata: |\n  k, s\n  a, 1\n", none);
        let svg = render_svg(&doc, "chart1");
        let group = svg
            .split("<g id=\"rev-0-0\">")
            .nth(1)
            .and_then(|rest| rest.split("</g>").next())
            .expect("mark group present");
        assert!(group.contains("mz-chart-bar"));
        assert!(group.contains("mz-chart-value"));
    }

    #[test]
    fn pie_slices_sum_to_full_circle() {
        let doc = parse_chart("type: pie\ndata: |\n  k, v\n  a, 1\n  b, 1\n  c, 2\n", none);
        let svg = render_svg(&doc, "c");
        assert_eq!(svg.matches("mz-chart-slice").count(), 3);
        assert!(svg.contains("50%")); // c is half the total
    }

    #[test]
    fn highlight_dims_other_series() {
        let doc = parse_chart(
            "type: line\nhighlight: b\ndata: |\n  k, a, b\n  x, 1, 2\n",
            none,
        );
        let svg = render_svg(&doc, "c");
        assert!(svg.contains("opacity=\"0.32\""));
    }

    /// The `cx` of every point mark, in the order they were emitted.
    fn point_xs(svg: &str) -> Vec<f64> {
        svg.split("mz-chart-point")
            .skip(1)
            .filter_map(|s| s.split("cx=\"").nth(1)?.split('"').next()?.parse().ok())
            .collect()
    }

    /// The text of every tick label, in the order they were emitted.
    fn ticks(svg: &str) -> Vec<String> {
        svg.split("class=\"mz-chart-tick\"")
            .skip(1)
            .filter_map(|s| Some(s.split('>').nth(1)?.split('<').next()?.to_string()))
            .collect()
    }

    #[test]
    fn a_numeric_category_column_places_points_by_value() {
        let doc = parse_chart(
            "type: line\ndata: |\n  k, v\n  0, 1\n  1, 2\n  9, 3\n",
            none,
        );
        let xs = point_xs(&render_svg(&doc, "c"));
        assert_eq!(xs.len(), 3);
        // `1` sits a ninth of the way along, not a half: the gap to `9` is
        // eight times the gap from `0`.
        let (first, second, last) = (xs[0], xs[1], xs[2]);
        let ratio = (last - second) / (second - first);
        assert!(
            (ratio - 8.0).abs() < 0.05,
            "{ratio} should be 8, from {xs:?}"
        );
    }

    #[test]
    fn text_categories_stay_evenly_spaced() {
        let doc = parse_chart(
            "type: line\ndata: |\n  k, v\n  a, 1\n  b, 2\n  z, 3\n",
            none,
        );
        let xs = point_xs(&render_svg(&doc, "c"));
        let (g1, g2) = (xs[1] - xs[0], xs[2] - xs[1]);
        assert!((g1 - g2).abs() < 0.2, "{xs:?} should be evenly spaced");
    }

    /// A bar's width is its slot, so bars are ordinal whatever the column says.
    #[test]
    fn bars_ignore_a_numeric_category_column() {
        let doc = parse_chart("type: bar\ndata: |\n  k, v\n  0, 1\n  1, 2\n  9, 3\n", none);
        let svg = render_svg(&doc, "c");
        let xs: Vec<f64> = svg
            .split("mz-chart-bar")
            .skip(1)
            .filter_map(|s| s.split("x=\"").nth(1)?.split('"').next()?.parse().ok())
            .collect();
        let (g1, g2) = (xs[1] - xs[0], xs[2] - xs[1]);
        assert!((g1 - g2).abs() < 0.2, "{xs:?} should be evenly spaced");
    }

    #[test]
    fn a_numeric_category_keeps_the_label_as_written() {
        let doc = parse_chart("type: line\ndata: |\n  hour, v\n  00, 1\n  04, 2\n", none);
        let svg = render_svg(&doc, "c");
        assert!(ticks(&svg).contains(&"00".to_string()), "{:?}", ticks(&svg));
    }

    #[test]
    fn a_line_follows_the_axis_not_the_row_order() {
        let doc = parse_chart(
            "type: line\ndata: |\n  k, v\n  9, 3\n  0, 1\n  5, 2\n",
            none,
        );
        let svg = render_svg(&doc, "c");
        let path = svg
            .split("points=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let xs: Vec<f64> = path
            .split_whitespace()
            .map(|p| p.split(',').next().unwrap().parse().unwrap())
            .collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "{path} runs backwards");
    }

    /// Emission order is a drawing decision; the id names the row.
    #[test]
    fn mark_ids_follow_rows_not_draw_order() {
        let doc = parse_chart("type: line\nid: p\ndata: |\n  k, v\n  9, 3\n  0, 1\n", none);
        let svg = render_svg(&doc, "c");
        let first = svg.split("id=\"p-0-0\"").nth(1).unwrap();
        let cx: f64 = first
            .split("cx=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let second = svg.split("id=\"p-0-1\"").nth(1).unwrap();
        let cx2: f64 = second
            .split("cx=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        // Row 0 holds `9`, so it is the rightmost point despite being drawn first.
        assert!(cx > cx2, "row 0 should sit right of row 1: {cx} vs {cx2}");
    }

    #[test]
    fn x_names_the_column_the_categories_come_from() {
        let doc = parse_chart(
            "type: line\nx: year\ndata: |\n  value, year\n  3, 2020\n  7, 2024\n",
            none,
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let t = doc.table.unwrap();
        assert_eq!(t.category_name, "year");
        assert_eq!(t.categories, vec!["2020", "2024"]);
        assert_eq!(t.series.len(), 1);
        assert_eq!(t.series[0].name, "value");
        assert_eq!(t.series[0].values, vec![3.0, 7.0]);
    }

    #[test]
    fn x_naming_no_column_reports_an_error() {
        let doc = parse_chart("type: line\nx: nope\ndata: |\n  k, v\n  a, 1\n", none);
        assert!(
            doc.errors.iter().any(|e| e.contains("nope")),
            "{:?}",
            doc.errors
        );
    }

    #[test]
    fn an_unknown_key_warns_and_the_chart_still_draws() {
        let doc = parse_chart("type: bar\nylabel: ms\ndata: |\n  k, v\n  a, 1\n", none);
        assert!(
            doc.errors.iter().any(|e| e.contains("ylabel")),
            "{:?}",
            doc.errors
        );
        assert!(
            !render_svg(&doc, "c").is_empty(),
            "the chart is still drawn"
        );
    }

    #[test]
    fn every_key_the_spec_reads_is_a_known_key() {
        let src = "type: line\nid: i\ntitle: t\nx: k\ny_label: y\nstacked: true\n\
                   legend: false\ncolors: [\"@accent1\"]\nhighlight: v\ndata: |\n  k, v\n  1, 2\n";
        let doc = parse_chart(src, none);
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
    }

    #[test]
    fn tick_labels_that_would_overlap_are_dropped() {
        // Twenty rows of six characters cannot all fit across 634px.
        let rows: String = (0..20)
            .map(|i| format!("  {}0000{i}, 1\n", i % 10))
            .collect();
        let doc = parse_chart(&format!("type: bar\ndata: |\n  k, v\n{rows}"), none);
        let svg = render_svg(&doc, "c");
        let drawn = ticks(&svg).len();
        // Five value ticks on the y axis are always there; the rest are rows.
        assert!(
            drawn < 20 + 5,
            "{drawn} labels drawn, expected some dropped"
        );
    }

    #[test]
    fn axis_max_is_rounded_up() {
        assert_eq!(nice_ceil(7.0), 10.0);
        assert_eq!(nice_ceil(120.0), 200.0);
        assert_eq!(nice_ceil(45.0), 50.0);
    }
}
