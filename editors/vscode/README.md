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

A deck [split across files](https://github.com/ayatough/Mirzam/blob/main/docs/syntax.md)
is previewed from the file that transcludes the others, and you can keep
working in any of them: editing a section re-renders the deck as you type,
saved or not, and the cursor there scrolls the preview to that section's
slides. A cursor on an `![[…]]` line shows the first slide of the file it
names.

Inside the preview: `←` `→` to navigate, `N` for speaker notes, `F` for fullscreen.

**Mirzam: Export as HTML** saves the deck as a single self-contained file. PDF
export lives in the CLI (`mirzam export pdf`).

## Diagnostics, with the CLI installed

If `mirzam` is on your `PATH`, the extension also starts `mirzam lsp` and the
deck gets underlined where it is wrong as you type: an unknown theme, a pane
that is not in the grid, a `<!-- layout: -->` naming no master, a citation key
nothing defines. Completion knows the names a deck refers to by name — pane
names, anchor ids, BibTeX keys, theme and master names — hover says what one
stands for, and `Ctrl+Shift+O` lists the deck's slides.

**Without the CLI nothing changes.** The preview runs the WebAssembly core in
the webview and needs nothing installed, so a missing binary means no
diagnostics rather than an error. Only files that look like decks are analysed,
so an ordinary README in the same folder is left alone. The server never opens
a browser: the layout checks — content clipped by its pane, panes overlapping —
stay `mirzam check`.

## Settings

| Setting | Default | Description |
|---|---|---|
| `mirzam.previewDelay` | 120 | Milliseconds between an edit and the preview update |
| `mirzam.maxAssetSize` | 20971520 | Maximum size in bytes of images and video inlined into the preview |
| `mirzam.languageServer` | `true` | Start `mirzam lsp` for diagnostics, completion, hover and go-to-definition |
| `mirzam.serverPath` | `""` | Path to the `mirzam` binary; empty means `mirzam` on `PATH` |

## Supported syntax

`pane` layouts, `::: pane` assignment, `chart`, `shape`, `connect`, `![[file.md]]`
transclusion, frontmatter variables and arithmetic, math, video and GIF, custom
themes, `[@key]` references against a `bibliography:` file, and speaker notes. See the [syntax
reference](https://github.com/ayatough/Mirzam/blob/main/docs/syntax.md).

Rendering happens in the webview, which has no filesystem of its own, so the
extension reads the deck's files and hands them over: transcluded sections, a
`masters:` file, a `bibliography:`, the stylesheets `theme:` names, images and
video, a pane's background photograph, and the CSV a chart names in `data:`. A
file the extension cannot read is reported in the strip under the preview
rather than left to look like a rendering bug.

## Building locally

```bash
# from the repository root
./scripts/build-vsix.sh
code --install-extension editors/vscode/mirzam-preview-*.vsix
```

## Limitations

- Rendering happens in the webview, so assets referenced by absolute paths outside
  the workspace are not resolved.
- PDF export is a CLI feature.
