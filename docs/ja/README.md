# Mirzam(日本語)

> このディレクトリは日本語話者向けの補助資料です。**正式なドキュメントは英語版**で、
> 使い方は英語版だけで完結します。翻訳が古い場合は英語版が正です。
>
> [README](../../README.md) · [クイックスタート](quickstart.md) · [記法](../syntax.md) · [アーキテクチャ](../architecture.md) · [ロードマップ](../roadmap.md) · [開発ガイド](../development.md)

## 一言でいうと

**リポジトリの中で暮らすプレゼン資料。** 素の Markdown を書き、レイアウトを ASCII で描くと、
グラフ・図・動画・数式を備えたデッキが単一 HTML または PDF として出てくる。

## 3 分で試す

インストール不要で試すなら **[ブラウザ版](https://ayatough.github.io/Mirzam/try/)**。
入口の一覧は[クイックスタート](quickstart.md)にあります。CLI なら:

```bash
cargo build --release
./target/release/mirzam build examples/pitch.md -o out   # 単一 HTML
./target/release/mirzam serve examples/showcase.md       # ホットリロード付きプレビュー
./target/release/mirzam export pdf examples/pitch.md
```

ビューア操作: `←` `→` ページ送り / `N` スピーカーノート / `F` 全画面 /
`P` 発表者ウィンドウ(次スライド・ノート・時計・経過時間) /
`/` ショートカット一覧(そのスライドの `effects` キーも表示)。
スマホではスワイプでページ送り、上スワイプでノート、2 本指タップで一覧
（長押しは文字選択のために空けてあります）。

VSCode 拡張:

```bash
./scripts/build-vsix.sh
code --install-extension editors/vscode/mirzam-preview-0.0.1.vsix
```

`.md` を開いて `Ctrl+K V`(Mac: `Cmd+K V`)。編集すると変更したスライドだけが再描画され、
カーソル位置にプレビューが追従します。

## 記法の要点

詳細は [記法リファレンス(英語)](../syntax.md)。ここでは要点だけ。

| 記法 | 用途 | 素の Markdown での見え方 |
|---|---|---|
| ` ```pane ` | ASCII でレイアウトを描く | コードブロック |
| `::: pane 名前` | ペインに内容を割り当て | 段落テキスト |
| ` ```chart ` | CSV からグラフ生成 | コードブロック |
| ` ```shape ` | 図形レイヤ | コードブロック |
| ` ```connect ` | 文章から図・グラフへの矢印 | コードブロック |
| `{bg=… dim=… blur=…}` | ペインの背景画像と可読性処理 | ただの文字 |
| `{#id .class}` | 属性(アンカー・装飾) | ただの文字 |
| `{{ price * 12 }}` | 変数と計算 | ただの文字 |
| `![[file.md]]` | ファイル分割 | 画像風リンク(Obsidian では埋め込み) |
| `<!-- next -->` | そのペインだけを次スライドへ送る(他のペインは静止) | 非表示 |
| `<!-- note: -->` | スピーカーノート | 非表示 |

**設計上の約束**: 拡張記法はすべて、素の CommonMark パーサで読んでも壊れません
(GitHub や Obsidian でそのまま読める)。これはテストで機械的に保証しています。

## サンプル

| デッキ | 内容 |
|---|---|
| `examples/pitch.md` | 営業向けピッチ。指標タイル、CSV からのグラフ、ダークテーマ |
| `examples/showcase.md` | 全コンポーネントをソースと並べて紹介 |
| `examples/seminar.md` | 研究発表(日本語)。数式・表・和文組版 |
| `examples/media.md` | 動画と GIF |

## 日本語まわりの注意

- テーマは日本語フォント(Hiragino / Noto Sans CJK JP / Yu Gothic / Meiryo)を明示指定
  しています。指定が無いと環境によって中華圏フォントにフォールバックし、漢字の字形が崩れます。
- PDF 出力は**実行マシンのフォント**を使います。日本語フォントが入っていない環境で
  エクスポートすると字形が崩れるので、その場合はフォントを導入してください。
- CJK と記号が隣接する強調(`**ページ座標(%)**で`)も正しく解釈されます。

## このディレクトリの他の文書

初期の設計検討メモです。現状とずれている箇所があるため、英語版を正としてください。

- [architecture.md](architecture.md) — 初期アーキテクチャ検討
- [roadmap.md](roadmap.md) — 初期ロードマップと MVP 定義
- [markup-spec.md](markup-spec.md) — 記法ドラフト v0
