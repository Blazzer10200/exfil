//! Crosshair model, persistence and pixel renderer.
//!
//! Crosshairs live in their own store (%APPDATA%\exfil-v2\crosshairs.json),
//! separate from color presets. The renderer is pure (style → RGBA bitmap) so
//! the in-app preview and the on-screen overlay draw the exact same pixels.
//!
//! Centering model: the overlay anchors the bitmap's center BOUNDARY (`half`)
//! on the target's center boundary `left + floor(width / 2)`. Every element is
//! an integer-aligned rect around that point, so lines are always crisp: even
//! thicknesses land exactly on the true center, odd ones sit half a pixel
//! right/down (the smallest possible error — a 1px line can't straddle two
//! pixels without blurring). The per-crosshair X/Y nudge covers games that
//! round their own center the other way.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct CrosshairStyle {
    /// Fill color, "#rrggbb".
    pub color: String,
    /// Whole-crosshair opacity, 0.1..=1.0.
    pub opacity: f32,
    pub arms: bool,
    pub length: u32,
    pub thickness: u32,
    pub gap: u32,
    /// Hide the top arm (T-shape).
    pub t_style: bool,
    pub dot: bool,
    pub dot_size: u32,
    pub ring: bool,
    pub ring_radius: u32,
    pub ring_thickness: u32,
    pub outline: bool,
    pub outline_thickness: u32,
    /// Outline color, "#rrggbb".
    pub outline_color: String,
    /// Pixel nudge applied on screen (not in the bitmap).
    pub offset_x: i32,
    pub offset_y: i32,
}

impl Default for CrosshairStyle {
    fn default() -> Self {
        CrosshairStyle {
            color: "#00ff66".into(),
            opacity: 1.0,
            arms: true,
            length: 6,
            thickness: 2,
            gap: 3,
            t_style: false,
            dot: false,
            dot_size: 2,
            ring: false,
            ring_radius: 10,
            ring_thickness: 1,
            outline: true,
            outline_thickness: 1,
            outline_color: "#000000".into(),
            offset_x: 0,
            offset_y: 0,
        }
    }
}

