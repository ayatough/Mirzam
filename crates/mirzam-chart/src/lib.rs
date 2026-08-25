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
    /// Points with no line through them.
    Scatter,
    Pie,
}

/// Whether a bar chart's series stand beside each other or on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stacking {
    /// Side by side, sharing the category's slot.
    #[default]
    None,
    /// On top of each other, the axis still counting the values.
    Sum,
    /// On top of each other and every column filled, so the segments read as
    /// shares of that column rather than as amounts.
    Percent,
}

impl<'de> Deserialize<'de> for Stacking {
    /// `true` is the answer most people mean, so it is the one a bare bool
    /// gives; `percent` is the other question the same key can ask.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Flag(bool),
            Name(String),
        }
        match Repr::deserialize(d)? {
            Repr::Flag(false) => Ok(Stacking::None),
            Repr::Flag(true) => Ok(Stacking::Sum),
            Repr::Name(name) => match name.trim().to_ascii_lowercase().as_str() {
                "none" | "no" => Ok(Stacking::None),
                "sum" | "yes" => Ok(Stacking::Sum),
                "percent" | "100%" | "share" => Ok(Stacking::Percent),
                other => Err(serde::de::Error::custom(format!(
                    "`stacked: {other}` is not one of true, false or percent"
                ))),
            },
        }
    }
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
    /// Stand the series of a bar chart on top of each other.
    pub stacked: Stacking,
    /// Lay a bar chart's categories down the side instead of along the bottom.
    pub horizontal: bool,
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
    if spec.stacked != Stacking::None && spec.kind != ChartKind::Bar {
        errors.push("chart: `stacked` applies to bar charts, ignored".to_string());
    }
    if spec.horizontal && spec.kind != ChartKind::Bar {
        errors.push("chart: `horizontal` applies to bar charts, ignored".to_string());
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
    "horizontal",
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
        ChartKind::Bar if spec.horizontal => render_bars_h(&mut svg, spec, table, &id, w, h),
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

/// Bars laid along the bottom rather than up the side. The two axes swap
/// roles, so this is its own pass rather than a flag threaded through the
/// vertical one: what shares between them is the arithmetic above, not the
/// geometry.
///
/// It exists for the case a vertical bar chart is bad at. Category names long
/// enough to be worth reading - "Deployment frequency", a survey question, a
/// country - have nowhere to go under a column, and the ranked list they
/// usually form reads top to bottom anyway.
fn render_bars_h(svg: &mut String, spec: &ChartSpec, table: &Table, id: &str, w: f64, h: f64) {
    let stack = stacking(spec);
    let max = axis_max(table, stack);

    // The names sit in the margin now, so the margin is as wide as the longest
    // of them - within reason, since past a point the bars have nowhere left.
    let longest = table
        .categories
        .iter()
        .map(|c| c.chars().count())
        .max()
        .unwrap_or(0) as f64;
    let left = (9.0 * longest + 20.0).clamp(66.0, w * 0.42);
    let right = 44.0; // room for the number written past the end of a bar
    let top = if spec.title.is_some() { 46.0 } else { 20.0 };
    let legend = table.series.len() > 1;
    let bottom = if legend { 66.0 } else { 44.0 } + if spec.y_label.is_some() { 22.0 } else { 0.0 };
    let (pw, ph) = (w - left - right, h - top - bottom);

    // The value axis runs along the bottom: its grid lines stand up.
    for i in 0..=4 {
        let v = max * i as f64 / 4.0;
        let x = left + pw * (i as f64 / 4.0);
        let _ = write!(
            svg,
            "<line class=\"mz-chart-grid\" x1=\"{x:.1}\" y1=\"{top:.1}\" x2=\"{x:.1}\" y2=\"{:.1}\"/>\
             <text class=\"mz-chart-tick\" x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            top + ph,
            top + ph + 22.0,
            if stack == Stacking::Percent {
                format!("{}%", num(v))
            } else {
                num(v)
            }
        );
    }
    if let Some(label) = &spec.y_label {
        let _ = write!(
            svg,
            "<text class=\"mz-chart-axis\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            left + pw / 2.0,
            top + ph + 44.0,
            esc(label)
        );
    }

    let n = table.categories.len().max(1) as f64;
    let slot = ph / n;
    let y_at = |ci: usize| top + slot * (ci as f64 + 0.5);

    // Category names down the side, dropping any that would run into the one
    // above. Unlike the horizontal axis these are all the same height, so the
    // test is the row pitch rather than the text.
    let mut filled = f64::NEG_INFINITY;
    for (ci, cat) in table.categories.iter().enumerate() {
        let y = y_at(ci);
        if y - 9.0 < filled {
            continue;
        }
        filled = y + 9.0 + 2.0;
        let _ = write!(
            svg,
            "<text class=\"mz-chart-tick\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
            left - 10.0,
            y + 5.0,
            esc(cat)
        );
    }

    let dim = |si: usize| -> &'static str {
        match &spec.highlight {
            Some(name) if table.series[si].name != *name => " opacity=\"0.32\"",
            _ => "",
        }
    };

    let count = table.series.len().max(1) as f64;
    let group = slot * 0.68;
    let thick = if stack == Stacking::None {
        group / count
    } else {
        group
    };
    let mut base = vec![0.0f64; table.categories.len()];
    for (si, s) in table.series.iter().enumerate() {
        let c = color_for(spec, si);
        for ci in 0..s.values.len() {
            let Some((shown, label)) = segment(table, stack, si, ci) else {
                continue;
            };
            let bw = (shown / max) * pw;
            let foot = match stack {
                Stacking::None => 0.0,
                _ => base.get(ci).copied().unwrap_or(0.0),
            };
            let x = left + foot;
            let y = top
                + slot * ci as f64
                + (slot - group) / 2.0
                + if stack == Stacking::None {
                    thick * si as f64
                } else {
                    0.0
                };
            if let Some(b) = base.get_mut(ci) {
                *b += bw;
            }
            let height = (thick - 3.0).max(1.0);
            let _ = write!(
                svg,
                "<g id=\"{id}-{si}-{ci}\"><rect class=\"mz-chart-bar\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{bw:.1}\" height=\"{height:.1}\" rx=\"3\" fill=\"{c}\"{}/>",
                dim(si)
            );
            // Past the end of the bar when there is only one, inside the
            // segment when the next one starts where the space was.
            let room = if stack == Stacking::None {
                true
            } else {
                bw >= 9.0 * label.chars().count() as f64
            };
            if room {
                let _ = write!(
                    svg,
                    "<text class=\"mz-chart-value{}\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"{}\">{label}</text>",
                    if stack == Stacking::None { "" } else { " mz-inset" },
                    if stack == Stacking::None {
                        x + bw + 6.0
                    } else {
                        x + bw / 2.0
                    },
                    y + height / 2.0 + 5.0,
                    if stack == Stacking::None {
                        "start"
                    } else {
                        "middle"
                    }
                );
            }
            svg.push_str("</g>");
        }
    }
}

