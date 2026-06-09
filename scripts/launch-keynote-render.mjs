// Deterministically capture the keynote as PNG frames by stepping window.seek(t).
import { chromium } from "playwright-core";
import { pathToFileURL } from "node:url";
import { rmSync, mkdirSync } from "node:fs";

const CHROME = "C:/Program Files/Google/Chrome/Application/chrome.exe";
const HTML = pathToFileURL("C:/Sparrow/scripts/launch-keynote.html").href;
const FRAMES = "C:/Sparrow/assets/launch/video/frames";

rmSync(FRAMES, { recursive: true, force: true });
mkdirSync(FRAMES, { recursive: true });

const browser = await chromium.launch({ executablePath: CHROME });
const page = await browser.newPage({
  viewport: { width: 1920, height: 1080 },
  deviceScaleFactor: 1,
});
await page.goto(HTML, { waitUntil: "networkidle" });

const FPS = await page.evaluate(() => window.__FPS);
const DUR = await page.evaluate(() => window.__DUR);
const total = Math.round(FPS * DUR);
console.log(`rendering ${total} frames @ ${FPS}fps (${DUR}s)`);

for (let f = 0; f < total; f++) {
  const t = f / FPS;
  await page.evaluate((t) => window.seek(t), t);
  const n = String(f).padStart(5, "0");
  await page.screenshot({ path: `${FRAMES}/f${n}.png`, clip: { x: 0, y: 0, width: 1920, height: 1080 } });
  if (f % 60 === 0) console.log(`  frame ${f}/${total}`);
}
console.log("done capturing");
await browser.close();
