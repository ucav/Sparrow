import { readFileSync } from 'node:fs';
import { chromium } from 'playwright';

const jobs = process.argv.slice(2);

if (jobs.length === 0 || jobs.length % 2 !== 0) {
  console.error('usage: node scripts/render-tui-png.mjs <input.svg> <output.png> [...]');
  process.exit(1);
}

const browser = await chromium.launch({ headless: true });

try {
  for (let i = 0; i < jobs.length; i += 2) {
    const inputPath = jobs[i];
    const outputPath = jobs[i + 1];
    const svg = readFileSync(inputPath, 'utf8');
    const width = Number(svg.match(/width="([0-9]+)"/)?.[1] ?? 1200);
    const height = Number(svg.match(/height="([0-9]+)"/)?.[1] ?? 760);
    const page = await browser.newPage({
      viewport: { width, height },
      deviceScaleFactor: 1,
    });
    page.setDefaultTimeout(0);
    await page.setContent(svg, { waitUntil: 'domcontentloaded' });
    await page.screenshot({ path: outputPath, timeout: 0 });
    await page.close();
  }
} finally {
  await browser.close();
}
