//! Design tokens for the "liquid glass bento" language, plus font setup.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use serde::{Deserialize, Serialize};

/// Fallback accent when no wallpaper color is available: electric violet.
pub const FALLBACK_ACCENT: Color32 = Color32::from_rgb(0x7C, 0x5C, 0xFF);

/// 8 px spacing grid.
pub const SPACE: f32 = 8.0;
pub const TILE_RADIUS: f32 = 16.0;
pub const TILE_PADDING: f32 = 12.0;

pub const SIZE_LABEL: f32 = 11.0;
pub const SIZE_BODY: f32 = 13.0;
pub const SIZE_BIG: f32 = 28.0;
pub const SIZE_ICON: f32 = 18.0;

pub const FAMILY_SEMIBOLD: &str = "zenith-semibold";
/// Thin display figures (Event Horizon numerals).
pub const FAMILY_LIGHT: &str = "zenith-light";
/// Monospace (Omega Mono numerals and labels).
pub const FAMILY_MONO: &str = "zenith-mono";
pub const FAMILY_ICON_FILL: &str = "phosphor-fill";
pub const FAMILY_ICON_REGULAR: &str = "phosphor";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Dark,
    Light,
    #[default]
    Auto,
}

/// Zenith's visual languages. Each sets its own palette, corner radii,
/// numeral font, signature accent and a small signature detail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// Void black, borderless tiles, thin figures, a glowing amber-rose
    /// horizon line along the bottom edge.
    Horizon,
    /// Night-blue glass with gold, soft large corners, a star in the pill.
    #[default]
    Night,
    /// Pure black, sharp corners, hairline rims, monospace figures and a
    /// single omega-orange signal color.
    Mono,
}

impl Style {
    pub const ALL: [Style; 3] = [Style::Horizon, Style::Night, Style::Mono];

    pub fn label(self) -> &'static str {
        match self {
            Style::Horizon => "Horizon",
            Style::Night => "Night",
            Style::Mono => "Mono",
        }
    }

    /// The style's own signature accent.
    pub fn accent(self) -> Color32 {
        match self {
            Style::Horizon => Color32::from_rgb(0xFF, 0xB0, 0x67),
            Style::Night => Color32::from_rgb(0xF5, 0xC2, 0x6B),
            Style::Mono => Color32::from_rgb(0xFF, 0x5A, 0x1F),
        }
    }

    /// Second gradient stop paired with `accent`.
    pub fn accent2(self, accent: Color32) -> Color32 {
        match self {
            Style::Horizon => Color32::from_rgb(0xFF, 0x5C, 0x8A),
            Style::Night => mix(accent, Color32::from_rgb(0xFF, 0xF0, 0xD0), 0.6),
            Style::Mono => accent,
        }
    }
}

/// User styling on top of the built-in palette (see the settings page).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Opacity of the glass over the blurred backdrop, 0..1.
    pub glass_opacity: f32,
    /// Color the glass is tinted toward; `None` keeps the neutral glass.
    pub tint: Option<Color32>,
    /// Film-grain strength, 0..1.
    pub grain: f32,
    /// Size of one grain speck in physical pixels.
    pub grain_px: f32,
    pub style: Style,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            glass_opacity: 0.70,
            tint: None,
            grain: 0.35,
            grain_px: 1.0,
            style: Style::Night,
        }
    }
}

/// Grain alpha at full strength. The grain is two-sided (light and dark
/// specks), so it reads at a lower alpha than a white-only one would.
const GRAIN_MAX_DARK: f32 = 22.0;
const GRAIN_MAX_LIGHT: f32 = 16.0;

/// Resolved colors for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub style: Style,
    pub accent: Color32,
    /// Second accent stop for gradients (progress bars, horizon line).
    pub accent2: Color32,
    pub tile_radius: f32,
    /// Corner radius of the open panel.
    pub panel_radius: f32,
    /// Corner radius of the collapsed pill.
    pub pill_radius: f32,
    /// Glass fill, top and bottom of the vertical gradient.
    pub glass_top: Color32,
    pub glass_bottom: Color32,
    /// 1 px rim around the shell (stands in for the outer shadow, see NOTES).
    pub rim: Color32,
    /// 1 px inner highlight along the top edge.
    pub highlight: Color32,
    pub noise_alpha: u8,
    /// Physical pixels per grain speck.
    pub grain_px: f32,
    pub tile_top: Color32,
    pub tile_bottom: Color32,
    pub tile_rim: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    /// The collapsed pill is always "ink" (a hardware-notch black), whatever
    /// the theme; it blends into the theme's glass as the shell opens.
    pub pill_top: Color32,
    pub pill_bottom: Color32,
    pub pill_rim: Color32,
    pub pill_highlight: Color32,
    pub pill_text: Color32,
    pub pill_text_dim: Color32,
}

