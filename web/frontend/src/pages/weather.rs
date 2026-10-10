//! Météo du week-end (Open-Meteo, gratuit et sans clé, appelé depuis le navigateur) :
//! conditions actuelles au circuit, prévisions jour par jour, détail de chaque séance,
//! évolution autour du départ et impact sur la course.

use std::rc::Rc;

use active::prelude::*;
use serde::Deserialize;
use wasm_bindgen::{JsCast, JsValue};

use crate::api::use_json;
use crate::components::{dynamic, loading, stat_grid};
use crate::i18n::{lang, t};
use crate::models::Race;
use crate::tr;
use crate::util::{local_date, now_ms, parse_ms, session_label};

const HOURLY: &str = "temperature_2m,apparent_temperature,relative_humidity_2m,dew_point_2m,\
precipitation_probability,precipitation,weather_code,cloud_cover,pressure_msl,wind_speed_10m,\
wind_direction_10m,wind_gusts_10m,uv_index,shortwave_radiation";
const DAILY: &str = "weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,\
precipitation_sum,precipitation_probability_max,wind_gusts_10m_max,uv_index_max";
const CURRENT: &str = "temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,\
cloud_cover,pressure_msl,wind_speed_10m,wind_direction_10m,wind_gusts_10m,precipitation";

