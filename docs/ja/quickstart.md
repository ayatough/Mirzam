# クイックスタート

手元にあるものに応じて 4 通りの入口があります。上から順に、当てはまる最初の行を選んでください。

| 手元にあるもの | 使うもの | 得られるもの |
|---|---|---|
| ブラウザ | **[ブラウザ版エディタ](https://ayatough.github.io/Mirzam/try/)** | 完成した `.html` デッキ。インストール不要、スマホでも動く |
| ターミナル | **`mirzam` CLI** | 全機能。ライブプレビュー、PDF 出力、ファイル分割、ローカル画像 |
| VS Code | **プレビュー拡張** | Markdown の横でデッキが即時再描画される |
| Obsidian | **手持ちの vault** | vault で執筆し、CLI かブラウザ版でビルド |

---

## 1. ブラウザ — インストール不要

**→ [ayatough.github.io/Mirzam/try](https://ayatough.github.io/Mirzam/try/)**

CLI と同じ Rust コアを WebAssembly にしたものです。左に Markdown を書くと右に
デッキが出て、**Download deck** を押すと自己完結した `.html` が 1 枚手に入ります
——`mirzam build` が書き出すものと同一です。開いてそのまま発表でき、メールにも
添付できます。

- **どこにもアップロードされません。** レンダリングはブラウザ内で完結し、下書きは
  そのブラウザのローカルストレージにのみ残ります。
- **画像が使えます。** 添付・ドラッグ&ドロップ・貼り付けのいずれでも `![](shot.png)`
  として挿入され、ダウンロードしたファイルの中にバイト列ごと収まります。論文の
  スクリーンショット引用はまさにこの用途です。
- **スマホ**ではエディタとプレビューがタブで切り替わります。
- **何もない状態から始められます。** **New** でエディタを空にし、**Sample** で
  サンプルデッキを戻せます。下書きはそのブラウザにしか存在しないので、どちらも
  置き換える前に確認します。残したいときは先に **Save .md** を。

ファイルシステムがないためブラウザ版でできないこと:

| できないこと | 代わりに |
|---|---|
| `![[section.md]]` によるファイル分割 | 1 ファイルにまとめる、または CLI を使う |
| ディスク上の `data: chart.csv` | `chart` ブロックの `data:` に CSV を直接書く |
| ディスク上の `theme: house.css` | `theme:` に組み込みテーマ名を書く、または CLI |
| PDF 出力 | CLI でビルドして `mirzam export pdf` |

## 2. コマンドライン — 全機能

**Rust は不要です。** リリースごとに macOS / Linux / Windows 向けのビルド済み
バイナリを配布しています。

```bash
curl -fsSL https://raw.githubusercontent.com/ayatough/Mirzam/main/scripts/install.sh | sh
```

環境に合ったアーカイブを取得し、公開されているチェックサムと照合してから
`~/.local/bin` に `mirzam` を置きます（`MIRZAM_BIN_DIR` で変更可）。Windows は
[リリースページ](https://github.com/ayatough/Mirzam/releases)から `.zip` を
取得し、`mirzam.exe` を `PATH` の通った場所に置いてください。

自分でビルドする場合は Rust 1.91 以降（[rustup.rs](https://rustup.rs)）が必要です。

```bash
git clone https://github.com/ayatough/Mirzam
cd Mirzam
cargo install --path crates/mirzam-cli --bin mirzam   # ~/.cargo/bin へ
```

（`cargo build --release` でも構いませんが、その場合は `PATH` に入らないので
`./target/release/mirzam` と書きます。）

```bash
mirzam new deck.md                   # 書き始めるためのデッキを 1 枚
mirzam serve deck.md                 # localhost:4321 でライブプレビュー
mirzam build deck.md -o out          # 自己完結 HTML 1 枚
mirzam export pdf deck.md -o deck.pdf
mirzam build notes.md --split h2     # 普通の文書をそのままデッキに
```

`new` はフロントマター・タイトルスライド・スライド区切りだけを書き出します
（[最初のデッキ](#最初のデッキ)と同じ形）。既存ファイルを上書きすることはありません。
`mirzam new deck.md --empty` なら空のファイルを作るので、雛形からではなく本当に
何もないところから始められます。`serve` は空のファイルもそのまま監視します。

`serve` は変更したスライドだけを再描画するので、大きなデッキでも執筆中の反応は
一定です。

## 3. VS Code

```bash
./scripts/build-vsix.sh
code --install-extension editors/vscode/mirzam-preview-*.vsix
```

`.md` を開いて `Ctrl+K V`（macOS は `Cmd+K V`）。編集したスライドだけが再描画され、
カーソル移動にプレビューが追従します。

拡張は WebAssembly コアを同梱しているので CLI を呼び出しません。ただし PDF 出力は
CLI の担当です。

## 4. Obsidian

Mirzam に Obsidian プラグインはありません。**書く**だけならなくても困りません。
すべての拡張記法は素の Markdown エディタで無害な形に退化し、トランスクルージョン
記法は Obsidian のものそのものだからです。

| Obsidian での見え方 | 理由 |
|---|---|
| `![[sections/method.md]]` が埋め込み表示される | Obsidian と同じ記法を使っている |
| `pane`・`chart`・`shape` はコードブロック | 実際にコードブロック |
| `::: pane main` はただの一行 | 実際にただの一行 |
| スピーカーノートは何も見えない | HTML コメント |

つまり vault の中にデッキを置いて執筆し、vault のパスを指して CLI でビルドする——
あるいは手元にマシンがないときはブラウザ版に貼り付ける、という運用になります。

## 5. スマホ

- **執筆:** 上のブラウザ版。これがツールチェイン全体です。**New** を押せば空の
  デッキから始められます。
- **確認:** ビルド済み `.html` をファイルアプリや共有から開くだけ。スワイプでページ
  送り、上スワイプでノート、2 本指タップでショートカット一覧。
- **発表:** デッキは 1 ファイルなので、AirDrop かクラウドフォルダに置けば配布は完了です。

---

## 最初のデッキ

6 行でデッキになります。

```markdown
---
title: 週次共有
---

# 今週の変更 {.title-slide}

---

## レイテンシ

キャッシュ導入後、p95 が**全リージョン**で下がりました。
```

そのうえでレイアウトを与えます。Mirzam がある理由はここです。

````markdown
```pane
+------------------+-----------------+
|  head                              |
+------------------+-----------------+
|  main            |  chart          |
+------------------+-----------------+
```

::: pane head
## キャッシュ導入後のレイテンシ
:::

::: pane main
p95 は全リージョンで低下し、最大の改善は `ap-ne` でした。
:::

::: pane chart
```chart
type: bar
data: |
  region, before, after
  us-east, 210, 120
  ap-ne, 380, 180
```
:::
````

この枠線の絵**そのもの**がレイアウトです。列幅は描いた文字数から、行の高さは
行数から決まります。

## 次に読むもの

- **[記法リファレンス](../syntax.md)** — 全ブロックとインライン記法、および素の
  Markdown パーサでの見え方
- **[レイアウトガイド](../layout.md)** — ペインの寸法、入りきらないときの対処、
  矢印を本文から遠ざける方法
- **[サンプル](../../examples/)** — まずは [`01-start.md`](../../examples/01-start.md)。
  02〜07 は分野別の記法リファレンスで、どこから読んでも構いません。
  [`research.md`](../../examples/research.md) は数式・グラフ・参考文献つきの研究報告(英語)、
  [`seminar.md`](../../examples/seminar.md) はその日本語版で図の引用と脚注つき
- **[全部動いているところ](https://ayatough.github.io/Mirzam/)**

デッキ上で `/` を押すと、そのビューアが応答する操作の一覧が出ます。
