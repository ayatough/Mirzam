//! Converts the ASCII art in a `pane` block into a proportional grid.
//!
//! The semantics map one-to-one onto CSS Grid's `grid-template-areas`:
//! - `+ - |` draw cell borders; the identifier inside a cell names the pane
//! - Cells sharing a name merge (the merged region must be rectangular)
//! - Column widths and row heights follow the character ratios between grid lines

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct GridSpec {
    /// Column widths, as character ratios used for `fr` values.
    pub cols: Vec<usize>,
    /// Row heights, as line-count ratios used for `fr` values.
    pub rows: Vec<usize>,
    /// `areas[row][col]` is the pane name, or `None` for an empty cell.
    pub areas: Vec<Vec<Option<String>>>,
}

impl GridSpec {
    /// Unique pane names, in order of first appearance.
    pub fn pane_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for row in &self.areas {
            for cell in row.iter().flatten() {
                if !names.contains(cell) {
                    names.push(cell.clone());
                }
            }
        }
        names
    }

    /// The CSS `grid-template-areas` value (empty cells become `.`).
    pub fn css_areas(&self) -> String {
        self.areas
            .iter()
            .map(|row| {
                let cells: Vec<&str> = row.iter().map(|c| c.as_deref().unwrap_or(".")).collect();
                format!("\"{}\"", cells.join(" "))
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn css_columns(&self) -> String {
        self.cols
            .iter()
            .map(|w| format!("{w}fr"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn css_rows(&self) -> String {
        self.rows
            .iter()
            .map(|h| format!("{h}fr"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GridError {
    /// Fewer than two border rows.
    TooFewBorders,
    /// Fewer than two column borders (`+`).
    TooFewColumns,
    /// The merged region for a pane is not rectangular.
    NonRectangularArea(String),
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GridError::TooFewBorders => {
                write!(f, "a pane block needs at least two border rows (`+---+`)")
            }
            GridError::TooFewColumns => write!(f, "a pane block needs more column borders (`+`)"),
            GridError::NonRectangularArea(name) => {
                write!(f, "the merged region for pane `{name}` is not rectangular")
            }
        }
    }
}

impl std::error::Error for GridError {}

/// Parses an ASCII grid.
pub fn parse_grid(src: &str) -> Result<GridSpec, GridError> {
    // Keep lines as char matrices; trailing whitespace is irrelevant.
    let lines: Vec<Vec<char>> = src
        .lines()
        .map(|l| l.trim_end().chars().collect::<Vec<char>>())
        .filter(|l| !l.is_empty())
        .collect();

    // Border rows are those whose first non-space character is '+'.
    let border_idx: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.iter().find(|c| !c.is_whitespace()) == Some(&'+'))
        .map(|(i, _)| i)
        .collect();
    if border_idx.len() < 2 {
        return Err(GridError::TooFewBorders);
    }

    // Column borders: the union of '+' x-positions across all border rows.
    let mut col_pos: Vec<usize> = Vec::new();
    for &bi in &border_idx {
        for (x, &c) in lines[bi].iter().enumerate() {
            if c == '+' && !col_pos.contains(&x) {
                col_pos.push(x);
            }
        }
    }
    col_pos.sort_unstable();
    if col_pos.len() < 2 {
        return Err(GridError::TooFewColumns);
    }

    // Track widths, counting the interior characters only.
    let cols: Vec<usize> = col_pos
        .windows(2)
        .map(|w| (w[1] - w[0]).saturating_sub(1).max(1))
        .collect();

    // Row bands sit between adjacent border rows; height is the line count (min 1).
    let mut rows: Vec<usize> = Vec::new();
    let mut areas: Vec<Vec<Option<String>>> = Vec::new();

    for w in border_idx.windows(2) {
        let (top, bottom) = (w[0], w[1]);
        let content: Vec<&Vec<char>> = lines[top + 1..bottom].iter().collect();
        rows.push(content.len().max(1));

        // Resolve the pane name for each track.
        // Content lines split into '|'-delimited segments; a segment can span
        // several tracks, since merged cells omit the interior '|'.
        let mut band: Vec<Option<String>> = vec![None; cols.len()];
        for (ti, tw) in col_pos.windows(2).enumerate() {
            let mid = (tw[0] + tw[1]) / 2;
            // Take the segment containing this track's midpoint.
            let mut name: Option<String> = None;
            for line in &content {
                let seg = segment_at(line, mid);
                let text: String = seg.trim().to_string();
                if !text.is_empty() && text != "." {
                    name = Some(text);
                    break;
                }
            }
            band[ti] = name;
        }
        areas.push(band);
    }

    // Verify merged regions are rectangular (a grid-template-areas constraint).
    validate_rectangular(&areas)?;

    Ok(GridSpec { cols, rows, areas })
}

/// Returns the text of the `|`-delimited segment containing position `x`.
fn segment_at(line: &[char], x: usize) -> String {
    let mut start = 0usize;
    let mut segments: Vec<(usize, usize, String)> = Vec::new();
    let mut cur = String::new();
    for (i, &c) in line.iter().enumerate() {
        if c == '|' {
            segments.push((start, i, cur.clone()));
            cur.clear();
            start = i + 1;
        } else {
            cur.push(c);
        }
    }
    segments.push((start, line.len().max(start), cur));
    for (s, e, text) in segments {
        if x >= s && x < e {
            return text;
        }
    }
    String::new()
}

fn validate_rectangular(areas: &[Vec<Option<String>>]) -> Result<(), GridError> {
    let mut boxes: BTreeMap<&str, (usize, usize, usize, usize)> = BTreeMap::new();
    for (r, row) in areas.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if let Some(name) = cell {
                let e = boxes.entry(name.as_str()).or_insert((r, c, r, c));
                e.0 = e.0.min(r);
                e.1 = e.1.min(c);
                e.2 = e.2.max(r);
                e.3 = e.3.max(c);
            }
        }
    }
    for (name, (r0, c0, r1, c1)) in &boxes {
        for row in areas.iter().take(*r1 + 1).skip(*r0) {
            for cell in row.iter().take(*c1 + 1).skip(*c0) {
                if cell.as_deref() != Some(*name) {
                    return Err(GridError::NonRectangularArea(name.to_string()));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_two_columns() {
        let g = parse_grid(
            "+------+---+\n\
             | a    | b |\n\
             +------+---+\n",
        )
        .unwrap();
        assert_eq!(g.cols, vec![6, 3]);
        assert_eq!(g.rows, vec![1]);
        assert_eq!(g.areas, vec![vec![Some("a".into()), Some("b".into())]]);
        assert_eq!(g.css_areas(), "\"a b\"");
        assert_eq!(g.css_columns(), "6fr 3fr");
    }

    #[test]
    fn horizontal_span_over_internal_border() {
        // `head` spans the top row: the border has a '+' but the content line has no '|'.
        let g = parse_grid(
            "+--------------------+-------------+\n\
             |  head                            |\n\
             +--------------------+-------------+\n\
             |                    |             |\n\
             |  main              |  fig        |\n\
             |                    |             |\n\
             +--------------------+-------------+\n\
             |  foot                            |\n\
             +----------------------------------+\n",
        )
        .unwrap();
        assert_eq!(g.cols, vec![20, 13]);
        assert_eq!(g.rows, vec![1, 3, 1]);
        assert_eq!(g.css_areas(), "\"head head\" \"main fig\" \"foot foot\"");
        assert_eq!(g.pane_names(), vec!["head", "main", "fig", "foot"]);
    }

    #[test]
    fn vertical_span_by_repeating_name() {
        let g = parse_grid(
            "+-----+-----+\n\
             | nav | a   |\n\
             +-----+-----+\n\
             | nav | b   |\n\
             +-----+-----+\n",
        )
        .unwrap();
        assert_eq!(g.css_areas(), "\"nav a\" \"nav b\"");
    }

    #[test]
    fn empty_cell_with_dot() {
        let g = parse_grid(
            "+-----+-----+\n\
             | a   | .   |\n\
             +-----+-----+\n",
        )
        .unwrap();
        assert_eq!(g.areas[0][1], None);
        assert_eq!(g.css_areas(), "\"a .\"");
    }

    #[test]
    fn row_height_follows_line_count() {
        let g = parse_grid(
            "+-----+\n\
             | a   |\n\
             | a?  |\n\
             +-----+\n\
             | b   |\n\
             +-----+\n",
        );
        // A band with two content lines has height 2.
        let g = g.unwrap();
        assert_eq!(g.rows, vec![2, 1]);
    }

    #[test]
    fn non_rectangular_area_is_error() {
        // An L-shaped `a` is rejected.
        let err = parse_grid(
            "+-----+-----+\n\
             | a   | a   |\n\
             +-----+-----+\n\
             | a   | b   |\n\
             +-----+-----+\n",
        )
        .unwrap_err();
        assert_eq!(err, GridError::NonRectangularArea("a".into()));
    }

    #[test]
    fn too_few_borders() {
        assert_eq!(parse_grid("| a |\n").unwrap_err(), GridError::TooFewBorders);
    }
}