type Series = Vec<Option<f64>>;

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Forecast {
    hourly: Hourly,
    #[serde(default)]
    daily: Option<Daily>,
    #[serde(default)]
    current: Option<Current>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Hourly {
    time: Vec<String>,
    #[serde(default)]
    temperature_2m: Series,
    #[serde(default)]
    apparent_temperature: Series,
    #[serde(default)]
    relative_humidity_2m: Series,
    #[serde(default)]
    dew_point_2m: Series,
    #[serde(default)]
    precipitation_probability: Series,
    #[serde(default)]
    precipitation: Series,
    #[serde(default)]
    weather_code: Series,
    #[serde(default)]
    cloud_cover: Series,
    #[serde(default)]
    pressure_msl: Series,
    #[serde(default)]
    wind_speed_10m: Series,
    #[serde(default)]
    wind_direction_10m: Series,
    #[serde(default)]
    wind_gusts_10m: Series,
    #[serde(default)]
    uv_index: Series,
    #[serde(default)]
    shortwave_radiation: Series,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Daily {
    time: Vec<String>,
    #[serde(default)]
    weather_code: Series,
    #[serde(default)]
    temperature_2m_max: Series,
    #[serde(default)]
    temperature_2m_min: Series,
    #[serde(default)]
    sunrise: Vec<Option<String>>,
    #[serde(default)]
    sunset: Vec<Option<String>>,
    #[serde(default)]
    precipitation_sum: Series,
    #[serde(default)]
    precipitation_probability_max: Series,
    #[serde(default)]
    wind_gusts_10m_max: Series,
    #[serde(default)]
    uv_index_max: Series,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Current {
    time: String,
    temperature_2m: Option<f64>,
    apparent_temperature: Option<f64>,
    relative_humidity_2m: Option<f64>,
    weather_code: Option<f64>,
    cloud_cover: Option<f64>,
    pressure_msl: Option<f64>,
    wind_speed_10m: Option<f64>,
    wind_direction_10m: Option<f64>,
    wind_gusts_10m: Option<f64>,
    precipitation: Option<f64>,
}

/// Conditions à une heure donnée.
#[derive(Clone, Copy, Default)]
struct Hour {
    ms: f64,
    temp: Option<f64>,
    feels: Option<f64>,
    humidity: Option<f64>,
    dew: Option<f64>,
    rain_prob: Option<f64>,
    rain_mm: Option<f64>,
    code: Option<f64>,
    cloud: Option<f64>,
    pressure: Option<f64>,
    wind: Option<f64>,
    wind_dir: Option<f64>,
    gusts: Option<f64>,
    uv: Option<f64>,
    radiation: Option<f64>,
}

impl Hour {
    /// Piste estimée : l'asphalte sombre chauffe au soleil bien au-delà de l'air
    /// (≈ +18 °C en plein soleil, ≈ +1 °C la nuit).
    fn track(&self) -> Option<f64> {
        Some(self.temp? + 1.0 + self.radiation.unwrap_or(0.0) * 0.021)
    }
}

fn get(s: &Series, i: usize) -> Option<f64> {
    s.get(i).copied().flatten()
}

impl Hourly {
    fn hour(&self, i: usize) -> Hour {
        Hour {
            ms: parse_ms(&format!("{}:00Z", self.time[i])),
            temp: get(&self.temperature_2m, i),
            feels: get(&self.apparent_temperature, i),
            humidity: get(&self.relative_humidity_2m, i),
            dew: get(&self.dew_point_2m, i),
            rain_prob: get(&self.precipitation_probability, i),
            rain_mm: get(&self.precipitation, i),
            code: get(&self.weather_code, i),
            cloud: get(&self.cloud_cover, i),
            pressure: get(&self.pressure_msl, i),
            wind: get(&self.wind_speed_10m, i),
            wind_dir: get(&self.wind_direction_10m, i),
            gusts: get(&self.wind_gusts_10m, i),
            uv: get(&self.uv_index, i),
            radiation: get(&self.shortwave_radiation, i),
        }
    }

    /// Heure pleine la plus proche d'un instant (ms).
    fn at_ms(&self, ms: f64) -> Option<Hour> {
        let d = js_sys::Date::new(&JsValue::from_f64(ms + 1_800_000.0));
        let iso: String = d.to_iso_string().into();
        let key = format!("{}:00", iso.get(..13)?);
        let i = self.time.iter().position(|x| *x == key)?;
        Some(self.hour(i))
    }

    fn at(&self, iso: &str) -> Option<Hour> {
        self.at_ms(parse_ms(iso))
    }
}

/// Code météo WMO → pictogramme.
fn icon(code: Option<f64>) -> &'static str {
    match code.map(|c| c as u32).unwrap_or(99) {
        0 => "☀️",
        1 | 2 => "🌤️",
        3 => "☁️",
        45 | 48 => "🌫️",
        51..=57 => "🌦️",
        61..=67 | 80..=82 => "🌧️",
        71..=77 | 85 | 86 => "🌨️",
        95..=99 => "⛈️",
        _ => "·",
    }
}

/// Code météo WMO → description.
fn describe(code: Option<f64>) -> &'static str {
    match code.map(|c| c as u32).unwrap_or(999) {
        0 => t("Ciel dégagé", "Clear sky"),
        1 => t("Plutôt dégagé", "Mainly clear"),
        2 => t("Partiellement nuageux", "Partly cloudy"),
        3 => t("Couvert", "Overcast"),
        45 | 48 => t("Brouillard", "Fog"),
        51..=55 => t("Bruine", "Drizzle"),
        56 | 57 => t("Bruine verglaçante", "Freezing drizzle"),
        61 => t("Pluie faible", "Light rain"),
        63 => t("Pluie", "Rain"),
        65 => t("Forte pluie", "Heavy rain"),
        66 | 67 => t("Pluie verglaçante", "Freezing rain"),
        71..=77 | 85 | 86 => t("Neige", "Snow"),
        80 => t("Averses", "Showers"),
        81 | 82 => t("Fortes averses", "Heavy showers"),
        95 => t("Orage", "Thunderstorm"),
        96..=99 => t("Orage et grêle", "Thunderstorm with hail"),
        _ => "–",
    }
}

/// Direction d'où vient le vent (degrés) → point cardinal.
pub fn compass(deg: f64) -> &'static str {
    let fr = ["N", "NE", "E", "SE", "S", "SO", "O", "NO"];
    let en = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let i = (((deg % 360.0) + 22.5) / 45.0) as usize % 8;
    t(fr[i], en[i])
}

