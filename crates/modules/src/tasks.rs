//! Tasks: a quick to-do list in one tile. Type and press Enter to add,
//! click the circle to tick one off, right-click to delete. Open tasks come
//! first, newest on top; done ones sink, struck through. Saved in
//! `tasks.txt` next to the other settings, one task per line (`- ` open,
//! `x ` done).

use crate::kit;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Align2, Id, Key, Rect, Sense, Stroke, Ui, pos2, vec2};
use zenith_ui::bento::tile_header;
use zenith_ui::icons;
use zenith_ui::module::{Module, TileCtx, TileSize};
use zenith_ui::paint;
use zenith_ui::theme::{SIZE_BODY, SIZE_LABEL, Theme, with_alpha};

const FILE: &str = "tasks.txt";
const ROW: f32 = 24.0;

#[derive(Clone, Debug, PartialEq)]
struct Task {
    text: String,
    done: bool,
}

pub struct Tasks {
    tasks: Vec<Task>,
    input: String,
}

impl Default for Tasks {
    fn default() -> Self {
        Self {
            tasks: load(),
            input: String::new(),
        }
    }
}

fn load() -> Vec<Task> {
    let Some(path) = kit::settings_path(FILE) else {
        return Vec::new();
    };
    std::fs::read_to_string(path)
        .map(|s| parse(&s))
        .unwrap_or_default()
}

fn parse(text: &str) -> Vec<Task> {
    text.lines()
        .filter_map(|l| {
            let (done, rest) = match l.split_at_checked(2) {
                Some(("x ", r)) => (true, r),
                Some(("- ", r)) => (false, r),
                _ => (false, l),
            };
            let rest = rest.trim();
            (!rest.is_empty()).then(|| Task {
                text: rest.to_owned(),
                done,
            })
        })
        .collect()
}

fn serialize(tasks: &[Task]) -> String {
    tasks
        .iter()
        .map(|t| format!("{} {}\n", if t.done { "x" } else { "-" }, t.text.replace('\n', " ")))
        .collect()
}

impl Tasks {
    fn save(&self) {
        let Some(path) = kit::settings_path(FILE) else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, serialize(&self.tasks)) {
            tracing::warn!("saving tasks: {e}");
        }
    }

    /// Indices in display order: open first, then done.
    fn order(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.tasks.len()).collect();
        idx.sort_by_key(|&i| self.tasks[i].done);
        idx
    }
}

impl Module for Tasks {
    fn id(&self) -> &'static str {
        "tasks"
    }

    fn name(&self) -> &str {
        "Görevler"
    }

    fn icon(&self) -> &'static str {
        icons::CHECK_SQUARE
    }

    fn size_hint(&self) -> TileSize {
        TileSize::Wide
    }

    fn ui_tile(&mut self, ui: &mut Ui, ctx: &mut TileCtx<'_>) {
        let theme = ctx.theme;
        let rect = ui.max_rect();
        tile_header(ui, icons::CHECK_SQUARE, self.name(), false, theme);
        let done = self.tasks.iter().filter(|t| t.done).count();
        if !self.tasks.is_empty() {
            ui.painter().text(
                pos2(rect.max.x, rect.min.y + 6.0),
                Align2::RIGHT_CENTER,
                format!("{done}/{}", self.tasks.len()),
                Theme::font(SIZE_LABEL),
                theme.text_faint,
            );
        }

        // Input at the bottom.
        let field = Rect::from_min_max(pos2(rect.min.x, rect.max.y - 28.0), rect.max);
        let field_id = Id::new("zenith-tasks-input");
        let resp = kit::field(
            ui,
            field,
            &mut self.input,
            "Görev ekle ve Enter'a bas",
            field_id,
            Theme::font(SIZE_LABEL + 1.0),
            theme,
        );
        if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            let text = self.input.trim().to_owned();
            if !text.is_empty() {
                self.tasks.insert(0, Task { text, done: false });
                self.input.clear();
                self.save();
                // Keep typing the next one.
                ui.memory_mut(|m| m.request_focus(field_id));
            }
        }

        // The list.
        let top = rect.min.y + 22.0;
        let room = ((field.min.y - 6.0 - top) / ROW).floor().max(0.0) as usize;
        let order = self.order();
        if order.is_empty() {
            ui.painter().text(
                pos2(rect.min.x, top + ROW * 0.5),
                Align2::LEFT_CENTER,
                "Yapılacak bir şey yok",
                Theme::font(SIZE_LABEL + 1.0),
                theme.text_faint,
            );
        }
        let mut toggle = None;
        let mut delete = None;
        for (slot, &i) in order.iter().take(room).enumerate() {
            let row = Rect::from_min_size(
                pos2(rect.min.x, top + slot as f32 * ROW),
                vec2(rect.width(), ROW),
            );
            let r = ui
                .interact(row, Id::new(("zenith-task", i)), Sense::click())
                .on_hover_text("Tıkla: tamamla · sağ tık: sil");
            if r.hovered() {
                ui.painter()
                    .rect_filled(row.expand2(vec2(4.0, 0.0)), 6.0, with_alpha(theme.text, 0.06));
            }
            let task = &self.tasks[i];
            let c = pos2(row.min.x + 8.0, row.center().y);
            if task.done {
                ui.painter().circle_filled(c, 7.0, theme.accent);
                paint::icon(ui.painter(), c, icons::CHECK, 10.0, kit::on_accent(), true);
            } else {
                ui.painter().circle_stroke(
                    c,
                    6.5,
                    Stroke::new(1.5, if r.hovered() { theme.accent } else { theme.text_dim }),
                );
            }
            let color = if task.done { theme.text_faint } else { theme.text };
            let mut format = TextFormat::simple(Theme::font(SIZE_BODY), color);
            if task.done {
                format.strikethrough = Stroke::new(1.0, theme.text_faint);
            }
            let mut job = LayoutJob::single_section(task.text.clone(), format);
            job.wrap = TextWrapping {
                max_width: row.width() - 26.0,
                max_rows: 1,
                break_anywhere: true,
                overflow_character: Some('…'),
            };
            let galley = ui.painter().layout_job(job);
            let pos = pos2(row.min.x + 22.0, row.center().y - galley.size().y * 0.5);
            ui.painter().galley(pos, galley, color);
            if r.clicked() {
                toggle = Some(i);
            }
            if r.secondary_clicked() {
                delete = Some(i);
            }
        }
        let hidden = order.len().saturating_sub(room);
        if hidden > 0 {
            ui.painter().text(
                pos2(rect.max.x, field.min.y - 4.0 - ROW * 0.5 + ROW),
                Align2::RIGHT_BOTTOM,
                format!("+{hidden}"),
                Theme::font(SIZE_LABEL - 1.0),
                theme.text_faint,
            );
        }
        if let Some(i) = toggle {
            self.tasks[i].done = !self.tasks[i].done;
            self.save();
        }
        if let Some(i) = delete {
            self.tasks.remove(i);
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_file() {
        let tasks = parse("- süt al\nx faturayı öde\n\nplain line\n");
        assert_eq!(
            tasks,
            vec![
                Task { text: "süt al".into(), done: false },
                Task { text: "faturayı öde".into(), done: true },
                Task { text: "plain line".into(), done: false },
            ]
        );
        assert_eq!(parse(&serialize(&tasks)), tasks);
    }
}
