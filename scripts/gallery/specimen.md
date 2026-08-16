---
title: Mirzam theme specimen
aspect: "16:9"
---

<!-- One slide, carrying one of everything a theme decides: the display face on
     a heading, the text face in a paragraph, the mono face in a code block, an
     accent on the eyebrow and the metric, the marker on a list, the rule on a
     quotation, and six series colours in a chart.

     It names no theme of its own. `scripts/make-theme-gallery.mjs` builds it
     once per theme and per mode with `--theme` and `--mode`, so what the
     gallery shows is whatever the stylesheets say today; nothing here is
     written twice.

     It is deliberately full to just under the edge. A specimen with room to
     spare would pass in every theme without proving that any of them fits, and
     the gallery's own build runs `mirzam check` over all twelve renderings for
     exactly that reason - a theme whose type is a size too large fails the
     site build instead of shipping a clipped heading. Keep any addition here
     small, and re-run the gallery before believing it. -->

<!-- chrome: none -->

```pane
+-----------------------------------------------------------+
|                                                           |
|  head                                                     |
|                                                           |
+-----------------------------+-----------------------------+
|                             |                             |
|                             |                             |
|  prose                      |  code                       |
|                             |                             |
|                             |                             |
+-----------------------------+-----------------------------+
|                             |                             |
|  metric                     |  chart                      |
|                             |                             |
+-----------------------------+-----------------------------+
```

::: pane head
[Theme specimen]{.eyebrow}
## A heading, in the theme's own voice
:::

::: pane prose
A theme is a **token set**, not a palette: the face, the size ladder, the
bullet and the rule under a heading all come from it. Inline `code` has a
stack of its own.

- What a list marker looks like
- And the colour it is drawn in
:::

::: pane code {.card}
```rust
fn main() -> Result<()> {
    // Syntax colours belong to the theme too.
    let deck = Deck::parse("slides.md")?;
    println!("{} slides", deck.len());
    Ok(())
}
```
:::

::: pane metric {valign=middle}
<div class="metric">2.3 ms</div>
<div class="metric-label">to re-render one edited slide</div>
:::

::: pane chart
```chart
type: bar
id: series
data: |
  region, before, after
  us-east, 210, 120
  eu-west, 260, 140
  ap-ne, 380, 180
```
:::