fn deg(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.0}°")).unwrap_or_else(|| "–".into())
}
fn pct(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.0}%")).unwrap_or_else(|| "–".into())
}
fn kmh(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.0} km/h"))
        .unwrap_or_else(|| "–".into())
}
fn wind_text(speed: Option<f64>, dir: Option<f64>) -> String {
    match (speed, dir) {
        (Some(s), Some(d)) => format!("{s:.0} km/h {}", compass(d)),
        (Some(s), None) => format!("{s:.0} km/h"),
        _ => "–".into(),
    }
}

/// « 06:42 » à l'heure locale du visiteur, depuis une heure UTC « 2026-10-04T06:42 ».
fn local_time(utc: &str) -> String {
    let d = js_sys::Date::new(&JsValue::from_str(&format!("{utc}:00Z")));
    if d.get_time().is_nan() {
        return "–".into();
    }
    format!("{:02}:{:02}", d.get_hours(), d.get_minutes())
}

fn hour_label(ms: f64) -> String {
    let h = js_sys::Date::new(&JsValue::from_f64(ms)).get_hours();
    if lang().code() == "fr" {
        format!("{h}h")
    } else {
        format!("{h}:00")
    }
}

/// Détail complet d'une heure (séance ou conditions actuelles).
fn hour_details(h: &Hour) -> Node {
    stat_grid(vec![
        (t("Air", "Air"), deg(h.temp)),
        (t("Ressenti", "Feels like"), deg(h.feels)),
        (t("Piste (est.)", "Track (est.)"), deg(h.track())),
        (t("Pluie", "Rain"), pct(h.rain_prob)),
        (
            t("Cumul", "Amount"),
            h.rain_mm
                .map(|v| format!("{v:.1} mm"))
                .unwrap_or_else(|| "–".into()),
        ),
        (t("Humidité", "Humidity"), pct(h.humidity)),
        (t("Vent", "Wind"), wind_text(h.wind, h.wind_dir)),
        (t("Rafales", "Gusts"), kmh(h.gusts)),
        (t("Nuages", "Cloud"), pct(h.cloud)),
        (t("Rosée", "Dew point"), deg(h.dew)),
        (
            t("Pression", "Pressure"),
            h.pressure
                .map(|v| format!("{v:.0} hPa"))
                .unwrap_or_else(|| "–".into()),
        ),
        (
            "UV",
            h.uv.map(|v| format!("{v:.0}"))
                .unwrap_or_else(|| "–".into()),
        ),
    ])
}

/// Ligne résumée d'une heure.
fn hour_summary(h: &Hour) -> String {
    let mut s = format!(
        "{} {} · 💧{} · 💨{}",
        icon(h.code),
        deg(h.temp),
        pct(h.rain_prob),
        kmh(h.wind)
    );
    if let Some(mm) = h.rain_mm.filter(|mm| *mm > 0.0) {
        s.push_str(&format!(" · {mm:.1} mm"));
    }
    s
}

