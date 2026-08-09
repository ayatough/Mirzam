#!/usr/bin/env python3
"""Draws the stand-in "figure clipped out of a paper" used by examples/seminar.md.

A reading-group deck quotes figures from the paper it is about, and the sample
deck has to quote *something*. Drawing it keeps the repository self-contained
and avoids shipping a copyrighted crop; it is deliberately styled like a
one-column journal figure so the deck's layout is exercised honestly.

    pip install Pillow
    python3 scripts/make-paper-figure.py
"""

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

W, H = 1400, 1040
OUT = Path(__file__).resolve().parent.parent / "examples" / "media"

PAPER = (252, 251, 248)
INK = (26, 26, 28)
GREY = (122, 124, 130)
RULE = (198, 198, 200)
S1 = (36, 80, 200)
S2 = (176, 48, 96)


def font(size, bold=False):
    for name in (
        "DejaVuSerif-Bold.ttf" if bold else "DejaVuSerif.ttf",
        "DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf",
    ):
        for base in ("/usr/share/fonts/truetype/dejavu/", "/usr/share/fonts/TTF/"):
            try:
                return ImageFont.truetype(base + name, size)
            except OSError:
                continue
    return ImageFont.load_default(size)


def main():
    img = Image.new("RGB", (W, H), PAPER)
    d = ImageDraw.Draw(img)

    # Plot frame, inset the way a journal figure is.
    L, R, T, B = 190, W - 90, 110, H - 300
    d.rectangle([L, T, R, B], outline=INK, width=3)

    # Axis ticks and labels.
    for i in range(6):
        y = B - (B - T) * i / 5
        d.line([L - 12, y, L, y], fill=INK, width=3)
        d.text((L - 24, y), f"{i * 20}", font=font(26), fill=INK, anchor="rm")
        if i:
            d.line([L + 1, y, R - 1, y], fill=RULE, width=1)
    for i in range(7):
        x = L + (R - L) * i / 6
        d.line([x, B, x, B + 12], fill=INK, width=3)
        d.text((x, B + 22), f"{i * 2}", font=font(26), fill=INK, anchor="ma")

    d.text(((L + R) / 2, B + 78), "integration time  τ  (µs)", font=font(30), fill=INK, anchor="ma")
    img_rot = Image.new("RGB", (420, 44), PAPER)
    ImageDraw.Draw(img_rot).text((210, 22), "readout fidelity  F  (%)", font=font(30), fill=INK, anchor="mm")
    img.paste(img_rot.rotate(90, expand=True), (46, int((T + B) / 2) - 210))

    # Two curves: a saturating baseline and a better one that peaks and decays.
    def plot(fn, color, dashed=False):
        pts = []
        for k in range(0, 601):
            t = k / 100.0
            v = fn(t)
            x = L + (R - L) * t / 12.0
            y = B - (B - T) * v / 100.0
            pts.append((x, y))
        if dashed:
            for i in range(0, len(pts) - 12, 24):
                d.line(pts[i : i + 12], fill=color, width=5)
        else:
            d.line(pts, fill=color, width=5)
        return pts

    base = lambda t: 96.0 * (1 - math.exp(-t / 1.6))
    best = lambda t: 99.4 * (1 - math.exp(-t / 0.7)) - 1.6 * max(0.0, t - 5.0) ** 2
    plot(base, S1, dashed=True)
    pts = plot(best, S2)

    # The peak, which is what the deck's annotation points at.
    peak = max(pts, key=lambda p: -p[1])
    d.ellipse([peak[0] - 9, peak[1] - 9, peak[0] + 9, peak[1] + 9], fill=S2)

    # Legend, in the empty lower-right of the plot.
    lx, ly = R - 380, B - 120
    d.line([lx, ly, lx + 60, ly], fill=S1, width=5)
    d.text((lx + 76, ly), "without JPA", font=font(28), fill=INK, anchor="lm")
    d.line([lx, ly + 46, lx + 60, ly + 46], fill=S2, width=5)
    d.text((lx + 76, ly + 46), "with JPA", font=font(28), fill=INK, anchor="lm")

    # Caption, set the way a paper sets one.
    cap_y = B + 118
    d.text((L, cap_y), "FIG. 3.", font=font(30, bold=True), fill=INK)
    d.text(
        (L + 122, cap_y),
        "Readout fidelity against integration time. The dashed\n"
        "trace saturates near 96%; adding the parametric amplifier reaches\n"
        "99.4% and degrades beyond τ ≈ 5 µs as the qubit relaxes.",
        font=font(28),
        fill=GREY,
        spacing=10,
    )

    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / "paper-fig3.png"
    img.save(path, optimize=True)
    print(f"wrote {path} ({path.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
