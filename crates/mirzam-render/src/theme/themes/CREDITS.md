# Theme credits

Mirzam ships palettes, not copied stylesheets: each built-in theme translates
the named colors of a real, published palette into Mirzam's own token set
(`--mz-bg`, `--mz-fg`, `--mz-accent1`, ...), for both light and dark mode. No
CSS was copied from any of these projects; only the named colors were, and
where a straight translation lost WCAG contrast against its new background, a
sibling shade from the same palette (or a darkened/lightened variant of the
same hue) was used instead so a unit test can hold every theme to the same
bar. See `every_theme_and_mode_meets_wcag_contrast` in `mod.rs`.

## `nord`

[Nord](https://www.nordtheme.com/) by Arctic Ice Studio and Sven Greb.
License: [MIT](https://github.com/nordtheme/nord/blob/main/LICENSE.md).

| Token | Light | Dark | Nord name |
|---|---|---|---|
| `--mz-slide-bg` | `#eceff4` | `#3b4252` | Snow Storm nord6 / Polar Night nord1 |
| `--mz-fg` | `#2e3440` | `#eceff4` | Polar Night nord0 / Snow Storm nord6 |
| `--mz-muted` | `#4c566a` | `#d8dee9` | Polar Night nord3 / Snow Storm nord4 |
| `--mz-accent1` | `#46658a`\* | `#88c0d0` | Frost nord10 (darkened) / nord8 |
| `--mz-accent2` | `#8fbcbb` | `#8fbcbb` | Frost nord7 |
| `--mz-chart3` (orange) | `#a85d3f`\* | `#d08770` | Aurora nord12 (darkened) / nord12 |
| `--mz-chart4` (purple) | `#8f6584`\* | `#b48ead` | Aurora nord15 (darkened) / nord15 |
| `--mz-chart5` (red) | `#bf616a` | `#d98088`\* | Aurora nord11 / nord11 (lightened) |
| `--mz-chart6` (blue) | `#4f6f92`\* | `#81a1c1` | Frost nord9 (darkened) / nord9 |

\* Darkened or lightened from the named Nord color to clear the WCAG
threshold this theme's contrast test enforces; same hue, adjusted lightness.

## `solarized`

[Solarized](https://ethanschoonover.com/solarized/) by Ethan Schoonover.
License: [MIT](https://github.com/altercation/solarized/blob/master/LICENSE).

| Token | Light | Dark | Solarized name |
|---|---|---|---|
| `--mz-slide-bg` | `#fdf6e3` | `#073642` | base3 / base02 |
| `--mz-fg` | `#586e75` | `#93a1a1` | base01 / base1 |
| `--mz-muted` | `#5f7278`\* | `#97a7a7`\* | base01-ish / base1-ish (adjusted) |
| `--mz-accent1` (blue) | `#1c6ca3`\* | `#4ba6e0`\* | blue, adjusted for contrast |
| `--mz-accent2` (cyan) | `#2aa198` | `#2aa198` | cyan |
| `--mz-chart3` (orange) | `#cb4b16` | `#e0672f`\* | orange / orange (lightened) |
| `--mz-chart4` (violet) | `#6c71c4` | `#8288d6`\* | violet / violet (lightened) |
| `--mz-chart5` (red) | `#dc322f` | `#ea5551`\* | red / red (lightened) |
| `--mz-chart6` (green) | `#6f8000`\* | `#859900` | green (darkened) / green |

\* Adjusted from the named Solarized color to clear the WCAG threshold this
theme's contrast test enforces; same hue, adjusted lightness.

## `vscode`

Visual Studio Code's default Light+/Dark+ themes, part of
[microsoft/vscode](https://github.com/microsoft/vscode). License:
[MIT](https://github.com/microsoft/vscode/blob/main/LICENSE.txt). Colors here
are drawn from memory of the published defaults (editor background/
foreground, the `textLink.foreground` accent, syntax-highlighting colors for
strings/keywords/types) rather than fetched from the live source, so treat
them as representative of the VS Code look rather than a pixel-exact extract.

## `mirzam`

Ours, from [`docs/brand/palette.md`](../../../../../docs/brand/palette.md). The
token half of `examples/themes/mirzam.css`; that file is still where the
identity's *type* lives — a built-in theme is loaded before `base.css` and so
can only set tokens, not typography.

Also the palette a deck gets when it names no theme, which is the common case —
a quick note, a README turned into slides, a sample showing one piece of markup
— and those used to come out in a generic blue-and-teal that looked like
nobody's. It was shipped twice for a while, the second copy keyed for a
`default` theme; that name is retired and this is the only sheet.

The brand sheet is drawn for a web page and a deck is not one, so two families
of value moved. `--mz-muted` in light mode is a step darker than the brand's
`#68708a`: that reads fine on the site's `#f7f8fc`, but a deck's raised surface
is lighter still and secondary text on a card came out at 4.4:1. The light chart
marks are darker than the brand's chart colours for the same reason — those are
drawn for `#f7f8fc`, a slide is white, and the yellow fell to 2.6:1 against it.
Same hues, enough ink to be seen.

`--mz-accent2` is deliberately *not* the brand's second violet. In a deck it is
also chart series 2, and two violets side by side in a bar chart is a chart that
has stopped saying anything; it takes the brand's cyan, and the second violet
becomes series 6, far from the first.

## `wuwei`

Ours, and not a translation of anything: a greyscale palette drawn for this
project rather than borrowed from a published one, so there is nothing to
credit but the intent.

The name is Laozi's 無為 — *wu wei*, acting without contrivance. It is the
theme for a deck that does not want to be looked at, only read: warm greys,
no accent colour, and body text at roughly 9:1 against its background rather
than the 12-14:1 a black-on-white theme reaches. Low contrast is the point,
and the WCAG floor (4.5:1 for text, 3:1 for chart marks, checked by
`every_theme_and_mode_meets_wcag_contrast`) is the line it stays comfortably
above while getting there.

Two decisions worth writing down:

- **Warm, not neutral.** Every grey is mixed towards the ink's own hue rather
  than sitting on the r=g=b axis. A neutral grey next to these reads as cold,
  and the paper stops looking like paper.
- **Chart series keep a whisper of hue.** Six series with only lightness to
  separate them is more than an audience can hold, so each series is nudged
  towards a different hue — taupe, sand, near-black, olive, rose, slate — at a
  saturation low enough to stay in the same family, and spaced in lightness
  from its neighbours. A chart in this theme wants its labels; that is the
  trade the palette makes deliberately.

Dark mode is drawn from scratch, not inverted: warm near-black paper, warm bone
ink, and each series re-picked for its new background.