/// Ce que la météo change pour la course.
fn impact(race_hours: &[Hour], quali: Option<&Hour>) -> Vec<String> {
    let max =
        |f: fn(&Hour) -> Option<f64>| race_hours.iter().filter_map(f).fold(f64::MIN, f64::max);
    let min =
        |f: fn(&Hour) -> Option<f64>| race_hours.iter().filter_map(f).fold(f64::MAX, f64::min);
    if race_hours.is_empty() {
        return Vec::new();
    }
    let rain = max(|h| h.rain_prob).max(0.0);
    let mm: f64 = race_hours.iter().filter_map(|h| h.rain_mm).sum();
    let heat = max(|h| h.temp);
    let track = max(|h| h.track());
    let cold_track = min(|h| h.track());
    let gusts = max(|h| h.gusts).max(0.0);
    let humidity = max(|h| h.humidity);
    let mut out = Vec::new();
    if rain >= 60.0 || mm >= 1.0 {
        out.push(tr!(
            "🌧️ Pluie probable ({rain:.0} %, {mm:.1} mm attendus) : pneus intermédiaires ou pluie, voiture de sécurité possible, stratégies très ouvertes.",
            "🌧️ Rain likely ({rain:.0}%, {mm:.1} mm expected): intermediates or wets, possible safety car, strategies wide open."
        ));
    } else if rain >= 30.0 {
        out.push(tr!(
            "🌦️ Risque d'averse ({rain:.0} %) : un changement de pneus au bon moment peut tout changer.",
            "🌦️ Chance of showers ({rain:.0}%): a well-timed tyre change could decide the race."
        ));
    } else {
        out.push(tr!(
            "☀️ Course sèche attendue (pluie {rain:.0} %).",
            "☀️ Dry race expected (rain {rain:.0}%)."
        ));
    }
    if track >= 45.0 || heat >= 30.0 {
        out.push(tr!(
            "🔥 Piste brûlante (≈ {track:.0} °C, air {heat:.0} °C) : dégradation thermique des pneus, freins et moteurs à refroidir — avantage aux voitures douces avec leurs gommes.",
            "🔥 Scorching track (≈ {track:.0}°C, air {heat:.0}°C): thermal tyre degradation, brakes and engines to cool — advantage to cars that are kind to their tyres."
        ));
    } else if cold_track < 20.0 {
        out.push(tr!(
            "🥶 Piste fraîche (≈ {cold_track:.0} °C) : pneus difficiles à mettre en température, risque de graining et de blocages au départ.",
            "🥶 Cool track (≈ {cold_track:.0}°C): tyres hard to bring up to temperature, risk of graining and lock-ups at the start."
        ));
    }
    if gusts >= 40.0 {
        out.push(tr!(
            "💨 Rafales jusqu'à {gusts:.0} km/h : voitures instables au freinage et en courbe rapide, aspiration et DRS plus ou moins efficaces selon les lignes droites.",
            "💨 Gusts up to {gusts:.0} km/h: cars unsettled under braking and in fast corners, slipstream and DRS more or less effective depending on the straight."
        ));
    }
    if humidity >= 85.0 && rain < 60.0 {
        out.push(tr!(
            "💦 Air très humide ({humidity:.0} %) : un peu moins de puissance moteur et une piste qui sèche lentement après une averse.",
            "💦 Very humid air ({humidity:.0}%): slightly less engine power and a track that dries slowly after a shower."
        ));
    }
    if let (Some(q), Some(r)) = (
        quali.and_then(Hour::track),
        race_hours.first().and_then(Hour::track),
    ) {
        let diff = r - q;
        if diff.abs() >= 8.0 {
            out.push(if diff > 0.0 {
                tr!(
                    "🌡️ Piste ≈ {:.0} °C plus chaude qu'en qualifications : les réglages trouvés samedi peuvent ne plus fonctionner.",
                    "🌡️ Track ≈ {:.0}°C hotter than in qualifying: Saturday's set-up may no longer work.",
                    diff.abs()
                )
            } else {
                tr!(
                    "🌡️ Piste ≈ {:.0} °C plus fraîche qu'en qualifications : les réglages trouvés samedi peuvent ne plus fonctionner.",
                    "🌡️ Track ≈ {:.0}°C cooler than in qualifying: Saturday's set-up may no longer work.",
                    diff.abs()
                )
            });
        }
    }
    out
}

// ---------- Autour du départ : deux petits graphiques à axe unique ----------

/// (instant, température, probabilité de pluie)
type WindowPoint = (f64, Option<f64>, Option<f64>);

const TEMP_COLOUR: &str = "#e5483f";
const RAIN_COLOUR: &str = "#3b9fd8";

