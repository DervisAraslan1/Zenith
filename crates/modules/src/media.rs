//! Media: now playing from any app that reports to Windows (Spotify,
//! browsers, the Media Player...) through SMTC, with album art, progress and
//! previous / play-pause / next. A note glyph shows in the pill while
//! something plays.
//!
//! The SMTC work runs on `zenith_platform::media`'s worker thread; this module
//! only drains its snapshots in `on_tick` and sends commands back.

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align2, Color32, ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Ui, pos2, vec2,
};
use zenith_platform::media::{MediaCommand, MediaWatcher, NowPlaying};
use zenith_ui::module::{Module, TileCtx, TileSize};
use zenith_ui::theme::{SIZE_LABEL, Theme, with_alpha};
use zenith_ui::{icons, paint};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
pub struct Media {
    watcher: Option<MediaWatcher>,
    now: Option<NowPlaying>,
    /// Texture for `now.art`, keyed by the art's allocation.
    art: Option<(usize, TextureHandle)>,
}

impl Media {
    fn send(&self, cmd: MediaCommand) {
        if let Some(w) = &self.watcher {
            w.send(cmd);
        }
    }

    fn art_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureId> {
        let art = self.now.as_ref()?.art.as_ref()?;
        let key = Arc::as_ptr(art) as usize;
        if self.art.as_ref().map(|(k, _)| *k) != Some(key) {
            let image =
                ColorImage::from_rgba_unmultiplied([art.width, art.height], art.rgba.as_slice());
            let tex = ctx.load_texture("zenith-media-art", image, TextureOptions::LINEAR);
            self.art = Some((key, tex));
        }
        self.art.as_ref().map(|(_, t)| t.id())
    }
}

