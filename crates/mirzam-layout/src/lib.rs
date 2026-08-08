//! ASCII アートで書かれた `pane` ブロックを比率グリッドに変換する。
//!
//! 意味論は CSS Grid の `grid-template-areas` と一対一:
//! - `+ - |` がセル境界、セル内の識別子がペイン名
//! - 同名セルは結合(結合領域は矩形でなければならない)
//! - 列幅・行高はグリッド線間の文字数比で決まる

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct GridSpec {
    /// 各列の幅(fr 値として使う文字数比)
    pub cols: Vec<usize>,
    /// 各行の高さ(fr 値として使う行数比)
    pub rows: Vec<usize>,
    /// areas[row][col] = ペイン名(空セルは None)
    pub areas: Vec<Vec<Option<String>>>,
}

impl GridSpec {
    /// 出現順のユニークなペイン名一覧
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

    /// CSS の grid-template-areas 文字列(空セルは `.`)
    pub fn css_areas(&self) -> String {
        self.areas
            .iter()
            .map(|row| {
                let cells: Vec<&str> = row
                    .iter()
                    .map(|c| c.as_deref().unwrap_or("."))
                    .collect();
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
    /// 罫線行が 2 本未満
    TooFewBorders,
    /// 列境界(`+`)が 2 本未満
    TooFewColumns,
    /// 同名ペインの結合領域が矩形でない
    NonRectangularArea(String),
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GridError::TooFewBorders => {
                write!(f, "pane ブロックに罫線行(`+---+`)が 2 本以上必要です")
            }
            GridError::TooFewColumns => write!(f, "pane ブロックに列境界(`+`)が不足しています"),
            GridError::NonRectangularArea(name) => {
                write!(f, "ペイン `{name}` の結合領域が矩形になっていません")
            }
        }
    }
}

impl std::error::Error for GridError {}

/// ASCII グリッドをパースする。
pub fn parse_grid(src: &str) -> Result<GridSpec, GridError> {
    // 行を char 行列として保持(行末空白は無視)
    let lines: Vec<Vec<char>> = src
        .lines()
        .map(|l| l.trim_end().chars().collect::<Vec<char>>())
        .filter(|l| !l.is_empty())
        .collect();

    // 罫線行: 最初の非空白文字が '+'
    let border_idx: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.iter().find(|c| !c.is_whitespace()) == Some(&&'+'))
        .map(|(i, _)| i)
        .collect();
    if border_idx.len() < 2 {
        return Err(GridError::TooFewBorders);
    }

    // 列境界位置: 全罫線行の '+' の x 座標の和集合
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

    // 列トラック幅(境界文字を除いた内側の文字数)
    let cols: Vec<usize> = col_pos
        .windows(2)
        .map(|w| (w[1] - w[0]).saturating_sub(1).max(1))
        .collect();

    // 行バンド: 隣接する罫線行の間。高さは内容行数(最低 1)
    let mut rows: Vec<usize> = Vec::new();
    let mut areas: Vec<Vec<Option<String>>> = Vec::new();

    for w in border_idx.windows(2) {
        let (top, bottom) = (w[0], w[1]);
        let content: Vec<&Vec<char>> = lines[top + 1..bottom].iter().collect();
        rows.push(content.len().max(1));

        // トラックごとにペイン名を解決する。
        // 内容行は '|' で区切られたセグメントに分かれる。セグメントは 1 つ以上の
        // トラックを覆う(セル結合時は内部境界に '|' が無い)。
        let mut band: Vec<Option<String>> = vec![None; cols.len()];
        for (ti, tw) in col_pos.windows(2).enumerate() {
            let mid = (tw[0] + tw[1]) / 2;
            // バンド内の行から、このトラックの中点を含むセグメントのテキストを収集
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

    // 同名セルの結合が矩形かどうか検証(grid-template-areas の制約)
    validate_rectangular(&areas)?;

    Ok(GridSpec { cols, rows, areas })
}

/// 行の中で x 位置を含む `|` 区切りセグメントの文字列を返す
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
                let e = boxes
                    .entry(name.as_str())
                    .or_insert((r, c, r, c));
                e.0 = e.0.min(r);
                e.1 = e.1.min(c);
                e.2 = e.2.max(r);
                e.3 = e.3.max(c);
            }
        }
    }
    for (name, (r0, c0, r1, c1)) in &boxes {
        for r in *r0..=*r1 {
            for c in *c0..=*c1 {
                if areas[r][c].as_deref() != Some(*name) {
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
        // head は上段全体に広がる(上罫線に '+' があっても内容行に '|' が無い)
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
        assert_eq!(
            g.css_areas(),
            "\"head head\" \"main fig\" \"foot foot\""
        );
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
        // 2 行の内容を持つバンドは高さ 2
        let g = g.unwrap();
        assert_eq!(g.rows, vec![2, 1]);
    }

    #[test]
    fn non_rectangular_area_is_error() {
        // L 字型の 'a' はエラー
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