impl CrosshairStyle {
    /// Clamp every field into its UI range. Guards the renderer (and its
    /// allocation size) against hand-edited or corrupt JSON.
    pub fn sanitized(&self) -> CrosshairStyle {
        CrosshairStyle {
            color: self.color.clone(),
            opacity: if self.opacity.is_finite() { self.opacity.clamp(0.1, 1.0) } else { 1.0 },
            arms: self.arms,
            length: self.length.clamp(1, 40),
            thickness: self.thickness.clamp(1, 10),
            gap: self.gap.min(30),
            t_style: self.t_style,
            dot: self.dot,
            dot_size: self.dot_size.clamp(1, 10),
            ring: self.ring,
            ring_radius: self.ring_radius.clamp(2, 60),
            ring_thickness: self.ring_thickness.clamp(1, 6),
            outline: self.outline,
            outline_thickness: self.outline_thickness.clamp(1, 3),
            outline_color: self.outline_color.clone(),
            offset_x: self.offset_x.clamp(-50, 50),
            offset_y: self.offset_y.clamp(-50, 50),
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

/// Canvas of per-pixel coverage (0..=1) for the fill and the outline.
struct Layers {
    size: i32,
    fill: Vec<f32>,
    outline: Vec<f32>,
}

impl Layers {
    fn new(size: u32) -> Self {
        let n = (size * size) as usize;
        Layers { size: size as i32, fill: vec![0.0; n], outline: vec![0.0; n] }
    }

    fn rect(buf: &mut [f32], size: i32, x0: i32, y0: i32, x1: i32, y1: i32) {
        for y in y0.max(0)..y1.min(size) {
            for x in x0.max(0)..x1.min(size) {
                if let Some(c) = buf.get_mut((y * size + x) as usize) {
                    *c = 1.0;
                }
            }
        }
    }

    /// Filled rect plus its outline (the rect grown by `o` on every side).
    fn solid(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, o: i32) {
        Self::rect(&mut self.fill, self.size, x0, y0, x1, y1);
        if o > 0 {
            Self::rect(&mut self.outline, self.size, x0 - o, y0 - o, x1 + o, y1 + o);
        }
    }

    /// Anti-aliased ring (4x4 supersampled) centered at (cx, cy): stroke
    /// centerline at radius `r`, `w` wide, outline `o` on both edges.
    fn ring(&mut self, cx: f32, cy: f32, r: f32, w: f32, o: f32) {
        const SS: i32 = 4;
        let reach = r + w / 2.0 + o + 1.0;
        let x0 = (cx - reach).floor().max(0.0) as i32;
        let y0 = (cy - reach).floor().max(0.0) as i32;
        let x1 = ((cx + reach).ceil() as i32).min(self.size);
        let y1 = ((cy + reach).ceil() as i32).min(self.size);
        for py in y0..y1 {
            for px in x0..x1 {
                let (mut f, mut ol) = (0, 0);
                for sy in 0..SS {
                    for sx in 0..SS {
                        let x = px as f32 + (sx as f32 + 0.5) / SS as f32;
                        let y = py as f32 + (sy as f32 + 0.5) / SS as f32;
                        let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                        let off = (d - r).abs();
                        if off <= w / 2.0 {
                            f += 1;
                        }
                        if o > 0.0 && off <= w / 2.0 + o {
                            ol += 1;
                        }
                    }
                }
                let idx = (py * self.size + px) as usize;
                let total = (SS * SS) as f32;
                if let Some(c) = self.fill.get_mut(idx) {
                    *c = c.max(f as f32 / total);
                }
                if let Some(c) = self.outline.get_mut(idx) {
                    *c = c.max(ol as f32 / total);
                }
            }
        }
    }
}

/// Render a style to a pixel-exact bitmap (see module docs for centering).
pub fn render(style: &CrosshairStyle) -> Bitmap {
    let s = style.sanitized();
    let fill_rgb = parse_hex(&s.color).unwrap_or([0, 255, 102]);
    let out_rgb = parse_hex(&s.outline_color).unwrap_or([0, 0, 0]);
    let o = if s.outline { s.outline_thickness } else { 0 };

    // Bitmap half-extent: farthest element + outline + AA margin.
    let mut ext = 1u32;
    if s.arms {
        ext = ext.max(s.gap + s.length + s.thickness);
    }
    if s.dot {
        ext = ext.max(s.dot_size);
    }
    if s.ring {
        ext = ext.max(s.ring_radius + s.ring_thickness);
    }
    let half = ext + o + 2;
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

    if s.arms {
        let (a, b) = span(c, s.thickness);
        let (g, l) = (s.gap as i32, s.length as i32);
        layers.solid(b + g, a, b + g + l, b, oi); // right
        layers.solid(a - g - l, a, a - g, b, oi); // left
        layers.solid(a, b + g, b, b + g + l, oi); // down
        if !s.t_style {
            layers.solid(a, a - g - l, b, a - g, oi); // up
        }
        if g == 0 {
            layers.solid(a, a, b, b, oi); // close the center of a gapless cross
        }
    }
    if s.dot {
        let (a, b) = span(c, s.dot_size);
        layers.solid(a, a, b, b, oi);
    }
    if s.ring {
        let (a, b) = span(c, primary);
        let rc = (a + b) as f32 / 2.0;
        layers.ring(rc, rc, s.ring_radius as f32, s.ring_thickness as f32, o as f32);
    }

    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let fa = layers.fill.get(i).copied().unwrap_or(0.0);
        let oa = if o > 0 { layers.outline.get(i).copied().unwrap_or(0.0) } else { 0.0 };
        let alpha = fa + (1.0 - fa) * oa;
        if alpha <= 0.0 {
            continue;
        }
        for (dst, (f, ol)) in px.iter_mut().zip(fill_rgb.iter().zip(out_rgb.iter())) {
            let v = (fa * *f as f32 + (1.0 - fa) * oa * *ol as f32) / alpha;
            *dst = v.round().clamp(0.0, 255.0) as u8;
        }
        px[3] = (alpha * s.opacity * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    Bitmap { size, half, rgba }
}

// ── Persistence ──

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Crosshair {
    pub id: String,
    pub name: String,
    pub style: CrosshairStyle,
    /// Bound program (lowercased exe basename). A bound crosshair shows only
    /// while that program is in the foreground; unbound shows everywhere.
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
            CrosshairStyle { arms: false, dot: true, dot_size: 4, color: "#00e5ff".into(), ..base.clone() },
        ),
        mk(
            "c3",
            "Circle dot",
            CrosshairStyle {
                arms: false,
                dot: true,
                dot_size: 2,
                ring: true,
                ring_radius: 12,
                ring_thickness: 2,
                color: "#ffffff".into(),
                ..base.clone()
            },
        ),
        mk(
            "c4",
            "T-shape",
            CrosshairStyle { t_style: true, length: 8, gap: 4, color: "#ffee00".into(), ..base },
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
    fn outline_grows_extent_and_t_style_drops_top_arm() {
        let st = CrosshairStyle { thickness: 2, gap: 3, length: 5, outline: true, outline_thickness: 1, t_style: true, ..Default::default() };
        let (x0, y0, x1, y1) = extents(&render(&st));
        assert_eq!((x0, x1), (-10, 10));
        assert_eq!(y1, 10);
        assert_eq!(y0, -2); // no top arm: only the horizontal bar + outline above center
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
