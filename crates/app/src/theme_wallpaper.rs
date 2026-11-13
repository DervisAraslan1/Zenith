//! Wallpapers drawn for each style at the primary monitor's resolution:
//! Night's starry sky over a planet's lit rim, Horizon's glowing line in the
//! void, Mono's hairline grid with one orange stroke. Rendered once per
//! style and size into `%APPDATA%\Zenith\wallpapers`, then set on every
//! monitor. The user's own wallpaper is remembered first, so choosing
//! "Kendi" in Settings puts it back.

use std::path::{Path, PathBuf};
use zenith_ui::theme::Style;

type Rgb = [f32; 3];

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn add(a: Rgb, b: Rgb, k: f32) -> Rgb {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

/// Small deterministic hash, for dithering and star placement.
fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn night(x: f32, y: f32, w: f32, h: f32) -> Rgb {
    let sky = mix([7.0, 10.0, 30.0], [30.0, 46.0, 122.0], (y / h).powf(1.3));
    // A huge planet whose top edge sits low on the screen.
    let r = w * 0.95;
    let (cx, cy) = (w * 0.5, h * 0.84 + r);
    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r;
    if d < 0.0 {
        let rim = (d / (h * 0.004)).exp();
        return add([5.0, 7.0, 20.0], [255.0, 210.0, 150.0], rim * 0.5);
    }
    let glow = (-d / (h * 0.07)).exp();
    let rim = (-d / (h * 0.0025)).exp();
    // Warm gold that holds its colour over the blue (a pale one turns lilac).
    add(add(sky, [255.0, 140.0, 40.0], glow * 0.85), [255.0, 205.0, 120.0], rim)
}

fn horizon(x: f32, y: f32, w: f32, h: f32) -> Rgb {
    let dx = (x - w * 0.5) / w;
    let dy = (y + h * 0.1) / h;
    let base = mix([28.0, 22.0, 50.0], [5.0, 5.0, 8.0], smoothstep((dx * dx + dy * dy).sqrt() / 1.1));
    let y0 = h * 0.64;
    let tx = x / w;
    let line = mix([255.0, 176.0, 103.0], [255.0, 92.0, 138.0], tx);
    let envelope = (std::f32::consts::PI * tx).sin().max(0.0).powf(0.6);
    let dist = y - y0;
    let core = (-(dist.abs()) / 1.3).exp();
    // The glow rises higher than it falls.
    let spread = if dist < 0.0 { h * 0.05 } else { h * 0.025 };
    let glow = (-(dist.abs()) / spread).exp() * 0.42;
    add(base, line, envelope * (core + glow))
}

fn mono(x: f32, y: f32, w: f32, h: f32) -> Rgb {
    let g = (h / 12.0).round().max(8.0);
    let (xi, yi) = (x as i32, y as i32);
    let gi = g as i32;
    let mut c = [0.0, 0.0, 0.0];
    if xi % gi == 0 || yi % gi == 0 {
        c = [15.0, 15.0, 15.0];
    }
    if xi % (gi * 4) == 0 || yi % (gi * 4) == 0 {
        c = [26.0, 26.0, 26.0];
    }
    // One omega-orange stroke and its end mark.
    let ly = (h * 2.0 / 3.0).round();
    if (y - ly).abs() < 1.5 && x > w * 0.08 && x < w * 0.32 {
        c = [255.0, 90.0, 31.0];
    }
    if (x - w * 0.32).abs() < 4.0 && (y - ly).abs() < 4.0 {
        c = [255.0, 90.0, 31.0];
    }
    c
}

/// Draws `style`'s wallpaper at `w` x `h` pixels.
pub fn render(style: Style, w: u32, h: u32) -> image::RgbImage {
    let (wf, hf) = (w as f32, h as f32);
    let mut img = image::RgbImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (xf, yf) = (x as f32 + 0.5, y as f32 + 0.5);
        let c = match style {
            Style::Night => night(xf, yf, wf, hf),
            Style::Horizon => horizon(xf, yf, wf, hf),
            Style::Mono => mono(xf, yf, wf, hf),
        };
        // Half a level of noise keeps the gradients from banding.
        let n = (hash(x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)) & 255) as f32 / 255.0 - 0.5;
        let dither = if style == Style::Mono { 0.0 } else { n };
        *px = image::Rgb(c.map(|v| (v + dither).round().clamp(0.0, 255.0) as u8));
    }
    if style == Style::Night {
        stars(&mut img);
    }
    img
}

