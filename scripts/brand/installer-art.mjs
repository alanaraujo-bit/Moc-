// Renders the NSIS installer artwork (sidebar 164×314, header 150×57) in headless Edge
// and writes 24-bit BMPs. Run: node scripts/brand/installer-art.mjs
// Provenance: drawn procedurally below (no external imagery).
import { chromium } from "playwright-core";
import fs from "node:fs";
import path from "node:path";

const OUT = path.resolve("apps/desktop/src-tauri/installer");
fs.mkdirSync(OUT, { recursive: true });

// Renders inside the running dev app (WebView2 over CDP, see scripts/dev/qa.mjs).
const browser = await chromium.connectOverCDP("http://127.0.0.1:9222");
const page = browser.contexts()[0].pages().find((p) => !p.url().includes("w=quick"));
await page.evaluate(() => {
  const c = document.createElement("canvas");
  c.id = "c";
  c.style.cssText = "position:fixed;left:0;top:0;z-index:99999";
  document.body.appendChild(c);
});

async function render(w, h, draw) {
  return page.evaluate(
    ({ w, h, draw }) => {
      const c = document.getElementById("c");
      c.width = w;
      c.height = h;
      const ctx = c.getContext("2d");
      // eslint-disable-next-line no-new-func
      new Function("ctx", "w", "h", draw)(ctx, w, h);
      return Array.from(ctx.getImageData(0, 0, w, h).data);
    },
    { w, h, draw: draw.toString().replace(/^[^{]*{/, "").replace(/}\s*$/, "") },
  );
}

function bmp(w, h, rgba) {
  const rowSize = Math.ceil((w * 3) / 4) * 4;
  const size = 54 + rowSize * h;
  const b = Buffer.alloc(size);
  b.write("BM", 0);
  b.writeUInt32LE(size, 2);
  b.writeUInt32LE(54, 10);
  b.writeUInt32LE(40, 14);
  b.writeInt32LE(w, 18);
  b.writeInt32LE(h, 22);
  b.writeUInt16LE(1, 26);
  b.writeUInt16LE(24, 28);
  b.writeUInt32LE(rowSize * h, 34);
  b.writeInt32LE(2835, 38);
  b.writeInt32LE(2835, 42);
  for (let y = 0; y < h; y++) {
    const row = 54 + (h - 1 - y) * rowSize;
    for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 4;
      b[row + x * 3] = rgba[i + 2];
      b[row + x * 3 + 1] = rgba[i + 1];
      b[row + x * 3 + 2] = rgba[i];
    }
  }
  return b;
}

// Shared drawing helpers are inlined into each draw body (they run in the page).
const sidebar = () => {
  const COBALT = "#2340a8";
  const GLAZE = "#f4f6fb";
  ctx.fillStyle = COBALT;
  ctx.fillRect(0, 0, w, h);
  const t = 41; // 4 columns
  let seed = 11;
  const rnd = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
  for (let r = 0; r < Math.ceil(h / t) - 2; r++) {
    for (let c = 0; c < 4; c++) {
      const x = c * t;
      const y = r * t;
      const m = Math.floor(rnd() * 6);
      const rot = Math.floor(rnd() * 4);
      ctx.save();
      ctx.translate(x + t / 2, y + t / 2);
      ctx.rotate((rot * Math.PI) / 2);
      ctx.translate(-t / 2, -t / 2);
      ctx.fillStyle = "rgba(255,255,255,0.14)";
      ctx.beginPath();
      if (m === 0) {
        ctx.moveTo(0, 0);
        ctx.arc(0, 0, t * 0.62, 0, Math.PI / 2);
      } else if (m === 1) {
        ctx.arc(t / 2, 0, t * 0.3, 0, Math.PI);
      } else if (m === 2) {
        ctx.fillRect(0, t * 0.18, t, t * 0.12);
        ctx.fillRect(0, t * 0.42, t, t * 0.12);
      } else if (m === 3) {
        ctx.arc(0, 0, t * 0.78, 0, Math.PI / 2);
        ctx.arc(0, 0, t * 0.56, Math.PI / 2, 0, true);
      }
      ctx.closePath();
      ctx.fill();
      ctx.restore();
      ctx.strokeStyle = "rgba(255,255,255,0.10)";
      ctx.strokeRect(x + 0.5, y + 0.5, t, t);
    }
  }
  // Mark: white tile with a cobalt doorway, bottom-left.
  const s = 44;
  const mx = 18;
  const my = h - 70;
  ctx.fillStyle = GLAZE;
  ctx.beginPath();
  ctx.roundRect(mx, my, s, s, 10);
  ctx.fill();
  ctx.fillStyle = COBALT;
  const dw = s * 0.38;
  ctx.beginPath();
  ctx.moveTo(mx + (s - dw) / 2, my + s);
  ctx.lineTo(mx + (s - dw) / 2, my + s * 0.58);
  ctx.arc(mx + s / 2, my + s * 0.58, dw / 2, Math.PI, 0);
  ctx.lineTo(mx + (s + dw) / 2, my + s);
  ctx.closePath();
  ctx.fill();
  ctx.fillStyle = "#ffffff";
  ctx.font = "600 26px 'Segoe UI Variable Display','Segoe UI',sans-serif";
  ctx.textBaseline = "middle";
  ctx.fillText("mocó", mx + s + 12, my + s / 2 + 1);
};

