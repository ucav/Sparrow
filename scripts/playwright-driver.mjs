import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function requireFrom(baseDir) {
  return createRequire(path.join(baseDir, "sparrow-playwright-runtime.cjs"));
}

function loadPlaywright() {
  const roots = [
    process.env.SPARROW_PLAYWRIGHT_ROOT,
    process.cwd(),
    __dirname,
  ].filter(Boolean);
  const tried = [];
  for (const root of roots) {
    const base = path.resolve(root);
    tried.push(base);
    try {
      return requireFrom(base)("playwright");
    } catch {
      // Try the next candidate. The final error includes all attempted roots.
    }
  }
  fail(
    "Playwright package not found",
    `Install runtime with \`npm install\` in the Sparrow checkout, or set SPARROW_PLAYWRIGHT_ROOT. Tried: ${tried.join(", ")}`
  );
}

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
  const { chromium } = loadPlaywright();
  let request;
  try {
    request = JSON.parse(readFileSync(0, "utf8") || "{}");
  } catch (error) {
    fail("invalid JSON request", error);
  }

  const action = request.action || "navigate";
  const viewport = request.viewport || {};
  const sessionId = typeof request.session_id === "string" && request.session_id.trim()
    ? request.session_id.replace(/[^a-zA-Z0-9_.-]/g, "_").slice(0, 80)
    : "";
  const userDataDir = sessionId
    ? path.join(tmpdir(), "sparrow-playwright-sessions", sessionId)
    : "";
  if (userDataDir) mkdirSync(userDataDir, { recursive: true });
  const statePath = userDataDir ? path.join(userDataDir, "sparrow-state.json") : "";
  let state = {};
  if (statePath) {
    try {
      state = JSON.parse(readFileSync(statePath, "utf8"));
    } catch {
      state = {};
    }
  }
  const hasExplicitUrl = Object.prototype.hasOwnProperty.call(request, "url");
  const url = hasExplicitUrl ? (request.url || "about:blank") : (state.last_url || "about:blank");

  const launchOptions = {
    headless: request.headless !== false,
    args: [
      "--disable-dev-shm-usage",
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
    ],
  };

  const browserOrContext = userDataDir
    ? await chromium.launchPersistentContext(userDataDir, {
        ...launchOptions,
        viewport: {
          width: Number(viewport.width || 1365),
          height: Number(viewport.height || 768),
        },
        deviceScaleFactor: Number(viewport.deviceScaleFactor || 1),
        ignoreHTTPSErrors: true,
      })
    : await chromium.launch(launchOptions);

  try {
    const context = userDataDir
      ? browserOrContext
      : await browserOrContext.newContext({
          viewport: {
            width: Number(viewport.width || 1365),
            height: Number(viewport.height || 768),
          },
          deviceScaleFactor: Number(viewport.deviceScaleFactor || 1),
          ignoreHTTPSErrors: true,
        });
    const existingPages = context.pages();
    const page = existingPages[0] || await context.newPage();
    page.setDefaultTimeout(Number(request.timeout_ms || 30000));

    if (url !== "about:blank") {
      await page.goto(url, {
        waitUntil: request.wait_until || "networkidle",
        timeout: Number(request.timeout_ms || 30000),
      });
    }

    const finish = (payload) => {
      if (statePath) {
        try {
          writeFileSync(statePath, JSON.stringify({ last_url: page.url() }));
        } catch {
          // Non-fatal: the action result matters more than session bookkeeping.
        }
      }
      process.stdout.write(JSON.stringify(payload));
    };

    if (action === "navigate") {
      finish({
        ok: true,
        action,
        url: page.url(),
        title: await page.title(),
      });
      return;
    }

    if (action === "screenshot") {
      const selector = request.selector;
      const buffer = selector
        ? await page.locator(selector).screenshot()
        : await page.screenshot({ fullPage: request.full_page !== false });
      finish({
        ok: true,
        action,
        url: page.url(),
        mime: "image/png",
        image_base64: buffer.toString("base64"),
      });
      return;
    }

    if (action === "get_text" || action === "extract") {
      const selector = request.selector || "body";
      const text = await page.locator(selector).innerText({ timeout: Number(request.timeout_ms || 30000) });
      finish({
        ok: true,
        action,
        url: page.url(),
        text,
      });
      return;
    }

    if (action === "click") {
      const hasCoords = Number.isFinite(Number(request.x)) && Number.isFinite(Number(request.y));
      if (hasCoords) {
        await page.mouse.click(Number(request.x), Number(request.y), {
          button: request.button || "left",
          clickCount: Number(request.click_count || 1),
        });
      } else {
        const selector = requireString(request.selector, "selector or x/y");
        await page.locator(selector).click();
      }
      finish({
        ok: true,
        action,
        url: page.url(),
        text: hasCoords ? `clicked ${Number(request.x)},${Number(request.y)}` : `clicked ${request.selector}`,
      });
      return;
    }

    if (action === "type") {
      const text = request.text || "";
      if (request.selector) {
        await page.locator(request.selector).fill(text);
      } else if (Number.isFinite(Number(request.x)) && Number.isFinite(Number(request.y))) {
        await page.mouse.click(Number(request.x), Number(request.y));
        await page.keyboard.type(text);
      } else {
        await page.keyboard.type(text);
      }
      finish({
        ok: true,
        action,
        url: page.url(),
        text: request.selector
          ? `typed ${text.length} chars into ${request.selector}`
          : `typed ${text.length} chars`,
      });
      return;
    }

    if (action === "press") {
      const key = requireString(request.key, "key");
      await page.keyboard.press(key);
      finish({
        ok: true,
        action,
        url: page.url(),
        text: `pressed ${key}`,
      });
      return;
    }

    if (action === "evaluate") {
      const js = requireString(request.js, "js");
      const result = await page.evaluate(js);
      finish({
        ok: true,
        action,
        url: page.url(),
        result,
      });
      return;
    }

    fail(`unknown action: ${action}`);
  } finally {
    await browserOrContext.close();
  }
}

main().catch((error) => {
  fail(
    "Playwright driver failed. Install runtime with `npm install` then `npx playwright install chromium`.",
    error?.stack || error
  );
});
