# Mirzam brand assets

Everything used to present Mirzam: the [README](../../README.md) header, the
[site](https://ayatough.github.io/Mirzam/), link previews, and any talk or post
about the project. Colour and type tokens are in
[`mirzam-theme.css`](mirzam-theme.css); the palette is written out in
[`palette.md`](palette.md).

These are *presentation* assets. They are deliberately separate from the deck
themes in `crates/mirzam-render/src/theme/themes/`, which ship inside every
rendered deck and must stay free of anything a user's own deck would not want.

## The mark

A four-point star rising over a horizon — Mirzam is the star that rises before
Sirius, and announces it.

| File | Use it for |
|---|---|
| `mirzam-wordmark-light.svg` / `-dark.svg` | The default lockup: mark plus name. README headers, site header, slides. |
| `mirzam-logo-light.svg` / `-dark.svg` | The same lockup with the tagline set under the name. Wide placements only — below about 420px the tagline stops being legible. |
| `mirzam-icon-light.svg` / `-dark.svg` | Square app mark: favicon, avatar, extension icon. Reads down to 16px. |
| `mirzam-icon-512.png` | Raster of the light icon, for the places that will not take an SVG (apple-touch-icon, app stores, some registries). |

Pick the variant by the background it sits on, not by the reader's theme:
`-light` is the ink-on-pale version, `-dark` is the pale-on-ink one. In Markdown,
serve both and let the browser choose:

```html
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/brand/mirzam-wordmark-dark.svg">
  <img src="docs/brand/mirzam-wordmark-light.svg" alt="Mirzam" width="360">
</picture>
```

**The wordmark carries its type as outlines, not as `<text>`.** GitHub renders
README images in a context that never loads a webfont, so a wordmark that named
Space Grotesk would arrive as Helvetica. If the name or tagline ever needs to
change, regenerate the outlines rather than editing the paths — the recipe is
under [Regenerating](#regenerating).

## Backgrounds

| File | Size | Use it for |
|---|---|---|
| `mirzam-hero-light.webp` / `mirzam-hero-dark.webp` | 1280×420 | The atmosphere art: a planet limb with the star flaring over it. Site hero, slide backgrounds, README banner. |
| `mirzam-background-light.svg` / `-dark.svg` | 1600×900 | The same idea drawn as vectors — a few KB, edits in a text editor, scales to any size. Use it when the raster is too heavy or the crop is wrong. |
| `mirzam-social-card.png` | 1200×630 | Open Graph / Twitter / Slack link preview. Generated, not hand-drawn. |
| `mirzam-concept-workflow-light.svg`, `-dark.svg` | 1400×420 | The pipeline diagram: Markdown → ASCII layout → components → HTML/PDF. One per mode — reference the pair with `<picture>`, which a deck rewrites to follow `D` rather than the machine. |

Behind text, the heroes need help: the flare in the lower left is bright enough
to swallow body copy. In a deck, that is what `dim` and `scrim` are for:

```markdown
::: pane hero {bg=docs/brand/mirzam-hero-dark.webp dim=0.35 scrim=bottom}
```

## Type

Three faces, all on Google Fonts and all under the OFL.

| Role | Face | Weight |
|---|---|---|
| Display — 40px and up | Space Grotesk | **300** Light, tracking `-0.03em` |
| Heading — 20 to 32px | Space Grotesk | **400** Regular, tracking `-0.02em` |
| Eyebrow / small caps label | Space Grotesk | **500** Medium, tracking `+0.12em` |
| Lead paragraph — 18px and up | Inter | **300** Light, but 400 on dark grounds |
| Body | Inter | **400** Regular |
| Code, data, the tagline | IBM Plex Mono | **400** Regular |

**About the light weights.** Space Grotesk's lightest cut *is* 300, and that is
not an oversight to work around — it is a display face, and its 300 is drawn to
hold at large sizes. Below roughly 32px the thin joins start to close up and it
reads as slightly muddy rather than slightly light, which is why the ladder above
gets *heavier* as the type gets smaller. So: keep 300 for the one or two places
that are genuinely large — a hero title, a slide title — and let 400 do the
ordinary headings. That reads as light overall, because the display line is the
one people register as the voice of the page.

For body text, Inter goes as thin as you like, but 300 at 16px is a legibility
problem rather than a style, and on a dark ground it thins out further. Inter 400
paired with Space Grotesk 300 already reads light, because the contrast between
the two is what carries the impression. **IBM Plex Sans** is the alternative body
face if you want a little more engineered texture; its 300 survives at body size
better than Inter's does, so that is the swap to make if the light-body look
matters more than the neutrality.

Japanese has no coverage in any of the three, so every stack in
`mirzam-theme.css` names Hiragino Sans and Noto Sans JP after the Latin face.
Noto Sans JP 300 next to Inter 400 is a good match; below that the kana get frail
at body size.

```html
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@300;400;500&family=Inter:wght@300;400;500&display=swap">
```

## Colour

Violet is the signature. Cyan, green, yellow and red are reserved for data,
status and annotation — spending them on decoration is what makes a chart stop
meaning anything. Full tables, light and dark, in [`palette.md`](palette.md).

## In a deck

One key, and it is the whole identity:

```yaml
theme: mirzam                 # the colours, the type, and the mark under a heading
```

**`theme: mirzam` gives a deck the identity, not only the palette.** A built-in
theme is a token set — its stylesheet is concatenated *before* `base.css` — and
the token vocabulary is now wide enough to hold Space Grotesk over Inter, the
weight ladder, the short violet rule under a section heading, and the dials
`.card`, `.eyebrow` and `.metric` read. There used to be a second file to load
beside it, `examples/themes/mirzam.css`, reached through the retired `css:`
key; everything it said is said in tokens now, and it is gone.

One thing the translation changed: which mode comes first. That file was
dark-first, because that is how the mark is drawn. A built-in cannot make that
choice — its bare selector is what a reader whose system prefers light gets —
so light comes first and dark arrives through `prefers-color-scheme`. A deck
that wants dark on every machine writes `mode: dark`, and the sample decks do.

Three things differ from the tables above, each for a reason a web page does not
have:

- **`--mz-accent2` is the cyan, not the second violet.** In a deck that token is
  also chart series 2, and two violets side by side in a bar chart is a chart
  that has stopped saying anything. The second violet becomes series 6.
- **The light theme's marks are darker.** The palette is drawn against a
  `#F7F8FC` page; a slide is white, and the yellow fell to 2.6:1 there.
- **The fonts are named, not fetched.** A deck is one self-contained file, so an
  `@import` would put a network request between the audience and the first
  slide — and the venue may have no network. Install the fonts, or accept the
  fallback that every stack ends in.

The built-in themes are held to `theme::tests` in `mirzam-render`, and any
theme a deck loads from a file — including
[`examples/themes/blueprint.css`](../../examples/themes/blueprint.css), which
is deliberately not this identity — is held to the same standard by `mirzam
check`: every colour defined in both modes, body text at 4.5:1 and chart marks
at 3:1 against both the slide and a raised surface.

## Regenerating

The rasters are built from the files next to them, so they can be rebuilt at any
time and nothing in this directory depends on a font being installed:

```bash
npm i playwright-core && npx playwright install --with-deps chromium
node scripts/make-brand-raster.mjs        # social card + icon PNG
```

The wordmark outlines were produced with [opentype.js][ot] from Space Grotesk
300 at 64px with `-2` tracking and IBM Plex Mono 400 at 14px, laid out glyph by
glyph so the tracking applies to the outlines. That is a one-off tool rather than
part of the build; the SVGs it produced are the source of truth now.

[ot]: https://github.com/opentypejs/opentype.js

## Licence

The assets in this directory are part of Mirzam and are MIT licensed, like the
rest of the repository. Space Grotesk, Inter, IBM Plex Sans and IBM Plex Mono are
each under the SIL Open Font License and are not redistributed here — they are
loaded from Google Fonts, or installed by you.
