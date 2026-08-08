> **注記**: この文書は初期の設計検討メモです(日本語)。
> 現行の正式なアーキテクチャは英語版を参照してください: [architecture.md](../architecture.md)

# Mirzam アーキテクチャ設計

## 1. 設計原則

1. **Markdown 互換を絶対に壊さない** — すべての拡張記法は、プレーンな CommonMark パーサで解釈したとき「コードブロック」「リンク」「コメント」など無害な要素に落ちること(graceful degradation)。Obsidian の思想に倣う。
2. **コアは Rust、1 つのエンジンをどこでも動かす** — ネイティブ CLI と WASM の両方にビルドし、ターミナル / VSCode / Obsidian / ブラウザ / スマホ(PWA)で同一のパース・レイアウト結果を保証する。実装済み: `scripts/build-wasm.sh` で wasm32 ビルド(約 2.9MB、未最適化)。I/O は抽象化されており(`FileProvider` / `AssetSource`)、ネイティブはファイルシステム、WASM はホスト注入のテーブルを使う。
3. **ページ単位のインクリメンタル処理** — パース・レイアウト・レンダリングのすべてをスライド単位でキャッシュし、変更されたスライドだけを再処理する。100 ページでも編集反映は一瞬、を設計目標とする。
4. **コアはジオメトリ、組版はランタイム** — Rust コアが計算するのは「ペインの矩形」「図形・コネクタの座標」まで。ペイン内のテキスト組版はブラウザ(HTML ランタイム)に委譲する。これによりコアが軽量になり、フォントメトリクス問題を初期段階で回避できる(将来の直接 PDF 生成時に組版エンジンを追加する)。
5. **AST を唯一の真実とする** — レイアウトもアニメーションも「AST 上の注釈」であり、レンダラはそれを解釈するだけ。AI エージェントは AST(または元 Markdown)を読み書きすれば全機能にアクセスできる。

## 2. パイプライン全体像

```
 .md ファイル群
   │  ①パース(スライド単位・差分)
   ▼
 AST(拡張 Markdown 構文木)
   │  ②意味解析: 変数評価・include 解決・アンカー収集
   ▼
 Deck IR(Deck → Section → Slide → Pane → Block)
   │  ③レイアウト: ASCII グリッド → ペイン矩形、shape 座標解決
   ▼
 Scene Graph(スライドごとの配置済み描画ツリー + アニメタイムライン)
   │  ④レンダリング
   ├──▶ HTML + CSS + ランタイム JS(プレビュー / 発表 / HTML エクスポート)
   ├──▶ PDF(初期: ヘッドレス Chromium 印刷、将来: 直接生成)
   └──▶ PPTX / Google スライド(将来)
```

- ①〜③ は Rust(`crates/`)。④の HTML 生成も Rust、実行時挙動(ナビゲーション・アニメ再生・動画・発表者モード)は TypeScript ランタイム(`web/`)。
- コネクタ(文章 ↔ 図形の矢印)は「レイアウト後」にアンカー座標を解決する後段パスなので、レイアウトが変わっても自動追従する。HTML 出力ではリサイズ時にランタイム側で再解決する。

## 3. コンポーネント分解

各コンポーネントは独立してテスト・利用可能な単位(crate / package)とする。

### コア(Rust ワークスペース)

