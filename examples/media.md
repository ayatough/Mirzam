---
title: Media embedding
aspect: "16:9"
---

```pane
+------------------------------------+
|  head                              |
+------------------+-----------------+
|                  |                 |
|  desc            |  movie          |
|                  |                 |
+------------------+-----------------+
```

::: pane head
## Video
:::

::: pane desc {valign=middle}
The image syntax switches to `<video>` based on the file extension:

```markdown
![Demo](demo.webm){.autoplay .loop}
```

- Flags: `.autoplay`, `.loop`, `.controls`, `.muted`
- `autoplay` implies **`muted`**, since browsers block audible autoplay
- In PDF the video becomes its `poster=` image, or a placeholder
- Supports `mp4`, `webm`, `ogv`, `mov`. *Prefer `webm`: Chromium builds without proprietary codecs cannot play H.264.*
:::

::: pane movie
![Demo clip](media/demo.webm){.autoplay .loop .controls poster=media/demo-poster.png fit=contain}
:::

---

```pane
+------------------------------------+
|  head                              |
+------------------+-----------------+
|                  |                 |
|  desc            |  anim           |
|                  |                 |
+------------------+-----------------+
```

::: pane head
## GIF
:::

::: pane desc {valign=middle}
GIFs stay `<img>` elements and loop on their own.

```markdown
![Motion](media/demo.gif){w=90%}
```

PDF output uses the first frame.
:::

::: pane anim {align=center valign=middle}
![Animated GIF](media/demo.gif){w=90%}
:::