/// How this chart's bars are put together. Only bars stack; `parse_chart` says
/// so when another kind asks to.
fn stacking(spec: &ChartSpec) -> Stacking {
    if spec.kind == ChartKind::Bar {
        spec.stacked
    } else {
        Stacking::None
    }
}

/// How far the value axis has to reach. Stacked, that is the tallest *column*
/// rather than the tallest value in it; as shares, it is always exactly 100.
fn axis_max(table: &Table, stack: Stacking) -> f64 {
    match stack {
        Stacking::Percent => 100.0,
        Stacking::Sum => nice_ceil(
            (0..table.categories.len())
                .map(|ci| column_total(table, ci))
                .fold(0.0f64, f64::max)
                .max(1.0),
        ),
        Stacking::None => nice_ceil(
            table
                .series
                .iter()
                .flat_map(|s| s.values.iter())
                .fold(0.0f64, |a, b| a.max(*b))
                .max(1.0),
        ),
    }
}

/// One segment's length along the value axis, and the number written on it.
/// `None` when the column has no shares to divide.
fn segment(table: &Table, stack: Stacking, si: usize, ci: usize) -> Option<(f64, String)> {
    let v = *table.series.get(si)?.values.get(ci)?;
    match stack {
        Stacking::Percent => {
            let total = column_total(table, ci);
            (total > 0.0).then(|| {
                let share = v / total * 100.0;
                (share, format!("{}%", num(share)))
            })
        }
        _ => Some((v, num(v))),
    }
}

