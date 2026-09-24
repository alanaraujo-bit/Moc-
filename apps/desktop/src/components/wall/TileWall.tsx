// A relief azulejo wall, in the manner of Athos Bulcão's panels: a handful of motifs,
// each tile rotated by the setter's hand, some left plain.
//
// - `opening` leaves a plain field (in tiles) around the center for content.
// - `progress` (0–1) glazes that fraction of tiles in cobalt, in a fixed random order,
//   animating newly set tiles — onboarding builds your panel as you go.
// - `turning` plays the unlock moment: every tile turns a quarter, center outward.

import { useEffect, useRef } from "react";
import styles from "./TileWall.module.css";

type Motif = (ctx: CanvasRenderingContext2D, s: number) => void;

const MOTIFS: Motif[] = [
  (c, s) => {
    c.beginPath();
    c.moveTo(0, 0);
    c.arc(0, 0, s * 0.62, 0, Math.PI / 2);
    c.closePath();
    c.fill();
    c.stroke();
  },
  (c, s) => {
    c.beginPath();
    c.arc(s / 2, 0, s * 0.3, 0, Math.PI);
    c.closePath();
    c.fill();
    c.stroke();
  },
  (c, s) => {
    const w = s * 0.34;
    const x = (s - w) / 2;
    c.beginPath();
    c.moveTo(x, s);
    c.lineTo(x, s * 0.55);
    c.arc(s / 2, s * 0.55, w / 2, Math.PI, 0);
    c.lineTo(x + w, s);
    c.closePath();
    c.fill();
    c.stroke();
  },
  (c, s) => {
    c.fillRect(0, s * 0.18, s, s * 0.12);
    c.fillRect(0, s * 0.42, s, s * 0.12);
    c.strokeRect(0.5, s * 0.18 + 0.5, s - 1, s * 0.12 - 1);
    c.strokeRect(0.5, s * 0.42 + 0.5, s - 1, s * 0.12 - 1);
  },
  (c, s) => {
    c.beginPath();
    c.arc(0, 0, s * 0.78, 0, Math.PI / 2);
    c.arc(0, 0, s * 0.56, Math.PI / 2, 0, true);
    c.closePath();
    c.fill();
    c.stroke();
  },
  (c, s) => {
    c.beginPath();
    c.arc(s, s, s * 0.22, Math.PI, Math.PI * 1.5);
    c.lineTo(s, s);
    c.closePath();
    c.fill();
    c.stroke();
    c.beginPath();
    c.arc(0, 0, s * 0.22, 0, Math.PI / 2);
    c.lineTo(0, 0);
    c.closePath();
    c.fill();
    c.stroke();
  },
  () => {},
];

function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface Props {
  tile?: number;
  opening?: { cols: number; rows: number };
  progress?: number;
  turning?: boolean;
  onTurned?: () => void;
  seed?: number;
  className?: string;
}

const easeOut = (t: number) => 1 - Math.pow(1 - t, 3);