/// Évolution de 2 h avant à 3 h après le départ : température (ligne) puis pluie (barres),
/// l'un sous l'autre sur le même axe des heures. Toucher un point affiche ses valeurs.
///
/// Les graphiques sont construits une fois : seuls la valeur lue et le repère vertical suivent
/// le doigt (le tracé ne se redessine pas à chaque mouvement).
fn race_window(points: Vec<WindowPoint>, start_ms: f64) -> Node {
    let n = points.len();
    if n < 2 {
        return Node::Empty;
    }
    let pts = Rc::new(points);
    let pick = use_state(None::<usize>);
    // `pick` est réécrit à chaque mouvement : la vue ne suit que l'heure réellement choisie.
    let picked = memo(move || pick.get());
    let (w, pad_l, pad_r) = (320.0f64, 30.0, 12.0);
    let x = move |i: usize| pad_l + (w - pad_l - pad_r) * i as f64 / (n - 1) as f64;
    let temps: Vec<f64> = pts.iter().filter_map(|pt| pt.1).collect();
    let (lo, hi) = (
        temps.iter().copied().fold(f64::MAX, f64::min).floor() - 1.0,
        temps.iter().copied().fold(f64::MIN, f64::max).ceil() + 1.0,
    );
    let (th, tt) = (70.0, 10.0);
    let ty = |v: f64| tt + th - (v - lo) / (hi - lo).max(1.0) * th;
    let temp_line: String = pts
        .iter()
        .enumerate()
        .filter_map(|(i, pt)| pt.1.map(|v| format!("{:.1},{:.1}", x(i), ty(v))))
        .collect::<Vec<_>>()
        .join(" ");
    let (rh, rt) = (56.0, 8.0);
    let bar_w = ((w - pad_l - pad_r) / n as f64 * 0.55).min(22.0);
    let start_i = pts
        .iter()
        .position(|pt| pt.0 >= start_ms - 60_000.0)
        .unwrap_or(0);

    let onpointer = move |e: Event| {
        let Some(pe) = e.raw().dyn_ref::<web_sys::PointerEvent>() else {
            return;
        };
        let Some(el) = e.current_target() else {
            return;
        };
        let r = el.get_bounding_client_rect();
        if r.width() <= 0.0 {
            return;
        }
        let vx = (pe.client_x() as f64 - r.left()) / r.width() * w;
        let i = ((vx - pad_l) / (w - pad_l - pad_r) * (n - 1) as f64).round();
        pick.set(Some(i.clamp(0.0, (n - 1) as f64) as usize));
    };
    let readout = {
        let pts = Rc::clone(&pts);
        move || match picked.get() {
            Some(i) => {
                let pt = pts[i];
                tr!(
                    "{} · {} · pluie {}",
                    "{} · {} · rain {}",
                    hour_label(pt.0),
                    deg(pt.1),
                    pct(pt.2)
                )
            }
            None => t(
                "Touche un graphique pour lire les valeurs heure par heure.",
                "Touch a chart to read hour-by-hour values.",
            )
            .into(),
        }
    };
    // Repère de l'heure touchée.
    let guide = move |height: f64| {
        dynamic(move || {
            picked
                .get()
                .map(|i| {
                    line()
                        .class("wx-guide")
                        .attr("x1", format!("{:.1}", x(i)))
                        .attr("x2", format!("{:.1}", x(i)))
                        .attr("y1", "0")
                        .attr("y2", format!("{height:.0}"))
                })
                .into()
        })
    };
    let start_mark = |height: f64| {
        line()
            .class("wx-start")
            .attr("x1", format!("{:.1}", x(start_i)))
            .attr("x2", format!("{:.1}", x(start_i)))
            .attr("y1", "0")
            .attr("y2", format!("{height:.0}"))
    };
    let ticks = |y: f64| {
        fragment(pts.iter().enumerate().map(|(i, pt)| {
            text_svg()
                .class("wx-tick")
                .attr("x", format!("{:.1}", x(i)))
                .attr("y", format!("{y:.0}"))
                .attr(
                    "text-anchor",
                    if i == 0 {
                        "start"
                    } else if i + 1 == n {
                        "end"
                    } else {
                        "middle"
                    },
                )
                .text(if i == start_i {
                    t("Départ", "Start").to_string()
                } else {
                    hour_label(pt.0)
                })
        }))
    };
    let grid_line = |class: &'static str, y: String| {
        line()
            .class(class)
            .attr("x1", format!("{pad_l}"))
            .attr("x2", format!("{:.0}", w - pad_r))
            .attr("y1", y.clone())
            .attr("y2", y)
    };
    let temp_h = tt + th + 22.0;
    let rain_h = rt + rh + 22.0;

    let temp_chart = svg()
        .class("wx-chart")
        .attr("viewBox", format!("0 0 {w:.0} {temp_h:.0}"))
        .attr("role", "img")
        .attr(
            "aria-label",
            t(
                "Température autour du départ",
                "Temperature around the start",
            ),
        )
        .on("pointerdown", onpointer)
        .on("pointermove", onpointer)
        .child(grid_line("wx-grid", format!("{:.1}", ty(hi))))
        .child(grid_line("wx-grid", format!("{:.1}", ty(lo))))
        .child(
            text_svg()
                .class("wx-tick")
                .attr("x", "2")
                .attr("y", format!("{:.1}", ty(hi) + 4.0))
                .text(format!("{hi:.0}°")),
        )
        .child(
            text_svg()
                .class("wx-tick")
                .attr("x", "2")
                .attr("y", format!("{:.1}", ty(lo) + 4.0))
                .text(format!("{lo:.0}°")),
        )
        .child(start_mark(tt + th))
        .child(guide(tt + th))
        .child(
            polyline()
                .class("chart-line")
                .attr("pathLength", "1")
                .attr("points", temp_line)
                .attr("fill", "none")
                .attr("stroke", TEMP_COLOUR)
                .attr("stroke-width", "2")
                .attr("stroke-linejoin", "round"),
        )
        .children(pts.iter().enumerate().filter_map(|(i, pt)| {
            pt.1.map(|v| {
                circle()
                    .class("wx-dot")
                    .attr("cx", format!("{:.1}", x(i)))
                    .attr("cy", format!("{:.1}", ty(v)))
                    .attr("r", "4")
                    .attr("fill", TEMP_COLOUR)
            })
        }))
        .child(ticks(temp_h - 4.0));

    let rain_chart = svg()
        .class("wx-chart")
        .attr("viewBox", format!("0 0 {w:.0} {rain_h:.0}"))
        .attr("role", "img")
        .attr(
            "aria-label",
            t(
                "Probabilité de pluie autour du départ",
                "Chance of rain around the start",
            ),
        )
        .on("pointerdown", onpointer)
        .on("pointermove", onpointer)
        .child(grid_line("wx-grid", format!("{rt}")))
        .child(grid_line("wx-axis", format!("{}", rt + rh)))
        .child(
            text_svg()
                .class("wx-tick")
                .attr("x", "2")
                .attr("y", format!("{}", rt + 4.0))
                .text("100"),
        )
        .child(
            text_svg()
                .class("wx-tick")
                .attr("x", "2")
                .attr("y", format!("{}", rt + rh))
                .text("0"),
        )
        .child(start_mark(rt + rh))
        .child(guide(rt + rh))
        .children(pts.iter().enumerate().map(|(i, pt)| {
            let v = pt.2.unwrap_or(0.0).clamp(0.0, 100.0);
            let hgt = (v / 100.0 * rh).max(if v > 0.0 { 2.0 } else { 0.0 });
            rect()
                .attr("x", format!("{:.1}", x(i) - bar_w / 2.0))
                .attr("y", format!("{:.1}", rt + rh - hgt))
                .attr("width", format!("{bar_w:.1}"))
                .attr("height", format!("{hgt:.1}"))
                .attr("rx", "3")
                .attr("fill", RAIN_COLOUR)
        }))
        .child(ticks(rain_h - 4.0));

    div()
        .class("wx-window")
        .child(
            p().class("chart-readout")
                .class_if("chart-readout-hint", move || picked.get().is_none())
                .attr("aria-live", "polite")
                .text_dyn(readout),
        )
        .child(
            p().class("wx-title")
                .text(t("Température de l'air (°C)", "Air temperature (°C)")),
        )
        .child(temp_chart)
        .child(
            p().class("wx-title")
                .text(t("Probabilité de pluie (%)", "Chance of rain (%)")),
        )
        .child(rain_chart)
        .into()
}