/// Single-line text, elided with "…" to `width`.
fn text_line(ui: &Ui, pos: Pos2, text: &str, width: f32, font: egui::FontId, color: Color32) {
    let mut job = LayoutJob::single_section(text.to_owned(), TextFormat::simple(font, color));
    job.wrap = TextWrapping {
        max_width: width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(pos, galley, color);
}

fn mmss(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Rounded image: a squircle-ish clip is overkill at 44 px, a rounded rect
/// mesh reads the same.
fn rounded_image(ui: &Ui, rect: Rect, tex: egui::TextureId, radius: f32) {
    ui.painter().add(
        egui::epaint::RectShape::filled(rect, radius, Color32::WHITE)
            .with_texture(tex, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
    );
}

impl Module for Media {
    fn id(&self) -> &'static str {
        "media"
    }

    fn name(&self) -> &str {
        "Müzik"
    }

    fn icon(&self) -> &'static str {
        icons::MUSIC_NOTES
    }

    fn size_hint(&self) -> TileSize {
        TileSize::Large
    }

    fn is_active(&self) -> bool {
        self.now.as_ref().is_some_and(|n| n.playing)
    }

    fn attach(&mut self, ctx: &egui::Context) {
        let ctx = ctx.clone();
        match MediaWatcher::spawn(move || ctx.request_repaint()) {
            Ok(w) => self.watcher = Some(w),
            Err(e) => tracing::warn!("media: {e}"),
        }
    }

    fn on_tick(&mut self) {
        if let Some(snapshot) = self.watcher.as_ref().and_then(MediaWatcher::poll) {
            self.now = snapshot;
            if self.now.as_ref().and_then(|n| n.art.as_ref()).is_none() {
                self.art = None;
            }
        }
    }

    fn next_repaint(&self, tile_visible: bool) -> Option<Duration> {
        // The progress bar and time advance once a second while visible.
        let n = self.now.as_ref()?;
        (tile_visible && n.playing && n.timeline.is_some()).then(|| Duration::from_secs(1))
    }

    fn ui_tile(&mut self, ui: &mut Ui, ctx: &mut TileCtx<'_>) {
        use crate::kit::{progress_bar, round_button, tag_font};
        use zenith_ui::theme::{Style, upper_tr};
        let theme = ctx.theme;
        let rect = ui.max_rect();

        let Some(now) = self.now.clone() else {
            let c = rect.center();
            paint::icon(
                ui.painter(),
                c - vec2(0.0, 10.0),
                icons::MUSIC_NOTES,
                30.0,
                theme.text_faint,
                false,
            );
            ui.painter().text(
                c + vec2(0.0, 22.0),
                Align2::CENTER_CENTER,
                "Şu an çalan yok",
                Theme::font(SIZE_LABEL + 1.0),
                theme.text_dim,
            );
            return;
        };

        // Art: a disc with a soft accent halo (a square in Mono).
        let round = theme.style != Style::Mono;
        let art_d = (rect.width() * 0.34).clamp(60.0, 92.0);
        let art_rect = Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(art_d, art_d));
        let art_radius = if round { art_d * 0.5 } else { 6.0 };
        if round {
            ui.painter().circle_filled(
                art_rect.center(),
                art_d * 0.5 + 4.0,
                with_alpha(theme.accent, 0.16),
            );
        }
        match self.art_texture(ui.ctx()) {
            Some(tex) => rounded_image(ui, art_rect, tex, art_radius),
            None => {
                ui.painter()
                    .rect_filled(art_rect, art_radius, with_alpha(theme.text, 0.08));
                paint::icon(
                    ui.painter(),
                    art_rect.center(),
                    icons::MUSIC_NOTES,
                    art_d * 0.32,
                    theme.text_faint,
                    false,
                );
            }
        }

        // Beside the art: source tag, title, artist.
        let col = art_rect.max.x + 14.0;
        let col_w = (rect.max.x - col).max(10.0);
        let cy = art_rect.center().y;
        let title = if now.title.is_empty() {
            "Bilinmeyen parça"
        } else {
            &now.title
        };
        text_line(ui, pos2(col, cy - 30.0), &upper_tr(&app_name(&now.source)), col_w, tag_font(theme), theme.text_faint);
        text_line(ui, pos2(col, cy - 13.0), title, col_w, Theme::font_semibold(17.0), theme.text);
        text_line(ui, pos2(col, cy + 9.0), &now.artist, col_w, Theme::font(13.0), theme.text_dim);

        // Progress and times.
        let ctrl_y = rect.max.y - 25.0;
        if let Some((pos, dur)) = now.position() {
            let bar_y = ctrl_y - 50.0;
            let bar = Rect::from_min_size(pos2(rect.min.x, bar_y), vec2(rect.width(), 5.0));
            progress_bar(ui.painter(), bar, (pos / dur.max(1.0)) as f32, theme);
            let tf = if theme.style == Style::Mono {
                Theme::font_mono(SIZE_LABEL - 0.5)
            } else {
                Theme::font(SIZE_LABEL - 0.5)
            };
            ui.painter().text(pos2(rect.min.x, bar_y + 16.0), Align2::LEFT_CENTER, mmss(pos), tf.clone(), theme.text_faint);
            ui.painter().text(pos2(rect.max.x, bar_y + 16.0), Align2::RIGHT_CENTER, mmss(dur), tf, theme.text_faint);
        }

        // Transport.
        let cx = rect.center().x;
        if now.can_previous
            && round_button(ui, pos2(cx - 62.0, ctrl_y), 38.0, icons::SKIP_BACK, false, theme)
                .on_hover_text("Önceki")
                .clicked()
        {
            self.send(MediaCommand::Previous);
        }
        let (glyph, tip) = if now.playing {
            (icons::PAUSE, "Duraklat")
        } else {
            (icons::PLAY, "Çal")
        };
        if now.can_toggle
            && round_button(ui, pos2(cx, ctrl_y), 50.0, glyph, true, theme)
                .on_hover_text(tip)
                .clicked()
        {
            self.send(MediaCommand::PlayPause);
            // Flip right away; the next snapshot confirms it.
            if let Some(n) = &mut self.now {
                if let Some((pos, dur)) = n.position() {
                    n.timeline = Some((pos, dur));
                    n.captured = std::time::Instant::now();
                }
                n.playing = !n.playing;
            }
        }
        if now.can_next
            && round_button(ui, pos2(cx + 62.0, ctrl_y), 38.0, icons::SKIP_FORWARD, false, theme)
                .on_hover_text("Sonraki")
                .clicked()
        {
            self.send(MediaCommand::Next);
        }
    }
}

/// A readable app name from an SMTC source id: "operasoftware.operagx..."
/// -> "operagx", "Spotify" stays.
fn app_name(source: &str) -> String {
    let s = source.trim().trim_end_matches(".exe");
    // Skip empty and numeric parts ("OperaGX.1793482591" -> "OperaGX").
    let last = s
        .rsplit('.')
        .find(|p| !p.is_empty() && !p.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(s);
    let name = last.split(['_', '!']).next().unwrap_or(last);
    if name.is_empty() { s.to_owned() } else { name.to_owned() }
}
