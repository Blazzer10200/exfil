//! Crosshair model, persistence and pixel renderer.
//!
//! Crosshairs live in their own store (%APPDATA%\exfil-v2\crosshairs.json),
//! separate from color presets. The renderer is pure (style → RGBA bitmap) so
//! the in-app preview and the on-screen overlay draw the exact same pixels.
//!
//! Centering model: the overlay anchors the bitmap's center BOUNDARY (`half`)
//! on the target's center boundary `left + floor(width / 2)`. Every axis-aligned
//! element is an integer rect around that point, so lines are always crisp:
//! even thicknesses land exactly on the true center, odd ones sit half a pixel
//! right/down (the smallest possible error — a 1px line can't straddle two
//! pixels without blurring). Diagonal shapes (X, chevron) are supersampled.
//! The per-crosshair X/Y nudge covers games that round their own center the
//! other way.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Inner-line shapes. Stored as a plain u8 so share codes stay compact.
pub const SHAPE_CROSS: u8 = 0;
pub const SHAPE_X: u8 = 1;
pub const SHAPE_T: u8 = 2;
pub const SHAPE_CHEVRON: u8 = 3;
pub const SHAPE_BRACKETS: u8 = 4;

fn default_monitor() -> String {
    "primary".into()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct CrosshairStyle {
    /// Fill color, "#rrggbb".
    pub color: String,
    /// Whole-crosshair opacity, 0.1..=1.0.
    pub opacity: f32,
    /// Inner lines on/off.
    pub arms: bool,
    /// Inner-line shape (SHAPE_*).
    pub shape: u8,
    pub length: u32,
    pub thickness: u32,
    pub gap: u32,
    /// Legacy (v2.4) T-style flag — migrated into `shape` by `sanitized()`.
    #[serde(skip_serializing)]
    pub t_style: bool,
    /// Outer lines: a second, dimmer cross further out.
    pub outer: bool,
    pub outer_length: u32,
    pub outer_thickness: u32,
    /// Distance from the center to the outer lines' inner end.
    pub outer_gap: u32,
    pub outer_opacity: f32,
    pub dot: bool,
    pub dot_size: u32,
    pub ring: bool,
    pub ring_radius: u32,
    pub ring_thickness: u32,
    pub outline: bool,
    pub outline_thickness: u32,
    /// Outline color, "#rrggbb".
    pub outline_color: String,
    /// Gaussian halo in the crosshair colour, under the outline.
    pub glow: bool,
    pub glow_radius: u32,
    /// Pixel nudge applied on screen (not in the bitmap).
    pub offset_x: i32,
    pub offset_y: i32,
    /// Target monitor ("primary" — reserved for a future per-monitor pick).
    pub monitor: String,
    /// Pixel sizes are authored at 1440p and scaled by monitor height / 1440.
    pub scale_with_resolution: bool,
}

impl Default for CrosshairStyle {
    fn default() -> Self {
        CrosshairStyle {
            color: "#00ff66".into(),
            opacity: 1.0,
            arms: true,
            shape: SHAPE_CROSS,
            length: 6,
            thickness: 2,
            gap: 3,
            t_style: false,
            outer: false,
            outer_length: 3,
            outer_thickness: 1,
            outer_gap: 13,
            outer_opacity: 0.7,
            dot: false,
            dot_size: 2,
            ring: false,
            ring_radius: 10,
            ring_thickness: 1,
            outline: true,
            outline_thickness: 1,
            outline_color: "#000000".into(),
            glow: false,
            glow_radius: 3,
            offset_x: 0,
            offset_y: 0,
            monitor: default_monitor(),
            scale_with_resolution: false,
        }
    }
}

fn clamp01(v: f32, lo: f32, fallback: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, 1.0) } else { fallback }
}

impl CrosshairStyle {
    /// Clamp every field into its UI range. Guards the renderer (and its
    /// allocation size) against hand-edited or corrupt JSON, and folds the
    /// legacy `t_style` flag into `shape`.
    pub fn sanitized(&self) -> CrosshairStyle {
        let shape = if self.t_style && self.shape == SHAPE_CROSS { SHAPE_T } else { self.shape.min(SHAPE_BRACKETS) };
        CrosshairStyle {
            color: self.color.clone(),
            opacity: clamp01(self.opacity, 0.1, 1.0),
            arms: self.arms,
            shape,
            length: self.length.clamp(1, 40),
            thickness: self.thickness.clamp(1, 10),
            gap: self.gap.min(30),
            t_style: false,
            outer: self.outer,
            outer_length: self.outer_length.clamp(1, 40),
            outer_thickness: self.outer_thickness.clamp(1, 10),
            outer_gap: self.outer_gap.min(60),
            outer_opacity: clamp01(self.outer_opacity, 0.1, 0.7),
            dot: self.dot,
            dot_size: self.dot_size.clamp(1, 10),
            ring: self.ring,
            ring_radius: self.ring_radius.clamp(2, 60),
            ring_thickness: self.ring_thickness.clamp(1, 6),
            outline: self.outline,
            outline_thickness: self.outline_thickness.clamp(1, 3),
            outline_color: self.outline_color.clone(),
            glow: self.glow,
            glow_radius: self.glow_radius.min(8),
            offset_x: self.offset_x.clamp(-50, 50),
            offset_y: self.offset_y.clamp(-50, 50),
            monitor: if self.monitor.trim().is_empty() { default_monitor() } else { self.monitor.clone() },
            scale_with_resolution: self.scale_with_resolution,
        }
    }

