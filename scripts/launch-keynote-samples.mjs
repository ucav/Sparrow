import { chromium } from "playwright-core";
import { pathToFileURL } from "node:url";
const CHROME = "C:/Program Files/Google/Chrome/Application/chrome.exe";
const HTML = pathToFileURL("C:/Sparrow/scripts/launch-keynote.html").href;
const OUT = "C:/Sparrow/assets/launch/video";
const browser = await chromium.launch({ executablePath: CHROME });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
await page.goto(HTML, { waitUntil: "networkidle" });
const stamps = [1.6, 6.2, 7.3, 10.4, 14.6, 20.2];
for (const t of stamps) {
  await page.evaluate((t) => window.seek(t), t);
  await page.screenshot({ path: `${OUT}/sample-${t}.png`, clip: { x: 0, y: 0, width: 1920, height: 1080 } });
  console.log("✓ t=" + t);
}
await browser.close();
