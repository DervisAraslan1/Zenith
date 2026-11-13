//! Settings: the gear tile in the bento grid and the in-notch settings page.

use crate::config::{AccentSource, Config, GrainSize, Material, MonitorMode, Renderer};
use egui::{
    Align, Align2, Area, Color32, Id, Layout, Order, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2,
};
use zenith_ui::bento::PANEL_PADDING;
use zenith_ui::module::{Module, TileCtx, TileSize};
use zenith_ui::shell::{ContentCtx, Placement};
use zenith_ui::theme::{
    FALLBACK_ACCENT, SIZE_BODY, SIZE_ICON, SIZE_LABEL, Style, Theme, ThemeMode, parse_hex, to_hex,
    with_alpha,
};
use zenith_ui::{icons, paint, widgets};

pub const SETTINGS_ID: &str = "settings";

const GAP: f32 = 8.0;
const HEADER: f32 = 28.0;
const ROW: f32 = 28.0;
/// Height of the Appearance and Behavior cards (title + four rows).
const TOP_CARD: f32 = 154.0;
/// Height of a collapsible card's clickable header.
const DISCLOSURE: f32 = 44.0;
/// Revealed height of each collapsible section's body.
const BODY: [f32; 2] = [4.0 * ROW + 14.0, 3.0 * ROW + 28.0];
/// Page height with every section collapsed, panel padding included.
const BASE: f32 =
    2.0 * PANEL_PADDING + HEADER + GAP + TOP_CARD + GAP + DISCLOSURE + GAP + DISCLOSURE;

/// Collapsible sections, in page order.
const CUSTOMIZE: usize = 0;
const ADVANCED: usize = 1;

/// Which collapsible section is open (at most one at a time).
fn open_section(ctx: &egui::Context) -> Option<usize> {
    ctx.data(|d| d.get_temp(Id::new("zenith-settings-open")))
        .flatten()
}

fn set_open_section(ctx: &egui::Context, section: Option<usize>) {
    ctx.data_mut(|d| d.insert_temp(Id::new("zenith-settings-open"), section));
}

/// How much taller than the regular panel (`regular`) the settings page
/// is; follows the open section, so the panel springs along with it.
pub fn extra_height(ctx: &egui::Context, regular: f32) -> f32 {
    let body = open_section(ctx).map_or(0.0, |i| BODY[i]);
    (BASE + body - regular).max(0.0)
}

/// Spring for a section's 0..1 reveal, tuned like the panel height so the
/// card and the panel move together.
fn reveal(ctx: &egui::Context, section: usize, open: bool) -> f32 {
    zenith_ui::anim::spring(
        ctx,
        Id::new("zenith-settings-reveal").with(section),
        if open { 1.0 } else { 0.0 },
        zenith_ui::spring::SpringParams::with_overshoot(220.0, 0.04),
    )
    .max(0.0)
}

/// Background tints offered in the Style card; `None` is neutral glass.
const BACKGROUNDS: [Option<Color32>; 7] = [
    None,
    Some(Color32::from_rgb(0x4A, 0x6F, 0xA5)),
    Some(Color32::from_rgb(0x2A, 0x9D, 0x8F)),
    Some(Color32::from_rgb(0x4F, 0x7D, 0x4A)),
    Some(Color32::from_rgb(0xD0, 0x8C, 0x3C)),
    Some(Color32::from_rgb(0xC8, 0x56, 0x6E)),
    Some(Color32::from_rgb(0x8A, 0x5C, 0xC2)),
];

/// Accent colors for the "Özel" accent.
const ACCENTS: [Option<Color32>; 7] = [
    Some(FALLBACK_ACCENT),
    Some(Color32::from_rgb(0x3D, 0x8B, 0xFF)),
    Some(Color32::from_rgb(0x18, 0xB4, 0xA6)),
    Some(Color32::from_rgb(0x3D, 0xBE, 0x6A)),
    Some(Color32::from_rgb(0xFF, 0x8A, 0x3D)),
    Some(Color32::from_rgb(0xFF, 0x4F, 0x8B)),
    Some(Color32::from_rgb(0xF0, 0x4A, 0x4A)),
];

/// Glass opacity slider range, in percent.
const OPACITY_MIN: u8 = 40;
const OPACITY_MAX: u8 = 100;

/// The gear tile. Clicking it opens the settings page (handled by the app).
pub struct SettingsTile;

