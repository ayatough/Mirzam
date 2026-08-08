#!/usr/bin/env python3
"""Generates the sample background images used by the example decks.

The samples are drawn rather than downloaded so the repository stays
self-contained and the examples build with no network access. For real photos
use `scripts/fetch-backgrounds.sh`, which pulls from Unsplash.

    pip install Pillow
    python3 scripts/make-sample-backgrounds.py
"""

import math
import random
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

W, H = 1600, 1000
OUT = Path(__file__).resolve().parent.parent / "examples" / "media" / "bg"


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def vertical_gradient(stops):
    """stops: [(position 0..1, (r, g, b)), ...] from top to bottom."""
    img = Image.new("RGB", (1, H))
    px = img.load()
    for y in range(H):
        t = y / (H - 1)
        lo = max(i for i, (p, _) in enumerate(stops) if p <= t)
        hi = min(lo + 1, len(stops) - 1)
        p0, c0 = stops[lo]
        p1, c1 = stops[hi]
        k = 0.0 if p1 == p0 else (t - p0) / (p1 - p0)
        px[0, y] = lerp(c0, c1, k)
    return img.resize((W, H))


def grain(img, amount=6):
    """A little noise keeps a flat gradient from banding."""
    noise = Image.effect_noise((W, H), amount).convert("L")
    return Image.blend(img, Image.merge("RGB", (noise, noise, noise)), 0.05)


def city_night():
    """Dusk skyline: the classic full-bleed title background."""
    img = vertical_gradient(
        [
            (0.00, (14, 18, 46)),
            (0.45, (38, 34, 86)),
            (0.72, (122, 66, 108)),
            (0.88, (222, 122, 92)),
            (1.00, (255, 176, 110)),
        ]
    )
    rng = random.Random(7)

    # Bokeh: soft light blobs, drawn large and blurred down, screened over the
    # sky so they add light instead of averaging it away.
    glow = Image.new("RGB", (W, H), (0, 0, 0))
    gd = ImageDraw.Draw(glow)
    for _ in range(45):
        x, y = rng.uniform(0, W), rng.uniform(0, H * 0.55)
        r = rng.uniform(10, 40)
        c = rng.choice([(120, 96, 60), (70, 82, 120), (110, 62, 54)])
        gd.ellipse([x - r, y - r, x + r, y + r], fill=c)
    img = ImageChops.screen(img, glow.filter(ImageFilter.GaussianBlur(30)))

    # Skyline: a far rank of hazy towers, then a darker near rank in front.
    # Keep them low — the sky is where the text goes.
    for rank, (lo, hi, tint) in enumerate(
        [(0.16, 0.34, (58, 50, 92)), (0.10, 0.24, (20, 18, 44))]
    ):
        layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
        ld = ImageDraw.Draw(layer)
        x = -40
        while x < W:
            w = rng.uniform(46, 120)
            h = rng.uniform(lo, hi) * H
            ld.rectangle([x, H - h, x + w, H], fill=tint + (255,))
            # Lit windows
            wx = x + 10
            while wx < x + w - 14:
                wy = H - h + 16
                while wy < H - 24:
                    if rng.random() < 0.30:
                        ld.rectangle([wx, wy, wx + 5, wy + 8], fill=(255, 226, 168, 205))
                    wy += 20
                wx += 15
            x += w + rng.uniform(8, 30)
        if rank == 0:
            layer = layer.filter(ImageFilter.GaussianBlur(3))
        img = Image.alpha_composite(img.convert("RGBA"), layer).convert("RGB")
    return grain(img)


def mountains():
    """Layered ridges: a calm background for a section divider."""
    img = vertical_gradient(
        [
            (0.00, (24, 46, 84)),
            (0.35, (58, 96, 140)),
            (0.62, (176, 148, 150)),
            (0.82, (244, 178, 128)),
            (1.00, (255, 214, 168)),
        ]
    )
    rng = random.Random(21)

    sun = Image.new("RGB", (W, H), (0, 0, 0))
    ImageDraw.Draw(sun).ellipse([W * 0.62, H * 0.46, W * 0.76, H * 0.60], fill=(190, 168, 120))
    img = ImageChops.screen(img, sun.filter(ImageFilter.GaussianBlur(60)))

    ridges = [(0.66, (52, 66, 104)), (0.76, (38, 48, 82)), (0.87, (24, 30, 56))]
    for i, (base, color) in enumerate(ridges):
        layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
        pts = [(0, H)]
        phase = rng.uniform(0, math.tau)
        for x in range(0, W + 20, 20):
            t = x / W
            y = base * H
            y -= math.sin(t * math.tau * (1.2 + i * 0.6) + phase) * H * (0.09 - i * 0.02)
            y -= math.sin(t * math.tau * 4.3 + phase * 2) * H * 0.025
            pts.append((x, y))
        pts.append((W, H))
        ImageDraw.Draw(layer).polygon(pts, fill=color + (255,))
        if i == 0:
            layer = layer.filter(ImageFilter.GaussianBlur(2))
        img = Image.alpha_composite(img.convert("RGBA"), layer).convert("RGB")
    return grain(img)


def mesh():
    """Soft colour mesh: a texture to sit behind a chart or a quote."""
    rng = random.Random(3)
    img = Image.new("RGB", (W, H), (16, 22, 40))
    blobs = Image.new("RGB", (W, H), (16, 22, 40))
    bd = ImageDraw.Draw(blobs)
    palette = [(48, 86, 211), (18, 184, 166), (86, 60, 190), (232, 92, 128)]
    for i in range(14):
        c = palette[i % len(palette)]
        x, y = rng.uniform(-0.1, 1.1) * W, rng.uniform(-0.1, 1.1) * H
        r = rng.uniform(0.18, 0.42) * W
        bd.ellipse([x - r, y - r, x + r, y + r], fill=c)
    blobs = blobs.filter(ImageFilter.GaussianBlur(150))
    img = Image.blend(img, blobs, 0.92)
    return grain(img, 4)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for name, fn in [("city-night", city_night), ("mountains", mountains), ("mesh", mesh)]:
        path = OUT / f"{name}.jpg"
        fn().save(path, "JPEG", quality=68, optimize=True, progressive=True)
        print(f"✓ {path.relative_to(OUT.parent.parent.parent)} ({path.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