| crate | 責務 | 主な依存 | フェーズ |
|---|---|---|---|
| `mirzam-syntax` | CommonMark + Mirzam 拡張のパース。スライド分割、fenced block(`pane`/`shape`/`anim`/`connect`)、インライン属性 `{#id .class k=v}`、`![[include]]` の認識。スライド単位の差分パース | comrak(または markdown-rs、スパイクで決定) | **MVP** |
| `mirzam-core` | Deck IR 定義。変数・式評価(`{{ }}`)、include 解決、アンカー表、テーマ解決、スライド単位のハッシュ/キャッシュ管理 | — | **MVP** |
| `mirzam-layout` | `pane` ブロックの ASCII グリッド解釈 → 比率グリッド(CSS grid-template-areas 相当)→ ペイン矩形。shape ブロックの座標解決 | taffy(必要になれば) | **MVP**(pane のみ) |
| `mirzam-render` | Scene Graph → HTML/CSS/SVG 文字列生成。テーマ(CSS variables)適用 | — | **MVP** |
| `mirzam-shape` | `shape` ブロック DSL のパースとビルド時 SVG レイヤ生成(ページ座標系 %、静的な図形間矢印の端点解決を含む) | — | **実装済** |
| `mirzam-connect` | `connect` ブロック DSL のパース。端点の実座標解決とルーティングは表示時にビューアランタイムが行う(リサイズ・ホットリロードに追従)。WASM でのアルゴリズム共有は将来 | serde_json | **実装済**(ルーティングは JS) |
| `mirzam-anim` | `anim` ブロック → タイムライン IR(トリガ・対象・エフェクト・イージング)。CSS keyframes / Web Animations API 命令列へコンパイル | — | Phase 3 |
| `mirzam-cli` | `mirzam build / serve / export`。ファイル監視 + WebSocket ホットリロード | notify, axum | **MVP** |
| `mirzam-wasm` | コアの WASM バインディング(エディタ拡張・ブラウザ編集用)。`Renderer` クラスで `render_page` / `render_slide` / `render_changed`(差分)/ `outline` を提供。ファイルシステムが無い環境向けに、include 対象とアセットはホストが JSON テーブルで注入する | wasm-bindgen | **実装済** |
| `mirzam-lsp` | Language Server(補完: ペイン名・アンカー ID、診断: 未定義参照、ホバー) | tower-lsp | Phase 3 |
| `mirzam-export-pptx` | Scene Graph → OOXML。表現力の差分は画像化フォールバック | — | Phase 5 |

### ランタイム・周辺(TypeScript)

| package | 責務 | フェーズ |
|---|---|---|
| `web/runtime` | スライドビューア: ナビゲーション、ホットリロードクライアント(変更スライドのみ DOM 差し替え)、動画/GIF 再生、数式レンダリング(KaTeX) | **MVP**(最小) |
| `web/presenter` | 発表者モード: スピーカーノート、次スライド、タイマー、ポインタ、アニメステップ制御 | Phase 3 |
| `editors/vscode` | VSCode 拡張: LSP クライアント + Webview ライブプレビュー(WASM コア) | Phase 2〜3 |
| `editors/obsidian` | Obsidian プラグイン | Phase 5 |
| `web/studio` | ブラウザ編集環境(PWA、スマホ対応) | Phase 5 |

### プラグインシステム(Phase 4)

- **コアプラグイン(WASM)**: AST 変換パス・カスタムブロックの登録。Rust 以外の言語でも書ける。
- **ランタイムプラグイン(JS)**: カスタムエフェクト・カスタムレンダラ。イージング関数の追加はこちら。
- テーマは CSS + マニフェストのみで完結する軽量拡張として別枠にする。

## 4. 重要な設計判断

### 4.1 なぜ HTML ランタイムを一級市民にするか

動画再生・リッチアニメーション・発表者モード・スマホ対応は、すべてブラウザ技術の上なら「実装済みの土台」がある。PDF は静的スナップショットとして位置づけ、初期はヘッドレス Chromium の印刷機能で生成する(HTML と PDF の見た目が原理的に一致するメリットもある)。

### 4.2 インクリメンタル処理の単位

- ソースをスライド区切り(`---` / 見出しルール)で分割し、スライドごとにソースハッシュを持つ。
- 変更があったスライドのみ再パース → 再レイアウト → 再レンダリングし、WebSocket でそのスライドの DOM だけ差し替える。
- デッキ全体に影響する変更(frontmatter、変数、テーマ、include 対象)は依存グラフで検出して必要範囲だけ無効化する。
- アンカー ID は明示指定(`{#id}`)を推奨し、スライドを跨ぐ参照の安定性を保証する。

