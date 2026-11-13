//! Tools: a 2x2 cluster of shortcuts in one small tile (text extractor,
//! QR code, calculator, settings). A click asks the app to run that
//! module's action through [`Module::take_launch`].

use egui::{Rect, Sense, Ui, pos2, vec2};
use zenith_ui::icons;
use zenith_ui::module::{Module, TileCtx, TileSize};
use zenith_ui::paint;
use zenith_ui::theme::{Style, with_alpha};

const SHORTCUTS: [(&str, &str, &str); 4] = [
    ("ocr", icons::TEXT_T, "Metin çıkar"),
    ("qr_code", icons::QR_CODE, "QR kod"),
    ("calculator", icons::CALCULATOR, "Hesap makinesi"),
    ("settings", icons::GEAR_SIX, "Ayarlar"),
];

#[derive(Default)]
pub struct Tools {
    launch: Option<&'static str>,
}

impl Module for Tools {
    fn id(&self) -> &'static str {
        "tools"
    }

    fn name(&self) -> &str {
        "Araçlar"
    }

    fn icon(&self) -> &'static str {
        icons::SQUARES_FOUR
    }

    fn size_hint(&self) -> TileSize {
        TileSize::Small
    }

    fn show_label(&self) -> bool {
        false
    }

    fn take_launch(&mut self) -> Option<&'static str> {
        self.launch.take()
    }

    fn ui_tile(&mut self, ui: &mut Ui, ctx: &mut TileCtx<'_>) {
        let theme = ctx.theme;
        // The cluster uses the tile's whole face, padding included.
        let rect = ui.max_rect().expand(6.0);
        let gap = 8.0;
        let cell = vec2((rect.width() - gap) * 0.5, (rect.height() - gap) * 0.5);
        let radius = match theme.style {
            Style::Mono => 0.0,
            _ => (theme.tile_radius * 0.7).min(14.0),
        };
        for (i, (id, glyph, tip)) in SHORTCUTS.iter().enumerate() {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;
            let r = Rect::from_min_size(
                pos2(rect.min.x + col * (cell.x + gap), rect.min.y + row * (cell.y + gap)),
                cell,
            );
            let resp = ui
                .interact(r, ui.id().with(("tool", *id)), Sense::click())
                .on_hover_text(*tip);
            let fill = if resp.hovered() { 0.16 } else { 0.08 };
            ui.painter()
                .rect_filled(r, radius, with_alpha(theme.text, fill));
            if theme.style == Style::Mono {
                ui.painter().rect_stroke(
                    r,
                    0.0,
                    egui::Stroke::new(1.0, theme.tile_rim),
                    egui::StrokeKind::Inside,
                );
            }
            paint::icon(
                ui.painter(),
                r.center(),
                glyph,
                18.0,
                if resp.hovered() { theme.text } else { theme.text_dim },
                false,
            );
            if resp.clicked() {
                self.launch = Some(id);
            }
        }
    }
}
