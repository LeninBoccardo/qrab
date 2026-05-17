/**
 * Extract individual icon / logo assets from the visual identity sheet.
 *
 * Usage:
 *   node scripts/extract-icons.mjs              extract all entries from JSON
 *   node scripts/extract-icons.mjs --preview    instead, write a single
 *                                               preview PNG with every
 *                                               crop drawn as a labeled
 *                                               red rectangle over the
 *                                               source. Lets you sanity-
 *                                               check coordinates before
 *                                               running the real extract.
 *
 * The crop map and per-entry background-removal mode live in
 * docs/branding/extractions.json — adjust there, no code changes needed.
 */

import { mkdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "..");

const configPath = resolve(repoRoot, "docs/branding/extractions.json");
const config = JSON.parse(readFileSync(configPath, "utf-8"));
const sourcePath = resolve(repoRoot, config.source);
const outDir = resolve(repoRoot, config.outDir);

const wantPreview = process.argv.includes("--preview");

mkdirSync(outDir, { recursive: true });

if (wantPreview) {
  await renderPreview();
} else {
  await extractAll();
}

async function renderPreview() {
  const { width, height } = await sharp(sourcePath).metadata();
  const palette = [
    "#e11d48",
    "#2563eb",
    "#16a34a",
    "#ca8a04",
    "#9333ea",
    "#0891b2",
    "#dc2626",
    "#0284c7",
  ];
  const svg = [
    `<svg width="${width}" height="${height}" xmlns="http://www.w3.org/2000/svg">`,
    ...config.outputs.flatMap((entry, i) => {
      const color = palette[i % palette.length];
      const { left, top, width: w, height: h } = entry.crop;
      return [
        `<rect x="${left}" y="${top}" width="${w}" height="${h}" fill="none" stroke="${color}" stroke-width="3"/>`,
        `<rect x="${left}" y="${top - 16}" width="${entry.name.length * 7 + 8}" height="14" fill="${color}"/>`,
        `<text x="${left + 4}" y="${top - 4}" font-family="monospace" font-size="11" fill="white" font-weight="bold">${entry.name}</text>`,
      ];
    }),
    `</svg>`,
  ].join("\n");

  const previewPath = resolve(outDir, "_preview.png");
  await sharp(sourcePath)
    .composite([{ input: Buffer.from(svg), top: 0, left: 0 }])
    .png()
    .toFile(previewPath);
  console.log(`✓ preview → ${previewPath}`);
}

async function extractAll() {
  let count = 0;
  for (const entry of config.outputs) {
    const outPath = resolve(outDir, `${entry.name}.png`);
    if (entry.transparent && entry.transparent !== "none") {
      await extractWithTransparency(entry, outPath);
    } else {
      await sharp(sourcePath).extract(entry.crop).png().toFile(outPath);
    }
    count++;
    console.log(`✓ ${entry.name}.png`);
  }
  console.log(`\nExtracted ${count} file(s) → ${outDir}`);
}

/**
 * Walk the cropped pixel buffer; flip alpha to 0 for pixels matching the
 * declared background tone. `lighten` zeroes near-white pixels (cells
 * designed on a light background where the logo is dark). `darken` does
 * the inverse for cells designed on dark backgrounds with white strokes.
 */
async function extractWithTransparency(entry, outPath) {
  const mode = entry.transparent;
  const tolerance = entry.tolerance ?? 30; // per-channel tolerance
  const { data, info } = await sharp(sourcePath)
    .extract(entry.crop)
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });

  const buf = Buffer.from(data);
  const channelHigh = 255 - tolerance;
  const channelLow = tolerance;

  for (let i = 0; i < buf.length; i += 4) {
    const r = buf[i];
    const g = buf[i + 1];
    const b = buf[i + 2];
    if (mode === "lighten") {
      // Near-white → transparent. Anti-aliased edges get partial alpha
      // proportional to how white they are.
      if (r >= channelHigh && g >= channelHigh && b >= channelHigh) {
        buf[i + 3] = 0;
      }
    } else if (mode === "darken") {
      // Near-black/dark-gray → transparent.
      if (r <= channelLow && g <= channelLow && b <= channelLow) {
        buf[i + 3] = 0;
      }
    }
  }

  await sharp(buf, {
    raw: { width: info.width, height: info.height, channels: 4 },
  })
    .png()
    .toFile(outPath);
}
