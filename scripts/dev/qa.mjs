// Drives the real Mocó dev app over WebView2's DevTools protocol.
// Start the app with:  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 pnpm dev
// Usage: node scripts/dev/qa.mjs <script.mjs>   — the script default-exports async (page, shot) => {}
import { chromium } from "playwright-core";
import path from "node:path";
import { pathToFileURL } from "node:url";

const OUT = process.env.QA_OUT ?? path.resolve("../moco-qa/shots");
const port = process.env.QA_PORT ?? "9222";

const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
const ctx = browser.contexts()[0];
const pages = ctx.pages().filter((p) => !p.url().startsWith("devtools"));
const page = (process.env.QA_WINDOW === "quick" ? pages.find((p) => p.url().includes("w=quick")) : pages.find((p) => !p.url().includes("w=quick"))) ?? pages[0];
page.setDefaultTimeout(8000);

const shot = async (name, opts = {}) => {
  const file = path.join(OUT, `${name}.png`);
  await page.waitForTimeout(opts.wait ?? 350);
  await page.screenshot({ path: file, fullPage: false });
  console.log("shot", file);
  return file;
};

const errors = [];
page.on("console", (m) => {
  if (m.type() === "error" || m.type() === "warning") errors.push(`[${m.type()}] ${m.text()}`);
});
page.on("pageerror", (e) => errors.push(`[pageerror] ${e.message}`));

const scriptPath = process.argv[2];
if (!scriptPath) {
  console.log("url:", page.url());
  await shot("current");
} else {
  const mod = await import(pathToFileURL(path.resolve(scriptPath)).href);
  await mod.default(page, shot);
}
if (errors.length) console.log("CONSOLE:\n" + errors.join("\n"));
await browser.close().catch(() => {});