export function TileWall({ tile = 56, opening, progress = 0, turning = false, onTurned, seed = 7, className }: Props) {
  const ref = useRef<HTMLCanvasElement>(null);
  const shownProgress = useRef(progress);
  const target = useRef(progress);
  const setAt = useRef<Map<number, number>>(new Map()); // tile index → time it got glazed
  const turnStart = useRef(0);
  const turnedCb = useRef(onTurned);
  turnedCb.current = onTurned;

  target.current = progress;

  useEffect(() => {
    const canvas = ref.current!;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let raf = 0;
    let running = false;

    const draw = (now: number) => {
      const dpr = window.devicePixelRatio || 1;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (!w || !h) return false;
      if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
        canvas.width = Math.round(w * dpr);
        canvas.height = Math.round(h * dpr);
      }
      const ctx = canvas.getContext("2d")!;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const css = getComputedStyle(canvas);
      const ground = css.getPropertyValue("--wall-ground").trim();
      const relief = css.getPropertyValue("--wall-relief").trim();
      const grout = css.getPropertyValue("--grout").trim();
      const glaze = css.getPropertyValue("--glaze-cobalt").trim();
      const glazeGround = css.getPropertyValue("--tile-field").trim();
      const edge = css.getPropertyValue("--wall-edge").trim();
      ctx.fillStyle = ground;
      ctx.fillRect(0, 0, w, h);

      const cols = Math.ceil(w / tile) + 1;
      const rows = Math.ceil(h / tile) + 1;
      const ox = Math.round((w - cols * tile) / 2);
      const oy = Math.round((h - rows * tile) / 2);
      const cx = cols / 2;
      const cy = rows / 2;
      const rand = rng(seed);
      const maxDist = Math.hypot(cx, cy);
      const total = cols * rows;

      // Advance displayed progress toward the target, glazing tiles one by one.
      const glazedCount = Math.round(shownProgress.current * total);
      const targetCount = Math.round(target.current * total);
      let animating = false;
      const tur = turning ? Math.min(1, (now - turnStart.current) / 620) : 0;
      if (turning && tur < 1) animating = true;

      let idx = 0;
      for (let r = 0; r < rows; r++) {
        for (let c = 0; c < cols; c++, idx++) {
          const motif = Math.floor(rand() * MOTIFS.length);
          const rot = Math.floor(rand() * 4);
          const rank = rand(); // order in which this tile gets glazed
          if (
            opening &&
            c >= Math.round(cx - opening.cols / 2) &&
            c < Math.round(cx - opening.cols / 2) + opening.cols &&
            r >= Math.round(cy - opening.rows / 2) &&
            r < Math.round(cy - opening.rows / 2) + opening.rows
          )
            continue;
          const x = ox + c * tile;
          const y = oy + r * tile;
          const isGlazed = rank < targetCount / total;
          let g = 0;
          if (isGlazed) {
            let t0 = setAt.current.get(idx);
            if (t0 === undefined) {
              // Stagger by rank so tiles land one after another.
              t0 = reduced ? now - 1000 : now + (rank * total - glazedCount) * 18;
              setAt.current.set(idx, t0);
            }
            g = reduced ? 1 : easeOut(Math.min(1, Math.max(0, (now - t0) / 380)));
            if (g < 1) animating = true;
          } else {
            setAt.current.delete(idx);
          }
          const dist = Math.hypot(c + 0.5 - cx, r + 0.5 - cy) / maxDist;
          const local = turning ? easeOut(Math.min(1, Math.max(0, (tur - dist * 0.55) / 0.45))) : 0;

          ctx.save();
          ctx.translate(x + tile / 2, y + tile / 2);
          if (g > 0) {
            // A tile being set: it arrives with a quarter turn and a slight lift.
            ctx.globalAlpha = g;
            ctx.fillStyle = glazeGround;
            ctx.fillRect(-tile / 2 + 1, -tile / 2 + 1, tile - 1, tile - 1);
          }
          ctx.rotate((rot + local + (1 - g) * (g > 0 ? 0.5 : 0)) * (Math.PI / 2));
          ctx.globalAlpha = (1 - local * 0.85) * (g > 0 ? g : 1);
          ctx.translate(-tile / 2, -tile / 2);
          ctx.fillStyle = g > 0 ? glaze : relief;
          ctx.strokeStyle = g > 0 ? glaze : edge;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.rect(0, 0, tile, tile);
          ctx.clip();
          MOTIFS[motif](ctx, tile);
          ctx.restore();
        }
      }
      shownProgress.current = target.current;

      ctx.strokeStyle = grout;
      ctx.globalAlpha = 0.6;
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let c = 0; c <= cols; c++) {
        const x = ox + c * tile + 0.5;
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
      }
      for (let r = 0; r <= rows; r++) {
        const y = oy + r * tile + 0.5;
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
      }
      ctx.stroke();
      ctx.globalAlpha = 1;
      if (opening) {
        // The opening is cut on grout lines and framed, so it reads as set into the wall.
        const left = ox + Math.round(cx - opening.cols / 2) * tile;
        const top = oy + Math.round(cy - opening.rows / 2) * tile;
        const ow = opening.cols * tile;
        const oh = opening.rows * tile;
        ctx.fillStyle = ground;
        ctx.fillRect(left, top, ow, oh);
        ctx.strokeStyle = edge;
        ctx.lineWidth = 1;
        ctx.strokeRect(left + 0.5, top + 0.5, ow - 1, oh - 1);
      }
      if (turning && tur >= 1) {
        turnedCb.current?.();
      }
      return animating;
    };

    const loop = (t: number) => {
      const more = draw(t);
      if (more) raf = requestAnimationFrame(loop);
      else running = false;
    };
    const kick = () => {
      if (!running) {
        running = true;
        raf = requestAnimationFrame(loop);
      }
    };
    if (turning) {
      turnStart.current = performance.now();
      if (reduced) {
        draw(performance.now() + 10_000);
        turnedCb.current?.();
      }
    }
    kick();
    const ro = new ResizeObserver(() => {
      running = false;
      cancelAnimationFrame(raf);
      kick();
    });
    ro.observe(canvas);
    const mo = new MutationObserver(kick);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      mo.disconnect();
    };
  }, [tile, opening?.cols, opening?.rows, turning, seed, progress]);

  return <canvas ref={ref} className={`${styles.wall} ${className ?? ""}`} aria-hidden="true" />;
}