const header = () => {
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, w, h);
  const COBALT = "#2340a8";
  // Three tiles on the right edge, fading in.
  const t = 19;
  for (let r = 0; r < 3; r++) {
    for (let c = 0; c < 4; c++) {
      const x = w - (c + 1) * t;
      const y = r * t;
      ctx.strokeStyle = "#e2e5eb";
      ctx.strokeRect(x + 0.5, y + 0.5, t, t);
      if ((r + c) % 3 === 0) {
        ctx.fillStyle = `rgba(35,64,168,${0.18 + 0.2 * (3 - c) / 3})`;
        ctx.beginPath();
        ctx.moveTo(x, y);
        ctx.arc(x, y, t * 0.7, 0, Math.PI / 2);
        ctx.closePath();
        ctx.fill();
      }
    }
  }
  ctx.fillStyle = COBALT;
  const s = 30;
  const mx = w - 4 * t - s - 10;
  const my = (h - s) / 2;
  ctx.beginPath();
  ctx.roundRect(mx, my, s, s, 7);
  ctx.fill();
  ctx.fillStyle = "#ffffff";
  const dw = s * 0.38;
  ctx.beginPath();
  ctx.moveTo(mx + (s - dw) / 2, my + s);
  ctx.lineTo(mx + (s - dw) / 2, my + s * 0.58);
  ctx.arc(mx + s / 2, my + s * 0.58, dw / 2, Math.PI, 0);
  ctx.lineTo(mx + (s + dw) / 2, my + s);
  ctx.closePath();
  ctx.fill();
};

for (const [name, w, h, fn] of [
  ["sidebar.bmp", 164, 314, sidebar],
  ["header.bmp", 150, 57, header],
]) {
  const px = await render(w, h, fn);
  fs.writeFileSync(path.join(OUT, name), bmp(w, h, px));
  // PNG preview for review.
  await page.evaluate(() => 0);
  console.log("wrote", name);
}

// Previews at 3x for visual QA.
const shot = async (fn, w, h, file) => {
  await page.evaluate(
    ({ w, h, draw }) => {
      const c = document.getElementById("c");
      c.width = w;
      c.height = h;
      new Function("ctx", "w", "h", draw)(c.getContext("2d"), w, h);
    },
    { w, h, draw: fn.toString().replace(/^[^{]*{/, "").replace(/}\s*$/, "") },
  );
  await page.screenshot({ path: file, clip: { x: 0, y: 0, width: w, height: h } });
};
await shot(sidebar, 164, 314, path.resolve("../moco-qa/shots/installer-sidebar.png"));
await shot(header, 150, 57, path.resolve("../moco-qa/shots/installer-header.png"));
await page.evaluate(() => document.getElementById("c")?.remove());
await browser.close().catch(() => {});
