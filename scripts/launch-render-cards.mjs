// Render the launch SVG cards to high-DPI PNGs for the X/Reddit posts.
// Uses the system Chrome via playwright-core (no browser download needed).
import { chromium } from "playwright-core";
import { readFileSync, readdirSync } from "node:fs";
import { pathToFileURL } from "node:url";

const CHROME = "C:/Program Files/Google/Chrome/Application/chrome.exe";
const SRC = "C:/Sparrow/assets/launch";
const OUT = "C:/Sparrow/assets/launch/png";

const cards = readdirSync(SRC).filter((f) => f.endsWith(".svg"));

const browser = await chromium.launch({ executablePath: CHROME });
const ctx = await browser.newContext({ deviceScaleFactor: 2 });
const page = await ctx.newPage();

for (const file of cards) {
  const svg = readFileSync(`${SRC}/${file}`, "utf8");
  const m = svg.match(/width="(\d+)"\s+height="(\d+)"/);
  const w = m ? +m[1] : 1200;
  const h = m ? +m[2] : 675;
  await page.setViewportSize({ width: w, height: h });
  // Embed the SVG full-bleed so the screenshot is exactly the card.
  await page.setContent(
    `<!doctype html><html><body style="margin:0;background:#0e0b08">${svg}</body></html>`,
    { waitUntil: "networkidle" }
  );
  const out = `${OUT}/${file.replace(/\.svg$/, "")}.png`;
  await page.screenshot({ path: out, clip: { x: 0, y: 0, width: w, height: h } });
  console.log(`✓ ${file}  ->  ${out}  (${w}x${h} @2x)`);
}

await browser.close();
