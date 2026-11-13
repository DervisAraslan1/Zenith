//! Weather: current conditions and a six-day forecast for one city, from
//! Open-Meteo (no account, no key). A worker thread asks every 30 minutes
//! and only sends the city's coordinates; the page searches places through
//! Open-Meteo's geocoding (only the typed name is sent). The city lives in
//! `weather.txt` next to the other settings (`city`, `lat`, `lon`);
//! Istanbul until changed.

use crate::kit;
use egui::{Align2, Id, Key, Rect, Sense, Ui, pos2, vec2};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;
use zenith_ui::icons;
use zenith_ui::module::{Module, TileCtx, TileSize};
use zenith_ui::paint;
use zenith_ui::theme::{SIZE_BODY, SIZE_LABEL, Theme, with_alpha};
use zenith_ui::widgets;

const REFRESH: Duration = Duration::from_secs(30 * 60);
const RETRY: Duration = Duration::from_secs(2 * 60);
const WEEKDAYS: [&str; 7] = ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"];

#[derive(Clone, Debug, PartialEq)]
struct Day {
    label: String,
    max: f32,
    min: f32,
    code: u32,
}

#[derive(Clone, Debug, PartialEq)]
struct Forecast {
    temp: f32,
    code: u32,
    day: bool,
    days: Vec<Day>,
}

#[derive(Clone, Debug)]
struct Place {
    name: String,
    area: String,
    lat: f64,
    lon: f64,
}

pub struct Weather {
    city: String,
    lat: f64,
    lon: f64,
    now: Option<Forecast>,
    error: bool,
    rx: Option<Receiver<Option<Forecast>>>,
    coords_tx: Option<Sender<(f64, f64)>>,
    query: String,
    results: Vec<Place>,
    search_rx: Option<Receiver<Result<Vec<Place>, String>>>,
    search_note: Option<String>,
    open_page: bool,
    ctx: Option<egui::Context>,
}

