<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../brand/mirzam-wordmark-dark.svg">
  <img src="../brand/mirzam-wordmark-light.svg" alt="Mirzam" width="300">
</picture>

# Mirzam(日本語)

> このディレクトリは日本語話者向けの補助資料です。**正式なドキュメントは英語版**で、
> 使い方は英語版だけで完結します。翻訳が古い場合は英語版が正です。
>
> [README](../../README.md) · [クイックスタート](quickstart.md) · [記法](../syntax.md) · [困ったときは](../troubleshooting.md) · [アーキテクチャ](../architecture.md) · [ロードマップ](../roadmap.md) · [開発ガイド](../development.md) · [ブランド](../brand/README.md)

## 一言でいうと

**リポジトリの中で暮らすプレゼン資料。** 素の Markdown を書き、レイアウトを ASCII で描くと、
グラフ・図・動画・数式を備えたデッキが単一 HTML または PDF として出てくる。

## 3 分で試す

インストール不要で試すなら **[ブラウザ版](https://ayatough.github.io/Mirzam/try/)**。
入口の一覧は[クイックスタート](quickstart.md)にあります。

CLI は **Rust なしで入ります**（macOS / Linux / Windows のビルド済みバイナリを
リリースごとに配布しています）:

```bash
curl -fsSL https://raw.githubusercontent.com/ayatough/Mirzam/main/scripts/install.sh | sh

mirzam build examples/pitch.md -o out   # 単一 HTML
mirzam serve examples/01-start.md       # ホットリロード付きプレビュー
mirzam export pdf examples/pitch.md
```

自分でビルドする場合は Rust 1.92 以降が必要で、`cargo build --release` の出力は
`./target/release/mirzam` に置かれます（`PATH` には入りません）。

ビューア操作: `←` `→` ページ送り / `O` 全スライド一覧(クリックで移動、番号入力も可) /
`N` スピーカーノート / `F` 全画面 / `H` コントローラーを隠す(投影時) /
`P` 発表者ウィンドウ(次スライド・ノート・時計・経過時間) /
`/` ショートカット一覧(そのスライドの `effects` キーも表示)。
`--embed-source` 付きでビルドしたデッキでは `V` でそのスライドの元 Markdown を
スライドの横に表示でき、`--editor-url` を足すとそこからブラウザエディタへ
デッキ全体を（見ていたスライドを開いた状態で）渡せます
（[公開サイト](https://ayatough.github.io/Mirzam/)のデッキはすべてこの形で
ビルドされています）。スマホではキーの代わりに右下の `</>` ボタンです。
スマホではスワイプでページ送り、上スワイプでノート、2 本指タップで一覧
（長押しは文字選択のために空けてあります）。

VSCode 拡張:

```bash
./scripts/build-vsix.sh
code --install-extension editors/vscode/mirzam-preview-*.vsix
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
| ` ```connect ` | 図形どうしを矢印で結ぶ | コードブロック |
| ` ```annotate ` | 図やグラフへの注釈、および文中の語句のハイライト/下線/囲み | コードブロック |
| `{bg=… dim=… blur=…}` | ペインの背景画像と可読性処理 | ただの文字 |
| `{#id .class}` | 属性(アンカー・装飾) | ただの文字 |
| `{{ price * 12 }}` | 変数と計算 | ただの文字 |
| `![[file.md]]` | ファイル分割 | 画像風リンク(Obsidian では埋め込み) |
| ` ```toc ` | 見出しから目次を自動生成(リンク・現在地表示つき) | コードブロック |
| `$...$`、`$$...$$` | 数式(既定は LaTeX) | ただの文字 |
| `<!-- next -->` | そのペインだけを次スライドへ送る(他のペインは静止) | 非表示 |
| `<!-- note: -->` | スピーカーノート | 非表示 |

**設計上の約束**: 拡張記法はすべて、素の CommonMark パーサで読んでも壊れません
(GitHub や Obsidian でそのまま読める)。これはテストで機械的に保証しています。

**v0.3.0 の新機能から二つ**: 数式は既定で LaTeX ですが、フロントマターに
`math: typst` と書くと同じ `$...$` が [Typst 風の数式構文](../syntax.md#typst-flavoured-math)
になります(`sum_(i=1)^n i`、`sqrt(x)` のように、バックスラッシュなしで書けます)。
また `.small` `.big` `.huge`(サイズ)、`.muted` `.accent` `.accent2` `.danger`(色)、
`.box`(枠付き)という組み込みクラスが、テーマを指定しなくても
`{.big .accent}` のように使えます([詳細](../syntax.md#attributes))。

### 記法リファレンスの章対照表

[記法リファレンス](../syntax.md)は英語版が正なので翻訳していませんが、1,000 行を超える
ため、目当ての章まで直接飛べるよう対応表を置きます。

| 内容 | 英語版の章 |
|---|---|
| デッキとスライド(フロントマター、スライドの区切り方、ファイル分割、スピーカーノート) | [Deck and slides](../syntax.md#deck-and-slides) |
| レイアウト(`pane` グリッド、`::: pane`、背景画像) | [Layout](../syntax.md#layout) |
| インライン記法(見出し・属性・強調・数式・変数・メディア) | [Inline syntax](../syntax.md#inline-syntax) |
| グラフ(`chart` ブロック) | [Charts](../syntax.md#charts) |
| 図形(`shape` ブロック — **スライド直下でのみ解釈される**) | [Shapes](../syntax.md#shapes) |
| コネクタ(`connect` ブロック) | [Connectors](../syntax.md#connectors) |
| 収まらないときの対処(`--fit shrink`、`<!-- next -->`) | [When a slide has too much on it](../syntax.md#when-a-slide-has-too-much-on-it) |
| 目次(`toc` ブロック) | [Table of contents](../syntax.md#table-of-contents) |
| 脚注(`[^key]` — **定義は参照と同じスライドに書く**) | [Citations](../syntax.md#citations) |
| 参考文献(`[@key]` と `bibliography` ブロック — BibTeX ファイルから採番・相互リンク) | [References](../syntax.md#references) |
| 演出(`effects` ブロック) | [Presentation effects](../syntax.md#presentation-effects) |
| 注釈(`annotate` ブロック) | [Annotations](../syntax.md#annotations) |
| アニメーション(`anim` ブロック) | [Animations](../syntax.md#animations) |
| ビューア操作・発表者ウィンドウ | [Driving the viewer](../syntax.md#driving-the-viewer) |
| テーマ(`theme:` — 組み込み名または自分の `.css`、`mode:`) | [Theming](../syntax.md#theming) |
| ペイン/スライド単位のテーマ(`{theme=…}`、`<!-- theme: … -->`) | [A theme smaller than a deck](../syntax.md#a-theme-smaller-than-a-deck) |

## サンプル

まず `examples/01-start.md` — 最小のデッキ、ページの区切り方、3 つのコマンドを 6 枚で。

記法のリファレンス。読む順序ではなく分野で分かれており、それぞれが説明している記法
そのもので書かれています:

| デッキ | 内容 |
|---|---|
| `examples/02-writing.md` | ペインの中身すべて。見出し・強調・リスト・表・数式・脚注・絵文字 |
| `examples/03-layout.md` | レイアウト規則を 1 スライド 1 ルールで |
| `examples/04-components.md` | 図形・コネクタ・メディア・注釈をソースと並べて |
| `examples/05-motion.md` | アニメーション。登場・クリック送り・ページ送り・演出 |
| `examples/06-theming.md` | テーマ(6 パレットのギャラリー付き)、フロントマターの全項目、属性、カスタム CSS |
| `examples/07-charts.md` | グラフ。全種類、インラインまたは CSV のデータ、コネクタが指せる各マーク |

ドキュメントではなく、デッキとして書かれたもの:

| デッキ | 内容 |
|---|---|
| `examples/pitch.md` | 営業向けピッチ。指標タイル、CSV からのグラフ、ダークテーマ |
| `examples/research.md` | 研究報告(英語)。数式・グラフ・4スライドから引かれる参考文献 |
| `examples/seminar.md` | 研究発表(日本語)。数式・表・脚注と参考文献・和文組版 |

## 日本語まわりの注意

- テーマは日本語フォント(Hiragino / Noto Sans CJK JP / Yu Gothic / Meiryo)を明示指定
  しています。指定が無いと環境によって中華圏フォントにフォールバックし、漢字の字形が崩れます。
- PDF 出力は**実行マシンのフォント**を使います。日本語フォントが入っていない環境で
  エクスポートすると字形が崩れるので、その場合はフォントを導入してください。
- CJK と記号が隣接する強調(`**ページ座標(%)**で`)も正しく解釈されます。

## 開発方針:`main` は作業ブランチです

**`main` は安定版ではありません。**開発は `main` に直接コミットしていきます。作者ひとり
と AI アシスタントで書いているためプルリクエストを読む二人目がおらず、ブランチに溜めても
唯一の実質的なレビューである「公開されたサイトを見る」が遅れるだけだからです。

サイトは[2 系統で配信](../../.github/workflows/pages.yml)しており、変更を取り込むことと
リリースすることが別々になっています。

| | ビルド元 | 対象 |
|---|---|---|
| **[ayatough.github.io/Mirzam](https://ayatough.github.io/Mirzam/)** | 最新のリリースタグ | リンクから来た人 |
| **[/next/](https://ayatough.github.io/Mirzam/next/)** | `main` の先端 | リリース前の変更を確認したい人 |

`/next/` にはどのコミットかと、CHANGELOG の未リリース項目が出ます。作業中のものなので、
プレビューとして見てください。

使う側として知っておくべきこと:

- **依存するなら[リリース](https://github.com/ayatough/Mirzam/releases)を指定してください。**
  安定点はタグです。`main` には作りかけの機能や、これから変わる記法、まだ実画面で確認して
  いない修正が乗っていることがあります。
- **ゲートは push のたびに走ります**(テスト・clippy・フォーマット・レイアウト検査・WASM
  ビルド)。作業ブランチであることは壊れていてよいという意味ではなく、**予告なく変わる**と
  いう意味です。
- **`main` に対する不具合報告も歓迎です。**バージョン番号が無いので、コミットを添えて
  ください。

外部からのコントリビュートは従来どおりプルリクエストです([CONTRIBUTING.md](../../CONTRIBUTING.md))。
直接 push は作者自身のコミットについての方針で、門を閉じたわけではありません。

## このディレクトリの他の文書

初期の設計検討メモです。現状とずれている箇所があるため、英語版を正としてください。

- [architecture.md](architecture.md) — 初期アーキテクチャ検討
- [roadmap.md](roadmap.md) — 初期ロードマップと MVP 定義
- [markup-spec.md](markup-spec.md) — 記法ドラフト v0