    /// Every pixel dimension multiplied by `k` (rounded, ≥1), for
    /// scale-with-resolution. Opacity/colour/flags untouched.
    pub fn scaled(&self, k: f32) -> CrosshairStyle {
        if !(k.is_finite() && k > 0.0) || (k - 1.0).abs() < 1e-3 {
            return self.clone();
        }
        let s = |v: u32| ((v as f32 * k).round() as u32).max(1);
        let g = |v: u32| (v as f32 * k).round() as u32;
        let n = |v: i32| (v as f32 * k).round() as i32;
        CrosshairStyle {
            length: s(self.length),
            thickness: s(self.thickness),
            gap: g(self.gap),
            outer_length: s(self.outer_length),
            outer_thickness: s(self.outer_thickness),
            outer_gap: g(self.outer_gap),
            dot_size: s(self.dot_size),
            ring_radius: s(self.ring_radius),
            ring_thickness: s(self.ring_thickness),
            outline_thickness: s(self.outline_thickness).min(3),
            glow_radius: g(self.glow_radius).min(8),
            offset_x: n(self.offset_x),
            offset_y: n(self.offset_y),
            ..self.clone()
        }
    }
}

/// Parse "#rrggbb" (leading # optional); None on anything else.
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// A rendered crosshair: square, straight (non-premultiplied) RGBA, row-major.
/// `half` is the center boundary in bitmap pixels — the overlay places it on
/// the target's center boundary.
pub struct Bitmap {
    pub size: u32,
    pub half: u32,
    pub rgba: Vec<u8>,
}

/// Integer span of `len` pixels centered on `center` (a boundary or a pixel
/// middle). A parity mismatch snaps half a pixel right/down, never blurs.
fn span(center: f32, len: u32) -> (i32, i32) {
    let start = (center - len as f32 / 2.0 + 0.5).floor() as i32;
    (start, start + len as i32)
}

/// Canvas of per-pixel coverage (0..=1) for the fill and the outline. `fill`
/// already carries per-element opacity (outer lines are dimmer).
struct Layers {
    size: i32,
    fill: Vec<f32>,
    outline: Vec<f32>,
}

const SS: i32 = 4;

impl Layers {
    fn new(size: u32) -> Self {
        let n = (size * size) as usize;
        Layers { size: size as i32, fill: vec![0.0; n], outline: vec![0.0; n] }
    }

    fn rect(buf: &mut [f32], size: i32, x0: i32, y0: i32, x1: i32, y1: i32, a: f32) {
        for y in y0.max(0)..y1.min(size) {
            for x in x0.max(0)..x1.min(size) {
                if let Some(c) = buf.get_mut((y * size + x) as usize) {
                    *c = c.max(a);
                }
            }
        }
    }

    /// Filled rect plus its outline (the rect grown by `o` on every side).
    fn solid(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, o: i32) {
        Self::rect(&mut self.fill, self.size, x0, y0, x1, y1, 1.0);
        if o > 0 {
            Self::rect(&mut self.outline, self.size, x0 - o, y0 - o, x1 + o, y1 + o, 1.0);
        }
    }

    /// Box with fractional edges: each pixel gets its covered area. Integer
    /// edges reproduce `rect` exactly; half-pixel edges split the coverage
    /// evenly on both sides, so an element whose width parity doesn't match
    /// the primary stroke stays centered on it instead of snapping right/down.
    fn rect_f(buf: &mut [f32], size: i32, x0: f32, y0: f32, x1: f32, y1: f32, a: f32) {
        let (px0, py0) = (x0.floor() as i32, y0.floor() as i32);
        let (px1, py1) = (x1.ceil() as i32, y1.ceil() as i32);
        for py in py0.max(0)..py1.min(size) {
            let cy = (y1.min(py as f32 + 1.0) - y0.max(py as f32)).max(0.0);
            for px in px0.max(0)..px1.min(size) {
                let cx = (x1.min(px as f32 + 1.0) - x0.max(px as f32)).max(0.0);
                if let Some(c) = buf.get_mut((py * size + px) as usize) {
                    *c = c.max(a * cx * cy);
                }
            }
        }
    }

