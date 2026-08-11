# Layout cookbook masters

The shapes `examples/03-layout.md` is drawn on. A heading names a master and the
`pane` block under it is its drawing; everything else on this page is prose, so
a master can say what it is for.

A deck picks one of these with `<!-- layout: contrast -->`, and the whole file
with `masters: masters/cookbook.md` in its frontmatter.

## contrast

Two versions of the same thing, side by side, under a heading band tall enough
for an eyebrow above the title. The panes are `bad` and `good`, so a slide
using it has to call them that.

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------+-----------------+
|                  |                 |
|                  |                 |
|  bad             |  good           |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

## stated

One claim and its evidence: the same heading band over a single full-width
`body`.

```pane
+------------------------------------+
|                                    |
|  head                              |
+------------------------------------+
|                                    |
|                                    |
|  body                              |
|                                    |
|                                    |
+------------------------------------+
```
