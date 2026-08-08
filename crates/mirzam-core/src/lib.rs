//! Deck のメタデータ(frontmatter)と、`{{ 変数・式 }}` の評価器。

mod expr;

pub use expr::{eval_expr, Value};

use serde::Deserialize;
use std::collections::BTreeMap;

/// frontmatter で指定するデッキ設定
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DeckMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub theme: Option<String>,
    /// "16:9" | "4:3" など
    pub aspect: Option<String>,
    /// カスタム CSS ファイルへのパス(入力ファイル基準の相対パス)
    pub css: Option<String>,
    pub vars: BTreeMap<String, serde_yaml::Value>,
}

impl DeckMeta {
    /// アスペクト比からスライドの論理サイズ (幅, 高さ) を返す。既定は 16:9。
    pub fn slide_size(&self) -> (u32, u32) {
        match self.aspect.as_deref() {
            Some("4:3") => (1024, 768),
            _ => (1280, 720),
        }
    }

    /// 式評価用の変数テーブル
    pub fn var_table(&self) -> BTreeMap<String, Value> {
        self.vars
            .iter()
            .map(|(k, v)| {
                let val = match v {
                    serde_yaml::Value::Number(n) => Value::Num(n.as_f64().unwrap_or(f64::NAN)),
                    serde_yaml::Value::Bool(b) => Value::Str(b.to_string()),
                    serde_yaml::Value::String(s) => {
                        // 数値として読めるなら数値扱い(計算に使えるように)
                        match s.parse::<f64>() {
                            Ok(n) => Value::Num(n),
                            Err(_) => Value::Str(s.clone()),
                        }
                    }
                    other => Value::Str(
                        serde_yaml::to_string(other)
                            .unwrap_or_default()
                            .trim()
                            .to_string(),
                    ),
                };
                (k.clone(), val)
            })
            .collect()
    }
}

/// frontmatter を YAML としてパースする。失敗時は既定値 + エラーメッセージ。
pub fn parse_meta(yaml: &str) -> Result<DeckMeta, String> {
    serde_yaml::from_str(yaml).map_err(|e| format!("frontmatter の解析に失敗: {e}"))
}

/// テキスト中の `{{ ... }}` を評価して置換する。
/// 評価できないものは原文のまま残す(壊さない)。
pub fn substitute_vars(text: &str, vars: &BTreeMap<String, Value>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let inner = &after[..end];
                match eval_expr(inner, vars) {
                    Ok(v) => out.push_str(&v.to_display()),
                    Err(_) => {
                        out.push_str("{{");
                        out.push_str(inner);
                        out.push_str("}}");
                    }
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> BTreeMap<String, Value> {
        let mut m = BTreeMap::new();
        m.insert("price".to_string(), Value::Num(1200.0));
        m.insert("product".to_string(), Value::Str("Mirzam".to_string()));
        m
    }

    #[test]
    fn substitute_simple_and_expr() {
        let v = vars();
        assert_eq!(
            substitute_vars("{{product}} は年額 {{price * 12}} 円", &v),
            "Mirzam は年額 14400 円"
        );
    }

    #[test]
    fn unknown_var_left_as_is() {
        let v = vars();
        assert_eq!(substitute_vars("{{unknown}}", &v), "{{unknown}}");
    }

    #[test]
    fn unterminated_braces_kept() {
        let v = vars();
        assert_eq!(substitute_vars("a {{price", &v), "a {{price");
    }
}