### 4.3 ASCII レイアウトの解釈

`pane` ブロックは「文字グリッド → 比率グリッド」への写像として定義する:

- `+ - |` で区切られたセルにペイン名を書く。同名セルは結合(CSS grid-template-areas と同じ意味論)。
- 列幅・行高は文字数の比率から決まる。つまり「広く描けば広くなる」。
- コアの出力は `grid-template-columns/rows/areas` に相当する比率情報のみで、描画はランタイムの CSS Grid に委ねる。

これにより実装が小さく、意味論が既存の CSS Grid と一対一なので学習・デバッグも容易。

### 4.4 文章と図形のリンク(コネクタ)

- インラインアンカー `[語句]{#id .u}` と、shape/図要素の ID を、`connect` ブロックで結ぶ。
- 端点はレイアウト後の実座標から解決するため、レイアウト変更に自動追従する。
- HTML ではコネクタを SVG オーバーレイとして描画し、ResizeObserver で再ルーティングする。

### 4.5 数式

- MVP(実装済): `$...$` / `$$...$$`(複数行可)を **math-core でビルド時に MathML Core へ変換**し、ブラウザのネイティブ MathML 描画に任せる。クライアント JS ゼロ・PDF 印刷にそのまま乗る。変換失敗時は TeX ソースをエラースタイルで表示(title にエラー内容)。
- 描画品質の保証のため、**数式を含むデッキには STIX Two Math フォント(OFL)を data URI で同梱**する(約 540KB。数式が無ければ付加されない)。閲覧側マシンに数式フォントが無くても TeX 品質で表示される。
- 教訓: 当初採用した latex2mathml 0.2 は `x_{a}^{b}` を入れ子の msub/msup に誤変換するバグがあり(添字が階段状にずれる)、保守されている math-core に乗り換えた。数式変換は属性記法(`[...]{...}`)より先に実行する(`\sqrt[3]{x}` の誤マッチ防止)。
- 将来: KaTeX はレンダリング品質が必要な場合のプラグイン候補。Typst Math(` ```math typst `)は Phase 4 で変換層 `mirzam-math` として追加。

## 5. 技術選定(現時点の候補)

| 領域 | 第一候補 | 備考 |
|---|---|---|
| Markdown パーサ | comrak | AST が取れて拡張しやすい。markdown-rs と比較スパイクで決定 |
| レイアウト | 自前(グリッド比率計算)+ 将来 taffy | MVP のペイングリッドに外部依存は不要 |
| CLI サーバ | tiny_http + ロングポーリング(スパイク実装済) | mtime 監視 200ms 間隔。依存を最小化するため axum/WebSocket は見送り。LSP 導入時に再評価 |
| PDF | headless Chromium(chromiumoxide 等) | 将来 typst/parley 系で直接生成を検討 |
| 数式 | math-core(ビルド時 MathML Core 変換、実装済)+ STIX Two Math 同梱 | KaTeX はプラグイン候補として保留 |
| WASM | wasm-bindgen | |
| ランタイム | TypeScript + Vite、フレームワークレス | 依存を最小に保つ |

## 6. リスクと対策

| リスク | 対策 |
|---|---|
| ASCII レイアウトが複雑なレイアウトで破綻 | 意味論を CSS Grid に限定し、入れ子 pane で階層化。Grid で表せないものは shape レイヤへ |
| インクリメンタル無効化のバグ(古い表示が残る) | 「全再構築との一致」をプロパティテスト化。`--no-cache` を常備 |
| PDF と HTML の見た目差 | Chromium 印刷を採用する限り原理的に一致。print CSS のみ管理 |
| PPTX エクスポートの表現力ギャップ | 変換不能な要素は SVG/画像化して埋める方針を最初から仕様化 |
| スコープ肥大 | roadmap.md の「MVP でやらないこと」を厳守 |
