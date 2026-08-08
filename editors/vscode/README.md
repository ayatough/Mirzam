# Mirzam Preview(VSCode 拡張)

Markdown ベースのスライドツール [Mirzam](https://github.com/ayatough/Mirzam) のライブプレビュー。
レンダリングは **CLI と同一の Rust コア**を WebAssembly として Webview 内で実行する。

## 使い方

1. `.md` ファイルを開く
2. コマンドパレットで **「Mirzam: プレビューを開く」**(または `Ctrl+K V` / `Cmd+K V`、エディタ右上のプレビューアイコン)
3. 横にプレビューが開き、編集すると**変更したスライドだけ**が再描画される

その他:

- カーソルを動かすと、対応するスライドへプレビューが追従する
- プレビュー内で `←` `→` ページ送り、`N` スピーカーノート、`F` 全画面
- **「Mirzam: HTML として書き出す」**で単一ファイル HTML を保存(プレビューを開いた状態で実行)

## 設定

| 設定 | 既定 | 内容 |
|---|---|---|
| `mirzam.previewDelay` | 120 | 編集からプレビュー更新までの待ち時間(ms) |
| `mirzam.maxAssetSize` | 20971520 | プレビューに埋め込む画像・動画の上限(バイト) |

## 対応している記法

`pane`(ASCII レイアウト)/ `::: pane`(コンテンツ割り当て)/ `shape`(図形)/ `connect`(追従コネクタ)/
`![[file.md]]`(ファイル分割)/ frontmatter 変数と計算 / 数式(MathML)/ 動画・GIF / スピーカーノート。

詳細は [記法仕様](https://github.com/ayatough/Mirzam/blob/main/docs/markup-spec.md) を参照。

## ローカルでのビルド

```bash
# リポジトリルートで
./scripts/build-vsix.sh
# → editors/vscode/mirzam-preview-0.0.1.vsix
```

インストール: VSCode の拡張ビューの「…」→「VSIX からのインストール」、または

```bash
code --install-extension editors/vscode/mirzam-preview-0.0.1.vsix
```

## 制限

- レンダリングは Webview 内で完結するため、ワークスペース外の絶対パス参照は解決されない
- PDF 書き出しは CLI(`mirzam export pdf`)側の機能
