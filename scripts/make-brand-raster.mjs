// Renders the raster brand assets that a vector file cannot stand in for: the
// social preview card (link unfurls want a PNG), the square app icon, and the
// extension icon, which the marketplaces will not take as an SVG either.
//
//   node scripts/make-brand-raster.mjs           # writes the files listed below
//   MIRZAM_CHROMIUM=/path/to/chrome node scripts/make-brand-raster.mjs
//
// Everything it composes is already in docs/brand/, and the wordmark carries its
// type as outlines, so this needs no font download and no network.

import { chromium } from "playwright-core";
import { readFileSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

const BRAND = resolve(dirname(fileURLToPath(import.meta.url)), "..", "docs", "brand");
const MIME = { svg: "image/svg+xml", webp: "image/webp", png: "image/png" };

/** docs/brand/<name> as a data: URI, so the page needs no file:// permissions. */
function asset(name) {
  const ext = name.split(".").pop();
  return `data:${MIME[ext]};base64,${readFileSync(join(BRAND, name)).toString("base64")}`;
}

// 1200x630 is what Open Graph, Twitter and Slack all crop from.
const CARD = { width: 1200, height: 630 };
const cardHtml = `<!doctype html><meta charset="utf-8"><style>
  html, body { margin: 0; }
  body {
    width: ${CARD.width}px; height: ${CARD.height}px; background: #080C18;
    display: flex; align-items: center;
  }
  .bg {
    position: absolute; inset: 0;
    background: url("${asset("mirzam-hero-dark.webp")}") 18% 72% / cover no-repeat;
  }
  /* The star flare sits bottom left, so the lockup goes above it and the
     gradient keeps the type off the brightest part of the limb. */
  .scrim {
    position: absolute; inset: 0;
    background: linear-gradient(180deg, rgba(8,12,24,.55) 0%, rgba(8,12,24,0) 55%);
  }
  img { position: relative; margin: 0 0 96px 96px; width: 720px; }
</style>
<div class="bg"></div><div class="scrim"></div>
<img src="${asset("mirzam-logo-dark.svg")}" alt="Mirzam">`;

const iconHtml = (svg) => `<!doctype html><meta charset="utf-8"><style>
  html, body { margin: 0; width: 512px; height: 512px; }
  img { display: block; width: 512px; height: 512px; }
</style>
<img src="${asset(svg)}" alt="Mirzam">`;

const ICON = { width: 512, height: 512 };
const ROOT = resolve(BRAND, "..", "..");

const browser = await chromium.launch({ executablePath: process.env.MIRZAM_CHROMIUM || undefined });
try {
  for (const [out, html, viewport, opts = {}] of [
    [join(BRAND, "mirzam-social-card.png"), cardHtml, CARD],
    [join(BRAND, "mirzam-icon-512.png"), iconHtml("mirzam-icon-light.svg"), ICON],

    // The extension icon. A marketplace shows one icon against a pale web page
    // *and* a dark extensions sidebar and takes no theme variants, so it gets
    // the tile that carries its own ground rather than the light one, which
    // would be a white square wherever it landed.
    //
    // `omitBackground` is what makes the tile's rounded corners transparent
    // rather than page-white: a screenshot has no alpha by default, and four
    // white triangles on a dark sidebar is the whole reason this file is not
    // just a copy of the light raster.
    [
      join(ROOT, "editors", "vscode", "media", "icon.png"),
      iconHtml("mirzam-icon-dark.svg"),
      ICON,
      { omitBackground: true },
    ],
  ]) {
    const page = await browser.newPage({ viewport });
    await page.setContent(html);
    await page.waitForTimeout(200);
    writeFileSync(out, await page.screenshot(opts));
    await page.close();
    console.log(`  ${out.slice(ROOT.length + 1)}  ${viewport.width}x${viewport.height}`);
  }
} finally {
  await browser.close();
}