impl Theme {
    /// `translucent` is true when a blurred backdrop (acrylic) sits behind
    /// the window; otherwise the glass is drawn nearly opaque.
    pub fn new(dark: bool, accent: Color32, translucent: bool) -> Self {
        Self::styled(dark, accent, translucent, &Look::default())
    }

    /// [`Theme::new`] with the user's [`Look`] applied.
    pub fn styled(dark: bool, accent: Color32, translucent: bool, look: &Look) -> Self {
        let style = look.style;
        let glass_alpha = if translucent {
            look.glass_opacity.clamp(0.2, 1.0)
        } else {
            0.96
        };
        let grain = look.grain.clamp(0.0, 1.0);
        let grain_px = look.grain_px.clamp(1.0, 4.0);
        // The tint keeps the theme's lightness: a dark theme gets a deep
        // shade of the color, a light theme a pale one, so text stays legible.
        let tinted = |base: Color32, amount: f32| match look.tint {
            Some(t) => mix(base, t, amount),
            None => base,
        };
        let rgb = Color32::from_rgb;
        let rgba = Color32::from_rgba_unmultiplied;
        let ink_alpha = if translucent { 0.92 } else { 1.0 };
        let ink = match style {
            Style::Horizon => Ink {
                pill_top: with_alpha(rgb(12, 12, 16), ink_alpha),
                pill_bottom: with_alpha(rgb(4, 4, 6), ink_alpha),
                pill_rim: rgba(255, 255, 255, 14),
                pill_highlight: rgba(255, 255, 255, 30),
                pill_text: rgb(242, 242, 245),
                pill_text_dim: rgb(124, 124, 144),
            },
            Style::Night => Ink {
                pill_top: with_alpha(rgb(36, 39, 58), ink_alpha),
                pill_bottom: with_alpha(rgb(7, 8, 15), ink_alpha),
                pill_rim: rgba(170, 190, 255, 30),
                pill_highlight: rgba(255, 255, 255, 40),
                pill_text: rgb(238, 242, 255),
                pill_text_dim: rgb(142, 155, 214),
            },
            Style::Mono => Ink {
                pill_top: with_alpha(rgb(0, 0, 0), ink_alpha),
                pill_bottom: with_alpha(rgb(0, 0, 0), ink_alpha),
                pill_rim: rgba(255, 255, 255, 40),
                pill_highlight: rgba(255, 255, 255, 0),
                pill_text: rgb(237, 237, 237),
                pill_text_dim: rgb(122, 122, 122),
            },
        };
        let (tile_radius, panel_radius, pill_radius) = match style {
            Style::Horizon => (14.0, 26.0, 17.0),
            Style::Night => (20.0, 30.0, 17.0),
            Style::Mono => (6.0, 14.0, 9.0),
        };
        let base = Theme {
            dark,
            style,
            accent,
            accent2: style.accent2(accent),
            tile_radius,
            panel_radius,
            pill_radius,
            grain_px,
            ..ink.into_theme()
        };
        if !dark {
            return Self {
                glass_top: with_alpha(tinted(rgb(252, 252, 253), 0.14), glass_alpha),
                glass_bottom: with_alpha(tinted(rgb(236, 236, 240), 0.20), glass_alpha),
                rim: rgba(0, 0, 0, 40),
                highlight: rgba(255, 255, 255, 200),
                noise_alpha: (grain * GRAIN_MAX_LIGHT).round() as u8,
                tile_top: rgba(255, 255, 255, 150),
                tile_bottom: rgba(255, 255, 255, 90),
                tile_rim: rgba(0, 0, 0, 14),
                text: rgb(24, 24, 28),
                text_dim: rgb(92, 92, 104),
                text_faint: rgb(132, 132, 142),
                ..base
            };
        }
        let noise_alpha = (grain * GRAIN_MAX_DARK).round() as u8;
        match style {
            Style::Horizon => Self {
                glass_top: with_alpha(tinted(rgb(14, 14, 20), 0.30), glass_alpha),
                glass_bottom: with_alpha(tinted(rgb(5, 5, 9), 0.20), glass_alpha),
                rim: rgba(0, 0, 0, 140),
                highlight: rgba(255, 255, 255, 26),
                noise_alpha,
                tile_top: rgba(255, 255, 255, 13),
                tile_bottom: rgba(255, 255, 255, 6),
                tile_rim: rgba(255, 255, 255, 0),
                text: rgb(242, 242, 245),
                text_dim: rgb(150, 150, 166),
                text_faint: rgb(112, 112, 132),
                ..base
            },
            Style::Night => Self {
                glass_top: with_alpha(tinted(rgb(22, 30, 80), 0.30), glass_alpha),
                glass_bottom: with_alpha(tinted(rgb(9, 13, 38), 0.20), glass_alpha),
                rim: rgba(170, 190, 255, 44),
                highlight: rgba(255, 255, 255, 44),
                noise_alpha,
                tile_top: rgba(120, 140, 240, 40),
                tile_bottom: rgba(60, 76, 170, 30),
                tile_rim: rgba(170, 190, 255, 28),
                text: rgb(238, 242, 255),
                text_dim: rgb(162, 174, 226),
                text_faint: rgb(120, 132, 192),
                ..base
            },
            Style::Mono => Self {
                glass_top: with_alpha(tinted(rgb(3, 3, 3), 0.30), glass_alpha),
                glass_bottom: with_alpha(tinted(rgb(0, 0, 0), 0.20), glass_alpha),
                rim: rgba(255, 255, 255, 34),
                highlight: rgba(255, 255, 255, 0),
                noise_alpha: noise_alpha / 2,
                tile_top: rgba(255, 255, 255, 8),
                tile_bottom: rgba(255, 255, 255, 8),
                tile_rim: rgba(255, 255, 255, 30),
                text: rgb(237, 237, 237),
                text_dim: rgb(152, 152, 152),
                text_faint: rgb(112, 112, 112),
                ..base
            },
        }
    }