// ---------- Carte ----------

/// Prévisions pour chaque séance du week-end, disponibles jusqu'à ~16 jours à l'avance.
/// `full` : carte complète (actuel, jours, séances, graphiques) ou seulement la course.
pub fn weekend_weather(race: Race, full: bool) -> Node {
    let sessions = race.sessions();
    let now = now_ms();
    let first = sessions.first().map(|s| parse_ms(&s.1)).unwrap_or(f64::NAN);
    let start = parse_ms(&race.start_iso());
    let loc = &race.circuit.location;
    // Open-Meteo prévoit à 16 jours ; rien à afficher pour une course passée.
    let in_range = start + 3.0 * 3_600_000.0 > now && first - now < 15.0 * 86_400_000.0;
    let url = match (&loc.lat, &loc.long, in_range) {
        (Some(lat), Some(lon), true) => format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
             &hourly={HOURLY}&daily={DAILY}&current={CURRENT}\
             &timezone=UTC&start_date={}&end_date={}",
            sessions.first().map(|s| &s.1[..10]).unwrap_or(&race.date),
            race.date
        ),
        _ => return Node::Empty,
    };
    let forecast = use_json::<Forecast>(Some(url));
    let shown: Rc<Vec<(&'static str, String)>> = Rc::new(
        sessions
            .iter()
            .filter(|(n, _)| full || *n == "Course")
            .map(|(n, iso)| (*n, iso.clone()))
            .collect(),
    );
    let quali_iso = sessions
        .iter()
        .find(|(n, _)| *n == "Qualifications")
        .map(|(_, iso)| iso.clone());

    // Seule l'arrivée des prévisions reconstruit le contenu de la carte.
    let body = dynamic(move || match forecast.get() {
        None => loading(),
        Some(Err(_)) => p()
            .class("muted")
            .text(t("Prévisions indisponibles.", "Forecast unavailable."))
            .into(),
        Some(Ok(f)) => {
            untrack(|| forecast_view(&f, full, now, start, &shown, quali_iso.as_deref()))
        }
    });

    section()
        .class("card")
        .child(h2().text(t("Météo du week-end", "Weekend weather")))
        .child(body)
        .into()
}

