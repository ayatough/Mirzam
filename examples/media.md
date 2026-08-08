---
title: メディア埋め込みのデモ
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
## 動画の埋め込み
:::

::: pane desc {valign=middle}
画像記法のまま、拡張子で `<video>` に切り替わる:

```markdown
![デモ](demo.webm){.autoplay .loop}
```

- `.autoplay .loop .controls .muted` を指定できる
- `autoplay` 指定時は **`muted` が自動付与**される(ブラウザの自動再生ポリシー)
- PDF では `poster=` の画像、無ければプレースホルダに置換される
- `mp4` / `webm` / `ogv` / `mov` に対応。*`mp4`(H.264)は一部の OSS ビルドの Chromium で再生できないため、配布用には `webm` が無難*
:::

::: pane movie
![デモ動画](media/demo.webm){.autoplay .loop .controls poster=media/demo-poster.png fit=contain}
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
## GIF の埋め込み
:::

::: pane desc {valign=middle}
GIF は従来どおり `<img>` として扱われ、そのままループ再生される。

```markdown
![動作](media/demo.gif){w=90%}
```

PDF に出力した場合は先頭フレームが使われる。
:::

::: pane anim {align=center valign=middle}
![動作 GIF](media/demo.gif){w=90%}
:::
