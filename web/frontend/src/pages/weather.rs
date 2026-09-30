//! Météo du week-end (prévisions Open-Meteo, gratuites et sans clé, appelées depuis le navigateur).

use serde::Deserialize;
use yew::prelude::*;

use crate::api::use_json;
use crate::components::loading;
use crate::i18n::t;
use crate::models::Race;
use crate::tr;
use crate::util::{local_date, now_ms, parse_ms, session_label};

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Forecast {
    hourly: Hourly,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Hourly {
    time: Vec<String>,
    temperature_2m: Vec<Option<f64>>,
    precipitation_probability: Vec<Option<f64>>,
    precipitation: Vec<Option<f64>>,
    wind_speed_10m: Vec<Option<f64>>,
    weather_code: Vec<Option<u32>>,
}

struct Hour {
    temp: Option<f64>,
    rain_prob: Option<f64>,
    rain_mm: Option<f64>,
    wind: Option<f64>,
    code: Option<u32>,
}

impl Hourly {
    fn at(&self, iso: &str) -> Option<Hour> {
        // « 2026-10-04T07:00:00Z » → heure « 2026-10-04T07:00 ».
        let key = iso.get(..13).map(|h| format!("{h}:00"))?;
        let i = self.time.iter().position(|x| *x == key)?;
        Some(Hour {
            temp: self.temperature_2m.get(i).copied().flatten(),
            rain_prob: self.precipitation_probability.get(i).copied().flatten(),
            rain_mm: self.precipitation.get(i).copied().flatten(),
            wind: self.wind_speed_10m.get(i).copied().flatten(),
            code: self.weather_code.get(i).copied().flatten(),
        })
    }
}

/// Code météo WMO → pictogramme.
fn icon(code: Option<u32>) -> &'static str {
    match code.unwrap_or(99) {
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

#[derive(Properties, PartialEq)]
pub struct WeatherProps {
    pub race: Race,
    /// Carte complète (toutes les sessions) ou seulement la course.
    #[prop_or(true)]
    pub full: bool,
}

/// Prévisions pour chaque session du week-end, disponibles jusqu'à ~16 jours à l'avance.
#[function_component]
pub fn WeekendWeather(props: &WeatherProps) -> Html {
    let race = &props.race;
    let sessions = race.sessions();
    let now = now_ms();
    let first = sessions.first().map(|s| parse_ms(&s.1)).unwrap_or(f64::NAN);
    let last = parse_ms(&race.start_iso());
    let loc = &race.circuit.location;
    // Open-Meteo prévoit à 16 jours ; rien à afficher pour une course passée.
    let in_range = last + 3.0 * 3_600_000.0 > now && first - now < 15.0 * 86_400_000.0;
    let url = match (&loc.lat, &loc.long, in_range) {
        (Some(lat), Some(lon), true) => Some(format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
             &hourly=temperature_2m,precipitation_probability,precipitation,wind_speed_10m,weather_code\
             &timezone=UTC&start_date={}&end_date={}",
            sessions.first().map(|s| &s.1[..10]).unwrap_or(&race.date),
            race.date
        )),
        _ => None,
    };
    let forecast = use_json::<Forecast>(url.clone());
    if url.is_none() {
        return html! {};
    }
    let shown: Vec<(&str, String)> = if props.full {
        sessions.iter().map(|(n, iso)| (*n, iso.clone())).collect()
    } else {
        sessions
            .iter()
            .filter(|(n, _)| *n == "Course")
            .map(|(n, iso)| (*n, iso.clone()))
            .collect()
    };

    let body = match &forecast {
        None => loading(),
        Some(Err(_)) => {
            html! { <p class="muted">{ t("Prévisions indisponibles.", "Forecast unavailable.") }</p> }
        }
        Some(Ok(f)) => {
            // Impact : pluie sur la durée de la course (départ → +2 h), chaleur, vent.
            let race_hours: Vec<Hour> = (0..3)
                .filter_map(|h| {
                    let ms = last + h as f64 * 3_600_000.0;
                    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms));
                    let iso: String = d.to_iso_string().into();
                    f.hourly.at(&iso)
                })
                .collect();
            let rain = race_hours
                .iter()
                .filter_map(|h| h.rain_prob)
                .fold(0.0, f64::max);
            let heat = race_hours
                .iter()
                .filter_map(|h| h.temp)
                .fold(f64::MIN, f64::max);
            let wind = race_hours.iter().filter_map(|h| h.wind).fold(0.0, f64::max);
            let mut impact: Vec<String> = Vec::new();
            if rain >= 60.0 {
                impact.push(tr!(
                    "🌧️ Pluie probable ({rain:.0} %) : pneus intermédiaires ou pluie possibles, stratégies très ouvertes.",
                    "🌧️ Rain likely ({rain:.0}%): intermediates or wets possible, strategies wide open."
                ));
            } else if rain >= 30.0 {
                impact.push(tr!(
                    "🌦️ Risque d'averse ({rain:.0} %) : un changement de pneus opportun peut tout changer.",
                    "🌦️ Chance of showers ({rain:.0}%): a well-timed tyre change could decide the race."
                ));
            } else if !race_hours.is_empty() {
                impact.push(tr!(
                    "☀️ Course sèche attendue (pluie {rain:.0} %).",
                    "☀️ Dry race expected (rain {rain:.0}%)."
                ));
            }
            if heat >= 30.0 {
                impact.push(tr!(
                    "🔥 Forte chaleur ({heat:.0} °C) : usure des pneus et refroidissement sous pression.",
                    "🔥 Hot conditions ({heat:.0}°C): tyre wear and cooling under pressure."
                ));
            }
            if wind >= 30.0 {
                impact.push(tr!(
                    "💨 Vent fort ({wind:.0} km/h) : voitures instables au freinage et en courbe.",
                    "💨 Strong wind ({wind:.0} km/h): cars unsettled under braking and in corners."
                ));
            }
            html! {
                <>
                    <ul class="sessions">
                        { for shown.iter().map(|(name, iso)| {
                            let h = f.hourly.at(iso);
                            html! {
                                <li class="session weather-row">
                                    <span class="session-name">
                                        { session_label(name) }
                                        <small class="muted">{ format!(" · {}", local_date(iso, true)) }</small>
                                    </span>
                                    <span class="session-time">
                                        { match h {
                                            Some(h) => format!(
                                                "{} {} · 💧{} · 💨{}",
                                                icon(h.code),
                                                h.temp.map(|v| format!("{v:.0}°")).unwrap_or_else(|| "–".into()),
                                                h.rain_prob.map(|v| format!("{v:.0}%")).unwrap_or_else(|| "–".into()),
                                                h.wind.map(|v| format!("{v:.0} km/h")).unwrap_or_else(|| "–".into()),
                                            ) + &h.rain_mm.filter(|mm| *mm > 0.0).map(|mm| format!(" · {mm:.1} mm")).unwrap_or_default(),
                                            None => t("prévision pas encore disponible", "forecast not available yet").into(),
                                        } }
                                    </span>
                                </li>
                            }
                        }) }
                    </ul>
                    if !impact.is_empty() {
                        <div class="impact">
                            <strong>{ t("Impact sur la course", "Race impact") }</strong>
                            { for impact.iter().map(|i| html! { <p>{ i }</p> }) }
                        </div>
                    }
                    <p class="muted">{ t("💧 = probabilité de pluie · 💨 = vent. Prévisions Open-Meteo à l'heure de chaque session.", "💧 = chance of rain · 💨 = wind. Open-Meteo forecast at each session's time.") }</p>
                </>
            }
        }
    };

    html! {
        <section class="card">
            <h2>{ t("Météo du week-end", "Weekend weather") }</h2>
            { body }
        </section>
    }
}
