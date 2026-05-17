/**
 * Extract individual icon / logo assets from the visual identity sheet.
 *
 * Strategy: each JSON entry specifies a *generous* bounding box around
 * the target cell. After we apply the entry's transparency mode (zero
 * alpha on near-white or near-black pixels), sharp's `.trim()` auto-
 * crops to the content bounds. So the JSON crop just needs to fully
 * contain the asset — centering happens automatically.
 *
 * Usage:
 *   node scripts/extract-icons.mjs              extract every entry
 *   node scripts/extract-icons.mjs --preview    overlay labeled rectangles
 *                                               on the source so you can
 *                                               sanity-check coordinates
 *
 * Per-entry options (see docs/branding/extractions.json):
 *   crop       { left, top, width, height }   — generous bbox in source px
 *   transparent  "lighten" | "darken" | "none"
 *   tolerance  channel tolerance for transparency (default 30)
 *   trim       true | false (default true except when transparent="none")
 *   trimThreshold  trim sensitivity (default 10)
 *   padding    transparent px added back after trim (default 0)
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
    "#7c3aed",
    "#059669",
    "#f59e0b",
    "#be185d",
    "#1d4ed8",
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
    await extractOne(entry, outPath);
    count++;
    console.log(`✓ ${entry.name}.png`);
  }
  console.log(`\nExtracted ${count} file(s) → ${outDir}`);
}

async function extractOne(entry, outPath) {
  const mode = entry.transparent ?? "none";
  const wantTrim = entry.trim ?? mode !== "none";
  const padding = entry.padding ?? 0;
  const trimThreshold = entry.trimThreshold ?? 10;

  // Step 1: extract the generous bounding box with alpha.
  let buf;
  let info;
  if (mode === "none") {
    ({ data: buf, info } = await sharp(sourcePath)
      .extract(entry.crop)
      .ensureAlpha()
      .raw()
      .toBuffer({ resolveWithObject: true }));
  } else {
    const tolerance = entry.tolerance ?? 30;
    const { data, info: infoOut } = await sharp(sourcePath)
      .extract(entry.crop)
      .ensureAlpha()
      .raw()
      .toBuffer({ resolveWithObject: true });
    info = infoOut;
    buf = Buffer.from(data);
    const channelHigh = 255 - tolerance;
    const channelLow = tolerance;
    for (let i = 0; i < buf.length; i += 4) {
      const r = buf[i];
      const g = buf[i + 1];
      const b = buf[i + 2];
      if (mode === "lighten") {
        if (r >= channelHigh && g >= channelHigh && b >= channelHigh) {
          buf[i + 3] = 0;
        }
      } else if (mode === "darken") {
        if (r <= channelLow && g <= channelLow && b <= channelLow) {
          buf[i + 3] = 0;
        }
      }
    }
  }

  // Step 2: re-wrap into a sharp pipeline.
  let pipeline = sharp(buf, {
    raw: { width: info.width, height: info.height, channels: 4 },
  });

  // Step 3: trim to content if requested. sharp.trim() with a transparent
  // background and threshold removes alpha-0 borders (and near-alpha-0
  // borders if threshold > 0).
  if (wantTrim) {
    pipeline = pipeline.trim({
      background: { r: 0, g: 0, b: 0, alpha: 0 },
      threshold: trimThreshold,
    });
  }

  // Step 4: optional uniform transparent padding so the asset breathes.
  if (padding > 0) {
    pipeline = pipeline.extend({
      top: padding,
      bottom: padding,
      left: padding,
      right: padding,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    });
  }

  await pipeline.png().toFile(outPath);
}