    /// Glass fill (top, bottom) at shell `progress`: ink pill at 0, theme
    /// glass from ~70% open.
    pub fn glass_at(&self, progress: f32) -> (Color32, Color32) {
        let t = blend(progress);
        (
            mix(self.pill_top, self.glass_top, t),
            mix(self.pill_bottom, self.glass_bottom, t),
        )
    }

    pub fn rim_at(&self, progress: f32) -> Color32 {
        mix(self.pill_rim, self.rim, blend(progress))
    }

    pub fn highlight_at(&self, progress: f32) -> Color32 {
        mix(self.pill_highlight, self.highlight, blend(progress))
    }

    /// Accent-tinted gradient for active tiles.
    pub fn active_tile(&self) -> (Color32, Color32) {
        let a = if self.dark { 0.42 } else { 0.30 };
        let b = if self.dark { 0.20 } else { 0.14 };
        (with_alpha(self.accent, a), with_alpha(self.accent, b))
    }

    /// Accent readable as text/icon color on the current glass.
    pub fn accent_fg(&self) -> Color32 {
        if self.dark {
            mix(self.accent, Color32::WHITE, 0.25)
        } else {
            mix(self.accent, Color32::BLACK, 0.15)
        }
    }

    pub fn font(size: f32) -> FontId {
        FontId::proportional(size)
    }

    pub fn font_semibold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(FAMILY_SEMIBOLD.into()))
    }

    /// Monospace text, for labels and figures in the Mono style.
    pub fn font_mono(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(FAMILY_MONO.into()))
    }

    /// Big figures (clock, timer, temperature) in the style's own face:
    /// thin for Horizon, semibold for Night, monospace for Mono.
    pub fn numeral_font(&self, size: f32) -> FontId {
        match self.style {
            Style::Horizon => FontId::new(size, FontFamily::Name(FAMILY_LIGHT.into())),
            Style::Night => Self::font_semibold(size),
            Style::Mono => Self::font_mono(size),
        }
    }
}