impl Default for Weather {
    fn default() -> Self {
        let mut w = Self {
            city: "İstanbul".into(),
            lat: 41.01,
            lon: 28.98,
            now: None,
            error: false,
            rx: None,
            coords_tx: None,
            query: String::new(),
            results: Vec::new(),
            search_rx: None,
            search_note: None,
            open_page: false,
            ctx: None,
        };
        for (k, v) in kit::read_settings("weather.txt") {
            match k.as_str() {
                "city" if !v.trim().is_empty() => w.city = v.trim().to_owned(),
                "lat" => w.lat = v.trim().parse().unwrap_or(w.lat),
                "lon" => w.lon = v.trim().parse().unwrap_or(w.lon),
                _ => {}
            }
        }
        w
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("Zenith/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Day of the week for a `YYYY-MM-DD` date (Sakamoto), 0 = Sunday.
fn weekday(date: &str) -> Option<usize> {
    let mut it = date.split('-').map(|p| p.parse::<i32>().ok());
    let (mut y, m, d) = (it.next()??, it.next()??, it.next()??);
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if m < 3 {
        y -= 1;
    }
    let idx = usize::try_from(m - 1).ok().filter(|&i| i < 12)?;
    Some(((y + y / 4 - y / 100 + y / 400 + T[idx] + d).rem_euclid(7)) as usize)
}

fn fetch(lat: f64, lon: f64) -> Result<Forecast, String> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.2}&longitude={lon:.2}\
         &current=temperature_2m,weather_code,is_day\
         &daily=weather_code,temperature_2m_max,temperature_2m_min\
         &forecast_days=6&timezone=auto"
    );
    let body: serde_json::Value = agent()
        .get(&url)
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    let cur = &body["current"];
    let daily = &body["daily"];
    let num = |v: &serde_json::Value, i: usize| v[i].as_f64().unwrap_or(0.0) as f32;
    let days = daily["time"]
        .as_array()
        .map(|times| {
            times
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let date = t.as_str().unwrap_or("");
                    Day {
                        label: if i == 0 {
                            "Bugün".to_owned()
                        } else {
                            weekday(date).map_or("—", |w| WEEKDAYS[w]).to_owned()
                        },
                        max: num(&daily["temperature_2m_max"], i),
                        min: num(&daily["temperature_2m_min"], i),
                        code: daily["weather_code"][i].as_u64().unwrap_or(0) as u32,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Forecast {
        temp: cur["temperature_2m"].as_f64().ok_or("sıcaklık yok")? as f32,
        code: cur["weather_code"].as_u64().unwrap_or(0) as u32,
        day: cur["is_day"].as_u64().unwrap_or(1) == 1,
        days,
    })
}

/// Percent-encodes a query parameter (UTF-8).
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn geocode(name: &str) -> Result<Vec<Place>, String> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=6&language=tr&format=json",
        encode(name.trim())
    );
    let body: serde_json::Value = agent()
        .get(&url)
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    Ok(body["results"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    let area = [r["admin1"].as_str(), r["country"].as_str()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(", ");
                    Some(Place {
                        name: r["name"].as_str()?.to_owned(),
                        area,
                        lat: r["latitude"].as_f64()?,
                        lon: r["longitude"].as_f64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

/// WMO weather code: a Turkish description and a Phosphor glyph.
fn describe(code: u32, day: bool) -> (&'static str, &'static str) {
    match code {
        0 => ("Açık", if day { icons::SUN } else { icons::MOON }),
        1 => ("Az bulutlu", if day { icons::SUN } else { icons::MOON }),
        2 => ("Parçalı bulutlu", if day { icons::CLOUD_SUN } else { icons::CLOUD_MOON }),
        3 => ("Kapalı", icons::CLOUD),
        45 | 48 => ("Sisli", icons::CLOUD_FOG),
        51..=57 => ("Çisenti", icons::CLOUD_RAIN),
        61..=67 | 80..=82 => ("Yağmurlu", icons::CLOUD_RAIN),
        71..=77 | 85 | 86 => ("Karlı", icons::CLOUD_SNOW),
        95..=99 => ("Fırtına", icons::CLOUD_LIGHTNING),
        _ => ("—", icons::CLOUD),
    }
}

impl Weather {
    fn search(&mut self) {
        let q = self.query.trim().to_owned();
        if q.chars().count() < 2 {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        self.search_note = Some("Aranıyor…".into());
        let ctx = self.ctx.clone();
        let _ = std::thread::Builder::new()
            .name("zenith-geocode".into())
            .spawn(move || {
                let _ = tx.send(geocode(&q));
                if let Some(c) = ctx {
                    c.request_repaint();
                }
            });
    }

    fn choose(&mut self, place: Place) {
        self.city = place.name;
        self.lat = place.lat;
        self.lon = place.lon;
        kit::write_settings(
            "weather.txt",
            &[
                ("city", self.city.clone()),
                ("lat", format!("{:.4}", self.lat)),
                ("lon", format!("{:.4}", self.lon)),
            ],
        );
        self.now = None;
        self.error = false;
        self.results.clear();
        self.query.clear();
        self.search_note = None;
        if let Some(tx) = &self.coords_tx {
            let _ = tx.send((self.lat, self.lon));
        }
    }
}

impl Module for Weather {
    fn id(&self) -> &'static str {
        "weather"
    }

    fn name(&self) -> &str {
        &self.city
    }

    fn icon(&self) -> &'static str {
        icons::CLOUD_SUN
    }

    fn size_hint(&self) -> TileSize {
        TileSize::Small
    }

    fn take_page_request(&mut self) -> bool {
        std::mem::take(&mut self.open_page)
    }

    fn attach(&mut self, ctx: &egui::Context) {
        self.ctx = Some(ctx.clone());
        let (tx, rx) = mpsc::channel();
        let (coords_tx, coords_rx) = mpsc::channel::<(f64, f64)>();
        self.rx = Some(rx);
        self.coords_tx = Some(coords_tx);
        let (mut lat, mut lon) = (self.lat, self.lon);
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("zenith-weather".into())
            .spawn(move || {
                loop {
                    let result = fetch(lat, lon);
                    if let Err(e) = &result {
                        tracing::warn!("weather: {e}");
                    }
                    let ok = result.is_ok();
                    if tx.send(result.ok()).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                    // Sleep until the next refresh, or until a new city.
                    match coords_rx.recv_timeout(if ok { REFRESH } else { RETRY }) {
                        Ok((la, lo)) => (lat, lon) = (la, lo),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::warn!("weather thread: {e}");
        }
    }

    fn on_tick(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(update) = rx.try_recv() {
                match update {
                    Some(now) => {
                        self.now = Some(now);
                        self.error = false;
                    }
                    None => self.error = self.now.is_none(),
                }
            }
        }
        if let Some(rx) = &self.search_rx
            && let Ok(result) = rx.try_recv()
        {
            match result {
                Ok(places) => {
                    self.search_note = places.is_empty().then(|| "Sonuç yok".to_owned());
                    self.results = places;
                }
                Err(e) => {
                    tracing::warn!("geocode: {e}");
                    self.search_note = Some("Bağlantı yok".into());
                    self.results.clear();
                }
            }
            self.search_rx = None;
        }
    }

    fn ui_tile(&mut self, ui: &mut Ui, ctx: &mut TileCtx<'_>) {
        let theme = ctx.theme;
        let rect = ui.max_rect();
        let resp = ui
            .interact(rect.expand(16.0), ui.id().with("weather-tile"), Sense::click())
            .on_hover_text("Şehri değiştir · 6 günlük tahmin");
        if resp.clicked() {
            self.open_page = true;
        }
        let painter = ui.painter();
        match &self.now {
            Some(now) => {
                let (text, glyph) = describe(now.code, now.day);
                painter.text(
                    pos2(rect.min.x - 2.0, rect.center().y - 4.0),
                    Align2::LEFT_CENTER,
                    format!("{}°", now.temp.round() as i32),
                    theme.numeral_font(38.0),
                    theme.text,
                );
                paint::icon(
                    painter,
                    pos2(rect.max.x - 12.0, rect.min.y - 11.0),
                    glyph,
                    18.0,
                    theme.accent_fg(),
                    false,
                );
                painter.text(
                    pos2(rect.min.x, rect.max.y - 6.0),
                    Align2::LEFT_CENTER,
                    text,
                    Theme::font(SIZE_LABEL + 1.0),
                    theme.text_dim,
                );
            }
            None => {
                painter.text(
                    pos2(rect.min.x, rect.center().y),
                    Align2::LEFT_CENTER,
                    if self.error { "Bağlantı yok" } else { "Yükleniyor…" },
                    Theme::font(SIZE_LABEL + 1.0),
                    theme.text_faint,
                );
            }
        }
    }

    fn ui_page(&mut self, ui: &mut Ui, _header: &mut Ui, body: Rect, theme: &Theme) -> bool {
        let left = Rect::from_min_size(body.min, vec2(300.0, body.height()));
        let painter = ui.painter().clone();

        // Now: big temperature, sky, place.
        match &self.now {
            Some(now) => {
                let (text, glyph) = describe(now.code, now.day);
                painter.text(
                    pos2(left.min.x, left.min.y + 30.0),
                    Align2::LEFT_CENTER,
                    format!("{}°", now.temp.round() as i32),
                    theme.numeral_font(60.0),
                    theme.text,
                );
                paint::icon(
                    &painter,
                    pos2(left.min.x + 150.0, left.min.y + 30.0),
                    glyph,
                    34.0,
                    theme.accent_fg(),
                    false,
                );
                painter.text(
                    pos2(left.min.x, left.min.y + 74.0),
                    Align2::LEFT_CENTER,
                    format!("{text} · {}", self.city),
                    Theme::font(SIZE_BODY + 1.0),
                    theme.text_dim,
                );
            }
            None => {
                painter.text(
                    pos2(left.min.x, left.min.y + 30.0),
                    Align2::LEFT_CENTER,
                    if self.error { "Bağlantı yok" } else { "Yükleniyor…" },
                    Theme::font(SIZE_BODY + 2.0),
                    theme.text_faint,
                );
            }
        }

        // Search.
        let field = Rect::from_min_size(pos2(left.min.x, left.min.y + 98.0), vec2(left.width(), 32.0));
        let resp = kit::field(
            ui,
            field,
            &mut self.query,
            "Şehir ara ve Enter'a bas",
            Id::new("zenith-weather-search"),
            Theme::font(SIZE_BODY),
            theme,
        );
        if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            self.search();
        }
        let mut y = field.max.y + 8.0;
        if let Some(note) = &self.search_note {
            painter.text(
                pos2(left.min.x + 4.0, y + 10.0),
                Align2::LEFT_CENTER,
                note,
                Theme::font(SIZE_LABEL + 1.0),
                theme.text_faint,
            );
            y += 22.0;
        }
        let mut chosen = None;
        for (i, place) in self.results.iter().enumerate() {
            let row = Rect::from_min_size(pos2(left.min.x, y), vec2(left.width(), 26.0));
            if row.max.y > body.max.y {
                break;
            }
            let r = ui.interact(row, Id::new(("zenith-weather-place", i)), Sense::click());
            if r.hovered() {
                painter.rect_filled(row, 8.0, with_alpha(theme.text, 0.08));
            }
            painter.text(
                pos2(row.min.x + 8.0, row.center().y),
                Align2::LEFT_CENTER,
                &place.name,
                Theme::font_semibold(SIZE_LABEL + 1.5),
                theme.text,
            );
            painter.text(
                pos2(row.max.x - 8.0, row.center().y),
                Align2::RIGHT_CENTER,
                &place.area,
                Theme::font(SIZE_LABEL),
                theme.text_faint,
            );
            if r.clicked() {
                chosen = Some(place.clone());
            }
            y += 28.0;
        }
        if let Some(p) = chosen {
            self.choose(p);
        }

        // Six days.
        if let Some(now) = &self.now
            && !now.days.is_empty()
        {
            let area = Rect::from_min_max(pos2(left.max.x + 28.0, body.min.y), body.max);
            let n = now.days.len() as f32;
            let gap = 10.0;
            let w = (area.width() - gap * (n - 1.0)) / n;
            let h = area.height().min(170.0);
            for (i, d) in now.days.iter().enumerate() {
                let card = Rect::from_min_size(
                    pos2(area.min.x + i as f32 * (w + gap), area.min.y),
                    vec2(w, h),
                );
                widgets::card(&painter, card, theme);
                let (_, glyph) = describe(d.code, true);
                let cx = card.center().x;
                painter.text(
                    pos2(cx, card.min.y + 22.0),
                    Align2::CENTER_CENTER,
                    &d.label,
                    Theme::font_semibold(SIZE_LABEL + 1.0),
                    if i == 0 { theme.accent_fg() } else { theme.text_dim },
                );
                paint::icon(&painter, pos2(cx, card.min.y + 62.0), glyph, 26.0, theme.text, false);
                painter.text(
                    pos2(cx, card.min.y + 104.0),
                    Align2::CENTER_CENTER,
                    format!("{}°", d.max.round() as i32),
                    theme.numeral_font(22.0),
                    theme.text,
                );
                painter.text(
                    pos2(cx, card.min.y + 130.0),
                    Align2::CENTER_CENTER,
                    format!("{}°", d.min.round() as i32),
                    Theme::font(SIZE_BODY),
                    theme.text_faint,
                );
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekdays_and_encoding() {
        assert_eq!(weekday("2026-11-13"), Some(5)); // a Friday
        assert_eq!(weekday("2024-02-29"), Some(4)); // a Thursday
        assert_eq!(encode("İzmir ş"), "%C4%B0zmir%20%C5%9F");
    }
}
