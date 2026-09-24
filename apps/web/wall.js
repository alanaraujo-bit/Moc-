// Relief azulejo wall (same motifs as the app). Draws once; turns tiles slowly on hover
// of nothing — it's a wall, it stays still unless motion is welcome.
(function () {
  function rng(seed) {
    let s = seed >>> 0;
    return function () {
      s = (s + 0x6d2b79f5) >>> 0;
      let t = s;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  const motifs = [
    (c, s) => { c.beginPath(); c.moveTo(0, 0); c.arc(0, 0, s * 0.62, 0, Math.PI / 2); c.closePath(); c.fill(); c.stroke(); },
    (c, s) => { c.beginPath(); c.arc(s / 2, 0, s * 0.3, 0, Math.PI); c.closePath(); c.fill(); c.stroke(); },
    (c, s) => { const w = s * 0.34, x = (s - w) / 2; c.beginPath(); c.moveTo(x, s); c.lineTo(x, s * 0.55); c.arc(s / 2, s * 0.55, w / 2, Math.PI, 0); c.lineTo(x + w, s); c.closePath(); c.fill(); c.stroke(); },
    (c, s) => { c.fillRect(0, s * 0.18, s, s * 0.12); c.fillRect(0, s * 0.42, s, s * 0.12); },
    (c, s) => { c.beginPath(); c.arc(0, 0, s * 0.78, 0, Math.PI / 2); c.arc(0, 0, s * 0.56, Math.PI / 2, 0, true); c.closePath(); c.fill(); c.stroke(); },
    () => {},
  ];

  function draw(canvas) {
    const tile = Number(canvas.dataset.tile || 64);
    const glazed = Number(canvas.dataset.glazed || 0);
    const onCobalt = canvas.dataset.variant === "cobalt";
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth, h = canvas.clientHeight;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    const ctx = canvas.getContext("2d");
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const css = getComputedStyle(document.documentElement);
    const ground = onCobalt ? css.getPropertyValue("--cobalt").trim() : css.getPropertyValue("--ground").trim();
    const relief = onCobalt ? "rgba(255,255,255,0.10)" : css.getPropertyValue("--relief").trim();
    const edge = onCobalt ? "rgba(255,255,255,0.14)" : css.getPropertyValue("--grout-strong").trim();
    const grout = onCobalt ? "rgba(255,255,255,0.12)" : css.getPropertyValue("--grout").trim();
    const cobalt = css.getPropertyValue("--cobalt").trim();
    ctx.fillStyle = ground;
    ctx.fillRect(0, 0, w, h);
    const cols = Math.ceil(w / tile) + 1, rows = Math.ceil(h / tile) + 1;
    const rand = rng(Number(canvas.dataset.seed || 7));
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const m = Math.floor(rand() * motifs.length), rot = Math.floor(rand() * 4), g = rand() < glazed;
        ctx.save();
        ctx.translate(c * tile + tile / 2, r * tile + tile / 2);
        ctx.rotate((rot * Math.PI) / 2);
        ctx.translate(-tile / 2, -tile / 2);
        ctx.beginPath();
        ctx.rect(0, 0, tile, tile);
        ctx.clip();
        ctx.fillStyle = g ? cobalt : relief;
        ctx.strokeStyle = g ? cobalt : edge;
        ctx.lineWidth = 1;
        motifs[m](ctx, tile);
        ctx.restore();
      }
    }
    ctx.strokeStyle = grout;
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let c = 0; c <= cols; c++) { ctx.moveTo(c * tile + 0.5, 0); ctx.lineTo(c * tile + 0.5, h); }
    for (let r = 0; r <= rows; r++) { ctx.moveTo(0, r * tile + 0.5); ctx.lineTo(w, r * tile + 0.5); }
    ctx.stroke();
  }

  const walls = Array.from(document.querySelectorAll("canvas[data-wall]"));
  const redraw = () => walls.forEach(draw);
  redraw();
  window.addEventListener("resize", redraw);
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", redraw);
})();