/// Contenu de la carte une fois les prévisions arrivées.
fn forecast_view(
    f: &Forecast,
    full: bool,
    now: f64,
    start: f64,
    shown: &[(&'static str, String)],
    quali_iso: Option<&str>,
) -> Node {
    let race_hours: Vec<Hour> = (0..3)
        .filter_map(|h| f.hourly.at_ms(start + h as f64 * 3_600_000.0))
        .collect();
    let quali = quali_iso.and_then(|iso| f.hourly.at(iso));
    let impact = impact(&race_hours, quali.as_ref());
    let window: Vec<WindowPoint> = (-2..=3)
        .filter_map(|h| f.hourly.at_ms(start + h as f64 * 3_600_000.0))
        .map(|h| (h.ms, h.temp, h.rain_prob))
        .collect();

    let current = f.current.as_ref().filter(|_| full).map(|c| {
        let h = Hour {
            ms: now,
            temp: c.temperature_2m,
            feels: c.apparent_temperature,
            humidity: c.relative_humidity_2m,
            code: c.weather_code,
            cloud: c.cloud_cover,
            pressure: c.pressure_msl,
            wind: c.wind_speed_10m,
            wind_dir: c.wind_direction_10m,
            gusts: c.wind_gusts_10m,
            rain_mm: c.precipitation,
            radiation: f.hourly.at_ms(now).and_then(|h| h.radiation),
            ..Hour::default()
        };
        div()
            .class("wx-now")
            .child(
                p().class("wx-now-head")
                    .child(
                        span()
                            .class("wx-now-icon")
                            .attr("aria-hidden", "true")
                            .text(icon(c.weather_code)),
                    )
                    .child(
                        span()
                            .child(strong().text(deg(c.temperature_2m)))
                            .text(format!(" · {}", describe(c.weather_code)))
                            .child(small().class("muted").text(tr!(
                                " · maintenant au circuit ({})",
                                " · now at the circuit ({})",
                                local_time(&c.time)
                            ))),
                    ),
            )
            .child(hour_details(&h))
    });

    let days = f.daily.as_ref().filter(|_| full).map(|d| {
        ul().class("wx-days")
            .children(d.time.iter().enumerate().map(|(i, day)| {
                let sunrise = d
                    .sunrise
                    .get(i)
                    .cloned()
                    .flatten()
                    .map(|s| local_time(&s))
                    .unwrap_or_default();
                let sunset = d
                    .sunset
                    .get(i)
                    .cloned()
                    .flatten()
                    .map(|s| local_time(&s))
                    .unwrap_or_default();
                li().class("wx-day")
                    .child(
                        span()
                            .class("wx-day-name")
                            .text(icon(get(&d.weather_code, i)))
                            .text(" ")
                            .text(local_date(&format!("{day}T12:00:00Z"), false)),
                    )
                    .child(span().class("wx-day-temp").text(format!(
                        "{} / {}",
                        deg(get(&d.temperature_2m_min, i)),
                        deg(get(&d.temperature_2m_max, i))
                    )))
                    .child(span().class("wx-day-more muted").text(format!(
                        "💧 {} · {:.1} mm · {} {} · UV {} · 🌅 {sunrise} · 🌇 {sunset}",
                        pct(get(&d.precipitation_probability_max, i)),
                        get(&d.precipitation_sum, i).unwrap_or(0.0),
                        t("rafales", "gusts"),
                        kmh(get(&d.wind_gusts_10m_max, i)),
                        get(&d.uv_index_max, i)
                            .map(|v| format!("{v:.0}"))
                            .unwrap_or_else(|| "–".into()),
                    )))
            }))
    });

    let session_rows = ul()
        .class("sessions")
        .children(shown.iter().map(|(name, iso)| {
            let h = f.hourly.at(iso);
            li().class("session-wx").child(
                details()
                    .child(
                        summary()
                            .class("session weather-row")
                            .child(
                                span()
                                    .class("session-name")
                                    .text(session_label(name).to_string())
                                    .child(
                                        small()
                                            .class("muted")
                                            .text(format!(" · {}", local_date(iso, true))),
                                    ),
                            )
                            .child(
                                span().class("session-time").text(match &h {
                                    Some(h) => hour_summary(h),
                                    None => t(
                                        "prévision pas encore disponible",
                                        "forecast not available yet",
                                    )
                                    .into(),
                                }),
                            ),
                    )
                    .child(h.as_ref().map(|h| {
                        fragment([
                            Node::from(p().class("muted").text(describe(h.code))),
                            hour_details(h),
                        ])
                    })),
            )
        }));

    fragment([
        Node::from(current),
        full.then(|| h3().class("wx-sub").text(t("Jour par jour", "Day by day")))
            .into(),
        days.into(),
        full.then(|| {
            h3().class("wx-sub")
                .text(t("Séance par séance", "Session by session"))
        })
        .into(),
        session_rows.into(),
        (full && window.len() >= 3)
            .then(|| {
                fragment([
                    Node::from(
                        h3().class("wx-sub")
                            .text(t("Autour du départ", "Around the start")),
                    ),
                    race_window(window, start),
                ])
            })
            .into(),
        (!impact.is_empty())
            .then(|| {
                div()
                    .class("impact")
                    .child(strong().text(t("Impact sur la course", "Race impact")))
                    .children(impact.into_iter().map(|text_line| p().text(text_line)))
            })
            .into(),
        p().class("muted")
            .text(t(
                "Touche une séance pour tout voir. 💧 = probabilité de pluie · 💨 = vent moyen. Piste estimée d'après l'air et l'ensoleillement. Prévisions Open-Meteo, à l'heure de chaque séance.",
                "Tap a session to see everything. 💧 = chance of rain · 💨 = mean wind. Track temperature estimated from air and sunshine. Open-Meteo forecast at each session's time.",
            ))
            .into(),
    ])
}
