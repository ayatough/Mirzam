---
title: Mirzam デモデッキ
author: Mirzam Project
theme: default
aspect: "16:9"
vars:
  product: Mirzam
  pages: 120
  before_ms: 850
  after_ms: 95
---

# {{product}} {.title-slide}

Markdown で書く、第三のスライドツール

<!-- note: 自己紹介は 30 秒で切り上げる -->

---

## なぜ作るのか

```pane
+------------------+------------------+
|                  |                  |
|  pain            |  goal            |
|                  |                  |
+------------------+------------------+
```

::: pane pain
**既存ツールの限界**

- レイアウト調整が苦行
- 動画が貼れない
- ページが増えると重い
- 独自記法の学習コスト
:::

::: pane goal
**{{product}} のゴール**

- 見たまま ASCII レイアウト
- HTML ネイティブ = 動画 OK
- ページ単位の差分レンダリング
- CommonMark 互換
:::

---

```pane
+----------------------------------+
|  head                            |
+---------------------+------------+
|                     |            |
|  main               |  fig       |
|                     |            |
|                     |            |
+---------------------+------------+
```

::: pane head
## 性能目標
:::

::: pane main
{{pages}} ページ編集時の反映速度:
{{before_ms}}ms → **{{after_ms}}ms**({{round(before_ms / after_ms)}} 倍)

差分レンダリングにより、[変更したスライドだけ]{#inc .u}を再処理する。

数式もそのまま: $T_{update} = O(1)$(デッキサイズに依存しない)
:::

::: pane fig
![ベンチマーク結果](img/bench.svg){fit=contain}
:::

<!-- note: 差分レンダリングの数値は roadmap の実測表を参照 -->

---

```pane
+------------------+------------------+
|  head                               |
+------------------+------------------+
|                  |                  |
|  main            |  canvas          |
|                  |                  |
|                  |                  |
+------------------+------------------+
```

::: pane head
## 図形とコネクタ
:::

::: pane main
- 図形は**ページ座標(%)**で宣言する
- 本文中の [パーサ]{#t-parser .u} や [レンダラ]{#t-render .u} から、図の要素へ矢印を張れる
- コネクタの端点は表示時に解決されるため、ウィンドウサイズやレイアウトが変わっても**自動で追従**する
:::

```shape
rect  #parser   at(72%, 34%) size(30%, 16%) label="mirzam-syntax"
rect  #renderer at(72%, 70%) size(30%, 16%) label="mirzam-render" fill=@shape-fill stroke=@accent2
arrow from(#parser.s) to(#renderer.n)
text  at(72%, 88%) "shape はビルド時に SVG 合成" .small
```

```connect
#t-parser -> #parser.w : color=@accent2
#t-render -> #renderer.w : color=@accent2 style=dashed
```

<!-- note: connect はランタイムで毎回ルーティングされる(リサイズしても矢印がついてくる) -->

---

![[sections/architecture.md]]

---

## まとめ {.center}

**書く・並べる・繋ぐ・動かす** — すべてをテキストで。

```anim
[enter] .center : chars fade-in 400ms stagger=25ms ease=out-cubic
```

<!-- note: anim ブロックは Phase 3 機能 -->
