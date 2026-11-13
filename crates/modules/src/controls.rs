//! Volume and brightness: two sliders in one wide tile. Drag or scroll a
//! row to change it; the speaker toggles mute. Values come from
//! `zenith_platform::levels` (shared with the notch's indicator) and are set
//! on its worker thread, so dragging never blocks the UI.

use egui::{Align2, Id, Rect, Sense, Ui, pos2, vec2};
use std::time::{Duration, Instant};
use zenith_platform::levels::{self, SetLevel};
use zenith_ui::bento::tile_header;
use zenith_ui::icons;
use zenith_ui::module::{Module, TileCtx, TileSize};
use zenith_ui::paint;
use zenith_ui::theme::{SIZE_LABEL, Theme, with_alpha};

/// A dragged value wins over the polled one for this long, so the knob does
/// not jump back while Windows catches up.
const HOLD: Duration = Duration::from_millis(900);
const STEP: f32 = 0.02;

#[derive(Default)]
pub struct Controls {
    volume: Option<(f32, Instant)>,
    brightness: Option<(f32, Instant)>,
}

/// What a row did this frame.
struct RowOut {
    value: Option<f32>,
    icon_clicked: bool,
}

fn row(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    glyph: &str,
    value: Option<f32>,
    dim: bool,
    theme: &Theme,
) -> RowOut {
    let mut out = RowOut {
        value: None,
        icon_clicked: false,
    };
    let cy = rect.center().y;
    // Icon (a button for mute).
    let icon_rect = Rect::from_center_size(pos2(rect.min.x + 11.0, cy), vec2(26.0, 26.0));
    let icon = ui.interact(icon_rect, id.with("icon"), Sense::click());
    out.icon_clicked = icon.clicked();
    paint::icon(
        ui.painter(),
        icon_rect.center(),
        glyph,
        17.0,
        if dim {
            theme.text_faint
        } else if icon.hovered() {
            theme.text
        } else {
            theme.accent_fg()
        },
        false,
    );
    let value_x = rect.max.x;
    let Some(v) = value else {
        ui.painter().text(
            pos2(rect.min.x + 32.0, cy),
            Align2::LEFT_CENTER,
            "Bu ekranda desteklenmiyor",
            Theme::font(SIZE_LABEL),
            theme.text_faint,
        );
        return out;
    };
    ui.painter().text(
        pos2(value_x, cy),
        Align2::RIGHT_CENTER,
        format!("{}", (v * 100.0).round() as i32),
        theme.numeral_font(15.0),
        if dim { theme.text_faint } else { theme.text },
    );
    let track = Rect::from_min_max(pos2(rect.min.x + 32.0, cy - 3.0), pos2(value_x - 34.0, cy + 3.0));
    let hit = ui.interact(
        track.expand2(vec2(8.0, 12.0)),
        id.with("track"),
        Sense::click_and_drag(),
    );
    let mut shown = v;
    if (hit.dragged() || hit.clicked() || hit.drag_started())
        && let Some(p) = hit.interact_pointer_pos()
    {
        shown = ((p.x - track.min.x) / track.width()).clamp(0.0, 1.0);
        out.value = Some(shown);
    }
    // Scroll over the row: one notch, one step.
    if ui.rect_contains_pointer(rect) {
        let notches: f32 = ui.input(|i| {
            i.events
                .iter()
                .map(|e| match e {
                    egui::Event::MouseWheel { unit, delta, .. } => match unit {
                        egui::MouseWheelUnit::Line => delta.y,
                        egui::MouseWheelUnit::Point => delta.y / 40.0,
                        egui::MouseWheelUnit::Page => delta.y * 3.0,
                    },
                    _ => 0.0,
                })
                .sum()
        });
        if notches.abs() > 0.01 {
            shown = (shown + STEP * notches).clamp(0.0, 1.0);
            out.value = Some(shown);
        }
    }
    paint::progress_bar(
        ui.painter(),
        track,
        if dim { 0.0 } else { shown },
        theme,
        with_alpha(theme.text, 0.12),
    );
    // Knob.
    let kx = track.min.x + track.width() * shown;
    let r = if hit.hovered() || hit.dragged() { 8.0 } else { 7.0 };
    ui.painter().circle_filled(pos2(kx, cy), r, theme.text);
    ui.painter()
        .circle_filled(pos2(kx, cy), r - 3.0, if dim { theme.text_faint } else { theme.accent });
    out
}

impl Module for Controls {
    fn id(&self) -> &'static str {
        "controls"
    }

    fn name(&self) -> &str {
        "Ses ve parlaklık"
    }

    fn icon(&self) -> &'static str {
        icons::SLIDERS_HORIZONTAL
    }

    fn size_hint(&self) -> TileSize {
        TileSize::Wide
    }

    fn next_repaint(&self, tile_visible: bool) -> Option<Duration> {
        // Follow changes made elsewhere (keys, other apps) while visible.
        tile_visible.then(|| Duration::from_millis(250))
    }

    fn ui_tile(&mut self, ui: &mut Ui, ctx: &mut TileCtx<'_>) {
        let theme = ctx.theme;
        let rect = ui.max_rect();
        tile_header(ui, icons::SLIDERS_HORIZONTAL, self.name(), false, theme);
        let (vol, bright) = levels::current();
        let now = Instant::now();
        let held = |v: Option<(f32, Instant)>| v.filter(|(_, at)| now - *at < HOLD).map(|(x, _)| x);

        let body = Rect::from_min_max(pos2(rect.min.x, rect.min.y + 22.0), rect.max);
        let h = body.height() / 2.0;
        let vrow = Rect::from_min_size(body.min, vec2(body.width(), h));
        let brow = Rect::from_min_size(pos2(body.min.x, body.min.y + h), vec2(body.width(), h));

        // Volume.
        let muted = vol.is_some_and(|(_, m)| m);
        let vol_value = held(self.volume).or(vol.map(|(v, _)| v));
        let glyph = match (muted, vol_value) {
            (true, _) => icons::SPEAKER_X,
            (_, Some(v)) if v < 0.005 => icons::SPEAKER_X,
            (_, Some(v)) if v < 0.34 => icons::SPEAKER_LOW,
            _ => icons::SPEAKER_HIGH,
        };
        let out = row(ui, vrow, Id::new("zenith-controls-volume"), glyph, vol_value, muted, theme);
        if out.icon_clicked {
            levels::set_level(SetLevel::Mute(!muted));
        }
        if let Some(v) = out.value {
            self.volume = Some((v, now));
            levels::set_level(SetLevel::Volume(v));
        }

        // Brightness.
        let b_value = held(self.brightness).or(bright);
        let out = row(ui, brow, Id::new("zenith-controls-brightness"), icons::SUN, b_value, false, theme);
        if let Some(b) = out.value {
            self.brightness = Some((b, now));
            levels::set_level(SetLevel::Brightness(b));
        }
    }
}
