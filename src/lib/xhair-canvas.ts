// Canvas helpers for the pixel-exact crosshair bitmaps the backend renders.
// Every canvas here is device-pixel sized, so zoom 1 = real screen pixels.

import type { CrosshairImage } from "./api";

export type Bitmap = { canvas: HTMLCanvasElement; half: number; size: number };

/** Upload the RGBA bitmap into an offscreen canvas (drawn many times). */
export function toBitmap(img: CrosshairImage): Bitmap {
  const c = document.createElement("canvas");
  c.width = img.image.width;
  c.height = img.image.height;
  c.getContext("2d")!.putImageData(img.image, 0, 0);
  return { canvas: c, half: img.half, size: img.image.width };
}

export type StageOpts = {
  cssW: number;
  cssH: number;
  zoom: number;
  grid?: boolean; // 1-screen-px grid (only meaningful at zoom ≥ 4)
  ticks?: boolean; // 2×12 centre ticks on each edge
  dx?: number; // extra offset in screen px (position nudge preview)
  dy?: number;
};

/**
 * Paint `bmp` onto `cv` so the bitmap's `half` boundary lands on the canvas
 * centre — the same contract the overlay uses on screen.
 */
export function paintStage(cv: HTMLCanvasElement, bmp: Bitmap | null, o: StageOpts) {
  const dpr = window.devicePixelRatio || 1;
  const W = Math.max(1, Math.round(o.cssW * dpr));
  const H = Math.max(1, Math.round(o.cssH * dpr));
  if (cv.width !== W) cv.width = W;
  if (cv.height !== H) cv.height = H;
  const ctx = cv.getContext("2d")!;
  ctx.clearRect(0, 0, W, H);
  const z = o.zoom;
  const cx = Math.floor(W / 2);
  const cy = Math.floor(H / 2);

  if (o.grid && z >= 4) {
    ctx.fillStyle = "rgba(255,255,255,0.06)";
    for (let x = cx % z; x < W; x += z) ctx.fillRect(x, 0, 1, H);
    for (let y = cy % z; y < H; y += z) ctx.fillRect(0, y, W, 1);
  }
  if (o.ticks) {
    ctx.fillStyle = "rgba(255,255,255,0.5)";
    const t = Math.round(2 * dpr);
    const l = Math.round(12 * dpr);
    ctx.fillRect(cx - t / 2, 0, t, l);
    ctx.fillRect(cx - t / 2, H - l, t, l);
    ctx.fillRect(0, cy - t / 2, l, t);
    ctx.fillRect(W - l, cy - t / 2, l, t);
  }
  if (!bmp) return;
  ctx.imageSmoothingEnabled = false;
  const x = cx - bmp.half * z + (o.dx ?? 0) * z;
  const y = cy - bmp.half * z + (o.dy ?? 0) * z;
  ctx.drawImage(bmp.canvas, x, y, bmp.size * z, bmp.size * z);
}

/** Fit-to-box thumbnail (integer upscale, smooth downscale) as a data URL. */
export function thumbUrl(bmp: Bitmap, box: number): string {
  const dpr = window.devicePixelRatio || 1;
  const px = Math.round(box * dpr);
  const c = document.createElement("canvas");
  c.width = px;
  c.height = px;
  const ctx = c.getContext("2d")!;
  const scale = bmp.size <= px ? Math.max(1, Math.floor(px / bmp.size)) : px / bmp.size;
  ctx.imageSmoothingEnabled = scale < 1;
  const s = bmp.size * scale;
  ctx.drawImage(bmp.canvas, (px - s) / 2, (px - s) / 2, s, s);
  return c.toDataURL();
}
