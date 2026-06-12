import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { chromium } from 'playwright';

const target =
  process.argv[2] ||
  process.env.SPARROW_A11Y_URL ||
  pathToFileURL(resolve('console.html')).href;

async function launchBrowser() {
  const candidates = [
    process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
    'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Google/Chrome/Application/chrome.exe',
  ].filter(Boolean);
  for (const executablePath of candidates) {
    try {
      return await chromium.launch({ executablePath, headless: true });
    } catch (_) {}
  }
  return chromium.launch({ headless: true });
}

function parseRgb(value) {
  const m = String(value).match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/i);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : [0, 0, 0];
}

function luminance([r, g, b]) {
  const f = (v) => {
    v /= 255;
    return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function contrastRatio(fg, bg) {
  const a = luminance(fg);
  const b = luminance(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

const browser = await launchBrowser();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
page.on('console', (msg) => {
  if (msg.type() !== 'error') return;
  const text = msg.text();
  if (
    target.startsWith('file:') &&
    (/Cross origin requests are only supported/.test(text) ||
      /Failed to load resource: net::ERR_FAILED/.test(text) ||
      /WebSocket connection to 'ws:\/\/ws\/' failed/.test(text))
  ) {
    return;
  }
  errors.push(text);
});

await page.goto(target, { waitUntil: 'domcontentloaded' });
await page.waitForTimeout(900);

const checks = [];
const pass = (name, ok, detail = '') => checks.push({ name, ok: Boolean(ok), detail });

const initial = await page.evaluate(() => ({
  view: document.documentElement.dataset.view,
  focusVisible: getComputedStyle(document.querySelector('.focus-actions')).display,
  railVisible: getComputedStyle(document.querySelector('.rail')).display,
  micLabel: document.getElementById('micBtn')?.getAttribute('aria-label') || '',
  focusPressed: document.getElementById('focusModeBtn')?.getAttribute('aria-pressed'),
  cockpitPressed: document.getElementById('cockpitModeBtn')?.getAttribute('aria-pressed'),
  quickLabels: [...document.querySelectorAll('.focus-actions button')].map((b) => b.textContent.trim()),
  termFont: getComputedStyle(document.getElementById('term')).fontSize,
  fg: getComputedStyle(document.getElementById('term')).color,
  bg: getComputedStyle(document.body).backgroundColor,
}));

pass('Focus mode is default', initial.view === 'focus', initial.view);
pass('Focus actions are visible', initial.focusVisible !== 'none', initial.focusVisible);
pass('Cockpit rail hidden in Focus', initial.railVisible === 'none', initial.railVisible);
pass('Focus toggle pressed state', initial.focusPressed === 'true' && initial.cockpitPressed === 'false');
pass('Persistent Focus actions', ['OK', 'Undo', 'Explain'].every((label) => initial.quickLabels.includes(label)), initial.quickLabels.join(', '));
pass('Mic has accessible label', /microphone/i.test(initial.micLabel), initial.micLabel);
pass('AA contrast for terminal text', contrastRatio(parseRgb(initial.fg), parseRgb(initial.bg)) >= 4.5, `${initial.fg} on ${initial.bg}`);

await page.click('#cockpitModeBtn');
await page.waitForTimeout(120);
const cockpit = await page.evaluate(() => ({
  view: document.documentElement.dataset.view,
  railVisible: getComputedStyle(document.querySelector('.rail')).display,
}));
pass('Cockpit toggle works', cockpit.view === 'cockpit' && cockpit.railVisible !== 'none', JSON.stringify(cockpit));

await page.keyboard.down('Alt');
await page.keyboard.press('KeyF');
await page.keyboard.up('Alt');
await page.waitForTimeout(120);
const afterAltF = await page.evaluate(() => document.documentElement.dataset.view);
pass('Alt+F toggles view', afterAltF === 'focus', afterAltF);

const beforeFont = parseFloat(initial.termFont);
await page.click('#fontUpBtn');
await page.click('#fontUpBtn');
await page.waitForTimeout(120);
const afterFont = await page.evaluate(() => ({
  size: parseFloat(getComputedStyle(document.getElementById('term')).fontSize),
  stored: localStorage.getItem('sparrow-read-scale'),
}));
pass('A+ increases reading size', afterFont.size > beforeFont, `${beforeFont} -> ${afterFont.size}`);
pass('Reading size persists', Number(afterFont.stored) > 1, String(afterFont.stored));

const mainWidthBeforeRightbar = await page.evaluate(() => document.querySelector('.main')?.getBoundingClientRect().width || 0);
await page.click('#rightbarBtn');
await page.waitForTimeout(180);
const mainWidthAfterRightbar = await page.evaluate(() => ({
  main: document.querySelector('.main')?.getBoundingClientRect().width || 0,
  rightbarOpen: document.body.classList.contains('rightbar-open'),
}));
pass(
  'Right tools panel overlays Focus without shifting center',
  mainWidthAfterRightbar.rightbarOpen && Math.abs(mainWidthAfterRightbar.main - mainWidthBeforeRightbar) <= 1,
  `${mainWidthBeforeRightbar} -> ${mainWidthAfterRightbar.main}`,
);
pass('No JavaScript runtime errors', errors.length === 0, errors.join('\n'));

const failed = checks.filter((c) => !c.ok);
console.log(JSON.stringify({ target, checks, errors }, null, 2));
await browser.close();
if (failed.length) process.exit(1);