impl Module for SettingsTile {
    fn id(&self) -> &'static str {
        SETTINGS_ID
    }

    fn name(&self) -> &str {
        "Ayarlar"
    }

    fn hint(&self) -> Option<&str> {
        Some("Görünüm ve davranış")
    }

    fn icon(&self) -> &'static str {
        icons::GEAR_SIX
    }

    fn size_hint(&self) -> TileSize {
        TileSize::Small
    }

    fn can_hide(&self) -> bool {
        false
    }

    fn ui_tile(&mut self, ui: &mut Ui, ctx: &mut TileCtx<'_>) {
        let c = ui.max_rect().center();
        let color = if ctx.hovered {
            ctx.theme.text
        } else {
            ctx.theme.text_dim
        };
        paint::icon(
            ui.painter(),
            c,
            icons::GEAR_SIX,
            SIZE_ICON + 2.0,
            color,
            false,
        );
    }
}

#[derive(Default)]
pub struct PageOutput {
    pub back: bool,
    pub changed: bool,
    pub startup_toggled: bool,
    /// "Eklentileri yönet" was clicked.
    pub open_plugins: bool,
}

/// The settings page, laid out in the expanded panel.
pub fn page(
    ui: &mut Ui,
    cctx: &ContentCtx,
    theme: &Theme,
    cfg: &mut Config,
) -> PageOutput {
    let mut out = PageOutput::default();
    let ctx = ui.ctx().clone();
    let alpha = cctx.alpha * cctx.stagger(0);
    if alpha <= 0.002 {
        return out;
    }
    let panel = cctx.panel.shrink(PANEL_PADDING);
    let area = Area::new(Id::new("zenith-settings"))
        .order(Order::Middle)
        .fixed_pos(panel.min)
        .movable(false)
        .constrain(false)
        .sense(Sense::hover())
        .show(&ctx, |ui| {
            ui.multiply_opacity(alpha);
            ui.set_min_size(panel.size());

            // Header: back button + title.
            let header = Rect::from_min_size(panel.min, vec2(panel.width(), 28.0));
            let mut hui = ui.new_child(
                UiBuilder::new()
                    .max_rect(header)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            if widgets::icon_button(&mut hui, icons::ARROW_LEFT, 28.0, false, theme).clicked() {
                out.back = true;
            }
            hui.add_space(4.0);
            hui.label(
                egui::RichText::new("Ayarlar")
                    .font(Theme::font_semibold(SIZE_BODY + 1.0))
                    .color(theme.text),
            );
            version_header(ui, header, theme);

            // Appearance and Behavior side by side, then the collapsible
            // Customize and Advanced cards.
            let top = Rect::from_min_size(
                pos2(panel.min.x, header.max.y + GAP),
                vec2(panel.width(), TOP_CARD),
            );
            let col_w = (top.width() - GAP) * 0.5;
            let left = Rect::from_min_size(top.min, vec2(col_w, top.height()));
            let right = Rect::from_min_size(
                pos2(top.min.x + col_w + GAP, top.min.y),
                vec2(col_w, top.height()),
            );

            widgets::card(ui.painter(), left, theme);
            widgets::card(ui.painter(), right, theme);

            let mut lui = card_ui(ui, left);
            section_title(&mut lui, "Görünüm", theme);
            // The three dark languages, plus the light palette.
            let mut style_idx = if cfg.theme == ThemeMode::Light {
                3
            } else {
                Style::ALL.iter().position(|&s| s == cfg.style).unwrap_or(1)
            };
            if row(&mut lui, "Stil", theme, |ui| {
                widgets::segmented(ui, &["Horizon", "Night", "Mono", "Açık"], &mut style_idx, theme)
            }) {
                if style_idx == 3 {
                    cfg.theme = ThemeMode::Light;
                } else {
                    cfg.style = Style::ALL[style_idx];
                    cfg.theme = ThemeMode::Dark;
                }
                out.changed = true;
            }
            let mut accent_idx = match cfg.accent {
                AccentSource::Theme => 0,
                AccentSource::Wallpaper => 1,
                AccentSource::System => 2,
                AccentSource::Custom => 3,
            };
            if row(&mut lui, "Vurgu", theme, |ui| {
                widgets::segmented(
                    ui,
                    &["Stil", "Duvar kâğıdı", "Windows", "Özel"],
                    &mut accent_idx,
                    theme,
                )
            }) {
                cfg.accent = [
                    AccentSource::Theme,
                    AccentSource::Wallpaper,
                    AccentSource::System,
                    AccentSource::Custom,
                ][accent_idx];
                if parse_hex(&cfg.accent_hex).is_none() {
                    cfg.accent_hex = to_hex(FALLBACK_ACCENT);
                }
                out.changed = true;
            }
            let mut pos_idx = Placement::ALL
                .iter()
                .position(|&p| p == cfg.placement)
                .unwrap_or(1);
            if row(&mut lui, "Konum", theme, |ui| {
                widgets::segmented(ui, &["Sol", "Orta", "Sağ"], &mut pos_idx, theme)
            }) {
                cfg.placement = Placement::ALL[pos_idx];
                out.changed = true;
            }
            let mut hide = cfg.hide_in_fullscreen;
            if row(&mut lui, "Tam ekranda gizle", theme, |ui| {
                widgets::toggle(ui, &mut hide, theme)
                    .on_hover_text("Oyun ya da video tam ekrandayken çentiği gizler")
            }) {
                cfg.hide_in_fullscreen = hide;
                out.changed = true;
            }

            let mut rui = card_ui(ui, right);
            section_title(&mut rui, "Davranış", theme);
            let mut mon_idx = match cfg.monitor {
                MonitorMode::Primary => 0,
                MonitorMode::Cursor => 1,
            };
            if row(&mut rui, "Ekran", theme, |ui| {
                widgets::segmented(ui, &["Birincil", "İmleci izle"], &mut mon_idx, theme)
            }) {
                cfg.monitor = [MonitorMode::Primary, MonitorMode::Cursor][mon_idx];
                out.changed = true;
            }
            const DELAYS: [u32; 3] = [60, 120, 250];
            let mut delay_idx = DELAYS
                .iter()
                .position(|&d| d == cfg.hover_delay_ms)
                .unwrap_or(1);
            if row(&mut rui, "Açılma gecikmesi", theme, |ui| {
                widgets::segmented(ui, &["Hızlı", "Normal", "Sakin"], &mut delay_idx, theme)
            }) {
                cfg.hover_delay_ms = DELAYS[delay_idx];
                out.changed = true;
            }
            let mut on_top = cfg.on_top;
            if row(&mut rui, "Her zaman üstte", theme, |ui| {
                widgets::toggle(ui, &mut on_top, theme)
            }) {
                cfg.on_top = on_top;
                out.changed = true;
            }
            let mut startup = cfg.launch_at_startup;
            if row(&mut rui, "Açılışta başlat", theme, |ui| {
                widgets::toggle(ui, &mut startup, theme)
            }) {
                cfg.launch_at_startup = startup;
                out.startup_toggled = true;
                out.changed = true;
            }

            let open = open_section(&ctx);
            let mut y = top.max.y + GAP;
            for section in [CUSTOMIZE, ADVANCED] {
                let t = reveal(&ctx, section, open == Some(section));
                let card = Rect::from_min_size(
                    pos2(panel.min.x, y),
                    vec2(panel.width(), DISCLOSURE + BODY[section] * t),
                );
                y = card.max.y + GAP;
                widgets::card(ui.painter(), card, theme);
                let head = Rect::from_min_size(card.min, vec2(card.width(), DISCLOSURE));
                let (glyph, title, summary) = match section {
                    CUSTOMIZE => (icons::PALETTE, "Özelleştir", customize_summary(cfg)),
                    _ => (icons::SLIDERS_HORIZONTAL, "Gelişmiş", advanced_summary(cfg)),
                };
                if widgets::disclosure_header(ui, head, glyph, title, &summary, t, theme).clicked()
                {
                    let next = (open != Some(section)).then_some(section);
                    set_open_section(&ctx, next);
                }
                if section == ADVANCED {
                    manage_plugins_button(ui, head, theme, &mut out);
                }
                if t > 0.01 {
                    // Hairline under the header, then the controls fading
                    // in and settling down from slightly above.
                    let fade = t.clamp(0.0, 1.0);
                    ui.painter().hline(
                        (card.min.x + 14.0)..=(card.max.x - 14.0),
                        head.max.y,
                        Stroke::new(1.0, with_alpha(theme.tile_rim, fade)),
                    );
                    let body = Rect::from_min_size(
                        pos2(card.min.x, head.max.y - 6.0 * (1.0 - fade)),
                        vec2(card.width(), BODY[section]),
                    );
                    let mut bui = ui.new_child(UiBuilder::new().max_rect(body));
                    bui.set_clip_rect(card.intersect(ui.clip_rect()));
                    bui.multiply_opacity(fade * fade);
                    match section {
                        CUSTOMIZE => customize_body(&mut bui, body, theme, cfg, &mut out),
                        _ => advanced_body(&mut bui, body, theme, cfg, &mut out),
                    }
                }
            }
        });
    let slide = 8.0 * (1.0 - cctx.stagger(0));
    ctx.set_transform_layer(
        area.response.layer_id,
        cctx.transform * egui::emath::TSTransform::from_translation(vec2(0.0, slide)),
    );
    out
}

/// Right end of the header: the running version.
fn version_header(ui: &mut Ui, header: Rect, theme: &Theme) {
    ui.painter().text(
        pos2(header.max.x, header.center().y),
        Align2::RIGHT_CENTER,
        format!("Zenith {}", env!("CARGO_PKG_VERSION")),
        Theme::font(SIZE_LABEL),
        theme.text_faint,
    );
}

/// One line beside the Customize title while it is collapsed.
fn customize_summary(cfg: &Config) -> String {
    let material = match cfg.material {
        Material::Acrylic => format!("Cam %{}", cfg.glass_opacity),
        Material::Solid => "Düz".to_owned(),
    };
    let tint = if parse_hex(&cfg.background_hex).is_some() {
        "tinted"
    } else {
        "nötr"
    };
    let grain = if cfg.grain == 0 {
        "gren yok".to_owned()
    } else {
        format!("gren %{}", cfg.grain)
    };
    format!("{material} · {tint} · {grain}")
}

fn advanced_summary(cfg: &Config) -> String {
    let on = |b: bool| if b { "açık" } else { "kapalı" };
    format!(
        "Geliştirici modu {}",
        on(cfg.plugin_dev)
    )
}

/// Material, background tint, glass opacity, accent color and film grain,
/// in two columns.
fn customize_body(ui: &mut Ui, rect: Rect, theme: &Theme, cfg: &mut Config, out: &mut PageOutput) {
    let inner = rect.shrink2(vec2(14.0, 6.0));
    let gap = 20.0;
    let col_w = (inner.width() - gap) * 0.5;
    let left = Rect::from_min_size(inner.min, vec2(col_w, inner.height()));
    let right = Rect::from_min_size(
        pos2(inner.min.x + col_w + gap, inner.min.y),
        vec2(col_w, inner.height()),
    );
    let column = |ui: &mut Ui, r: Rect| {
        ui.new_child(
            UiBuilder::new()
                .max_rect(r)
                .layout(Layout::top_down(Align::Min)),
        )
    };

    let mut lui = column(ui, left);
    let mut mat_idx = match cfg.material {
        Material::Acrylic => 0,
        Material::Solid => 1,
    };
    if row(&mut lui, "Malzeme", theme, |ui| {
        widgets::segmented(ui, &["Cam", "Düz"], &mut mat_idx, theme)
    }) {
        cfg.material = [Material::Acrylic, Material::Solid][mat_idx];
        out.changed = true;
    }
    let mut bg_idx = match parse_hex(&cfg.background_hex) {
        None => Some(0),
        Some(c) => BACKGROUNDS.iter().position(|b| *b == Some(c)),
    };
    if row(&mut lui, "Arka plan", theme, |ui| {
        widgets::swatches(ui, &BACKGROUNDS, &mut bg_idx, theme)
    }) && let Some(i) = bg_idx
    {
        cfg.background_hex = BACKGROUNDS[i].map(to_hex).unwrap_or_default();
        out.changed = true;
    }
    let glass = cfg.material == Material::Acrylic;
    // With Windows "Transparency effects" off there is no blur to see
    // through, so the glass is drawn opaque and opacity has no effect. Only
    // read while this card is open.
    let transparency = zenith_platform::system::transparency_enabled();
    let span = (OPACITY_MAX - OPACITY_MIN) as f32;
    let mut t = (cfg.glass_opacity.clamp(OPACITY_MIN, OPACITY_MAX) - OPACITY_MIN) as f32 / span;
    if row(&mut lui, "Cam opaklığı", theme, |ui| {
        if !glass || !transparency {
            // The solid material is always opaque.
            ui.disable();
        }
        let resp = widgets::slider(ui, &mut t, 110.0, 12, theme);
        value_label(ui, &format!("{}%", cfg.glass_opacity), theme);
        resp
    }) {
        cfg.glass_opacity = OPACITY_MIN + (t * span).round() as u8;
        out.changed = true;
    }
    let mut accent_idx = match cfg.accent {
        AccentSource::Custom => {
            parse_hex(&cfg.accent_hex).and_then(|c| ACCENTS.iter().position(|a| *a == Some(c)))
        }
        _ => None,
    };
    if row(&mut lui, "Vurgu rengi", theme, |ui| {
        widgets::swatches(ui, &ACCENTS, &mut accent_idx, theme)
    }) && let Some(Some(c)) = accent_idx.map(|i| ACCENTS[i])
    {
        cfg.accent = AccentSource::Custom;
        cfg.accent_hex = to_hex(c);
        out.changed = true;
    }

    let mut rui = column(ui, right);
    let mut grain = cfg.grain.min(100) as f32 / 100.0;
    if row(&mut rui, "Gren", theme, |ui| {
        let resp = widgets::slider(ui, &mut grain, 110.0, 20, theme);
        let text = if cfg.grain == 0 {
            "Kapalı".to_owned()
        } else {
            format!("{}%", cfg.grain)
        };
        value_label(ui, &text, theme);
        resp
    }) {
        cfg.grain = (grain * 100.0).round() as u8;
        out.changed = true;
    }
    let mut size_idx = GrainSize::ALL
        .iter()
        .position(|&s| s == cfg.grain_size)
        .unwrap_or(0);
    if row(&mut rui, "Gren boyutu", theme, |ui| {
        if cfg.grain == 0 {
            ui.disable();
        }
        widgets::segmented(ui, &["İnce", "Orta", "Kalın"], &mut size_idx, theme)
    }) {
        cfg.grain_size = GrainSize::ALL[size_idx];
        out.changed = true;
    }
    let w = rui.available_width();
    let (reset_row, _) = rui.allocate_exact_size(vec2(w, ROW), Sense::hover());
    let reset = Rect::from_min_size(
        pos2(reset_row.max.x - 120.0, reset_row.center().y - 13.0),
        vec2(120.0, 26.0),
    );
    if zenith_modules::kit::button(
        ui,
        reset,
        "Stili sıfırla",
        Some(icons::ARROW_COUNTER_CLOCKWISE),
        false,
        theme,
    )
    .clicked()
    {
        cfg.reset_look();
        out.changed = true;
    }

    if !transparency {
        let (note, _) = rui.allocate_exact_size(vec2(w, ROW), Sense::hover());
        ui.painter().text(
            pos2(note.min.x, note.center().y),
            Align2::LEFT_CENTER,
            "Windows saydamlığı kapalı",
            Theme::font(SIZE_LABEL),
            theme.text_dim,
        );
        let turn_on = Rect::from_min_size(
            pos2(note.max.x - 96.0, note.center().y - 13.0),
            vec2(96.0, 26.0),
        );
        if zenith_modules::kit::button(ui, turn_on, "Aç", Some(icons::ARROW_SQUARE_OUT), false, theme)
            .on_hover_text(
                "Cam ve opaklık için Windows Ayarlar > Kişiselleştirme > Renkler > Saydamlık efektleri açık olmalı",
            )
            .clicked()
            && let Err(e) = zenith_platform::shell::open("ms-settings:colors")
        {
            tracing::warn!("opening color settings: {e}");
        }
    }
}

/// Small dim value text next to a slider (right-to-left row layout).
fn value_label(ui: &mut Ui, text: &str, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(vec2(32.0, 20.0), Sense::hover());
    ui.painter().text(
        pos2(rect.max.x - 4.0, rect.center().y),
        Align2::RIGHT_CENTER,
        text,
        Theme::font(SIZE_LABEL),
        theme.text_dim,
    );
}

/// "Eklentileri yönet" at the right of the Advanced header, left of the chevron.
fn manage_plugins_button(ui: &mut Ui, head: Rect, theme: &Theme, out: &mut PageOutput) {
    let manage = Rect::from_min_size(
        pos2(head.max.x - 44.0 - 132.0, head.center().y - 13.0),
        vec2(132.0, 26.0),
    );
    if zenith_modules::kit::button(
        ui,
        manage,
        "Eklentileri yönet",
        Some(icons::PUZZLE_PIECE),
        false,
        theme,
    )
    .clicked()
    {
        out.open_plugins = true;
    }
}

/// Plugin dev mode and the renderer on the left, the log on the right.
fn advanced_body(ui: &mut Ui, rect: Rect, theme: &Theme, cfg: &mut Config, out: &mut PageOutput) {
    let inner = rect.shrink2(vec2(14.0, 6.0));
    let gap = 20.0;
    let col_w = (inner.width() - gap) * 0.5;
    let mut lui = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(inner.min, vec2(col_w, inner.height())))
            .layout(Layout::top_down(Align::Min)),
    );
    let mut rui = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                pos2(inner.min.x + col_w + gap, inner.min.y),
                vec2(col_w, inner.height()),
            ))
            .layout(Layout::top_down(Align::Min)),
    );
    let mut dev = cfg.plugin_dev;
    if row(&mut lui, "Eklenti geliştirici modu", theme, |ui| {
        widgets::toggle(ui, &mut dev, theme)
    }) {
        cfg.plugin_dev = dev;
        out.changed = true;
    }
    hint(&mut lui, "Dosyaları kaydedilince eklentiyi yeniden başlatır", theme);

    let mut renderer_idx = match cfg.renderer {
        Renderer::Auto => 0,
        Renderer::DirectX => 1,
        Renderer::OpenGl => 2,
    };
    if row(&mut lui, "Çizim motoru", theme, |ui| {
        widgets::segmented(ui, &["Otomatik", "DirectX", "OpenGL"], &mut renderer_idx, theme)
            .on_hover_text("Panel titrerse OpenGL'i, siyah kutu görünürse DirectX'i dene")
    }) {
        cfg.renderer = [Renderer::Auto, Renderer::DirectX, Renderer::OpenGl][renderer_idx];
        out.changed = true;
    }
    hint(
        &mut lui,
        &format!("Kullanılan: {} · yeniden başlatınca uygulanır", crate::diag::active_renderer()),
        theme,
    );

    // "Günlüğü aç" at the right, level with the first row.
    let w = rui.available_width();
    let (log_row, _) = rui.allocate_exact_size(vec2(w, ROW), Sense::hover());
    let open_log = Rect::from_min_size(
        pos2(log_row.max.x - 120.0, log_row.center().y - 13.0),
        vec2(120.0, 26.0),
    );
    if zenith_modules::kit::button(ui, open_log, "Günlüğü aç", Some(icons::FILE_TEXT), false, theme)
        .on_hover_text("Hata bildirimine eklemek için zenith.log'u Gezgin'de gösterir")
        .clicked()
        && let Some(path) = crate::diag::log_path()
        && let Err(e) = zenith_platform::shell::reveal(&path)
    {
        tracing::warn!("revealing log: {e}");
    }
    let mut hide_osd = cfg.hide_system_osd;
    if row(&mut rui, "Windows ses penceresini gizle", theme, |ui| {
        widgets::toggle(ui, &mut hide_osd, theme)
    }) {
        cfg.hide_system_osd = hide_osd;
        out.changed = true;
    }
    hint(&mut rui, "Ses tuşlarını Zenith karşılar, gösterge çentikte çıkar", theme);
}

/// Small faint line under a row.
fn hint(ui: &mut Ui, text: &str, theme: &Theme) {
    ui.label(
        egui::RichText::new(text)
            .font(Theme::font(SIZE_LABEL - 1.0))
            .color(theme.text_faint),
    );
}

fn card_ui(ui: &mut Ui, rect: Rect) -> Ui {
    ui.new_child(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(14.0, 10.0)))
            .layout(Layout::top_down(Align::Min)),
    )
}

fn section_title(ui: &mut Ui, text: &str, theme: &Theme) {
    ui.label(
        egui::RichText::new(zenith_ui::theme::upper_tr(text))
            .font(Theme::font_semibold(SIZE_LABEL - 1.0))
            .color(theme.text_faint),
    );
    ui.add_space(2.0);
}

/// Label on the left, control on the right. Returns whether it changed.
fn row(
    ui: &mut Ui,
    label: &str,
    theme: &Theme,
    control: impl FnOnce(&mut Ui) -> egui::Response,
) -> bool {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, ROW), Sense::hover());
    ui.painter().text(
        pos2(rect.min.x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        Theme::font(SIZE_BODY),
        theme.text,
    );
    let mut cui = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    control(&mut cui).changed()
}
