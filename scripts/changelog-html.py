#!/usr/bin/env python3
"""The `## [Unreleased]` section of CHANGELOG.md, as HTML for the dev site.

    ./scripts/changelog-html.py            # the section, on stdout
    ./scripts/changelog-html.py --check    # the self-tests, then today's file

`scripts/build-site.sh` puts the result on /next/, so the question that site
exists to answer - what does this build have that the last release did not -
is on the page instead of one tap away on GitHub.

It lives here rather than inside `build-site.sh` because that script is not
run by CI: it runs in the Pages job, after CI is already green, which is a
place a broken converter is discovered by a reader. A run of backticks
quoting a fence went out that way - `--check` is the step that would have
stopped it, and it needs nothing but python3 to run.

The converter handles the subset the changelog actually uses: `### Heading`,
`- bullet` with indented continuation lines, and inline code, bold and links.
Anything else passes through as escaped text, which is wrong-looking rather
than broken, and is the trade for not carrying a Markdown library here.
"""

import html
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CHANGELOG = os.path.join(ROOT, "CHANGELOG.md")


def code_span(m):
    body = m.group(2)
    # CommonMark drops one space from each end when both are there, which is
    # what lets a run of backticks quote a token that starts with one.
    if len(body) > 1 and body[0] == " " and body[-1] == " " and body.strip():
        body = body[1:-1]
    return f"<code>{body}</code>"


def inline(text):
    out = html.escape(text)
    # A code span is delimited by a *run* of backticks, not by one: ```` quotes
    # a ```js fence, which is how the changelog writes a fence at all. Matching
    # only single backticks used to pair the last backtick of an opening ````
    # with the first of the fence inside it, and the entry came out as loose
    # backticks around an empty <code>.
    #
    # The lookarounds are what pin each run to its full length. Without them a
    # greedy `+ backtracks to a shorter one, and an unpaired ``` - which is not
    # a code span anywhere - gets read as 1 + 1 + 1 and opens one.
    out = re.sub(r"(?<!`)(`+)(?!`)(.+?)(?<!`)\1(?!`)", code_span, out)
    out = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", out)
    out = re.sub(r"\*([^*]+)\*", r"<em>\1</em>", out)  # bold is already gone
    return re.sub(r"\[([^\]]+)\]\((https?://[^)]+)\)", r'<a href="\2">\1</a>', out)


def _section_lines(path):
    """The lines between `## [Unreleased]` and the next `## ` heading."""
    lines, inside = [], False
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith("## [Unreleased]"):
                inside = True
                continue
            if inside and line.startswith("## "):
                break
            if inside:
                lines.append(line.rstrip())
    return lines


def _blocks(path):
    """The section as ('h3'|'item'|'break', text) pairs.

    One walk, two readers: the HTML below and the check further down, which
    would otherwise each carry their own copy of what a bullet is.
    """
    item = None
    for line in _section_lines(path):
        if line.startswith("### "):
            if item is not None:
                yield ("item", item)
                item = None
            yield ("break", "")
            yield ("h3", line[4:])
        elif line.startswith("- "):
            if item is not None:
                yield ("item", item)
            item = line[2:]
        elif line.strip() and item is not None:
            item += " " + line.strip()  # an indented continuation of the bullet
        elif not line.strip():
            if item is not None:
                yield ("item", item)
                item = None
            yield ("break", "")
    if item is not None:
        yield ("item", item)


def unreleased_items(path=CHANGELOG):
    """The section's bullets, as source text - one string per `- ` item."""
    return [text for kind, text in _blocks(path) if kind == "item"]


def unreleased_html(path=CHANGELOG):
    """The `## [Unreleased]` section of the changelog, as HTML."""
    out, items = [], []

    def flush():
        # A list closes on the first line that is not part of one.
        if items:
            out.append("<ul>" + "".join(f"<li>{inline(i)}</li>" for i in items) + "</ul>")
            items.clear()

    for kind, text in _blocks(path):
        if kind == "item":
            items.append(text)
        elif kind == "h3":
            out.append(f"<h3>{inline(text)}</h3>")
        else:
            flush()
    flush()
    return "\n".join(out)


# --- the check -------------------------------------------------------------
#
# Two halves: the converter still does what it is supposed to, and today's
# changelog is written in the subset it understands. The first catches an edit
# here, the second catches an entry that reaches for something this does not
# do - which is the direction the failure actually came from.

# Each case is (name, source, expected). The fence ones are the regression:
# before matching by run length they came out as loose backticks around an
# empty <code>, with the rest of the entry swallowed by the span left open.
CASES = [
    ("a plain span", "a `code` span", "a <code>code</code> span"),
    (
        "two spans on a line",
        "`a` and `b`",
        "<code>a</code> and <code>b</code>",
    ),
    (
        "a run quoting a fence",
        "```` ```js {2,4-5 lines} ```` washes",
        "<code>```js {2,4-5 lines}</code> washes",
    ),
    (
        "a run quoting a bare fence name",
        "```` ```each ```` renders",
        "<code>```each</code> renders",
    ),
    (
        "a double run holding a single backtick",
        "``a `b` c`` here",
        "<code>a `b` c</code> here",
    ),
    # An unpaired run is not a code span anywhere - GitHub prints the
    # backticks as text - so the converter has to agree rather than guess.
    ("an unpaired run stays text", "```each renders", "```each renders"),
    ("markup is escaped", "a <b> & co", "a &lt;b&gt; &amp; co"),
    ("bold", "**loud** here", "<strong>loud</strong> here"),
    (
        "a link",
        "see [the docs](https://example.com/x)",
        'see <a href="https://example.com/x">the docs</a>',
    ),
]


def run_cases():
    bad = []
    for name, src, want in CASES:
        got = inline(src)
        ok = got == want
        print(f"  {'ok   ' if ok else 'WRONG'} {name}")
        if not ok:
            bad.append((name, src, want, got))
    for name, src, want, got in bad:
        print(f"\n  {name}\n    in       {src}\n    expected {want}\n    got      {got}")
    return not bad


def lint_changelog():
    """Today's `[Unreleased]` section, rendered and looked over."""
    problems = []
    for item in unreleased_items():
        rendered = inline(item)
        # A backtick that survives outside a <code> is a span that never
        # opened or never closed; either way the entry is being shown wrong.
        outside = re.sub(r"<code>.*?</code>", "", rendered, flags=re.S)
        if "`" in outside:
            problems.append(("a backtick outside any code span", item, rendered))
        elif re.search(r"<code>\s*</code>", rendered):
            problems.append(("an empty code span", item, rendered))

    if problems:
        print(f"  WRONG {len(problems)} entr{'y' if len(problems) == 1 else 'ies'}"
              " will not render on /next/")
        for why, item, rendered in problems:
            print(f"\n  {why}:\n    source   {item[:120]}\n    renders  {rendered[:160]}")
        print(
            "\n  A fence is quoted with a longer run than the fence itself:"
            "\n    ```` ```js ````   not   ```js"
        )
        return False
    print(f"  ok    {len(unreleased_items())} entries render")
    return True


def main():
    if "--check" not in sys.argv[1:]:
        sys.stdout.write(unreleased_html())
        return 0

    print("the converter:")
    cases_ok = run_cases()
    print("CHANGELOG.md [Unreleased]:")
    lint_ok = lint_changelog()
    if cases_ok and lint_ok:
        print("✓ the changelog renders on the site")
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
