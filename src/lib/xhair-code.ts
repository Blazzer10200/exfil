// Share codes. EXFIL-1 is the native format; Valorant and CS2 codes import
// (lossy where their models don't map).
//
// EXFIL-1:
//   X1;<hex>;A<opacity%>;S<shape>;I<len>.<thick>.<gap>|I0;
//   O<len>.<thick>.<gap>.<opacity%>|O0;D<size>|D0;R<radius>.<thick>|R0;
//   L<thick>.<hex>|L0;G<radius>|G0;P<dx>.<dy>;Z<0|1>
//   e.g. X1;00FF66;A100;S0;I8.2.4;O6.1.17.70;D0;R0;L1.000000;G3;P0.0;Z0

import { DEFAULT_STYLE, type CrosshairStyle } from "./api";

const hex6 = (c: string) => c.replace("#", "").toUpperCase().padStart(6, "0").slice(0, 6);
const pct = (v: number) => Math.round(v * 100);

export function encode(s: CrosshairStyle): string {
  const parts = [
    "X1",
    hex6(s.color),
    `A${pct(s.opacity)}`,
    `S${s.shape}`,
    s.arms ? `I${s.length}.${s.thickness}.${s.gap}` : "I0",
    s.outer ? `O${s.outer_length}.${s.outer_thickness}.${s.outer_gap}.${pct(s.outer_opacity)}` : "O0",
    s.dot ? `D${s.dot_size}` : "D0",
    s.ring ? `R${s.ring_radius}.${s.ring_thickness}` : "R0",
    s.outline ? `L${s.outline_thickness}.${hex6(s.outline_color)}` : "L0",
    s.glow ? `G${s.glow_radius}` : "G0",
    `P${s.offset_x}.${s.offset_y}`,
    `Z${s.scale_with_resolution ? 1 : 0}`,
  ];
  return parts.join(";");
}

const num = (v: string | undefined, fallback: number) => {
  const n = Number(v);
  return Number.isFinite(n) ? n : fallback;
};
const isHex = (v: string) => /^[0-9a-fA-F]{6}$/.test(v);

function decodeExfil(code: string): CrosshairStyle | null {
  const seg = code.split(";").map((p) => p.trim());
  if (seg[0] !== "X1" || seg.length < 4) return null;
  const s: CrosshairStyle = { ...DEFAULT_STYLE };
  if (isHex(seg[1])) s.color = `#${seg[1].toLowerCase()}`;
  else return null;
  for (const p of seg.slice(2)) {
    const tag = p[0];
    const body = p.slice(1);
    const f = body.split(".");
    switch (tag) {
      case "A":
        s.opacity = Math.min(1, Math.max(0.1, num(body, 100) / 100));
        break;
      case "S":
        s.shape = Math.min(4, Math.max(0, Math.round(num(body, 0))));
        break;
      case "I":
        if (body === "0") s.arms = false;
        else {
          s.arms = true;
          s.length = num(f[0], s.length);
          s.thickness = num(f[1], s.thickness);
          s.gap = num(f[2], s.gap);
        }
        break;
      case "O":
        if (body === "0") s.outer = false;
        else {
          s.outer = true;
          s.outer_length = num(f[0], s.outer_length);
          s.outer_thickness = num(f[1], s.outer_thickness);
          s.outer_gap = num(f[2], s.outer_gap);
          s.outer_opacity = Math.min(1, Math.max(0.1, num(f[3], 70) / 100));
        }
        break;
      case "D":
        if (body === "0") s.dot = false;
        else {
          s.dot = true;
          s.dot_size = num(body, s.dot_size);
        }
        break;
      case "R":
        if (body === "0") s.ring = false;
        else {
          s.ring = true;
          s.ring_radius = num(f[0], s.ring_radius);
          s.ring_thickness = num(f[1], s.ring_thickness);
        }
        break;
      case "L":
        if (body === "0") s.outline = false;
        else {
          s.outline = true;
          s.outline_thickness = num(f[0], s.outline_thickness);
          if (f[1] && isHex(f[1])) s.outline_color = `#${f[1].toLowerCase()}`;
        }
        break;
      case "G":
        if (body === "0") s.glow = false;
        else {
          s.glow = true;
          s.glow_radius = num(body, s.glow_radius);
        }
        break;
      case "P":
        s.offset_x = num(f[0], 0);
        s.offset_y = num(f[1], 0);
        break;
      case "Z":
        s.scale_with_resolution = body === "1";
        break;
    }
  }
  return s;
}

// Valorant colour indices (0–7) + custom "b" hex.
const VAL_COLORS = ["#ffffff", "#00ff00", "#7fff00", "#dfff00", "#ffff00", "#00ffff", "#ff00ff", "#ff0000"];