    /// `solid` for a box centered on an arbitrary point (secondary elements:
    /// dot, outer lines), with per-element opacity `a`.
    fn solid_f(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, o: f32, a: f32) {
        Self::rect_f(&mut self.fill, self.size, x0, y0, x1, y1, a);
        if o > 0.0 {
            Self::rect_f(&mut self.outline, self.size, x0 - o, y0 - o, x1 + o, y1 + o, 1.0);
        }
    }

    /// Supersampled coverage of `inside(x, y) -> (fill, outline)` over a box.
    fn sample<F: Fn(f32, f32) -> (bool, bool)>(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, a: f32, inside: F) {
        let total = (SS * SS) as f32;
        for py in y0.max(0)..y1.min(self.size) {
            for px in x0.max(0)..x1.min(self.size) {
                let (mut f, mut ol) = (0, 0);
                for sy in 0..SS {
                    for sx in 0..SS {
                        let x = px as f32 + (sx as f32 + 0.5) / SS as f32;
                        let y = py as f32 + (sy as f32 + 0.5) / SS as f32;
                        let (fi, oi) = inside(x, y);
                        f += fi as i32;
                        ol += oi as i32;
                    }
                }
                let idx = (py * self.size + px) as usize;
                if let Some(c) = self.fill.get_mut(idx) {
                    *c = c.max(a * f as f32 / total);
                }
                if let Some(c) = self.outline.get_mut(idx) {
                    *c = c.max(ol as f32 / total);
                }
            }
        }
    }

    /// Anti-aliased ring centered at (cx, cy): stroke centerline at radius
    /// `r`, `w` wide, outline `o` on both edges.
    fn ring(&mut self, cx: f32, cy: f32, r: f32, w: f32, o: f32) {
        let reach = r + w / 2.0 + o + 1.0;
        let (x0, y0) = ((cx - reach).floor() as i32, (cy - reach).floor() as i32);
        let (x1, y1) = ((cx + reach).ceil() as i32, (cy + reach).ceil() as i32);
        self.sample(x0, y0, x1, y1, 1.0, |x, y| {
            let off = (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r).abs();
            (off <= w / 2.0, o > 0.0 && off <= w / 2.0 + o)
        });
    }

    /// Anti-aliased line segment (ax, ay)→(bx, by), `w` wide with square-ish
    /// caps, outline `o`.
    fn seg(&mut self, ax: f32, ay: f32, bx: f32, by: f32, w: f32, o: f32) {
        let reach = w / 2.0 + o + 1.0;
        let x0 = (ax.min(bx) - reach).floor() as i32;
        let y0 = (ay.min(by) - reach).floor() as i32;
        let x1 = (ax.max(bx) + reach).ceil() as i32;
        let y1 = (ay.max(by) + reach).ceil() as i32;
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        self.sample(x0, y0, x1, y1, 1.0, |x, y| {
            let t = (((x - ax) * dx + (y - ay) * dy) / len2).clamp(0.0, 1.0);
            let (px, py) = (ax + t * dx, ay + t * dy);
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            (d <= w / 2.0, o > 0.0 && d <= w / 2.0 + o)
        });
    }
}