/// Smoothstep of `progress / 0.7`: how far the pill look has turned into
/// the panel look.
fn blend(progress: f32) -> f32 {
    let t = (progress / 0.7).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The theme-independent pill colors.
struct Ink {
    pill_top: Color32,
    pill_bottom: Color32,
    pill_rim: Color32,
    pill_highlight: Color32,
    pill_text: Color32,
    pill_text_dim: Color32,
}

impl Ink {
    /// A `Theme` carrying only the ink fields, for struct-update syntax.
    fn into_theme(self) -> Theme {
        let z = Color32::TRANSPARENT;
        Theme {
            dark: true,
            style: Style::Night,
            accent: z,
            accent2: z,
            tile_radius: TILE_RADIUS,
            panel_radius: 32.0,
            pill_radius: 17.0,
            glass_top: z,
            glass_bottom: z,
            rim: z,
            highlight: z,
            noise_alpha: 0,
            grain_px: 1.0,
            tile_top: z,
            tile_bottom: z,
            tile_rim: z,
            text: z,
            text_dim: z,
            text_faint: z,
            pill_top: self.pill_top,
            pill_bottom: self.pill_bottom,
            pill_rim: self.pill_rim,
            pill_highlight: self.pill_highlight,
            pill_text: self.pill_text,
            pill_text_dim: self.pill_text_dim,
        }
    }
}

/// Multiplies a color's alpha (unmultiplied semantics).
pub fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let a = (a as f32 * alpha.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Mixes two colors in gamma space; `t = 0` is `a`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.to_srgba_unmultiplied();
    let [br, bg, bb, ba] = b.to_srgba_unmultiplied();
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(ar, br), l(ag, bg), l(ab, bb), l(aa, ba))
}

/// Parses `#RRGGBB`.
pub fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub fn to_hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// Installs the text and icon fonts.
///
/// Text uses the system's Segoe UI (regular + semibold), whose digits are
/// tabular by default; egui cannot switch OpenType features such as `tnum`,
/// which rules out Inter's proportional default figures. If the files are
/// missing, egui's bundled font is used. See docs/NOTES.md.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let load = |file: &str| std::fs::read(format!("{windir}\\Fonts\\{file}")).ok();

    let regular = load("segoeui.ttf");
    let semibold = load("seguisb.ttf");
    if let Some(bytes) = regular {
        fonts
            .font_data
            .insert("segoe".into(), FontData::from_owned(bytes).into());
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe".into());
        }
    }
    let mut semibold_family = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(bytes) = semibold {
        fonts
            .font_data
            .insert("segoe-semibold".into(), FontData::from_owned(bytes).into());
        semibold_family.insert(0, "segoe-semibold".into());
    }
    fonts
        .families
        .insert(FontFamily::Name(FAMILY_SEMIBOLD.into()), semibold_family);

    // Style faces. Each family falls back to the proportional list, so a
    // missing file never leaves a family undefined.
    let proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family, key, files) in [
        (FAMILY_LIGHT, "zenith-light-data", &["segoeuil.ttf", "segoeuisl.ttf"][..]),
        (FAMILY_MONO, "zenith-mono-data", &["CascadiaMono.ttf", "consola.ttf"][..]),
    ] {
        let mut list = proportional.clone();
        if let Some(bytes) = files.iter().find_map(|f| load(f)) {
            fonts
                .font_data
                .insert(key.into(), FontData::from_owned(bytes).into());
            list.insert(0, key.into());
        }
        fonts.families.insert(FontFamily::Name(family.into()), list);
    }

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    egui_phosphor::add_font_bytes_as_family(
        &mut fonts,
        FAMILY_ICON_FILL,
        egui_phosphor::Variant::Fill.font_bytes(),
    );
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(parse_hex("#7C5CFF"), Some(FALLBACK_ACCENT));
        assert_eq!(to_hex(FALLBACK_ACCENT), "#7C5CFF");
        assert_eq!(parse_hex("nope"), None);
    }

    #[test]
    fn alpha_and_mix() {
        assert_eq!(with_alpha(Color32::WHITE, 0.5).a(), 128);
        assert_eq!(mix(Color32::BLACK, Color32::WHITE, 1.0), Color32::WHITE);
    }

    #[test]
    fn look_controls_grain_and_glass() {
        let off = Look {
            grain: 0.0,
            ..Look::default()
        };
        assert_eq!(
            Theme::styled(true, FALLBACK_ACCENT, true, &off).noise_alpha,
            0
        );
        let clear = Look {
            glass_opacity: 0.5,
            ..Look::default()
        };
        let t = Theme::styled(true, FALLBACK_ACCENT, true, &clear);
        assert_eq!(t.glass_top.to_srgba_unmultiplied()[3], 128);
        // The solid material ignores the opacity setting.
        let t = Theme::styled(true, FALLBACK_ACCENT, false, &clear);
        assert_eq!(t.glass_top.to_srgba_unmultiplied()[3], 245);
    }
}