/** Valorant profile code: `0;P;c;1;o;1;0l;4;0o;2;0a;1;0f;0;1b;0` … */
function decodeValorant(code: string): CrosshairStyle | null {
  const t = code.split(";");
  if (t[0] !== "0" || !t.includes("P")) return null;
  // Primary section starts after "P"; ends at "A" (ADS) or "S" (sniper).
  const start = t.indexOf("P") + 1;
  let end = t.length;
  for (const stop of ["A", "S"]) {
    const i = t.indexOf(stop, start);
    if (i > 0) end = Math.min(end, i);
  }
  const kv = new Map<string, string>();
  for (let i = start; i + 1 < end; i += 2) kv.set(t[i], t[i + 1]);
  const s: CrosshairStyle = { ...DEFAULT_STYLE };
  const colorIdx = num(kv.get("c"), 0);
  s.color = kv.has("u") && isHex(kv.get("u")!) ? `#${kv.get("u")!.toLowerCase()}` : VAL_COLORS[colorIdx] ?? "#ffffff";
  s.outline = kv.get("h") !== "0";
  s.outline_thickness = Math.min(3, Math.max(1, num(kv.get("t"), 1)));
  s.outline_color = "#000000";
  s.dot = kv.get("d") === "1";
  s.dot_size = Math.max(1, num(kv.get("z"), 2));
  // Inner lines
  s.arms = kv.get("0b") !== "0";
  s.length = Math.max(1, num(kv.get("0l"), 6));
  s.thickness = Math.max(1, num(kv.get("0t"), 2));
  s.gap = Math.max(0, num(kv.get("0o"), 3));
  // Outer lines (on by default in Valorant unless 1b;0)
  s.outer = kv.get("1b") !== "0";
  s.outer_length = Math.max(1, num(kv.get("1l"), 2));
  s.outer_thickness = Math.max(1, num(kv.get("1t"), 2));
  s.outer_gap = Math.max(0, num(kv.get("1o"), 10));
  s.outer_opacity = Math.min(1, Math.max(0.1, num(kv.get("1a"), 0.35)));
  s.opacity = Math.min(1, Math.max(0.1, num(kv.get("0a"), 1)));
  s.shape = 0;
  return s;
}

// CS2 share codes: base-58-ish "CSGO-xxxxx-xxxxx-xxxxx-xxxxx-xxxxx" packing
// 18 bytes (gap, outline, r, g, b, alpha, flags…). Decoded per the
// community-documented layout.
const CS_ALPHABET = "ABCDEFGHJKLMNOPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";

function decodeCs2(code: string): CrosshairStyle | null {
  const m = code.trim().match(/^CSGO(-[A-Za-z2-9]{5}){5}$/);
  if (!m) return null;
  const chars = code.slice(5).replace(/-/g, "").split("").reverse();
  let big = 0n;
  for (const ch of chars) {
    const v = CS_ALPHABET.indexOf(ch);
    if (v < 0) return null;
    big = big * 58n + BigInt(v);
  }
  const bytes: number[] = [];
  for (let i = 0; i < 18; i++) {
    bytes.push(Number(big & 0xffn));
    big >>= 8n;
  }
  const i8 = (b: number) => (b > 127 ? b - 256 : b);
  const gap = i8(bytes[2]) / 10;
  const outline = bytes[3] / 2;
  const r = bytes[4];
  const g = bytes[5];
  const b = bytes[6];
  const alpha = bytes[7];
  const split = bytes[8] / 10; // unused by EXFIL (split distance)
  void split;
  const thickness = (bytes[12] & 0x3f) / 10;
  const length = bytes[14] / 10;
  const flags = bytes[13];
  const hasOutline = (flags & 8) !== 0;
  const hasDot = (flags & 16) !== 0;
  const tStyle = (flags & 64) !== 0;
  const s: CrosshairStyle = { ...DEFAULT_STYLE };
  const hex = (n: number) => n.toString(16).padStart(2, "0");
  s.color = `#${hex(r)}${hex(g)}${hex(b)}`;
  s.opacity = Math.min(1, Math.max(0.1, alpha / 255));
  s.arms = true;
  s.shape = tStyle ? 2 : 0;
  s.length = Math.max(1, Math.round(length * 2));
  s.thickness = Math.max(1, Math.round(thickness * 2));
  s.gap = Math.max(0, Math.round(gap * 2 + 4));
  s.outline = hasOutline;
  s.outline_thickness = Math.min(3, Math.max(1, Math.round(outline)));
  s.dot = hasDot;
  s.dot_size = Math.max(1, Math.round(thickness * 2));
  return s;
}

export type Decoded = { style: CrosshairStyle; source: "exfil" | "valorant" | "cs2" };

/** Parse any supported code; null when it's none of them. */
export function decode(raw: string): Decoded | null {
  const code = raw.trim();
  if (!code) return null;
  const exfil = decodeExfil(code);
  if (exfil) return { style: exfil, source: "exfil" };
  const cs = decodeCs2(code);
  if (cs) return { style: cs, source: "cs2" };
  const val = decodeValorant(code);
  if (val) return { style: val, source: "valorant" };
  return null;
}
