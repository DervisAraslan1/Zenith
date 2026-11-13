//! Weather: current temperature and sky for one city, from Open-Meteo
//! (no account, no key). A worker thread asks every 30 minutes and only
//! sends the city's coordinates. The city lives in `weather.txt` next to
//! the other settings (`city`, `lat`, `lon`); Istanbul until changed.

use crate::kit;
use egui::{Align2, Ui, pos2};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use zenith_ui::icons;
use zenith_ui::module::{Module, TileCtx, TileSize};
use zenith_ui::paint;
use zenith_ui::theme::{SIZE_LABEL, Theme};

const REFRESH: Duration = Duration::from_secs(30 * 60);
const RETRY: Duration = Duration::from_secs(2 * 60);

#[derive(Clone, Debug, PartialEq)]
struct Now {
    temp: f32,
    code: u32,
    day: bool,
}

pub struct Weather {
    city: String,
    lat: f64,
    lon: f64,
    now: Option<Now>,
    error: bool,
    rx: Option<Receiver<Option<Now>>>,
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

fn fetch(lat: f64, lon: f64) -> Result<Now, String> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.2}&longitude={lon:.2}\
         &current=temperature_2m,weather_code,is_day&timezone=auto"
    );
    let body: serde_json::Value = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("Zenith/", env!("CARGO_PKG_VERSION")))
        .build()
        .get(&url)
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    let cur = &body["current"];
    Ok(Now {
        temp: cur["temperature_2m"].as_f64().ok_or("no temperature")? as f32,
        code: cur["weather_code"].as_u64().unwrap_or(0) as u32,
        day: cur["is_day"].as_u64().unwrap_or(1) == 1,
    })
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

    fn attach(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let (lat, lon) = (self.lat, self.lon);
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
                    std::thread::sleep(if ok { REFRESH } else { RETRY });
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
    }

    fn ui_tile(&mut self, ui: &mut Ui, ctx: &mut TileCtx<'_>) {
        let theme = ctx.theme;
        let rect = ui.max_rect();
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
}
