// Audit interactif du cockpit WebView (v0.8.0) — pilote Edge via playwright-core.
// Usage: node scripts/audit-webview.mjs <step>
// Dumps: screenshots + erreurs console + transcript dans /tmp/sparrow-audit/
import { chromium } from 'playwright-core';
import fs from 'node:fs';

const OUT = 'C:/tmp/sparrow-audit';
fs.mkdirSync(OUT, { recursive: true });
const URL = 'http://127.0.0.1:9888';

const browser = await chromium.launch({
  executablePath: 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  headless: true,
});
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });

const consoleMsgs = [];
page.on('console', (m) => consoleMsgs.push(`[${m.type()}] ${m.text()}`));
page.on('pageerror', (e) => consoleMsgs.push(`[PAGEERROR] ${e.message}`));

const wsFrames = [];
page.on('websocket', (ws) => {
  ws.on('framereceived', (f) => wsFrames.push(`<< ${String(f.payload).slice(0, 500)}`));
  ws.on('framesent', (f) => wsFrames.push(`>> ${String(f.payload).slice(0, 500)}`));
});

async function snap(name) {
  await page.screenshot({ path: `${OUT}/${name}.png` });
}
async function dump(name) {
  fs.writeFileSync(`${OUT}/${name}-console.txt`, consoleMsgs.join('\n'));
  fs.writeFileSync(`${OUT}/${name}-ws.txt`, wsFrames.join('\n'));
}
async function transcriptText() {
  return page.evaluate(() => {
    const main = document.querySelector('.main');
    return main ? main.innerText : document.body.innerText;
  });
}

const step = process.argv[2] || 'hello';

await page.goto(URL, { waitUntil: 'networkidle' });
await page.waitForTimeout(2500);
await snap('01-boot');

if (step === 'hello') {
  // Conversation simple
  const composer = page.locator('#taskInput');
  await composer.click();
  await composer.fill('Bonjour ! Présente-toi en deux phrases. Qui es-tu et que sais-tu faire ?');
  await composer.press('Enter');
  await page.waitForTimeout(30000);
  await snap('02-hello-reply');
  fs.writeFileSync(`${OUT}/02-transcript.txt`, await transcriptText());
} else if (step === 'task') {
  // Tâche avec écriture de fichier → doit déclencher l'approbation
  const composer = page.locator('#taskInput');
  await composer.click();
  await composer.fill('Crée un fichier poeme.txt contenant un haïku sur les moineaux. Rien d\'autre.');
  await composer.press('Enter');
  await page.waitForTimeout(20000);
  await snap('03-task-running');
  // approbation visible ?
  const approveBtn = page.locator('.approval-card button, .approval-modal button').first();
  if (await approveBtn.isVisible().catch(() => false)) {
    await snap('04-approval');
    const labels = await page.locator('.approval-card button, .approval-modal button').allInnerTexts();
    fs.writeFileSync(`${OUT}/04-approval-buttons.txt`, labels.join(' | '));
    await approveBtn.click();
  }
  await page.waitForTimeout(40000);
  await snap('05-task-done');
  fs.writeFileSync(`${OUT}/05-transcript.txt`, await transcriptText());
} else if (step === 'approve') {
  // La page vient d'être rechargée (replay-on-connect). L'approbation du run
  // précédent est-elle restaurée et cliquable ?
  await snap('06-after-reload');
  fs.writeFileSync(`${OUT}/06-transcript.txt`, await transcriptText());
  const onceBtn = page.locator('.approval-actions button', { hasText: 'once' }).last();
  if (await onceBtn.isVisible().catch(() => false)) {
    await onceBtn.click();
    fs.appendFileSync(`${OUT}/ui-notes.txt`, 'approve: bouton once cliqué après reload\n');
  } else {
    fs.appendFileSync(`${OUT}/ui-notes.txt`, 'approve: AUCUN bouton d approbation après reload\n');
    // run encore vivant ? on retente la même tâche
    const composer = page.locator('#taskInput');
    await composer.click();
    await composer.fill('Crée un fichier poeme.txt contenant un haïku sur les moineaux. Rien d\'autre.');
    await composer.press('Enter');
    await page.waitForTimeout(15000);
    const btn2 = page.locator('.approval-actions button', { hasText: 'once' }).last();
    if (await btn2.isVisible().catch(() => false)) {
      await btn2.click();
      fs.appendFileSync(`${OUT}/ui-notes.txt`, 'approve: once cliqué sur nouveau run\n');
    }
  }
  await page.waitForTimeout(45000);
  await snap('07-after-approve');
  fs.writeFileSync(`${OUT}/07-transcript.txt`, await transcriptText());
} else if (step === 'ui') {
  // Tour de l'UI sans modèle : panneaux du rail, palette, thème
  const rail = page.locator('.rail [data-panel], .rail button, .rail a');
  const count = await rail.count();
  fs.appendFileSync(`${OUT}/ui-notes.txt`, `rail items: ${count}\n`);
  for (let i = 0; i < count && i < 14; i++) {
    await rail.nth(i).click().catch(() => {});
    await page.waitForTimeout(400);
    await snap(`ui-rail-${i}`);
  }
  await page.keyboard.press('Control+k');
  await page.waitForTimeout(600);
  await snap('ui-palette');
  await page.keyboard.press('Escape');
}

await dump(step);
await browser.close();
console.log('OK — artefacts dans', OUT);
console.log('--- erreurs console ---');
console.log(consoleMsgs.filter((m) => m.includes('ERROR') || m.includes('[error]')).join('\n') || '(aucune)');
