import { chromium } from "playwright";
import { readFileSync } from "node:fs";

function fail(message, detail) {
  const payload = { ok: false, error: message };
  if (detail) payload.detail = String(detail);
  process.stdout.write(JSON.stringify(payload));
  process.exit(0);
}

function requireString(value, name) {
  if (typeof value !== "string" || value.trim() === "") {
    fail(`missing required string: ${name}`);
  }
  return value;
}

async function main() {
  let request;
  try {
    request = JSON.parse(readFileSync(0, "utf8") || "{}");
  } catch (error) {
    fail("invalid JSON request", error);
  }

  const action = request.action || "navigate";
  const url = request.url || "about:blank";
  const viewport = request.viewport || {};
  const browser = await chromium.launch({
    headless: request.headless !== false,
    args: [
      "--disable-dev-shm-usage",
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
    ],
  });

  try {
    const context = await browser.newContext({
      viewport: {
        width: Number(viewport.width || 1365),
        height: Number(viewport.height || 768),
      },
      deviceScaleFactor: Number(viewport.deviceScaleFactor || 1),
      ignoreHTTPSErrors: true,
    });
    const page = await context.newPage();
    page.setDefaultTimeout(Number(request.timeout_ms || 30000));

    if (url !== "about:blank") {
      await page.goto(url, {
        waitUntil: request.wait_until || "networkidle",
        timeout: Number(request.timeout_ms || 30000),
      });
    }

    if (action === "navigate") {
      process.stdout.write(JSON.stringify({
        ok: true,
        action,
        url: page.url(),
        title: await page.title(),
      }));
      return;
    }

    if (action === "screenshot") {
      const selector = request.selector;
      const buffer = selector
        ? await page.locator(selector).screenshot()
        : await page.screenshot({ fullPage: request.full_page !== false });
      process.stdout.write(JSON.stringify({
        ok: true,
        action,
        url: page.url(),
        mime: "image/png",
        image_base64: buffer.toString("base64"),
      }));
      return;
    }

    if (action === "get_text" || action === "extract") {
      const selector = request.selector || "body";
      const text = await page.locator(selector).innerText({ timeout: Number(request.timeout_ms || 30000) });
      process.stdout.write(JSON.stringify({
        ok: true,
        action,
        url: page.url(),
        text,
      }));
      return;
    }

    if (action === "click") {
      const selector = requireString(request.selector, "selector");
      await page.locator(selector).click();
      process.stdout.write(JSON.stringify({
        ok: true,
        action,
        url: page.url(),
        text: `clicked ${selector}`,
      }));
      return;
    }

    if (action === "type") {
      const selector = requireString(request.selector, "selector");
      const text = request.text || "";
      await page.locator(selector).fill(text);
      process.stdout.write(JSON.stringify({
        ok: true,
        action,
        url: page.url(),
        text: `typed ${text.length} chars into ${selector}`,
      }));
      return;
    }

    if (action === "evaluate") {
      const js = requireString(request.js, "js");
      const result = await page.evaluate(js);
      process.stdout.write(JSON.stringify({
        ok: true,
        action,
        url: page.url(),
        result,
      }));
      return;
    }

    fail(`unknown action: ${action}`);
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  fail(
    "Playwright driver failed. Install runtime with `npm install` then `npx playwright install chromium`.",
    error?.stack || error
  );
});
