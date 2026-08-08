# Mirzam

**Markdown ベースの次世代スライド作成システム** — Google スライド / PowerPoint に代わる「第三のツール」を目指すプロジェクト。

> Mirzam(ミルザム)= おおいぬ座 β 星。「先駆けて告げるもの(The Announcer)」の意。

## ビジョン

プレーンな Markdown として読める原稿から、PowerPoint 並みの表現力を持つスライドを、軽量・高速に生成する。人間にも AI エージェントにも読み書きしやすい記法で、テキストエディタだけでレイアウトまで完結させる。

## 既存ツールの課題と Mirzam のアプローチ

| 既存ツール(Marp / Touying 等)の不満 | Mirzam のアプローチ |
|---|---|
| 直感的なレイアウト操作ができない | ASCII アートによるペイン定義(`pane` ブロック)で、見たまま = レイアウト |
| 画像の配置調整ができない | ペイン参照 + `fit` / `align` 属性で宣言的に配置 |
| 図形描画の自由度が低い | 専用の `shape` レイヤ(ページ座標系の自由描画) |
| 動画を埋め込めない | HTML ランタイムを一級市民とし、video / GIF をネイティブ再生 |
| エクスポートが限定的 | HTML(アニメ対応)/ PDF を標準搭載、PPTX / Google スライドを拡張で |
| スマホから編集できない | Rust コアを WASM 化し、ブラウザ / PWA で同一エンジンを動作 |
| ページが増えると重い | ページ単位の差分パース・差分レンダリング(インクリメンタル設計) |
| 独自マークアップが覚えにくい | CommonMark 準拠。拡張はすべて「Markdown として壊れない」形で追加 |
| ファイル分割ができない | Obsidian 互換の埋め込み記法 `![[file.md]]` によるトランスクルージョン |
| アニメーションは妥協 | タイムライン IR を持つ `anim` ブロック。文字単位・イージング対応 |

## 記法の雰囲気

````markdown
---
title: Mirzam の紹介
aspect: "16:9"
vars:
  product: Mirzam
---

## {{product}} のアーキテクチャ

```pane
+--------------------+-------------+
|                    |             |
|  main              |  fig        |
|                    |             |
+--------------------+-------------+
|  foot                            |
+----------------------------------+
```

::: pane main
コアは **Rust** で実装し、[パーサ]{#p} と [レイアウトエンジン]{#l} を分離する。
:::

::: pane fig
![アーキテクチャ図](img/arch.svg){fit=contain}
:::

<!-- note: ここでパイプラインの図を指しながら説明する -->
````

この原稿は GitHub や Obsidian でもそのまま「読める Markdown」として表示される — これが Mirzam の設計原則。

## ドキュメント

| ドキュメント | 内容 |
|---|---|
| [docs/architecture.md](docs/architecture.md) | コンポーネント分解・データフロー・技術選定 |
| [docs/markup-spec.md](docs/markup-spec.md) | マークアップ言語(Mirzam Flavored Markdown)ドラフト仕様 |
| [docs/roadmap.md](docs/roadmap.md) | MVP 定義と開発ロードマップ |
| [examples/demo.md](examples/demo.md) | 記法サンプルデッキ |

## リポジトリ構成(計画)

```
Mirzam/
├── crates/            # Rust ワークスペース(コア)
│   ├── mirzam-syntax/     # パーサ(CommonMark + 拡張 → AST)
│   ├── mirzam-core/       # ドキュメントモデル(IR)・変数評価・差分管理
│   ├── mirzam-layout/     # ペイングリッド → ジオメトリ解決
│   ├── mirzam-render/     # シーングラフ → HTML / SVG 出力
│   ├── mirzam-connect/    # アンカー解決・コネクタ(矢印)ルーティング
│   ├── mirzam-anim/       # アニメーションタイムライン IR
│   ├── mirzam-cli/        # CLI(build / serve / export)
│   └── mirzam-wasm/       # WASM バインディング
├── web/               # TypeScript(ビューア / プレゼンタランタイム)
├── editors/           # VSCode 拡張・Obsidian プラグイン
├── docs/              # 設計ドキュメント
└── examples/          # サンプルデッキ
```

## ステータス

設計フェーズ。現在は要求整理・アーキテクチャ・記法仕様のドラフトを作成中。MVP のスコープは [docs/roadmap.md](docs/roadmap.md) を参照。