/// Separable Gaussian blur of `src` (size×size), kernel half-width `r`.
fn blur(src: &[f32], size: usize, r: usize) -> Vec<f32> {
    if r == 0 {
        return src.to_vec();
    }
    let sigma = r as f32 / 2.0;
    let kernel: Vec<f32> = (0..=2 * r)
        .map(|i| {
            let d = i as f32 - r as f32;
            (-d * d / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let norm: f32 = kernel.iter().sum();
    let pass = |input: &[f32], horizontal: bool| -> Vec<f32> {
        let mut out = vec![0.0; size * size];
        for y in 0..size {
            for x in 0..size {
                let mut acc = 0.0;
                for (k, kw) in kernel.iter().enumerate() {
                    let off = k as isize - r as isize;
                    let (sx, sy) = if horizontal { (x as isize + off, y as isize) } else { (x as isize, y as isize + off) };
                    if sx >= 0 && sy >= 0 && (sx as usize) < size && (sy as usize) < size {
                        acc += kw * input[sy as usize * size + sx as usize];
                    }
                }
                out[y * size + x] = acc / norm;
            }
        }
        out
    };
    let h = pass(src, true);
    pass(&h, false)
}

/// Render a style to a pixel-exact bitmap (see module docs for centering).
pub fn render(style: &CrosshairStyle) -> Bitmap {
    let s = style.sanitized();
    let fill_rgb = parse_hex(&s.color).unwrap_or([0, 255, 102]);
    let out_rgb = parse_hex(&s.outline_color).unwrap_or([0, 0, 0]);
    let o = if s.outline { s.outline_thickness } else { 0 };
    let glow_r = if s.glow { s.glow_radius } else { 0 };

    // Bitmap half-extent: farthest element + outline + glow + AA margin.
    let mut ext = 1u32;
    if s.arms {
        ext = ext.max(s.gap + s.length + s.thickness);
    }
    if s.outer {
        ext = ext.max(s.outer_gap + s.outer_length + s.outer_thickness);
    }
    if s.dot {
        ext = ext.max(s.dot_size);
    }
    if s.ring {
        ext = ext.max(s.ring_radius + s.ring_thickness);
    }
    let half = ext + o + glow_r + 2;
    let size = half * 2 + 1; // +1: room for the half-pixel right/down snap
    let mut layers = Layers::new(size);

    // The crosshair's center: on the boundary for even primary strokes, on a
    // pixel middle for odd ones — so the primary element is always crisp.
    let primary = if s.arms {
        s.thickness
    } else if s.dot {
        s.dot_size
    } else {
        2
    };
    let c = half as f32 + if primary % 2 == 1 { 0.5 } else { 0.0 };
    let oi = o as i32;
    let of = o as f32;

    if s.arms {
        let (a, b) = span(c, s.thickness);
        let (g, l) = (s.gap as i32, s.length as i32);
        let (t, lf, gf) = (s.thickness as f32, s.length as f32, s.gap as f32);
        match s.shape {
            SHAPE_X => {
                // Four diagonal arms; distances measured along the diagonal.
                let k = std::f32::consts::FRAC_1_SQRT_2;
                for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                    let (ax, ay) = (c + sx * gf * k, c + sy * gf * k);
                    let (bx, by) = (c + sx * (gf + lf) * k, c + sy * (gf + lf) * k);
                    layers.seg(ax, ay, bx, by, t, of);
                }
            }
            SHAPE_CHEVRON => {
                // "^" whose apex sits `gap` below the center, arms going down-out.
                let k = std::f32::consts::FRAC_1_SQRT_2;
                let (ax, ay) = (c, c + gf);
                layers.seg(ax, ay, ax - lf * k, ay + lf * k, t, of);
                layers.seg(ax, ay, ax + lf * k, ay + lf * k, t, of);
            }
            SHAPE_BRACKETS => {
                // Corner brackets of a box `gap` from the center: each corner
                // runs `length` along both box edges.
                let (a2, b2) = span(c, s.thickness); // stroke spans
                let (inner_lo, inner_hi) = (a2 - g, b2 + g); // box edges
                let (lo0, lo1) = (inner_lo - s.thickness as i32, inner_lo); // outer stroke
                let (hi0, hi1) = (inner_hi, inner_hi + s.thickness as i32);
                let far_lo = lo0 + l;
                let far_hi = hi1 - l;
                // top-left
                layers.solid(lo0, lo0, far_lo, lo1, oi);
                layers.solid(lo0, lo0, lo1, far_lo, oi);
                // top-right
                layers.solid(far_hi, lo0, hi1, lo1, oi);
                layers.solid(hi0, lo0, hi1, far_lo, oi);
                // bottom-left
                layers.solid(lo0, hi0, far_lo, hi1, oi);
                layers.solid(lo0, far_hi, lo1, hi1, oi);
                // bottom-right
                layers.solid(far_hi, hi0, hi1, hi1, oi);
                layers.solid(hi0, far_hi, hi1, hi1, oi);
            }
            _ => {
                layers.solid(b + g, a, b + g + l, b, oi); // right
                layers.solid(a - g - l, a, a - g, b, oi); // left
                layers.solid(a, b + g, b, b + g + l, oi); // down
                if s.shape != SHAPE_T {
                    layers.solid(a, a - g - l, b, a - g, oi); // up
                }
                if g == 0 {
                    layers.solid(a, a, b, b, oi); // close the center of a gapless cross
                }
            }
        }
    }
    // Secondary elements sit on the primary's center `c` exactly. When their
    // width parity matches the primary they're pixel-crisp; when it doesn't,
    // the half-pixel of coverage is split across both edges (symmetric, never
    // shifted a whole pixel to one side).
    if s.outer {
        let ht = s.outer_thickness as f32 / 2.0;
        let (a, b) = (c - ht, c + ht);
        let (g, l) = (s.outer_gap as f32, s.outer_length as f32);
        let op = s.outer_opacity;
        layers.solid_f(b + g, a, b + g + l, b, of, op);
        layers.solid_f(a - g - l, a, a - g, b, of, op);
        layers.solid_f(a, b + g, b, b + g + l, of, op);
        layers.solid_f(a, a - g - l, b, a - g, of, op);
    }
    if s.dot {
        let hd = s.dot_size as f32 / 2.0;
        layers.solid_f(c - hd, c - hd, c + hd, c + hd, of, 1.0);
    }
    if s.ring {
        layers.ring(c, c, s.ring_radius as f32, s.ring_thickness as f32, of);
    }

    let glow = if glow_r > 0 {
        let cov: Vec<f32> = layers.fill.iter().map(|v| if *v > 0.0 { 1.0 } else { 0.0 }).collect();
        blur(&cov, size as usize, glow_r as usize)
    } else {
        Vec::new()
    };

    // Composite (straight alpha): fill over outline over glow.
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let fa = layers.fill.get(i).copied().unwrap_or(0.0);
        let oa = if o > 0 { layers.outline.get(i).copied().unwrap_or(0.0) } else { 0.0 };
        let ga = glow.get(i).copied().unwrap_or(0.0) * 0.55;
        // Gather from the bottom up.
        let (mut rgb, mut a) = ([0.0f32; 3], 0.0f32);
        for (src_rgb, src_a) in [(fill_rgb, ga), (out_rgb, oa), (fill_rgb, fa)] {
            if src_a <= 0.0 {
                continue;
            }
            let na = src_a + a * (1.0 - src_a);
            for k in 0..3 {
                rgb[k] = (src_rgb[k] as f32 * src_a + rgb[k] * a * (1.0 - src_a)) / na;
            }
            a = na;
        }
        if a <= 0.0 {
            continue;
        }
        for (dst, v) in px.iter_mut().zip(rgb.iter()) {
            *dst = v.round().clamp(0.0, 255.0) as u8;
        }
        px[3] = (a * s.opacity * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    Bitmap { size, half, rgba }
}

// ── Persistence ──

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Crosshair {
    pub id: String,
    pub name: String,
    pub style: CrosshairStyle,
    /// Bound program (lowercased exe basename). A bound crosshair switches in
    /// while that program is in the foreground.
    #[serde(default)]
    pub exe: Option<String>,
}

/// `selected` value for "no crosshair outside bound games" — the crosshair
/// counterpart of the color side's Normal. Never a real crosshair id (`c{n}`).
pub const NONE: &str = "none";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrosshairStore {
    pub crosshairs: Vec<Crosshair>,
    /// The user's pick: shown whenever no bound game is in front (and edited
    /// in the UI). `NONE` = nothing outside bound games.
    pub selected: String,
    /// Master overlay switch (UI toggle + Ctrl+Shift+F11). Off by default so
    /// an upgrade never drops a crosshair on someone's desktop unannounced.
    #[serde(default)]
    pub enabled: bool,
    pub next_id: u32,
}

fn builtins() -> Vec<Crosshair> {
    let base = CrosshairStyle::default();
    let mk = |id: &str, name: &str, style: CrosshairStyle| Crosshair {
        id: id.into(),
        name: name.into(),
        style,
        exe: None,
    };
    vec![
        mk("c1", "Classic", base.clone()),
        mk(
            "c2",
            "Dot",
            CrosshairStyle { arms: false, dot: true, dot_size: 3, color: "#00e5ff".into(), ..base.clone() },
        ),
        mk(
            "c3",
            "Circle dot",
            CrosshairStyle {
                arms: false,
                dot: true,
                dot_size: 2,
                ring: true,
                ring_radius: 8,
                ring_thickness: 2,
                color: "#ffffff".into(),
                ..base.clone()
            },
        ),
        mk(
            "c4",
            "T-cross",
            CrosshairStyle { shape: SHAPE_T, length: 7, gap: 3, color: "#ffee00".into(), ..base },
        ),
    ]
}

impl Default for CrosshairStore {
    fn default() -> Self {
        CrosshairStore { crosshairs: builtins(), selected: "c1".into(), enabled: false, next_id: 5 }
    }
}

fn store_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("exfil-v2").join("crosshairs.json")
}

impl CrosshairStore {
    pub fn load() -> Self {
        let mut s: CrosshairStore = match std::fs::read_to_string(store_path()) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
                log::warn!("crosshairs.json parse failed ({e}); using defaults");
                CrosshairStore::default()
            }),
            Err(_) => CrosshairStore::default(),
        };
        if s.crosshairs.is_empty() {
            s = CrosshairStore::default();
        }
        // Fold v2.4 `t_style` into `shape` and clamp everything once.
        for c in s.crosshairs.iter_mut() {
            c.style = c.style.sanitized();
        }
        if s.selected != NONE && s.get(&s.selected).is_none() {
            s.selected = s.crosshairs.first().map(|c| c.id.clone()).unwrap_or_default();
        }
        s
    }

    /// Atomic write-then-rename, same as the preset store.
    pub fn save(&self) -> Result<(), String> {
        let path = store_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    pub fn get(&self, id: &str) -> Option<&Crosshair> {
        self.crosshairs.iter().find(|c| c.id == id)
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut Crosshair, String> {
        self.crosshairs.iter_mut().find(|c| c.id == id).ok_or_else(|| "unknown crosshair".into())
    }

    /// Add a crosshair with a fresh monotonic `c{n}` id (never reused).
    pub fn add(&mut self, name: String, style: CrosshairStyle) -> Crosshair {
        let id = format!("c{}", self.next_id);
        self.next_id += 1;
        let name = if name.trim().is_empty() {
            format!("Crosshair {}", self.crosshairs.len() + 1)
        } else {
            name.trim().into()
        };
        let c = Crosshair { id, name, style: style.sanitized(), exe: None };
        self.crosshairs.push(c.clone());
        c
    }

    /// Delete a crosshair. The last one is kept so the list is never empty.
    /// Deleting the selected one falls back to `NONE` (as colors fall back to
    /// Normal).
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        if self.crosshairs.len() <= 1 {
            return Err("Keep at least one crosshair".into());
        }
        let before = self.crosshairs.len();
        self.crosshairs.retain(|c| c.id != id);
        if self.crosshairs.len() == before {
            return Err("unknown crosshair".into());
        }
        if self.selected == id {
            self.selected = NONE.into();
        }
        Ok(())
    }

    pub fn rename(&mut self, id: &str, name: String) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("name cannot be empty".into());
        }
        self.get_mut(id)?.name = name.into();
        Ok(())
    }

    pub fn update_style(&mut self, id: &str, style: CrosshairStyle) -> Result<(), String> {
        self.get_mut(id)?.style = style.sanitized();
        Ok(())
    }

    pub fn select(&mut self, id: &str) -> Result<(), String> {
        if id != NONE {
            self.get_mut(id)?;
        }
        self.selected = id.into();
        Ok(())
    }

    /// The pick after `selected` in list order, wrapping through `NONE` so
    /// cycling doubles as on/off (Ctrl+Shift+F12).
    pub fn next_pick(&self) -> String {
        if self.selected == NONE {
            return self.crosshairs.first().map(|c| c.id.clone()).unwrap_or_else(|| NONE.into());
        }
        match self.crosshairs.iter().position(|c| c.id == self.selected) {
            Some(i) if i + 1 < self.crosshairs.len() => self.crosshairs[i + 1].id.clone(),
            _ => NONE.into(),
        }
    }

    /// Bind (or clear) a program. One exe maps to one crosshair — binding it
    /// here clears it off any other crosshair.
    pub fn set_binding(&mut self, id: &str, exe: Option<String>) -> Result<(), String> {
        let exe = exe.and_then(|e| {
            let e = e.trim().to_lowercase();
            if e.is_empty() { None } else { Some(e) }
        });
        self.get_mut(id)?;
        if let Some(ref target) = exe {
            for c in self.crosshairs.iter_mut() {
                if c.id != id && c.exe.as_deref() == Some(target.as_str()) {
                    c.exe = None;
                }
            }
        }
        self.get_mut(id)?.exe = exe;
        Ok(())
    }

    /// What the overlay should draw for the current foreground program
    /// (`fg_exe` = None when it's EXFIL itself or unknown). Master switch off →
    /// nothing. Same model as color presets: a crosshair bound to the
    /// foreground exe switches in; otherwise the user's pick shows (bound or
    /// not — a binding means "auto-switch to this", not "only here"), and
    /// `NONE` shows nothing.
    pub fn resolve(&self, fg_exe: Option<&str>) -> Option<CrosshairStyle> {
        if !self.enabled {
            return None;
        }
        if let Some(exe) = fg_exe {
            if let Some(c) = self.crosshairs.iter().find(|c| c.exe.as_deref() == Some(exe)) {
                return Some(c.style.clone());
            }
        }
        self.get(&self.selected).map(|c| c.style.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opaque-pixel columns/rows of a bitmap, relative to its center boundary.
    fn extents(b: &Bitmap) -> (i32, i32, i32, i32) {
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for y in 0..b.size as i32 {
            for x in 0..b.size as i32 {
                if b.rgba[((y * b.size as i32 + x) * 4 + 3) as usize] > 0 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                    y1 = y1.max(y + 1);
                }
            }
        }
        let h = b.half as i32;
        (x0 - h, y0 - h, x1 - h, y1 - h)
    }

    #[test]
    fn even_thickness_cross_is_exactly_centered() {
        let st = CrosshairStyle { thickness: 2, gap: 3, length: 5, outline: false, ..Default::default() };
        let (x0, y0, x1, y1) = extents(&render(&st));
        // symmetric around the boundary: -(1+3+5) .. +(1+3+5)
        assert_eq!((x0, x1), (-9, 9));
        assert_eq!((y0, y1), (-9, 9));
    }

    #[test]
    fn odd_thickness_cross_snaps_half_pixel_right_and_down() {
        let st = CrosshairStyle { thickness: 1, gap: 2, length: 4, outline: false, ..Default::default() };
        let (x0, _, x1, _) = extents(&render(&st));
        // centered on the pixel right of the boundary: -6 .. +7 (13 px wide)
        assert_eq!((x0, x1), (-6, 7));
    }

    #[test]
    fn outline_grows_extent_and_t_shape_drops_top_arm() {
        let st = CrosshairStyle { thickness: 2, gap: 3, length: 5, outline: true, outline_thickness: 1, shape: SHAPE_T, ..Default::default() };
        let (x0, y0, x1, y1) = extents(&render(&st));
        assert_eq!((x0, x1), (-10, 10));
        assert_eq!(y1, 10);
        assert_eq!(y0, -2); // no top arm: only the horizontal bar + outline above center
    }

    #[test]
    fn legacy_t_style_migrates_into_shape() {
        let s = CrosshairStyle { t_style: true, ..Default::default() }.sanitized();
        assert_eq!(s.shape, SHAPE_T);
        assert!(!s.t_style);
        let json: serde_json::Value = serde_json::to_value(&s).unwrap();
        assert!(json.get("t_style").is_none());
    }

    #[test]
    fn center_pixel_is_filled_only_for_gapless_cross() {
        let at_center = |st: &CrosshairStyle| {
            let b = render(st);
            let i = ((b.half * b.size + b.half) * 4 + 3) as usize;
            b.rgba[i]
        };
        let base = CrosshairStyle { outline: false, ..Default::default() };
        assert_eq!(at_center(&CrosshairStyle { gap: 3, ..base.clone() }), 0);
        assert_eq!(at_center(&CrosshairStyle { gap: 0, ..base }), 255);
    }

    #[test]
    fn diagonal_shapes_and_brackets_are_symmetric_and_bounded() {
        for shape in [SHAPE_X, SHAPE_BRACKETS] {
            let st = CrosshairStyle { shape, thickness: 2, gap: 4, length: 6, outline: false, ..Default::default() };
            let (x0, y0, x1, y1) = extents(&render(&st));
            assert_eq!(-x0, x1, "shape {shape} x-symmetric");
            assert_eq!(-y0, y1, "shape {shape} y-symmetric");
            assert!(x1 <= 14);
        }
        let ch = CrosshairStyle { shape: SHAPE_CHEVRON, thickness: 2, gap: 2, length: 6, outline: false, ..Default::default() };
        let (_, y0, _, _) = extents(&render(&ch));
        assert!(y0 >= 1, "chevron sits below the center");
    }

    #[test]
    fn outer_lines_are_dimmer_and_further_out() {
        let st = CrosshairStyle {
            outline: false,
            outer: true,
            outer_gap: 12,
            outer_length: 3,
            outer_thickness: 2,
            outer_opacity: 0.5,
            ..Default::default()
        };
        let b = render(&st);
        let (x0, _, x1, _) = extents(&b);
        // stroke half-width 1 + gap 12 + length 3 = 16
        assert_eq!((x0, x1), (-16, 16));
        let h = b.half as i32;
        let alpha_at = |x: i32| b.rgba[(((h * b.size as i32) + h + x) * 4 + 3) as usize];
        assert_eq!(alpha_at(14), 128); // outer arm at half opacity
        assert_eq!(alpha_at(5), 255); // inner arm full
    }

    #[test]
    fn mismatched_parity_dot_and_outer_lines_stay_centered() {
        // 1px arms (center on a pixel middle) + 2px dot: the dot must be
        // symmetric around the arm column, not hang one pixel to the right.
        let st = CrosshairStyle { thickness: 1, gap: 4, length: 5, dot: true, dot_size: 2, outline: false, ..Default::default() };
        let b = render(&st);
        let h = b.half as i32;
        let alpha_at = |x: i32, y: i32| b.rgba[((((h + y) * b.size as i32) + h + x) * 4 + 3) as usize];
        assert_eq!(alpha_at(0, 0), 255); // arm column / dot center
        assert_eq!(alpha_at(-1, 0), alpha_at(1, 0));
        assert_eq!(alpha_at(0, -1), alpha_at(0, 1));
        assert!(alpha_at(-1, 0) > 0 && alpha_at(2, 0) == 0);

        // 2px arms (center on the boundary) + 1px outer lines: the outer line
        // straddles the boundary evenly instead of sitting below it.
        let st = CrosshairStyle { thickness: 2, outer: true, outer_thickness: 1, outer_gap: 12, outer_length: 3, outer_opacity: 0.7, outline: false, ..Default::default() };
        let b = render(&st);
        let h = b.half as i32;
        let alpha_at = |x: i32, y: i32| b.rgba[((((h + y) * b.size as i32) + h + x) * 4 + 3) as usize];
        assert_eq!(alpha_at(14, -1), alpha_at(14, 0));
        assert!(alpha_at(14, 0) > 0);
        assert_eq!(alpha_at(14, 1), 0);
        assert_eq!(alpha_at(14, -2), 0);
    }

    #[test]
    fn glow_adds_soft_halo_outside_the_stroke() {
        let st = CrosshairStyle { outline: false, glow: true, glow_radius: 3, gap: 0, length: 4, ..Default::default() };
        let b = render(&st);
        let h = b.half as i32;
        let alpha_at = |x: i32, y: i32| b.rgba[((((h + y) * b.size as i32) + h + x) * 4 + 3) as usize];
        assert_eq!(alpha_at(0, 0), 255);
        let halo = alpha_at(3, 3); // diagonal from the center, off the arms
        assert!(halo > 0 && halo < 200, "halo alpha {halo}");
    }

    #[test]
    fn scaled_rounds_and_keeps_minimums() {
        let st = CrosshairStyle { length: 6, thickness: 1, gap: 3, offset_x: 10, ..Default::default() };
        let s = st.scaled(1.5);
        assert_eq!((s.length, s.thickness, s.gap, s.offset_x), (9, 2, 5, 15));
        let t = st.scaled(0.5);
        assert_eq!(t.thickness, 1);
        assert_eq!(st.scaled(1.0), st);
    }

    #[test]
    fn sanitize_bounds_the_bitmap() {
        let st = CrosshairStyle { length: 9999, gap: 9999, ring_radius: 9999, ring: true, opacity: f32::NAN, ..Default::default() };
        let b = render(&st);
        assert!(b.size < 200);
        assert_eq!(b.rgba.len(), (b.size * b.size * 4) as usize);
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex("#00ff66"), Some([0, 255, 102]));
        assert_eq!(parse_hex("FFFFFF"), Some([255, 255, 255]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#zzzzzz"), None);
    }

    #[test]
    fn resolve_rules() {
        let mut s = CrosshairStore::default();
        assert!(s.resolve(None).is_none()); // master off
        s.enabled = true;
        assert_eq!(s.resolve(None), s.get("c1").map(|c| c.style.clone())); // unbound selected = everywhere
        s.set_binding("c2", Some("Game.exe".into())).unwrap();
        assert_eq!(s.resolve(Some("game.exe")), s.get("c2").map(|c| c.style.clone())); // bound wins in its game
        s.set_binding("c1", Some("other.exe".into())).unwrap();
        // a bound pick still shows outside games — binding = auto-switch, not game-only
        assert_eq!(s.resolve(Some("notepad.exe")), s.get("c1").map(|c| c.style.clone()));
        s.select(NONE).unwrap();
        assert!(s.resolve(Some("notepad.exe")).is_none()); // None = nothing outside games
        assert!(s.resolve(None).is_none());
        assert_eq!(s.resolve(Some("game.exe")), s.get("c2").map(|c| c.style.clone())); // bound still switches in
        assert!(s.select("c99").is_err());
    }

    #[test]
    fn next_pick_cycles_through_none() {
        let mut s = CrosshairStore::default();
        s.select(NONE).unwrap();
        assert_eq!(s.next_pick(), "c1");
        s.select("c4").unwrap();
        assert_eq!(s.next_pick(), NONE);
        s.select("c2").unwrap();
        assert_eq!(s.next_pick(), "c3");
    }

    #[test]
    fn deleting_the_pick_falls_back_to_none() {
        let mut s = CrosshairStore::default();
        s.select("c2").unwrap();
        s.delete("c2").unwrap();
        assert_eq!(s.selected, NONE);
        s.select("c1").unwrap();
        s.delete("c3").unwrap();
        assert_eq!(s.selected, "c1"); // deleting another one leaves the pick alone
    }

    #[test]
    fn store_ids_monotonic_binding_exclusive_last_kept() {
        let mut s = CrosshairStore::default();
        let a = s.add("".into(), CrosshairStyle::default());
        assert_eq!(a.id, "c5");
        s.delete(&a.id).unwrap();
        assert_eq!(s.add("X".into(), CrosshairStyle::default()).id, "c6");
        s.set_binding("c1", Some("g.exe".into())).unwrap();
        s.set_binding("c2", Some("G.EXE".into())).unwrap();
        assert_eq!(s.get("c1").and_then(|c| c.exe.clone()), None);
        while s.crosshairs.len() > 1 {
            let id = s.crosshairs[0].id.clone();
            s.delete(&id).unwrap();
        }
        assert!(s.delete(&s.crosshairs[0].id.clone()).is_err());
    }
}
