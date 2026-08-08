//! 大原則の自動検証:
//! **Mirzam の拡張記法は、素の CommonMark パーサで解釈しても壊れない。**
//!
//! GitHub や Obsidian でサンプルデッキを開いたときに、
//! 拡張ブロックがコードブロックとして無害に表示されることを確認する。

mod common;

use common::{example, EXAMPLE_DECKS};

/// 拡張を一切有効にしない素の CommonMark として描画する
fn plain_commonmark(md: &str) -> String {
    let options = comrak::Options::default(); // 拡張なし・raw HTML はエスケープ
    comrak::markdown_to_html(md, &options)
}

#[test]
fn extension_blocks_degrade_to_code_blocks() {
    let src = "\
```pane
+---+---+
| a | b |
+---+---+
```

```shape
rect #r at(50%, 50%) size(10%, 10%)
```

```connect
#a -> #r
```
";
    let html = plain_commonmark(src);
    // fenced block は info string がクラスになったコードブロックとして出る
    assert_eq!(
        html.matches("<pre>").count(),
        3,
        "3 つのコードブロック: {html}"
    );
    assert!(html.contains("language-pane"));
    assert!(html.contains("language-shape"));
    assert!(html.contains("language-connect"));
    // 中身がそのまま読める(情報が失われない)
    assert!(html.contains("rect #r at(50%, 50%)"));
}

#[test]
fn pane_divs_and_vars_stay_readable_text() {
    let src = "::: pane main\n\n本文 {{price}} 円\n\n:::\n";
    let html = plain_commonmark(src);
    // div 記法は段落テキストとして残るだけで、内容は失われない
    assert!(html.contains("::: pane main"));
    assert!(html.contains("本文 {{price}} 円"));
}

#[test]
fn speaker_notes_are_hidden_comments() {
    let html = plain_commonmark("本文\n\n<!-- note: 内緒のメモ -->\n");
    // raw HTML を無効化した既定設定でもコメントは表示テキストにならない
    assert!(!html.contains("内緒のメモ") || html.contains("&lt;!--"));
}

/// すべてのサンプルデッキが素の CommonMark でも「読める文書」であること
#[test]
fn all_examples_render_as_plain_markdown() {
    for deck in EXAMPLE_DECKS {
        let src = std::fs::read_to_string(example(deck)).expect("サンプル読み込み");
        let (_, body) = mirzam_syntax::split_frontmatter(&src);
        let html = plain_commonmark(body);

        assert!(!html.is_empty(), "{deck}: 出力が空");
        // 見出しが失われていない
        assert!(
            html.contains("<h1") || html.contains("<h2"),
            "{deck}: 見出しが 1 つも残っていない"
        );
        // 拡張ブロックはすべてコードブロックに落ちている(生の記法が段落に漏れない)
        for marker in ["grid-template-areas", "<section class=\"slide\""] {
            assert!(
                !html.contains(marker),
                "{deck}: 素の Markdown 出力に Mirzam 固有の出力 `{marker}` が混入"
            );
        }
    }
}
