---
name: Something renders wrong
about: A deck that does not come out the way it reads
labels: bug
---

**The Markdown that does it.** A deck is a text file, so the smallest one that
still shows the problem is usually the whole report:

```markdown

```

**What you expected, and what you got.** A screenshot helps for anything
visual — the layout checker and the golden tests both miss things the eye
catches immediately.

**Where.** `mirzam build` / `serve` / `export pdf` / the browser editor / the
VS Code extension, and the version (`mirzam --version`).
