# Mirzam Preview

Live preview for [Mirzam](https://github.com/ayatough/Mirzam), a Markdown-based
slide tool. Rendering runs **the same Rust core the CLI uses**, compiled to
WebAssembly and executed inside the webview.

## Usage

1. Open a `.md` file.
2. Run **Mirzam: Open Preview** from the command palette, press `Ctrl+K V`
   (`Cmd+K V` on macOS), or click the preview icon in the editor title bar.

The preview opens beside your editor. Editing re-renders only the slide you
changed, and moving the cursor scrolls the preview to the matching slide.

Inside the preview: `←` `→` to navigate, `N` for speaker notes, `F` for fullscreen.

**Mirzam: Export as HTML** saves the deck as a single self-contained file. PDF
export lives in the CLI (`mirzam export pdf`).

## Settings

| Setting | Default | Description |
|---|---|---|
| `mirzam.previewDelay` | 120 | Milliseconds between an edit and the preview update |
| `mirzam.maxAssetSize` | 20971520 | Maximum size in bytes of images and video inlined into the preview |

## Supported syntax

`pane` layouts, `::: pane` assignment, `chart`, `shape`, `connect`, `![[file.md]]`
transclusion, frontmatter variables and arithmetic, math, video and GIF, custom
themes, and speaker notes. See the [syntax
reference](https://github.com/ayatough/Mirzam/blob/main/docs/syntax.md).

## Building locally

```bash
# from the repository root
./scripts/build-vsix.sh
code --install-extension editors/vscode/mirzam-preview-0.0.1.vsix
```

## Limitations

- Rendering happens in the webview, so assets referenced by absolute paths outside
  the workspace are not resolved.
- PDF export is a CLI feature.