fn render_cartesian(svg: &mut String, spec: &ChartSpec, table: &Table, id: &str, w: f64, h: f64) {
    let (left, right, top) = (66.0, 20.0, if spec.title.is_some() { 46.0 } else { 20.0 });
    let bottom = if table.series.len() > 1 { 66.0 } else { 44.0 };
    let (pw, ph) = (w - left - right, h - top - bottom);

    let stack = stacking(spec);
    let max = axis_max(table, stack);

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
            if stack == Stacking::Percent {
                format!("{}%", num(v))
            } else {
                num(v)
            }
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
    let span = matches!(
        spec.kind,
        ChartKind::Line | ChartKind::Area | ChartKind::Scatter
    )
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

    // The x axis is labelled row by row while every row's label fits, because
    // those are the words the author wrote - `00` is an hour, `2024 Q1` is a
    // quarter. Once they stop fitting, showing whichever subset happened to
    // survive is worse than showing the scale itself, so a value axis takes
    // over. A category axis has no scale to fall back on and keeps as many
    // labels as it can.
    //
    // The widths are estimates: the renderer has no font metrics, so they are
    // deliberately generous. A label too few is better than two overlapping.
    let baseline = top + ph + 22.0;
    let tick = |svg: &mut String, x: f64, text: &str| {
        let _ = write!(
            svg,
            "<text class=\"mz-chart-tick\" x=\"{x:.1}\" y=\"{baseline:.1}\" text-anchor=\"middle\">{}</text>",
            esc(text)
        );
    };
    let fits = |xs: &[(f64, String)]| {
        let mut filled = f64::NEG_INFINITY;
        xs.iter().all(|(x, text)| {
            let half = 4.5 * text.chars().count() as f64;
            let room = x - half >= filled;
            filled = x + half + 6.0;
            room
        })
    };

    let rows: Vec<(f64, String)> = order
        .iter()
        .map(|&ci| (x_at(ci), table.categories[ci].clone()))
        .collect();
    match span {
        Some((_, lo, hi)) if !fits(&rows) => {
            for (x, text) in value_ticks(lo, hi) {
                tick(
                    svg,
                    left + inset + (x - lo) / (hi - lo) * (pw - 2.0 * inset),
                    &text,
                );
            }
        }
        _ => {
            let mut filled = f64::NEG_INFINITY;
            for (x, text) in &rows {
                let half = 4.5 * text.chars().count() as f64;
                if x - half < filled {
                    continue;
                }
                filled = x + half + 6.0;
                tick(svg, *x, text);
            }
        }
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
            // Stacked, the slot holds one bar; side by side, it is shared.
            let bw = if stack == Stacking::None {
                group / count
            } else {
                group
            };
            // How much of each column is already drawn, for the next segment
            // to stand on.
            let mut base = vec![0.0f64; table.categories.len()];
            for (si, s) in table.series.iter().enumerate() {
                let c = color_for(spec, si);
                for ci in 0..s.values.len() {
                    // A column that sums to nothing has no shares to draw.
                    let Some((shown, label)) = segment(table, stack, si, ci) else {
                        continue;
                    };
                    let bh = (shown / max) * ph;
                    let x = left
                        + slot * ci as f64
                        + (slot - group) / 2.0
                        + if stack == Stacking::None {
                            bw * si as f64
                        } else {
                            0.0
                        };
                    // Side by side, every bar stands on the axis; stacked, it
                    // stands on what this column has drawn so far.
                    let foot = match stack {
                        Stacking::None => 0.0,
                        _ => base.get(ci).copied().unwrap_or(0.0),
                    };
                    let y = top + ph - bh - foot;
                    if let Some(b) = base.get_mut(ci) {
                        *b += bh;
                    }
                    // The mark's id sits on a group holding the bar *and* its
                    // value label, so animating `#chart-0-1` moves the number
                    // with the bar instead of leaving it floating.
                    let _ = write!(
                        svg,
                        "<g id=\"{id}-{si}-{ci}\"><rect class=\"mz-chart-bar\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{:.1}\" height=\"{bh:.1}\" rx=\"3\" fill=\"{c}\"{}/>",
                        (bw - 3.0).max(1.0),
                        dim(si)
                    );
                    // Stacked, the number goes inside its own segment - above
                    // the bar is where the segment above it is - and only when
                    // the segment is deep enough to hold it.
                    if stack == Stacking::None || bh >= 22.0 {
                        let _ = write!(
                            svg,
                            "<text class=\"mz-chart-value{}\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{label}</text>",
                            if stack == Stacking::None { "" } else { " mz-inset" },
                            x + (bw - 3.0).max(1.0) / 2.0,
                            if stack == Stacking::None {
                                y - 6.0
                            } else {
                                y + bh / 2.0 + 5.0
                            }
                        );
                    }
                    svg.push_str("</g>");
                }
            }
        }
        ChartKind::Scatter => {
            for (si, s) in table.series.iter().enumerate() {
                let c = color_for(spec, si);
                for (ci, v) in s.values.iter().enumerate() {
                    let _ = write!(
                        svg,
                        "<circle class=\"mz-chart-point\" id=\"{id}-{si}-{ci}\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"5\" fill=\"{c}\"{}/>",
                        x_at(ci),
                        top + ph - (v / max) * ph,
                        dim(si)
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

/// What one category's series add up to.
fn column_total(table: &Table, ci: usize) -> f64 {
    table.series.iter().filter_map(|s| s.values.get(ci)).sum()
}

/// Round numbers across a range, for an axis whose rows are too crowded to
/// label one by one. The range itself is not moved - the points are where the
/// data put them - so the first and last tick sit inside it rather than on it.
fn value_ticks(lo: f64, hi: f64) -> Vec<(f64, String)> {
    let step = nice_ceil((hi - lo) / 4.0);
    if step <= 0.0 || !step.is_finite() {
        return Vec::new();
    }
    let first = (lo / step).ceil() * step;
    let mut out = Vec::new();
    let mut v = first;
    while v <= hi + step * 1e-9 && out.len() < 12 {
        // `-0` is the same tick as `0` and reads worse.
        out.push((v, num(if v == 0.0 { 0.0 } else { v })));
        v += step;
    }
    out
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
        let src = "type: bar\nid: i\ntitle: t\nx: k\ny_label: y\nstacked: true\n\
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

    /// One mark's bar as `(x, y, width, height)`.
    fn bar(svg: &str, mark: &str) -> (f64, f64, f64, f64) {
        let g = svg
            .split(&format!("<g id=\"{mark}\">"))
            .nth(1)
            .unwrap_or_else(|| panic!("no mark {mark} in {svg}"));
        let attr = |name: &str| -> f64 {
            g.split(&format!("{name}=\""))
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap()
                .parse()
                .unwrap()
        };
        (attr("x"), attr("y"), attr("width"), attr("height"))
    }

    #[test]
    fn stacked_bars_stand_on_each_other_in_one_slot() {
        let doc = parse_chart(
            "type: bar\nid: b\nstacked: true\ndata: |\n  k, a, c\n  x, 3, 1\n",
            none,
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let svg = render_svg(&doc, "c");
        let (x0, y0, w0, h0) = bar(&svg, "b-0-0");
        let (x1, y1, w1, _) = bar(&svg, "b-1-0");
        assert!((x0 - x1).abs() < 0.2, "one slot, not two: {x0} vs {x1}");
        assert!((w0 - w1).abs() < 0.2, "same width: {w0} vs {w1}");
        // The second series starts where the first one ended.
        assert!(
            (y1 + bar(&svg, "b-1-0").3 - y0).abs() < 0.2,
            "{y1} should end at {y0}"
        );
        assert!(h0 > 0.0);
    }

    #[test]
    fn grouped_bars_still_stand_side_by_side() {
        let doc = parse_chart("type: bar\nid: b\ndata: |\n  k, a, c\n  x, 3, 1\n", none);
        let svg = render_svg(&doc, "c");
        let (x0, y0, w0, h0) = bar(&svg, "b-0-0");
        let (x1, y1, _, h1) = bar(&svg, "b-1-0");
        assert!(x1 > x0 + w0 - 4.0, "{x1} should sit right of {x0}+{w0}");
        // Both stand on the axis. Carrying the stacking baseline into a
        // grouped chart would lift the second series off it.
        assert!(
            (y0 + h0 - (y1 + h1)).abs() < 0.2,
            "both feet on the axis: {} vs {}",
            y0 + h0,
            y1 + h1
        );
    }

    /// Unstacked the axis reaches the tallest bar; stacked it has to reach the
    /// tallest column, or the top segment leaves the chart.
    #[test]
    fn a_stacked_axis_reaches_the_column_total() {
        let flat = parse_chart("type: bar\nid: b\ndata: |\n  k, a, c\n  x, 60, 60\n", none);
        let piled = parse_chart(
            "type: bar\nid: b\nstacked: true\ndata: |\n  k, a, c\n  x, 60, 60\n",
            none,
        );
        // 60 rounds to an axis of 100; 120 rounds to 200, so the same bar is
        // half the height it was.
        let tall = bar(&render_svg(&flat, "c"), "b-0-0").3;
        let short = bar(&render_svg(&piled, "c"), "b-0-0").3;
        assert!(short < tall * 0.7, "{short} should be well under {tall}");
    }

    #[test]
    fn percent_stacking_fills_every_column() {
        let doc = parse_chart(
            "type: bar\nid: b\nstacked: percent\ndata: |\n  k, a, c\n  x, 1, 3\n  y, 90, 10\n",
            none,
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let svg = render_svg(&doc, "c");
        let col = |ci: usize| bar(&svg, &format!("b-0-{ci}")).3 + bar(&svg, &format!("b-1-{ci}")).3;
        assert!(
            (col(0) - col(1)).abs() < 0.3,
            "both columns fill the axis: {} vs {}",
            col(0),
            col(1)
        );
        // `a` is a quarter of the first column and nine tenths of the second.
        let a0 = bar(&svg, "b-0-0").3;
        assert!((a0 / col(0) - 0.25).abs() < 0.01, "{a0} of {}", col(0));
        assert!(svg.contains("100%"), "the axis is labelled in shares");
    }

    #[test]
    fn percent_stacking_skips_a_column_that_sums_to_nothing() {
        let doc = parse_chart(
            "type: bar\nid: b\nstacked: percent\ndata: |\n  k, a, c\n  x, 0, 0\n  y, 1, 1\n",
            none,
        );
        let svg = render_svg(&doc, "c");
        assert!(
            !svg.contains("id=\"b-0-0\""),
            "nothing to draw for an empty column"
        );
        assert!(svg.contains("id=\"b-0-1\""));
    }

    #[test]
    fn stacking_a_chart_that_does_not_stack_warns() {
        let doc = parse_chart("type: line\nstacked: true\ndata: |\n  k, a\n  x, 1\n", none);
        assert!(
            doc.errors.iter().any(|e| e.contains("stacked")),
            "{:?}",
            doc.errors
        );
        assert!(!render_svg(&doc, "c").is_empty(), "the line is still drawn");
    }

    #[test]
    fn a_stacking_mode_nobody_defined_is_refused() {
        let doc = parse_chart(
            "type: bar\nstacked: sideways\ndata: |\n  k, a\n  x, 1\n",
            none,
        );
        assert!(
            doc.errors.iter().any(|e| e.contains("sideways")),
            "{:?}",
            doc.errors
        );
    }

    #[test]
    fn scatter_draws_points_with_no_line_through_them() {
        let doc = parse_chart(
            "type: scatter\nid: p\ndata: |\n  x, y\n  1, 2\n  4, 3\n  9, 1\n",
            none,
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let svg = render_svg(&doc, "c");
        assert_eq!(svg.matches("mz-chart-point").count(), 3);
        assert!(!svg.contains("mz-chart-line"), "a scatter has no line");
        assert!(!svg.contains("mz-chart-area"));
        assert!(svg.contains("id=\"p-0-2\""), "marks keep their row ids");
    }

    #[test]
    fn scatter_places_points_by_value() {
        let doc = parse_chart(
            "type: scatter\ndata: |\n  x, y\n  0, 1\n  1, 1\n  9, 1\n",
            none,
        );
        let xs = point_xs(&render_svg(&doc, "c"));
        let ratio = (xs[2] - xs[1]) / (xs[1] - xs[0]);
        assert!(
            (ratio - 8.0).abs() < 0.05,
            "{ratio} should be 8, from {xs:?}"
        );
    }

    /// Two series over one shared x column, which is what a wide table holds.
    #[test]
    fn scatter_colours_a_second_series() {
        let doc = parse_chart(
            "type: scatter\nid: p\ndata: |\n  x, a, b\n  1, 2, 5\n  4, 3, 6\n",
            none,
        );
        let svg = render_svg(&doc, "c");
        assert!(svg.contains("id=\"p-1-0\""));
        assert_eq!(svg.matches("mz-chart-point").count(), 4);
    }

    /// While every row's label fits, those are the words the author wrote.
    #[test]
    fn a_few_rows_keep_their_own_labels() {
        let doc = parse_chart(
            "type: line\ndata: |\n  hour, v\n  00, 1\n  04, 2\n  08, 3\n",
            none,
        );
        let t = ticks(&render_svg(&doc, "c"));
        assert!(t.contains(&"00".to_string()), "{t:?}");
        assert!(t.contains(&"08".to_string()), "{t:?}");
    }

    /// Once they stop fitting, the scale is better than whichever subset of
    /// them happened to survive.
    #[test]
    fn crowded_rows_give_way_to_a_value_axis() {
        let rows: String = (0..60).map(|i| format!("  {i}, 1\n", i = i)).collect();
        let doc = parse_chart(&format!("type: scatter\ndata: |\n  x, y\n{rows}"), none);
        let t = ticks(&render_svg(&doc, "c"));
        // Five value ticks on the y axis, then round numbers across 0..59.
        assert!(t.contains(&"20".to_string()), "{t:?}");
        assert!(t.contains(&"40".to_string()), "{t:?}");
        assert!(!t.contains(&"37".to_string()), "no row labels left: {t:?}");
    }

    #[test]
    fn a_category_axis_never_falls_back_to_numbers() {
        let rows: String = (0..40).map(|i| format!("  label-{i}, 1\n")).collect();
        let doc = parse_chart(&format!("type: bar\ndata: |\n  k, v\n{rows}"), none);
        let t = ticks(&render_svg(&doc, "c"));
        assert!(
            t.iter().any(|s| s.starts_with("label-")),
            "text categories have no scale to fall back on: {t:?}"
        );
    }

    #[test]
    fn value_ticks_are_round_numbers_inside_the_range() {
        let t: Vec<String> = value_ticks(0.0, 59.0).into_iter().map(|(_, s)| s).collect();
        assert_eq!(t, vec!["0", "20", "40"]);
        let t: Vec<String> = value_ticks(2020.0, 2024.0)
            .into_iter()
            .map(|(_, s)| s)
            .collect();
        assert_eq!(t, vec!["2020", "2021", "2022", "2023", "2024"]);
    }

    #[test]
    fn horizontal_bars_run_from_one_left_edge() {
        let doc = parse_chart(
            "type: bar\nid: b\nhorizontal: true\ndata: |\n  k, v\n  a, 10\n  b, 40\n",
            none,
        );
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        let svg = render_svg(&doc, "c");
        let (x0, y0, w0, _) = bar(&svg, "b-0-0");
        let (x1, y1, w1, _) = bar(&svg, "b-0-1");
        assert!(
            (x0 - x1).abs() < 0.2,
            "both start at the axis: {x0} vs {x1}"
        );
        assert!(w1 > w0 * 3.0, "40 is four times 10: {w1} vs {w0}");
        assert!(y1 > y0, "rows run down the side: {y0} then {y1}");
    }

    #[test]
    fn horizontal_categories_sit_in_the_margin() {
        let doc = parse_chart(
            "type: bar\nhorizontal: true\ndata: |\n  k, v\n  Mean time to restore, 41\n",
            none,
        );
        let svg = render_svg(&doc, "c");
        assert!(
            svg.contains("text-anchor=\"end\">Mean time to restore</text>"),
            "{svg}"
        );
    }

    /// A name worth reading is the reason to lay a bar chart on its side, so
    /// the margin has to grow to hold one.
    #[test]
    fn a_long_name_widens_the_margin_and_a_short_one_does_not() {
        let short = parse_chart(
            "type: bar\nid: b\nhorizontal: true\ndata: |\n  k, v\n  a, 1\n",
            none,
        );
        let long = parse_chart(
            "type: bar\nid: b\nhorizontal: true\ndata: |\n  k, v\n  Change failure rate, 1\n",
            none,
        );
        let near = bar(&render_svg(&short, "c"), "b-0-0").0;
        let far = bar(&render_svg(&long, "c"), "b-0-0").0;
        assert!(far > near, "{far} should be further right than {near}");
    }

    #[test]
    fn horizontal_stacks_along_the_row() {
        let doc = parse_chart(
            "type: bar\nid: b\nhorizontal: true\nstacked: true\ndata: |\n  k, a, c\n  x, 3, 1\n",
            none,
        );
        let svg = render_svg(&doc, "c");
        let (x0, y0, w0, h0) = bar(&svg, "b-0-0");
        let (x1, y1, _, h1) = bar(&svg, "b-1-0");
        assert!(
            (x0 + w0 - x1).abs() < 0.2,
            "{x1} should start at {}",
            x0 + w0
        );
        assert!((y0 - y1).abs() < 0.2, "one row, not two: {y0} vs {y1}");
        assert!((h0 - h1).abs() < 0.2);
    }

    #[test]
    fn horizontal_grouped_rows_do_not_carry_a_stacking_baseline() {
        let doc = parse_chart(
            "type: bar\nid: b\nhorizontal: true\ndata: |\n  k, a, c\n  x, 3, 1\n",
            none,
        );
        let svg = render_svg(&doc, "c");
        let (x0, y0, _, h0) = bar(&svg, "b-0-0");
        let (x1, y1, _, _) = bar(&svg, "b-1-0");
        assert!(
            (x0 - x1).abs() < 0.2,
            "both start at the axis: {x0} vs {x1}"
        );
        assert!(y1 > y0 + h0 - 4.0, "{y1} should sit below {y0}+{h0}");
    }

    #[test]
    fn laying_a_chart_that_is_not_a_bar_chart_on_its_side_warns() {
        let doc = parse_chart(
            "type: line\nhorizontal: true\ndata: |\n  k, v\n  a, 1\n",
            none,
        );
        assert!(
            doc.errors.iter().any(|e| e.contains("horizontal")),
            "{:?}",
            doc.errors
        );
        assert!(!render_svg(&doc, "c").is_empty(), "the line is still drawn");
    }

    #[test]
    fn axis_max_is_rounded_up() {
        assert_eq!(nice_ceil(7.0), 10.0);
        assert_eq!(nice_ceil(120.0), 200.0);
        assert_eq!(nice_ceil(45.0), 50.0);
    }
}
