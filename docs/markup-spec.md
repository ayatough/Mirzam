# Mirzam Flavored Markdown(MFM)ドラフト仕様 v0

> ステータス: ドラフト。MVP 実装前のたたき台。記法は実装スパイクの結果でフィードバックする。

## 0. 大原則

**MFM のすべての拡張は、CommonMark パーサで解釈したとき無害な要素に落ちなければならない。**

| 拡張 | プレーン Markdown での見え方 |
|---|---|
| fenced block(` ```pane ` 等) | コードブロック |
| フェンス付き div(`::: pane main`) | ただの段落テキスト |
| インライン属性 `{#id .u}` | 直後のテキスト(Pandoc 互換環境では属性として解釈) |
| `{{変数}}` | そのままのテキスト |
| `![[file.md]]` | 画像風リンク(Obsidian では埋め込みとして機能) |
| スピーカーノート `<!-- note: -->` | 非表示(HTML コメント) |

## 1. デッキとスライド

### 1.1 frontmatter(デッキ設定)

```yaml
---
title: 発表タイトル
author: 発表者
theme: default        # テーマ名 or パス
aspect: "16:9"        # 16:9 | 4:3 | A4 など
vars:                 # デッキ変数
  product: Mirzam
  price: 1200
---
```

### 1.2 スライド区切り

- 水平線 `---` で区切る(Marp 互換)。
- オプションで「`##` 見出しごとに新スライド」モードも提供(frontmatter で `split: h2`)。
- `#`(h1)はセクション扉スライドとして扱える。

### 1.3 ファイル分割(トランスクルージョン)

```markdown
![[sections/02-method.md]]
```

- 埋め込まれたファイルの内容がその位置に展開される(スライド区切りも有効)。
- Obsidian ではそのまま埋め込みプレビューになるため、分割編集体験が一致する。
- 循環参照はエラー。相対パスはファイル基準。

### 1.4 スピーカーノート

```markdown
<!-- note:
ここで具体例を 2 つ話す。時間が押していたら 1 つに。
-->
```

- スライド内の HTML コメントのうち `note:` で始まるものをノートとして収集。
- 代替記法として `::: note` ブロックも許容(こちらは本文に薄く表示したい人向け)。

## 2. インライン拡張

### 2.1 属性記法(Pandoc 互換)

```markdown
[重要な語句]{#latency .u .em}
![図](img/a.png){#fig1 pane=fig fit=contain}
## 見出し {#sec-intro .center}
```

- `#id`: アンカー ID(コネクタ・アニメーションの参照先)。
- `.class`: テーマ/ユーザ定義スタイル。`.u`(下線)などの標準クラスを定義する。
- `key=val`: 要素固有の属性(`fit`, `align`, `w`, `h` など)。

### 2.2 変数と式

```markdown
{{product}} は月額 {{price}} 円、年額 {{price * 12}} 円です。
```

- frontmatter の `vars` を参照。四則演算・比較・簡単な関数(`round`, `percent` 等)を評価。
- 表のセル内でも使用可能(列集計などの表計算的な拡張は将来検討)。

### 2.3 数式

```markdown
インライン: $E = mc^2$
ブロック:
$$
\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}
$$
```

- 既定は LaTeX 記法。ビルド時に MathML へ変換され、ブラウザがネイティブ描画する(クライアント JS 不要)。変換に失敗した場合は TeX ソースがエラースタイルで表示される。
- 将来 ` ```math typst ` ブロックで Typst Math を追加。KaTeX はプラグイン候補。

## 3. レイアウト(`pane` ブロック)

スライドごとに 1 つの `pane` ブロックでレイアウトを宣言する(無ければ単一ペインの既定レイアウト)。

````markdown
```pane
+--------------------+-------------+
|  head                            |
+--------------------+-------------+
|                    |             |
|  main              |  fig        |
|                    |             |
+--------------------+-------------+
|  foot                            |
+----------------------------------+
```
````

### 3.1 意味論(= CSS Grid)

- `+ - |` がセル境界。セル内の識別子がペイン名。
- 同名の隣接セルは結合される(`grid-template-areas` と同一の制約: 結合領域は矩形であること)。
- 列幅・行高はグリッド線間の文字数比で決まる。上例では main:fig ≒ 20:13。
- 空セルは `.` または空白のまま。
- 微調整はブロックの info string で: ` ```pane cols=2fr,1fr rows=auto,1fr,auto `(ASCII の比率を上書き)。

### 3.2 ペインへのコンテンツ割り当て

```markdown
::: pane main
本文。通常の Markdown がすべて使える。
:::

::: pane fig
![結果グラフ](img/result.svg){fit=contain}
:::
```

- `::: pane <名前>` 〜 `:::` のフェンス付き div(Pandoc 互換)。
- 短い内容には属性直付けも可: `![図](a.png){pane=fig}`。
- どのペインにも割り当てられていないブロックは `main`(または最初のペイン)に流し込む。

### 3.3 ペイン属性

```markdown
::: pane fig {align=center valign=middle pad=8}
```

- `align/valign`, `pad`, `bg`, `scroll` など。テーマ側でペイン名規約(`head` はタイトル装飾等)も持てる。

## 4. 図形(`shape` ブロック)— Phase 2

ページ座標系(0–100%)への自由描画レイヤ。

````markdown
```shape
rect   #cache  at(55%, 20%) size(30%, 14%) label="キャッシュ層" fill=@accent2 radius=8
ellipse #db    at(55%, 60%) size(30%, 18%) label="DB"
arrow  #a1     from(#cache.s) to(#db.n) style=dashed
text   #cap    at(70%, 82%) "ヒット率 95%" .small
```
````

- 各要素は `#id` を持ち、コネクタ・アニメーションから参照できる。
- `@accent2` 等はテーマパレット参照。
- 端点指定 `.n .s .e .w .c`(方位)。
- グラフ・チャート系は無理に自前 DSL 化せず、` ```mermaid ` / ` ```d2 ` 埋め込みをプラグインで対応する方針。

## 5. コネクタ(`connect` ブロック)— Phase 2

文章中のアンカーと図形要素をリンクする。**座標はレイアウト後に自動解決されるため、レイアウト変更に追従する。**

````markdown
その結果、[レイテンシが半減]{#lat .u}した。

```connect
#lat -> #fig1/bar2   : arrow color=@accent1
#cache -- #note1     : line  style=dotted
```
````

- `->`(矢印)、`--`(線)、`<->`(双方向)。
- 参照先は shape 要素 ID、画像内の名前付き領域(`#fig1/bar2` は SVG 内の `id=bar2`)、他のアンカー。
- ルーティングは自動(直交 / ベジェをオプションで選択)。

## 6. アニメーション(`anim` ブロック)— Phase 3

トリガ駆動のタイムライン記述。

````markdown
```anim
[enter]   .title      : chars fade-in 400ms stagger=30ms ease=out-cubic
[click 1] #lat        : underline-draw 300ms
[click 2] #fig1/bar2  : grow-y 500ms ease=spring(1, 80, 10)
[click 2] #a1         : draw 600ms delay=200ms
[exit]    slide       : iris-out 500ms
```
````

- トリガ: `[enter]`(表示時)、`[click N]`(N ステップ目)、`[exit]`(退場時)、`[after #id]`(連鎖)。
- 対象: アンカー ID / クラス / `slide`(ページ全体)/ `chars|words|lines` 修飾で文字単位分解。
- エフェクト名は標準セット(fade, slide, draw, grow, iris, flip …)+ プラグインで拡張可能。
- `ease=` は標準イージング名、`cubic-bezier(...)`、`spring(...)` を受け付ける。
- ページ切り替えエフェクトは frontmatter またはスライド属性 `<!-- slide: transition=push-left -->` でも指定可。

## 7. メディア

```markdown
![デモ動画](media/demo.webm){.autoplay .loop .controls poster=media/first.png fit=contain}
![動作 GIF](media/anim.gif){w=60%}
```

- 画像記法のまま、拡張子で出力要素が切り替わる: `mp4 / webm / ogv / mov` → `<video>`、`gif` などの画像 → `<img>`。
- 真偽属性は `.autoplay .loop .controls .muted`(クラス記法)。`autoplay` を指定すると `muted` が自動付与される(ブラウザの自動再生ポリシーで無音でないと自動再生できないため)。
- `poster=` はサムネイル。`fit` / `w` / `h` / `align` は画像と共通。
- PDF エクスポート時は `poster` の画像に置換。`poster` 未指定なら再生アイコン付きのプレースホルダになる。
- 配布時の注意: `mp4`(H.264)はプロプライエタリコーデック非搭載の Chromium ビルドで再生できないことがある。確実性を優先するなら `webm` を使う。

## 8. 予約 fenced block 一覧

| info string | 役割 | フェーズ |
|---|---|---|
| `pane` | レイアウト定義 | **実装済** |
| `shape` | 図形描画 | **実装済**(基本サブセット) |
| `connect` | コネクタ | **実装済**(基本サブセット) |
| `anim` | アニメーション | Phase 3 |
| `math typst` | Typst 数式 | Phase 4 |
| `chart` | データ駆動グラフ(下記) | Phase 4(構想) |
| その他未知の info string | 通常のコードブロック(ハイライト表示) | MVP |

### 8.1 `chart` ブロック(構想)

Excel のように、データ(インラインまたは CSV/TSV ファイル参照)からグラフを生成する。

````markdown
```chart
type: bar            # bar | line | scatter | pie
data: results.csv    # ファイル参照。または下のようにインライン:
# data:
#   - [手法, 時間ms]
#   - [フルビルド, 850]
#   - [差分, 95]
x: 手法
y: 時間ms
```
````

- ビルド時に SVG としてレンダリングし、系列・要素に自動 ID(`#chart1/bar-0` 等)を振る。
  → `connect` / `anim` から個々の棒・点を参照できる(手描き画像ではできない芸当)。
- データファイルは include と同様に監視対象となり、CSV の更新でグラフもホットリロードされる。
- 高度なグラフはプラグインで拡張(内蔵はシンプルな 4〜5 種に留める)。

## 9. 未決事項(スパイクで決める)

1. パーサ基盤: comrak vs markdown-rs(拡張の書きやすさ・性能を比較)。
2. `::: pane` のネスト可否(入れ子レイアウトを pane ブロックのネストで表すか、div のネストで表すか)。
3. 変数式の評価器スコープ(自作ミニ評価器 or 既存式言語 crate)。
4. `connect` の参照が画像(ラスタ)しかない場合の領域指定方法(`area=x,y,w,h` 属性を許すか)。
5. スライド単位設定の記法統一(`<!-- slide: ... -->` vs 見出し属性)。