/// Scattered stars in the upper sky, a few with a soft cross.
fn stars(img: &mut image::RgbImage) {
    let (w, h) = img.dimensions();
    let count = (w as u64 * h as u64 / 7000) as u32;
    for i in 0..count {
        let hx = hash(i.wrapping_mul(2_654_435_761));
        let hy = hash(hx ^ 0x9e37_79b9);
        let hb = hash(hy ^ 0x85eb_ca6b);
        let x = hx % w;
        let y = hy % (h * 7 / 10);
        // Fewer and dimmer toward the horizon.
        let fade = 1.0 - y as f32 / (h as f32 * 0.7);
        let b = (60.0 + (hb % 196) as f32) * fade;
        let mut plot = |px: i64, py: i64, k: f32| {
            if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                let p = img.get_pixel_mut(px as u32, py as u32);
                for c in 0..3 {
                    p.0[c] = (p.0[c] as f32 + b * k).min(255.0) as u8;
                }
            }
        };
        let (xi, yi) = (x as i64, y as i64);
        plot(xi, yi, 1.0);
        if hb % 23 == 0 {
            for d in 1..=3i64 {
                let k = 0.5 / d as f32;
                plot(xi + d, yi, k);
                plot(xi - d, yi, k);
                plot(xi, yi + d, k);
                plot(xi, yi - d, k);
            }
        }
    }
}

fn folder() -> Option<PathBuf> {
    crate::config::dir().ok().map(|d| d.join("wallpapers"))
}

/// True for a wallpaper Zenith made (so it is never taken for the user's own).
pub fn is_ours(path: &Path) -> bool {
    folder().is_some_and(|f| path.starts_with(&f))
}

fn primary_size() -> (u32, u32) {
    zenith_platform::monitor::primary()
        .map(|m| (m.bounds.width().max(640) as u32, m.bounds.height().max(480) as u32))
        .unwrap_or((1920, 1080))
}

/// Renders (if needed) and sets `style`'s wallpaper, off the UI thread.
pub fn apply(style: Style) {
    let spawned = std::thread::Builder::new()
        .name("zenith-theme-wallpaper".into())
        .spawn(move || {
            let Some(dir) = folder() else {
                return;
            };
            let (w, h) = primary_size();
            let name = format!("zenith-{}-{w}x{h}.png", style.label().to_lowercase());
            let path = dir.join(name);
            if !path.is_file() {
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    tracing::warn!("theme wallpaper folder: {e}");
                    return;
                }
                let started = std::time::Instant::now();
                if let Err(e) = render(style, w, h).save(&path) {
                    tracing::warn!("saving theme wallpaper: {e}");
                    return;
                }
                tracing::info!("theme wallpaper {w}x{h} drawn in {:?}", started.elapsed());
            }
            if let Err(e) = zenith_platform::wallpaper::set_desktop(&path) {
                tracing::warn!("setting theme wallpaper: {e}");
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("theme wallpaper thread: {e}");
    }
}

/// Puts the user's own wallpaper back, off the UI thread.
pub fn restore(own: PathBuf) {
    let _ = std::thread::Builder::new()
        .name("zenith-restore-wallpaper".into())
        .spawn(move || {
            if own.is_file()
                && let Err(e) = zenith_platform::wallpaper::set_desktop(&own)
            {
                tracing::warn!("restoring wallpaper: {e}");
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_each_style() {
        for s in Style::ALL {
            let img = render(s, 64, 36);
            assert_eq!(img.dimensions(), (64, 36));
        }
        // Night's top is darker than its horizon glow.
        let img = render(Style::Night, 200, 100);
        let top = img.get_pixel(100, 2).0[0];
        let glow = img.get_pixel(100, 82).0[0];
        assert!(glow > top, "glow {glow} top {top}");
    }

    /// Writes full-size previews: `ZENITH_WALL_DIR=<dir> cargo test -p
    /// zenith-app dump_previews -- --ignored`.
    #[test]
    #[ignore = "writes files; set ZENITH_WALL_DIR"]
    fn dump_previews() {
        let dir = std::path::PathBuf::from(std::env::var("ZENITH_WALL_DIR").expect("ZENITH_WALL_DIR"));
        for s in Style::ALL {
            render(s, 1920, 1080)
                .save(dir.join(format!("wall-{}.png", s.label().to_lowercase())))
                .unwrap();
        }
    }
}
