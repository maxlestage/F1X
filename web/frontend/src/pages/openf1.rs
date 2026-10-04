//! Données OpenF1 complètes (les 18 points d'accès, via le relais mis en cache du serveur) :
//! réunions, séances, résultats, grille, championnat, tours, positions, écarts, pneus, arrêts,
//! dépassements, télémétrie comparée, positions GPS, météo, direction de course, radios.

use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::Value;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::api::use_json;
use crate::components::{Layout, Tab, loading, stat_grid};
use crate::i18n::t;
use crate::tr;
use crate::util::{current_year, local_date};

type Json = Option<Result<Rc<Value>, (u16, String)>>;

fn of1(endpoint: &str, query: &str) -> Option<String> {
    Some(format!("/api/of1/{endpoint}?{query}"))
}
fn list(v: &Json) -> Vec<Value> {
    match v {
        Some(Ok(v)) => v.as_array().cloned().unwrap_or_default(),
        _ => Vec::new(),
    }
}
fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}
fn num(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}
fn int(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(Value::as_i64)
}
fn ms(iso: &str) -> f64 {
    js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso)).get_time()
}
fn lap_str(sec: f64) -> String {
    f1x_protocol::format_lap(sec)
}

/// Palette catégorielle validée (fond sombre), dans un ordre fixe.
const SERIES: [&str; 5] = ["#3987e5", "#d95926", "#199e70", "#c98500", "#d55181"];

fn state_view(v: &Json) -> Option<Html> {
    match v {
        None => Some(loading()),
        Some(Err((404, _))) => Some(
            html! { <p class="muted">{ t("Pas de données pour cette séance.", "No data for this session.") }</p> },
        ),
        Some(Err(_)) => Some(
            html! { <p class="muted">{ t("Données OpenF1 momentanément indisponibles (limite de requêtes) — réessaie dans une minute.", "OpenF1 data temporarily unavailable (rate limit) — try again in a minute.") }</p> },
        ),
        Some(Ok(_)) => None,
    }
}

// ---------- Graphique en lignes générique (un seul axe) ----------

#[derive(Clone, PartialEq)]
pub struct Series {
    pub label: String,
    pub color: String,
    pub points: Vec<(f64, f64)>,
}

#[derive(Properties, PartialEq)]
pub struct LineProps {
    pub series: Rc<Vec<Series>>,
    /// Libellé d'une valeur de x (abscisse) et d'une valeur de y.
    pub fmt_x: Callback<f64, String>,
    pub fmt_y: Callback<f64, String>,
    #[prop_or_default]
    pub invert: bool,
    #[prop_or(false)]
    pub step: bool,
    #[prop_or(180.0)]
    pub height: f64,
}

#[function_component]
pub fn LineChart(p: &LineProps) -> Html {
    let pick = use_state(|| None::<f64>);
    let all: Vec<(f64, f64)> = p
        .series
        .iter()
        .flat_map(|s| s.points.iter().copied())
        .collect();
    if all.len() < 2 {
        return html! { <p class="muted">{ t("Pas assez de données.", "Not enough data.") }</p> };
    }
    let (w, h, ml, mr, mt, mb) = (340.0, p.height, 40.0, 10.0, 10.0, 22.0);
    let (x0, x1) = all
        .iter()
        .fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q.0), a.1.max(q.0)));
    let (y0, y1) = all
        .iter()
        .fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q.1), a.1.max(q.1)));
    let pad = ((y1 - y0) * 0.05).max(0.5);
    let (y0, y1) = (y0 - pad, y1 + pad);
    let x = move |v: f64| ml + (v - x0) / (x1 - x0).max(1e-9) * (w - ml - mr);
    let invert = p.invert;
    let y = move |v: f64| {
        let r = (v - y0) / (y1 - y0).max(1e-9);
        if invert {
            mt + r * (h - mt - mb)
        } else {
            h - mb - r * (h - mt - mb)
        }
    };
    let path = |s: &Series| {
        let mut d = String::new();
        for (i, &(a, b)) in s.points.iter().enumerate() {
            if p.step && i > 0 {
                d.push_str(&format!(" H{:.1} V{:.1}", x(a), y(b)));
            } else {
                d.push_str(&format!(
                    "{}{:.1},{:.1}",
                    if i == 0 { "M" } else { " L" },
                    x(a),
                    y(b)
                ));
            }
        }
        d
    };
    let onpointer = {
        let pick = pick.clone();
        Callback::from(move |e: PointerEvent| {
            let Some(el) = e
                .current_target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            else {
                return;
            };
            let r = el.get_bounding_client_rect();
            if r.width() <= 0.0 {
                return;
            }
            let vx = (e.client_x() as f64 - r.left()) / r.width() * w;
            pick.set(Some(x0 + (vx - ml) / (w - ml - mr) * (x1 - x0)));
        })
    };
    let readout = match *pick {
        Some(px) => {
            let mut parts = vec![p.fmt_x.emit(px)];
            for s in p.series.iter() {
                if let Some(&(_, v)) = s
                    .points
                    .iter()
                    .min_by(|a, b| (a.0 - px).abs().total_cmp(&(b.0 - px).abs()))
                {
                    parts.push(format!("{} {}", s.label, p.fmt_y.emit(v)));
                }
            }
            parts.join(" · ")
        }
        None => t(
            "Touche le graphique pour lire les valeurs.",
            "Touch the chart to read the values.",
        )
        .into(),
    };
    let ticks_y = [y0 + (y1 - y0) * 0.1, (y0 + y1) / 2.0, y1 - (y1 - y0) * 0.1];
    let ticks_x = [x0, (x0 + x1) / 2.0, x1];
    html! {
        <div class="of1-chart">
            <p class={classes!("chart-readout", pick.is_none().then_some("chart-readout-hint"))} aria-live="polite">{ readout }</p>
            <svg class="chart" viewBox={format!("0 0 {w} {h}")} role="img" onpointerdown={onpointer.clone()} onpointermove={onpointer}>
                { for ticks_y.iter().map(|v| html! {
                    <>
                        <line class="chart-grid" x1={ml.to_string()} x2={(w - mr).to_string()} y1={format!("{:.1}", y(*v))} y2={format!("{:.1}", y(*v))} />
                        <text class="chart-axis" x={(ml - 4.0).to_string()} y={format!("{:.1}", y(*v) + 3.5)} text-anchor="end">{ p.fmt_y.emit(*v) }</text>
                    </>
                }) }
                { for ticks_x.iter().enumerate().map(|(i, v)| html! {
                    <text class="chart-axis" x={format!("{:.1}", x(*v))} y={(h - 6.0).to_string()}
                        text-anchor={["start", "middle", "end"][i]}>{ p.fmt_x.emit(*v) }</text>
                }) }
                if let Some(px) = *pick {
                    <line class="chart-crosshair" x1={format!("{:.1}", x(px))} x2={format!("{:.1}", x(px))} y1={mt.to_string()} y2={(h - mb).to_string()} />
                }
                { for p.series.iter().map(|s| html! {
                    <path d={path(s)} fill="none" stroke={s.color.clone()} stroke-width="2" stroke-linejoin="round" />
                }) }
            </svg>
            if p.series.len() > 1 {
                <div class="chart-legend">
                    { for p.series.iter().map(|s| html! {
                        <span><span class="legend-line" style={format!("background:{}", s.color)}></span>{ format!(" {}", s.label) }</span>
                    }) }
                </div>
            }
        </div>
    }
}

// ---------- Années et réunions ----------

#[derive(Properties, PartialEq)]
pub struct YearProps {
    pub year: u32,
}

#[function_component]
pub fn DataYearPage(p: &YearProps) -> Html {
    let meetings = use_json::<Value>(of1("meetings", &format!("year={}", p.year)));
    let years: Vec<u32> = (2023..=current_year()).rev().collect();
    let body = state_view(&meetings).unwrap_or_else(|| {
        let mut list = list(&meetings);
        list.reverse();
        html! {
            <ol class="rows">
                { for list.iter().map(|m| {
                    let key = int(m, "meeting_key").unwrap_or(0) as u32;
                    html! {
                        <li class="row">
                            <Link<Route> to={Route::DataMeeting { key }} classes="row-main">
                                <span class="row-title">{ s(m, "meeting_name") }</span>
                                <span class="row-sub">{ format!("{} · {} · {}", s(m, "location"), s(m, "country_name"), local_date(&s(m, "date_start"), false)) }</span>
                            </Link<Route>>
                        </li>
                    }
                }) }
            </ol>
        }
    });
    html! {
        <Layout title={t("Données OpenF1", "OpenF1 data")} tab={Tab::Archives}>
            <section class="card">
                <h2>{ t("Toutes les données OpenF1", "All OpenF1 data") }</h2>
                <p class="muted">{ t(
                    "Chaque séance depuis 2023 en détail : résultats, grille, championnat, tours, positions, écarts, pneus, arrêts, dépassements, télémétrie, météo, direction de course et radios. Les 18 sources d'OpenF1, mises en cache par le serveur.",
                    "Every session since 2023 in detail: results, grid, championship, laps, positions, gaps, tyres, stops, overtakes, telemetry, weather, race control and radio. All 18 OpenF1 sources, cached by the server.",
                ) }</p>
                <div class="year-pills">
                    { for years.iter().map(|y| html! {
                        <Link<Route> to={Route::DataYear { year: *y }} classes={classes!("pill", (*y == p.year).then_some("pill-on"))}>{ y.to_string() }</Link<Route>>
                    }) }
                </div>
            </section>
            <section class="card">
                <h2>{ tr!("Grands Prix {}", "{} Grands Prix", p.year) }</h2>
                { body }
            </section>
        </Layout>
    }
}

#[derive(Properties, PartialEq)]
pub struct KeyProps {
    pub key_: u32,
}

#[function_component]
pub fn DataMeetingPage(p: &KeyProps) -> Html {
    let meeting = use_json::<Value>(of1("meetings", &format!("meeting_key={}", p.key_)));
    let sessions = use_json::<Value>(of1("sessions", &format!("meeting_key={}", p.key_)));
    let grid = use_json::<Value>(of1("starting_grid", &format!("meeting_key={}", p.key_)));
    let drivers = use_json::<Value>(of1("drivers", &format!("meeting_key={}", p.key_)));
    let m = list(&meeting).first().cloned().unwrap_or(Value::Null);
    let names = driver_map(&list(&drivers));
    html! {
        <Layout title={s(&m, "meeting_name")} tab={Tab::Archives}>
            <section class="card hero">
                <p class="eyebrow">{ format!("{} · {}", s(&m, "location"), s(&m, "country_name")) }</p>
                <h2 class="hero-title">{ s(&m, "meeting_name") }</h2>
                <p class="muted">{ s(&m, "meeting_official_name") }</p>
                { stat_grid(vec![
                    (t("Circuit", "Circuit"), s(&m, "circuit_short_name")),
                    (t("Type", "Type"), s(&m, "circuit_type")),
                    (t("Année", "Year"), s(&m, "year")),
                ]) }
            </section>
            <section class="card">
                <h2>{ t("Séances", "Sessions") }</h2>
                { state_view(&sessions).unwrap_or_else(|| html! {
                    <ol class="rows">
                        { for list(&sessions).iter().map(|x| {
                            let key = int(x, "session_key").unwrap_or(0) as u32;
                            html! {
                                <li class="row">
                                    <Link<Route> to={Route::DataSession { key }} classes="row-main">
                                        <span class="row-title">{ s(x, "session_name") }</span>
                                        <span class="row-sub">{ local_date(&s(x, "date_start"), true) }</span>
                                    </Link<Route>>
                                    <span class="row-meta">{ "→" }</span>
                                </li>
                            }
                        }) }
                    </ol>
                }) }
            </section>
            if !list(&grid).is_empty() {
                <section class="card">
                    <h2>{ t("Grille de départ", "Starting grid") }</h2>
                    <ol class="rows">
                        { for list(&grid).iter().map(|g| {
                            let d = names.get(&int(g, "driver_number").unwrap_or(0));
                            html! {
                                <li class="row" style={d.map(|d| format!("--team:#{}", d.2)).unwrap_or_default()}>
                                    <span class="pos">{ s(g, "position") }</span>
                                    <div class="row-main">
                                        <span class="row-title">{ d.map(|d| d.0.clone()).unwrap_or_else(|| s(g, "driver_number")) }</span>
                                        <span class="row-sub">{ d.map(|d| d.1.clone()).unwrap_or_default() }</span>
                                    </div>
                                    <span class="row-meta">{ num(g, "lap_duration").map(lap_str).unwrap_or_default() }</span>
                                </li>
                            }
                        }) }
                    </ol>
                </section>
            }
        </Layout>
    }
}

/// numéro → (nom, écurie, couleur, acronyme).
fn driver_map(drivers: &[Value]) -> BTreeMap<i64, (String, String, String, String)> {
    drivers
        .iter()
        .filter_map(|d| {
            Some((
                int(d, "driver_number")?,
                (
                    s(d, "full_name"),
                    s(d, "team_name"),
                    s(d, "team_colour"),
                    s(d, "name_acronym"),
                ),
            ))
        })
        .collect()
}

// ---------- Séance ----------

#[derive(Clone, Copy, PartialEq)]
enum View {
    Results,
    Driver,
    Laps,
    Positions,
    Tyres,
    Telemetry,
    Race,
    Radio,
    Weather,
}

#[function_component]
pub fn DataSessionPage(p: &KeyProps) -> Html {
    let key = p.key_;
    let view = use_state(|| View::Results);
    let session = use_json::<Value>(of1("sessions", &format!("session_key={key}")));
    let drivers = use_json::<Value>(of1("drivers", &format!("session_key={key}")));
    let sess = list(&session).first().cloned().unwrap_or(Value::Null);
    let names = Rc::new(driver_map(&list(&drivers)));
    let tab = |v: View, label: &'static str| {
        let view = view.clone();
        html! {
            <button class={classes!("seg", (*view == v).then_some("seg-active"))} onclick={move |_| view.set(v)}>{ label }</button>
        }
    };
    let content = match *view {
        View::Results => html! { <ResultsView key_={key} names={names.clone()} /> },
        View::Driver => html! { <DriverView key_={key} names={names.clone()} /> },
        View::Laps => html! { <LapsView key_={key} names={names.clone()} /> },
        View::Positions => html! { <PositionsView key_={key} names={names.clone()} /> },
        View::Tyres => html! { <TyresView key_={key} names={names.clone()} /> },
        View::Telemetry => html! { <TelemetryView key_={key} names={names.clone()} /> },
        View::Race => html! { <RaceView key_={key} names={names.clone()} /> },
        View::Radio => html! { <RadioView key_={key} names={names.clone()} /> },
        View::Weather => html! { <WeatherView key_={key} /> },
    };
    html! {
        <Layout title={format!("{} · {}", s(&sess, "location"), s(&sess, "session_name"))} tab={Tab::Archives}>
            <section class="card hero">
                <p class="eyebrow">{ format!("{} · {}", s(&sess, "country_name"), s(&sess, "year")) }</p>
                <h2 class="hero-title">{ format!("{} — {}", s(&sess, "location"), s(&sess, "session_name")) }</h2>
                <p class="muted">{ local_date(&s(&sess, "date_start"), true) }</p>
                if let Some(mk) = int(&sess, "meeting_key") {
                    <Link<Route> to={Route::DataMeeting { key: mk as u32 }} classes="link">{ t("← Toutes les séances du week-end", "← All weekend sessions") }</Link<Route>>
                }
            </section>
            <div class="segmented segmented-4 of1-tabs">
                { tab(View::Results, t("Résultats", "Results")) }
                { tab(View::Driver, t("Pilote", "Driver")) }
                { tab(View::Laps, t("Tours", "Laps")) }
                { tab(View::Positions, t("Positions", "Positions")) }
                { tab(View::Tyres, t("Pneus", "Tyres")) }
                { tab(View::Telemetry, t("Télémétrie", "Telemetry")) }
                { tab(View::Race, t("Course", "Race")) }
                { tab(View::Radio, t("Radios", "Radio")) }
                { tab(View::Weather, t("Météo", "Weather")) }
            </div>
            { content }
        </Layout>
    }
}

#[derive(Properties, PartialEq)]
struct ViewProps {
    key_: u32,
    names: Rc<BTreeMap<i64, (String, String, String, String)>>,
}

fn name(names: &BTreeMap<i64, (String, String, String, String)>, n: i64) -> String {
    names
        .get(&n)
        .map(|d| d.0.clone())
        .unwrap_or_else(|| format!("#{n}"))
}
fn code(names: &BTreeMap<i64, (String, String, String, String)>, n: i64) -> String {
    names
        .get(&n)
        .map(|d| d.3.clone())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| format!("#{n}"))
}

/// Résultat de la séance + championnat après la séance.
#[function_component]
fn ResultsView(p: &ViewProps) -> Html {
    let res = use_json::<Value>(of1("session_result", &format!("session_key={}", p.key_)));
    let champ_d = use_json::<Value>(of1(
        "championship_drivers",
        &format!("session_key={}", p.key_),
    ));
    let champ_t = use_json::<Value>(of1(
        "championship_teams",
        &format!("session_key={}", p.key_),
    ));
    let n = &p.names;
    let move_label = |a: Option<i64>, b: Option<i64>| match (a, b) {
        (Some(a), Some(b)) if b < a => format!(" ▲{}", a - b),
        (Some(a), Some(b)) if b > a => format!(" ▼{}", b - a),
        _ => String::new(),
    };
    html! {
        <>
            <section class="card">
                <h2>{ t("Classement de la séance", "Session classification") }</h2>
                { state_view(&res).unwrap_or_else(|| html! {
                    <ol class="rows">
                        { for list(&res).iter().map(|r| {
                            let d = int(r, "driver_number").unwrap_or(0);
                            let flags = [("dnf", "DNF"), ("dns", "DNS"), ("dsq", "DSQ")].iter()
                                .filter(|(k, _)| r.get(*k).and_then(Value::as_bool).unwrap_or(false)).map(|(_, l)| *l).collect::<Vec<_>>().join(" ");
                            let gap = match r.get("gap_to_leader") {
                                Some(Value::Number(x)) if x.as_f64() == Some(0.0) => num(r, "duration").map(lap_str).unwrap_or_default(),
                                Some(Value::Number(x)) => format!("+{:.3}", x.as_f64().unwrap_or(0.0)),
                                Some(Value::String(x)) => x.clone(),
                                Some(Value::Array(a)) => a.last().and_then(Value::as_f64).map(|x| format!("+{x:.3}")).unwrap_or_default(),
                                _ => String::new(),
                            };
                            html! {
                                <li class="row" style={n.get(&d).map(|x| format!("--team:#{}", x.2)).unwrap_or_default()}>
                                    <span class="pos">{ s(r, "position") }</span>
                                    <div class="row-main">
                                        <span class="row-title">{ name(n, d) }</span>
                                        <span class="row-sub">{ format!("{}{}{}", n.get(&d).map(|x| x.1.clone()).unwrap_or_default(),
                                            int(r, "number_of_laps").map(|l| tr!(" · {} tours", " · {} laps", l)).unwrap_or_default(),
                                            if flags.is_empty() { String::new() } else { format!(" · {flags}") }) }</span>
                                    </div>
                                    <span class="row-meta">{ gap }{ num(r, "points").filter(|x| *x > 0.0).map(|x| format!(" · {x} pts")).unwrap_or_default() }</span>
                                </li>
                            }
                        }) }
                    </ol>
                }) }
            </section>
            if !list(&champ_d).is_empty() {
                <section class="card">
                    <h2>{ t("Championnat pilotes après la séance", "Drivers' championship after the session") }</h2>
                    <ol class="rows">
                        { for list(&champ_d).iter().take(22).map(|c| {
                            let d = int(c, "driver_number").unwrap_or(0);
                            let gained = num(c, "points_current").unwrap_or(0.0) - num(c, "points_start").unwrap_or(0.0);
                            html! {
                                <li class="row" style={n.get(&d).map(|x| format!("--team:#{}", x.2)).unwrap_or_default()}>
                                    <span class="pos">{ s(c, "position_current") }</span>
                                    <div class="row-main">
                                        <span class="row-title">{ name(n, d) }{ move_label(int(c, "position_start"), int(c, "position_current")) }</span>
                                        <span class="row-sub">{ if gained > 0.0 { tr!("+{} pts sur la séance", "+{} pts this session", gained) } else { String::new() } }</span>
                                    </div>
                                    <span class="row-meta">{ format!("{} pts", s(c, "points_current")) }</span>
                                </li>
                            }
                        }) }
                    </ol>
                </section>
            }
            if !list(&champ_t).is_empty() {
                <section class="card">
                    <h2>{ t("Championnat constructeurs", "Constructors' championship") }</h2>
                    <ol class="rows">
                        { for list(&champ_t).iter().map(|c| html! {
                            <li class="row">
                                <span class="pos">{ s(c, "position_current") }</span>
                                <div class="row-main">
                                    <span class="row-title">{ s(c, "team_name") }{ move_label(int(c, "position_start"), int(c, "position_current")) }</span>
                                </div>
                                <span class="row-meta">{ format!("{} pts", s(c, "points_current")) }</span>
                            </li>
                        }) }
                    </ol>
                </section>
            }
        </>
    }
}

/// Premiers du classement (ou des pilotes connus) pour les graphiques.
fn top_drivers(
    res: &Json,
    names: &BTreeMap<i64, (String, String, String, String)>,
    k: usize,
) -> Vec<i64> {
    let from_res: Vec<i64> = list(res)
        .iter()
        .filter_map(|r| int(r, "driver_number"))
        .take(k)
        .collect();
    if from_res.is_empty() {
        names.keys().copied().take(k).collect()
    } else {
        from_res
    }
}

/// Temps au tour de chaque pilote, meilleurs secteurs.
#[function_component]
fn LapsView(p: &ViewProps) -> Html {
    let laps = use_json::<Value>(of1("laps", &format!("session_key={}", p.key_)));
    let res = use_json::<Value>(of1("session_result", &format!("session_key={}", p.key_)));
    if let Some(v) = state_view(&laps) {
        return html! { <section class="card">{ v }</section> };
    }
    let all = list(&laps);
    let top = top_drivers(&res, &p.names, 5);
    // Médiane pour écarter les tours anormaux (stands, voiture de sécurité).
    let mut durations: Vec<f64> = all.iter().filter_map(|l| num(l, "lap_duration")).collect();
    durations.sort_by(f64::total_cmp);
    let median = durations.get(durations.len() / 2).copied().unwrap_or(100.0);
    let series: Vec<Series> = top
        .iter()
        .enumerate()
        .map(|(i, d)| Series {
            label: code(&p.names, *d),
            color: SERIES[i % SERIES.len()].into(),
            points: all
                .iter()
                .filter(|l| int(l, "driver_number") == Some(*d))
                .filter_map(|l| Some((int(l, "lap_number")? as f64, num(l, "lap_duration")?)))
                .filter(|(_, v)| *v < median * 1.12)
                .collect(),
        })
        .collect();
    let best = |k: &str| {
        all.iter()
            .filter_map(|l| Some((num(l, k)?, int(l, "driver_number")?)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    };
    let fastest = best("lap_duration");
    let speed_trap = all
        .iter()
        .filter_map(|l| Some((num(l, "st_speed")?, int(l, "driver_number")?)))
        .max_by(|a, b| a.0.total_cmp(&b.0));
    html! {
        <>
            <section class="card">
                <h2>{ t("Temps au tour", "Lap times") }</h2>
                <LineChart series={Rc::new(series)} fmt_x={Callback::from(|v: f64| tr!("T{}", "L{}", v.round()))} fmt_y={Callback::from(lap_str)} />
                <p class="muted">{ t("5 premiers de la séance ; tours lents (stands, neutralisations) écartés.", "Top 5 of the session; slow laps (pits, neutralisations) left out.") }</p>
            </section>
            <section class="card">
                <h2>{ t("Records de la séance", "Session bests") }</h2>
                { stat_grid(vec![
                    (t("Meilleur tour", "Fastest lap"), fastest.map(|(v, d)| format!("{} {}", lap_str(v), code(&p.names, d))).unwrap_or_default()),
                    (t("Secteur 1", "Sector 1"), best("duration_sector_1").map(|(v, d)| format!("{v:.3} {}", code(&p.names, d))).unwrap_or_default()),
                    (t("Secteur 2", "Sector 2"), best("duration_sector_2").map(|(v, d)| format!("{v:.3} {}", code(&p.names, d))).unwrap_or_default()),
                    (t("Secteur 3", "Sector 3"), best("duration_sector_3").map(|(v, d)| format!("{v:.3} {}", code(&p.names, d))).unwrap_or_default()),
                    (t("Pointe (radar)", "Speed trap"), speed_trap.map(|(v, d)| format!("{v:.0} {}", code(&p.names, d))).unwrap_or_default()),
                    (t("Tours", "Laps"), all.iter().filter_map(|l| int(l, "lap_number")).max().unwrap_or(0).to_string()),
                ]) }
            </section>
        </>
    }
}

/// Positions au fil de la séance et écart au leader.
#[function_component]
fn PositionsView(p: &ViewProps) -> Html {
    let pos = use_json::<Value>(of1("position", &format!("session_key={}", p.key_)));
    let res = use_json::<Value>(of1("session_result", &format!("session_key={}", p.key_)));
    let top = top_drivers(&res, &p.names, 5);
    let first_driver = top.first().copied().unwrap_or(1);
    let second_driver = top.get(1).copied().unwrap_or(first_driver);
    let int_a = use_json::<Value>(of1(
        "intervals",
        &format!("session_key={}&driver_number={}", p.key_, second_driver),
    ));
    let int_b = use_json::<Value>(of1(
        "intervals",
        &format!(
            "session_key={}&driver_number={}",
            p.key_,
            top.get(2).copied().unwrap_or(second_driver)
        ),
    ));
    if let Some(v) = state_view(&pos) {
        return html! { <section class="card">{ v }</section> };
    }
    let all = list(&pos);
    let t0 = all.first().map(|x| ms(&s(x, "date"))).unwrap_or(0.0);
    let minutes = move |iso: &str| (ms(iso) - t0) / 60_000.0;
    let series: Vec<Series> = top
        .iter()
        .enumerate()
        .map(|(i, d)| Series {
            label: code(&p.names, *d),
            color: SERIES[i % SERIES.len()].into(),
            points: all
                .iter()
                .filter(|x| int(x, "driver_number") == Some(*d))
                .filter_map(|x| Some((minutes(&s(x, "date")), num(x, "position")?)))
                .collect(),
        })
        .collect();
    let gaps: Vec<Series> = [(&int_a, top.get(1)), (&int_b, top.get(2))]
        .iter()
        .enumerate()
        .filter_map(|(i, (v, d))| {
            let d = **d.as_ref()?;
            Some(Series {
                label: code(&p.names, d),
                color: SERIES[(i + 1) % SERIES.len()].into(),
                points: list(v)
                    .iter()
                    .filter_map(|x| Some((minutes(&s(x, "date")), num(x, "gap_to_leader")?)))
                    .filter(|(_, g)| *g < 120.0)
                    .collect(),
            })
        })
        .collect();
    html! {
        <>
            <section class="card">
                <h2>{ t("Positions au fil de la séance", "Positions through the session") }</h2>
                <LineChart series={Rc::new(series)} invert=true step=true fmt_x={Callback::from(|v: f64| format!("{v:.0} min"))} fmt_y={Callback::from(|v: f64| format!("P{}", v.round()))} />
            </section>
            <section class="card">
                <h2>{ tr!("Écart avec {}", "Gap to {}", code(&p.names, first_driver)) }</h2>
                <LineChart series={Rc::new(gaps)} fmt_x={Callback::from(|v: f64| format!("{v:.0} min"))} fmt_y={Callback::from(|v: f64| format!("{v:.1} s"))} />
            </section>
        </>
    }
}

/// Relais de pneus et arrêts aux stands.
#[function_component]
fn TyresView(p: &ViewProps) -> Html {
    let stints = use_json::<Value>(of1("stints", &format!("session_key={}", p.key_)));
    let res = use_json::<Value>(of1("session_result", &format!("session_key={}", p.key_)));
    if let Some(v) = state_view(&stints) {
        return html! { <section class="card">{ v }</section> };
    }
    let order = top_drivers(&res, &p.names, 22);
    let all = list(&stints);
    let total = all
        .iter()
        .filter_map(|x| int(x, "lap_end"))
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let colour = |c: &str| match c {
        "SOFT" => "#e10600",
        "MEDIUM" => "#f5c518",
        "HARD" => "#ededed",
        "INTERMEDIATE" => "#2fb34a",
        "WET" => "#2f7fe0",
        _ => "#777",
    };
    html! {
        <>
            <section class="card">
                <h2>{ t("Stratégie des pneus", "Tyre strategy") }</h2>
                <ol class="strategy">
                    { for order.iter().map(|d| html! {
                        <li class="strat-row">
                            <span class="strat-code">{ code(&p.names, *d) }</span>
                            <span class="strat-bar">
                                { for all.iter().filter(|x| int(x, "driver_number") == Some(*d)).map(|x| {
                                    let (a, b) = (int(x, "lap_start").unwrap_or(1) as f64, int(x, "lap_end").unwrap_or(1) as f64);
                                    html! { <span class="strat-stint" title={s(x, "compound")}
                                        style={format!("width:{:.2}%;background:{}", (b - a + 1.0) / total * 100.0, colour(&s(x, "compound")))}></span> }
                                }) }
                            </span>
                        </li>
                    }) }
                </ol>
                <p class="muted">{ t("Rouge tendre · jaune medium · blanc dur · vert intermédiaire · bleu pluie.", "Red soft · yellow medium · white hard · green intermediate · blue wet.") }</p>
            </section>
            <PitDetail query={format!("session_key={}", p.key_)} />
        </>
    }
}

/// Télémétrie comparée du meilleur tour (car_data) et tracé GPS (location).
#[function_component]
fn TelemetryView(p: &ViewProps) -> Html {
    let res = use_json::<Value>(of1("session_result", &format!("session_key={}", p.key_)));
    let top = top_drivers(&res, &p.names, 2);
    let a = use_state(|| None::<i64>);
    let b = use_state(|| None::<i64>);
    let da = (*a).or(top.first().copied());
    let db = (*b).or(top.get(1).copied());
    let url = match (da, db) {
        (Some(x), Some(y)) => Some(format!(
            "/api/of1/telemetry?session_key={}&drivers={x},{y}",
            p.key_
        )),
        _ => None,
    };
    let tel = use_json::<Value>(url);
    let select = |state: UseStateHandle<Option<i64>>, current: Option<i64>| {
        let onchange = Callback::from(move |e: Event| {
            if let Some(sel) = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
            {
                state.set(sel.value().parse().ok());
            }
        });
        html! {
            <label class="select">
                <select {onchange}>
                    { for p.names.iter().map(|(n, d)| html! { <option value={n.to_string()} selected={Some(*n) == current}>{ d.0.clone() }</option> }) }
                </select>
            </label>
        }
    };
    let body = state_view(&tel).unwrap_or_else(|| {
        let drivers = list(&tel);
        let make = |field: &str| -> Rc<Vec<Series>> {
            Rc::new(drivers.iter().enumerate().map(|(i, d)| {
                let dist: Vec<f64> = d.get("distance").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
                let vals: Vec<f64> = d.get(field).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
                Series {
                    label: code(&p.names, int(d, "driver_number").unwrap_or(0)),
                    color: SERIES[i % 2].into(),
                    points: dist.into_iter().zip(vals).map(|(x, y)| (x / 1000.0, y)).collect(),
                }
            }).collect())
        };
        let laps = drivers.iter().map(|d| format!("{} {} (T{})", code(&p.names, int(d, "driver_number").unwrap_or(0)), num(d, "lap_duration").map(lap_str).unwrap_or_default(), s(d, "lap_number"))).collect::<Vec<_>>().join(" · ");
        let km = Callback::from(|v: f64| format!("{v:.1} km"));
        html! {
            <>
                <p class="muted">{ laps }</p>
                <h3 class="wx-sub">{ t("Vitesse (km/h)", "Speed (km/h)") }</h3>
                <LineChart series={make("speed")} fmt_x={km.clone()} fmt_y={Callback::from(|v: f64| format!("{v:.0}"))} />
                <h3 class="wx-sub">{ t("Accélérateur (%)", "Throttle (%)") }</h3>
                <LineChart series={make("throttle")} height={120.0} fmt_x={km.clone()} fmt_y={Callback::from(|v: f64| format!("{v:.0}%"))} />
                <h3 class="wx-sub">{ t("Rapport engagé", "Gear") }</h3>
                <LineChart series={make("gear")} height={110.0} step=true fmt_x={km} fmt_y={Callback::from(|v: f64| format!("{v:.0}"))} />
            </>
        }
    });
    html! {
        <section class="card">
            <h2>{ t("Télémétrie : meilleurs tours comparés", "Telemetry: fastest laps compared") }</h2>
            <div class="tel-pick">{ select(a.clone(), da) }{ select(b.clone(), db) }</div>
            { body }
            <p class="muted">{ t("Données car_data OpenF1 (~4 mesures/s), alignées sur la distance parcourue.", "OpenF1 car_data (~4 samples/s), aligned on distance travelled.") }</p>
        </section>
    }
}

/// Fiche course d'un pilote : résumé, tour par tour (temps, secteurs, vitesses, position,
/// écarts, pneus, arrêts), télémétrie du tour choisi (vitesse, gaz, frein, rapport, régime,
/// DRS), messages de la direction de course et radios.
#[function_component]
fn DriverView(p: &ViewProps) -> Html {
    let res = use_json::<Value>(of1("session_result", &format!("session_key={}", p.key_)));
    let top = top_drivers(&res, &p.names, 1);
    let pick = use_state(|| None::<i64>);
    let lap = use_state(|| None::<u32>);
    let driver = (*pick).or(top.first().copied());
    let data = use_json::<Value>(
        driver.map(|d| format!("/api/of1/driverrace?session_key={}&driver={d}", p.key_)),
    );
    let laps = data
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|v| v.get("laps").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    // Tour analysé : celui choisi, sinon le meilleur tour (hors sorties des stands).
    let best_lap = laps
        .iter()
        .filter(|l| !l.get("pit_out").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|l| Some((num(l, "time")?, int(l, "lap")? as u32)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|x| x.1);
    let chosen = (*lap).or(best_lap);
    let tel = use_json::<Value>(match (driver, chosen) {
        (Some(d), Some(n)) => Some(format!(
            "/api/of1/telemetry?session_key={}&drivers={d}&lap={n}",
            p.key_
        )),
        _ => None,
    });
    let on_driver = {
        let (pick, lap) = (pick.clone(), lap.clone());
        Callback::from(move |e: Event| {
            if let Some(sel) = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
            {
                pick.set(sel.value().parse().ok());
                lap.set(None);
            }
        })
    };
    let on_lap = {
        let lap = lap.clone();
        Callback::from(move |e: Event| {
            if let Some(sel) = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
            {
                lap.set(sel.value().parse().ok());
            }
        })
    };
    let picker = html! {
        <label class="select">
            <select onchange={on_driver}>
                { for p.names.iter().map(|(n, d)| html! { <option value={n.to_string()} selected={Some(*n) == driver}>{ d.0.clone() }</option> }) }
            </select>
        </label>
    };
    let Some(Ok(v)) = &data else {
        return html! {
            <section class="card">
                <h2>{ t("Fiche course du pilote", "Driver race file") }</h2>
                { picker }
                { state_view(&data).unwrap_or_else(loading) }
            </section>
        };
    };
    let v = v.as_ref();
    let pos = |k: &str| {
        int(v, k)
            .map(|p| format!("P{p}"))
            .unwrap_or_else(|| "–".into())
    };
    let champ = match (int(v, "champ_before"), int(v, "champ_after")) {
        (Some(a), Some(b)) => format!("P{a} → P{b}"),
        _ => "–".into(),
    };
    let champ_pts = match (num(v, "champ_points_before"), num(v, "champ_points_after")) {
        (Some(a), Some(b)) => format!("{a:.0} → {b:.0} pts"),
        _ => "–".into(),
    };
    let finish = if s(v, "status").is_empty() {
        pos("finish")
    } else {
        s(v, "status")
    };
    // Graphique des temps au tour (tours lents écartés : arrêts, voiture de sécurité).
    let fastest = laps
        .iter()
        .filter_map(|l| num(l, "time"))
        .fold(f64::MAX, f64::min);
    let times = Rc::new(vec![Series {
        label: s(v, "code"),
        color: format!("#{}", s(v, "colour")),
        points: laps
            .iter()
            .filter_map(|l| {
                Some((
                    num(l, "lap")?,
                    num(l, "time").filter(|t| *t < fastest * 1.1)?,
                ))
            })
            .collect(),
    }]);
    let tyre_dot = |c: &str| html! { <i class="tyre-dot" style={format!("background:{}", tyre_colour(c))}></i> };
    let gap_str = |x: &Value, k: &str| match x.get(k) {
        Some(Value::Number(n)) => n
            .as_f64()
            .map(|g| {
                if g == 0.0 {
                    "—".to_string()
                } else {
                    format!("+{g:.1} s")
                }
            })
            .unwrap_or_default(),
        Some(Value::String(t0)) => t0.clone(),
        _ => "–".into(),
    };
    let tel_body = state_view(&tel).unwrap_or_else(|| {
        let first = list(&tel).into_iter().next().unwrap_or(Value::Null);
        let series = |field: &str, color: &str| -> Rc<Vec<Series>> {
            let dist: Vec<f64> = first.get("distance").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
            let vals: Vec<f64> = first.get(field).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
            Rc::new(vec![Series { label: field.into(), color: color.into(), points: dist.into_iter().zip(vals).map(|(x, y)| (x / 1000.0, y)).collect() }])
        };
        let km = Callback::from(|v: f64| format!("{v:.1} km"));
        let n0 = |v: f64| format!("{v:.0}");
        // Pas de DRS depuis 2026 : graphique masqué quand la donnée n'existe pas.
        let has_drs = first.get("drs").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
        html! {
            <>
                <h3 class="wx-sub">{ t("Vitesse (km/h)", "Speed (km/h)") }</h3>
                <LineChart series={series("speed", SERIES[0])} fmt_x={km.clone()} fmt_y={Callback::from(n0)} />
                <h3 class="wx-sub">{ t("Régime moteur (tr/min)", "Engine speed (rpm)") }</h3>
                <LineChart series={series("rpm", SERIES[1])} height={120.0} fmt_x={km.clone()} fmt_y={Callback::from(n0)} />
                <h3 class="wx-sub">{ t("Accélérateur (%)", "Throttle (%)") }</h3>
                <LineChart series={series("throttle", SERIES[2])} height={110.0} fmt_x={km.clone()} fmt_y={Callback::from(|v: f64| format!("{v:.0}%"))} />
                <h3 class="wx-sub">{ t("Freinage", "Braking") }</h3>
                <LineChart series={series("brake", SERIES[4])} height={80.0} step=true fmt_x={km.clone()} fmt_y={Callback::from(|v: f64| if v > 50.0 { t("oui", "on").to_string() } else { t("non", "off").to_string() })} />
                <h3 class="wx-sub">{ t("Rapport engagé", "Gear") }</h3>
                <LineChart series={series("gear", SERIES[3])} height={110.0} step=true fmt_x={km.clone()} fmt_y={Callback::from(n0)} />
                if has_drs {
                    <h3 class="wx-sub">{ t("DRS (aileron ouvert)", "DRS (flap open)") }</h3>
                    <LineChart series={series("drs", SERIES[0])} height={80.0} step=true fmt_x={km} fmt_y={Callback::from(|v: f64| if v > 75.0 { t("ouvert", "open").to_string() } else if v > 25.0 { t("autorisé", "armed").to_string() } else { t("fermé", "closed").to_string() })} />
                }
            </>
        }
    });
    let messages = v
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let radios = v
        .get("radio")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    html! {
        <>
            <section class="card" style={format!("--team:#{}", s(v, "colour"))}>
                <h2>{ t("Fiche course du pilote", "Driver race file") }</h2>
                { picker }
                <p class="drv-title">{ s(v, "name") }<small class="muted">{ format!(" · {}", s(v, "team")) }</small></p>
                { stat_grid(vec![
                    (t("Départ → arrivée", "Start → finish"), format!("{} → {}", pos("grid"), finish)),
                    (t("Points", "Points"), num(v, "points").map(|x| format!("{x:.0}")).unwrap_or_else(|| "0".into())),
                    (t("Championnat", "Championship"), champ),
                    (t("Points au championnat", "Championship points"), champ_pts),
                    (t("Dépassements faits / subis", "Overtakes made / lost"), format!("{} / {}", s(v, "overtakes_made"), s(v, "overtakes_lost"))),
                    (t("Arrêts aux stands", "Pit stops"), s(v, "pit_count")),
                ]) }
            </section>
            <section class="card">
                <h2>{ t("Temps au tour", "Lap times") }</h2>
                <LineChart series={times} fmt_x={Callback::from(|v: f64| tr!("T{v:.0}", "L{v:.0}"))} fmt_y={Callback::from(lap_str)} />
                <p class="muted">{ t("Tours lents (arrêts, voiture de sécurité) écartés du graphique.", "Slow laps (pit stops, safety car) left out of the chart.") }</p>
            </section>
            <section class="card">
                <h2>{ tr!("Tour par tour ({})", "Lap by lap ({})", laps.len()) }</h2>
                <ol class="rows lap-rows">
                    { for laps.iter().map(|l| {
                        let sec = |k: &str| num(l, k).map(|x| format!("{x:.1}")).unwrap_or_else(|| "–".into());
                        let spd = |k: &str| int(l, k).map(|x| x.to_string()).unwrap_or_else(|| "–".into());
                        let age = int(l, "tyre_age").map(|a| tr!(" · {a} t.", " · {a} laps")).unwrap_or_default();
                        let badge = if l.get("pit_in").and_then(Value::as_bool).unwrap_or(false) {
                            num(l, "stop").map(|x| tr!("🔧 Arrêt {x:.1} s", "🔧 Stop {x:.1} s")).unwrap_or_else(|| t("🔧 Arrêt", "🔧 Pit").into())
                        } else if l.get("pit_out").and_then(Value::as_bool).unwrap_or(false) {
                            t("🔧 Sortie", "🔧 Out").into()
                        } else { String::new() };
                        html! {
                            <li class="row lap-row">
                                <span class="pos">{ format!("T{}", s(l, "lap")) }</span>
                                <div class="row-main">
                                    <span class="row-title">
                                        { num(l, "time").map(lap_str).unwrap_or_else(|| "–".into()) }
                                        <small class="muted">{ format!("  ·  P{}", s(l, "position")) }</small>
                                        if !badge.is_empty() { <span class="lap-badge">{ badge }</span> }
                                    </span>
                                    <span class="row-sub">{ format!("S1 {} · S2 {} · S3 {} · {}/{}/{} km/h", sec("s1"), sec("s2"), sec("s3"), spd("i1"), spd("i2"), spd("st")) }</span>
                                    <span class="row-sub">{ tyre_dot(&s(l, "compound")) }{ format!("{}{}", tyre_name(&s(l, "compound")), age) }{ tr!(" · leader {} · devant {}", " · leader {} · ahead {}", gap_str(l, "gap"), gap_str(l, "interval")) }</span>
                                </div>
                            </li>
                        }
                    }) }
                </ol>
            </section>
            <section class="card">
                <h2>{ t("Télémétrie d'un tour", "Lap telemetry") }</h2>
                <label class="select">
                    <select onchange={on_lap}>
                        { for laps.iter().filter_map(|l| int(l, "lap")).map(|n| html! {
                            <option value={n.to_string()} selected={Some(n as u32) == chosen}>{ if Some(n as u32) == best_lap { tr!("Tour {n} (meilleur)", "Lap {n} (fastest)") } else { tr!("Tour {n}", "Lap {n}") } }</option>
                        }) }
                    </select>
                </label>
                { tel_body }
                <p class="muted">{ t("Données car_data OpenF1 (~4 mesures/s) sur la distance du tour. DRS « autorisé » : le pilote pourra l'ouvrir dans la prochaine zone.", "OpenF1 car_data (~4 samples/s) over the lap distance. DRS “armed”: the driver can open it in the next zone.") }</p>
            </section>
            if let Some(first) = list(&tel).into_iter().next() {
                <section class="card">
                    <h2>{ match chosen { Some(n) => tr!("Replay du tour {n}", "Lap {n} replay"), None => t("Replay du tour", "Lap replay").to_string() } }</h2>
                    <LapReplay tel={Rc::new(first)} colour={s(v, "colour")} />
                </section>
            }
            if !messages.is_empty() {
                <section class="card">
                    <h2>{ tr!("Direction de course ({})", "Race control ({})", messages.len()) }</h2>
                    <ol class="rows">
                        { for messages.iter().map(|m| html! {
                            <li class="row"><div class="row-main">
                                <span class="row-title">{ s(m, "message") }</span>
                                <span class="row-sub">{ int(m, "lap").map(|n| tr!("Tour {n}", "Lap {n}")).unwrap_or_default() }{ format!(" · {}", local_date(&s(m, "date"), true)) }</span>
                            </div></li>
                        }) }
                    </ol>
                </section>
            }
            if !radios.is_empty() {
                <section class="card">
                    <h2>{ tr!("Radios ({})", "Radio ({})", radios.len()) }</h2>
                    <ol class="rows">
                        { for radios.iter().map(|r| html! {
                            <li class="row radio-row"><div class="row-main">
                                <span class="row-sub">{ local_date(&s(r, "date"), true) }</span>
                                <audio controls=true preload="none" src={s(r, "url")}></audio>
                            </div></li>
                        }) }
                    </ol>
                </section>
            }
        </>
    }
}

/// Dépassements, direction de course.
#[function_component]
fn RaceView(p: &ViewProps) -> Html {
    let overtakes = use_json::<Value>(of1("overtakes", &format!("session_key={}", p.key_)));
    let rc = use_json::<Value>(of1("race_control", &format!("session_key={}", p.key_)));
    let ov = list(&overtakes);
    let mut count: BTreeMap<i64, usize> = BTreeMap::new();
    for o in &ov {
        if let Some(d) = int(o, "overtaking_driver_number") {
            *count.entry(d).or_default() += 1;
        }
    }
    let mut ranking: Vec<(i64, usize)> = count.into_iter().collect();
    ranking.sort_by_key(|x| std::cmp::Reverse(x.1));
    html! {
        <>
            <section class="card">
                <h2>{ tr!("Dépassements ({})", "Overtakes ({})", ov.len()) }</h2>
                { state_view(&overtakes).unwrap_or_else(|| html! {
                    <>
                        <ol class="rows">
                            { for ranking.iter().take(8).map(|(d, c)| html! {
                                <li class="row">
                                    <div class="row-main"><span class="row-title">{ name(&p.names, *d) }</span></div>
                                    <span class="row-meta">{ tr!("{} dépassements", "{} overtakes", c) }</span>
                                </li>
                            }) }
                        </ol>
                        <details class="of1-details">
                            <summary>{ t("Tous les dépassements", "Every overtake") }</summary>
                            <ol class="rows">
                                { for ov.iter().map(|o| html! {
                                    <li class="row">
                                        <div class="row-main">
                                            <span class="row-title">{ format!("{} → {}", code(&p.names, int(o, "overtaking_driver_number").unwrap_or(0)), code(&p.names, int(o, "overtaken_driver_number").unwrap_or(0))) }</span>
                                            <span class="row-sub">{ local_date(&s(o, "date"), true) }</span>
                                        </div>
                                        <span class="row-meta">{ format!("P{}", s(o, "position")) }</span>
                                    </li>
                                }) }
                            </ol>
                        </details>
                    </>
                }) }
            </section>
            <section class="card">
                <h2>{ t("Direction de course", "Race control") }</h2>
                { state_view(&rc).unwrap_or_else(|| html! {
                    <ol class="rows">
                        { for list(&rc).iter().rev().map(|m| html! {
                            <li class="row">
                                <div class="row-main">
                                    <span class="row-title">{ s(m, "message") }</span>
                                    <span class="row-sub">{ [s(m, "category"), s(m, "flag"), int(m, "lap_number").map(|l| tr!("tour {}", "lap {}", l)).unwrap_or_default()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ") }</span>
                                </div>
                            </li>
                        }) }
                    </ol>
                }) }
            </section>
        </>
    }
}

/// Radios d'équipe (fichiers audio publiés par la F1, liés tels quels).
#[function_component]
fn RadioView(p: &ViewProps) -> Html {
    let radio = use_json::<Value>(of1("team_radio", &format!("session_key={}", p.key_)));
    let items = list(&radio);
    html! {
        <section class="card">
            <h2>{ tr!("Radios d'équipe ({})", "Team radio ({})", items.len()) }</h2>
            { state_view(&radio).unwrap_or_else(|| html! {
                <ol class="rows">
                    { for items.iter().rev().take(60).map(|r| {
                        let d = int(r, "driver_number").unwrap_or(0);
                        html! {
                            <li class="row radio-row" style={p.names.get(&d).map(|x| format!("--team:#{}", x.2)).unwrap_or_default()}>
                                <div class="row-main">
                                    <span class="row-title">{ name(&p.names, d) }</span>
                                    <span class="row-sub">{ local_date(&s(r, "date"), true) }</span>
                                    <audio controls=true preload="none" src={s(r, "recording_url")}></audio>
                                </div>
                            </li>
                        }
                    }) }
                </ol>
            }) }
            <p class="muted">{ t("Enregistrements publiés par la Formula 1 et référencés par OpenF1 (60 plus récents).", "Recordings published by Formula 1 and referenced by OpenF1 (latest 60).") }</p>
        </section>
    }
}

/// Météo de la séance (air, piste, vent, pluie).
#[function_component]
fn WeatherView(p: &KeyProps) -> Html {
    let w = use_json::<Value>(of1("weather", &format!("session_key={}", p.key_)));
    if let Some(v) = state_view(&w) {
        return html! { <section class="card">{ v }</section> };
    }
    let all = list(&w);
    let t0 = all.first().map(|x| ms(&s(x, "date"))).unwrap_or(0.0);
    let pts = |k: &str| {
        all.iter()
            .filter_map(|x| Some(((ms(&s(x, "date")) - t0) / 60_000.0, num(x, k)?)))
            .collect::<Vec<_>>()
    };
    let temps = Rc::new(vec![
        Series {
            label: t("Piste", "Track").into(),
            color: SERIES[1].into(),
            points: pts("track_temperature"),
        },
        Series {
            label: "Air".into(),
            color: SERIES[0].into(),
            points: pts("air_temperature"),
        },
    ]);
    let wind = Rc::new(vec![Series {
        label: t("Vent", "Wind").into(),
        color: SERIES[2].into(),
        points: pts("wind_speed")
            .into_iter()
            .map(|(a, b)| (a, b * 3.6))
            .collect(),
    }]);
    let rain = all.iter().any(|x| num(x, "rainfall").unwrap_or(0.0) > 0.0);
    let last = all.last().cloned().unwrap_or(Value::Null);
    let min = Callback::from(|v: f64| format!("{v:.0} min"));
    html! {
        <section class="card">
            <h2>{ t("Météo de la séance", "Session weather") }</h2>
            { stat_grid(vec![
                ("Air", num(&last, "air_temperature").map(|v| format!("{v:.1}°")).unwrap_or_default()),
                (t("Piste", "Track"), num(&last, "track_temperature").map(|v| format!("{v:.1}°")).unwrap_or_default()),
                (t("Humidité", "Humidity"), num(&last, "humidity").map(|v| format!("{v:.0}%")).unwrap_or_default()),
                (t("Pression", "Pressure"), num(&last, "pressure").map(|v| format!("{v:.0} hPa")).unwrap_or_default()),
                (t("Vent", "Wind"), num(&last, "wind_speed").map(|v| format!("{:.0} km/h", v * 3.6)).unwrap_or_default()),
                (t("Pluie", "Rain"), if rain { t("oui", "yes").into() } else { t("non", "no").into() }),
            ]) }
            <h3 class="wx-sub">{ t("Températures (°C)", "Temperatures (°C)") }</h3>
            <LineChart series={temps} fmt_x={min.clone()} fmt_y={Callback::from(|v: f64| format!("{v:.0}°"))} />
            <h3 class="wx-sub">{ t("Vent (km/h)", "Wind (km/h)") }</h3>
            <LineChart series={wind} height={120.0} fmt_x={min} fmt_y={Callback::from(|v: f64| format!("{v:.0}"))} />
        </section>
    }
}

/// Lien « Analyse OpenF1 » d'une page de Grand Prix : retrouve la réunion d'après la date.
#[derive(Properties, PartialEq)]
pub struct MeetingLinkProps {
    pub year: u32,
    /// Date de la course « 2025-07-06 ».
    pub date: AttrValue,
}

#[function_component]
pub fn MeetingLink(p: &MeetingLinkProps) -> Html {
    let meetings =
        use_json::<Value>((p.year >= 2023).then(|| format!("/api/of1/meetings?year={}", p.year)));
    let race = ms(&format!("{}T12:00:00Z", p.date));
    let found = list(&meetings).into_iter().find(|m| {
        let start = ms(&s(m, "date_start"));
        race >= start - 86_400_000.0 && race <= start + 5.0 * 86_400_000.0
    });
    match found.and_then(|m| int(&m, "meeting_key")) {
        Some(key) => html! {
            <Link<Route> to={Route::DataMeeting { key: key as u32 }} classes="btn btn-ghost">
                { t("📊 Analyse détaillée (données OpenF1)", "📊 Detailed analysis (OpenF1 data)") }
            </Link<Route>>
        },
        None => html! {},
    }
}

/// « Tout savoir sur la course » en haut de la page d'un Grand Prix terminé : la fiche de
/// chaque pilote, les tours, positions, pneus, télémétrie, direction de course… affichés
/// directement dans la page, un bouton par rubrique.
#[function_component]
pub fn RaceDataLinks(p: &MeetingLinkProps) -> Html {
    let view = use_state(|| View::Driver);
    let sessions = use_json::<Value>(
        (p.year >= 2023).then(|| format!("/api/of1/sessions?year={}&session_name=Race", p.year)),
    );
    let race = ms(&format!("{}T12:00:00Z", p.date));
    let key = list(&sessions)
        .into_iter()
        .find(|x| (ms(&s(x, "date_start")) - race).abs() <= 2.0 * 86_400_000.0)
        .and_then(|x| int(&x, "session_key"))
        .map(|k| k as u32);
    let drivers = use_json::<Value>(key.and_then(|k| of1("drivers", &format!("session_key={k}"))));
    let Some(key) = key else {
        return html! {};
    };
    let names = Rc::new(driver_map(&list(&drivers)));
    let tab = |v: View, label: &'static str| {
        let view = view.clone();
        html! {
            <button class={classes!("btn", "btn-ghost", "race-data-btn", (*view == v).then_some("on"))}
                onclick={move |_| view.set(v)}>{ label }</button>
        }
    };
    let content = match *view {
        View::Results => html! { <ResultsView key_={key} names={names.clone()} /> },
        View::Driver => html! { <DriverView key_={key} names={names.clone()} /> },
        View::Laps => html! { <LapsView key_={key} names={names.clone()} /> },
        View::Positions => html! { <PositionsView key_={key} names={names.clone()} /> },
        View::Tyres => html! { <TyresView key_={key} names={names.clone()} /> },
        View::Telemetry => html! { <TelemetryView key_={key} names={names.clone()} /> },
        View::Race => html! { <RaceView key_={key} names={names.clone()} /> },
        View::Radio => html! { <RadioView key_={key} names={names.clone()} /> },
        View::Weather => html! { <WeatherView key_={key} /> },
    };
    html! {
        <>
            <section class="card">
                <h2>{ t("Tout savoir sur la course", "Everything about the race") }</h2>
                <p class="muted">{ t(
                    "Fiche de chaque pilote tour par tour, secteurs, écarts, pneus, arrêts, télémétrie, direction de course et radios.",
                    "Each driver's lap-by-lap file, sectors, gaps, tyres, stops, telemetry, race control and radio.",
                ) }</p>
                <div class="race-data-grid">
                    { tab(View::Driver, t("👤 Fiche pilote", "👤 Driver file")) }
                    { tab(View::Results, t("🏁 Résultats", "🏁 Results")) }
                    { tab(View::Laps, t("⏱ Tours", "⏱ Laps")) }
                    { tab(View::Positions, t("↕ Positions", "↕ Positions")) }
                    { tab(View::Tyres, t("🛞 Pneus", "🛞 Tyres")) }
                    { tab(View::Telemetry, t("📈 Télémétrie", "📈 Telemetry")) }
                    { tab(View::Race, t("🚩 Direction de course", "🚩 Race control")) }
                    { tab(View::Radio, t("🎧 Radios", "🎧 Radio")) }
                    { tab(View::Weather, t("🌦 Météo", "🌦 Weather")) }
                </div>
            </section>
            { content }
        </>
    }
}

/// Replay automatique d'une course terminée (2023 et après) sur la page du Grand Prix :
/// démarre tout seul à ×30, voitures à leurs positions réelles sur le circuit 3D.
#[function_component]
pub fn RaceReplay(p: &MeetingLinkProps) -> Html {
    use f1x_protocol::ClientMsg;
    let sessions =
        use_json::<Value>((p.year >= 2023).then(|| format!("/api/of1/sessions?year={}", p.year)));
    let race = ms(&format!("{}T12:00:00Z", p.date));
    // Séances du week-end qu'on peut rejouer : course, qualifications, sprint, qualifs sprint.
    let order = [
        "Race",
        "Qualifying",
        "Sprint",
        "Sprint Qualifying",
        "Sprint Shootout",
    ];
    let mut weekend: Vec<(String, u32)> = list(&sessions)
        .into_iter()
        .filter(|x| {
            let d = race - ms(&s(x, "date_start"));
            d < 4.0 * 86_400_000.0 && d > -2.0 * 86_400_000.0
        })
        .filter(|x| order.contains(&s(x, "session_name").as_str()))
        .filter_map(|x| Some((s(&x, "session_name"), int(&x, "session_key")? as u32)))
        .collect();
    weekend.sort_by_key(|w| order.iter().position(|o| *o == w.0).unwrap_or(9));
    let chosen = use_state(|| None::<u32>);
    let key = (*chosen).or(weekend.first().map(|w| w.1));
    let label = |name: &str| match name {
        "Race" => t("Course", "Race"),
        "Qualifying" => t("Qualifs", "Quali"),
        "Sprint" => "Sprint",
        _ => t("Qualifs sprint", "Sprint quali"),
    };
    let racing = weekend
        .iter()
        .find(|w| Some(w.1) == key)
        .is_none_or(|w| w.0 == "Race" || w.0 == "Sprint");
    let live = crate::live::use_live();
    {
        let send = live.send.clone();
        use_effect_with(key, move |key| {
            if let Some(session_key) = *key {
                send.emit(ClientMsg::Replay {
                    session_key,
                    speed: 30,
                });
            }
            let send = send.clone();
            move || send.emit(ClientMsg::Stop)
        });
    }
    if key.is_none() {
        return html! {};
    }
    let st = &*live.state;
    let send = live.send.clone();
    let cmd = |m: ClientMsg| {
        let send = send.clone();
        Callback::from(move |_: MouseEvent| send.emit(m.clone()))
    };
    let body = match (&st.snapshot, &st.track) {
        (Some(snap), Some(track)) => {
            let markers: Vec<crate::gl3d::Marker> = snap
                .cars
                .iter()
                .filter(|c| c.lap_progress.is_some() && !c.retired)
                .map(|c| crate::gl3d::Marker {
                    key: c.code.clone(),
                    label: format!("{} {}", c.position, c.code),
                    colour: c.colour.clone(),
                    fraction: c.lap_progress.unwrap_or(0.0),
                })
                .collect();
            let lap = match snap.total_laps {
                _ if !racing => {
                    // Qualifications : heure de la séance plutôt qu'un compteur de tours.
                    let name = weekend
                        .iter()
                        .find(|w| Some(w.1) == key)
                        .map(|w| label(&w.0))
                        .unwrap_or_default();
                    format!("{name} · {}", local_date(&snap.clock, true))
                }
                Some(total) => tr!("Tour {}/{total}", "Lap {}/{total}", snap.lap.min(total)),
                None => tr!("Tour {}", "Lap {}", snap.lap),
            };
            let mut order: Vec<_> = snap.cars.iter().collect();
            order.sort_by_key(|c| c.position);
            let speeds = [1u32, 5, 10, 30, 60];
            html! {
                <>
                    <p class="replay-lap"><strong>{ lap }</strong></p>
                    <div class="progress"><span style={format!("width:{:.1}%", snap.progress * 100.0)}></span></div>
                    <crate::gl3d::View3D scene={crate::gl3d::Scene::Track { map: track.clone(), ghost: false }} markers={Rc::new(markers)} />
                    <div class="replay-controls">
                        <button class="btn btn-ghost" onclick={cmd(ClientMsg::Seek { seconds: -120 })}>{ "−2 min" }</button>
                        <button class="btn" onclick={cmd(if snap.paused { ClientMsg::Resume } else { ClientMsg::Pause })}>
                            { if snap.paused { t("▶ Lecture", "▶ Play") } else { t("⏸ Pause", "⏸ Pause") } }
                        </button>
                        <button class="btn btn-ghost" onclick={cmd(ClientMsg::Seek { seconds: 120 })}>{ "+2 min" }</button>
                        { for speeds.iter().map(|&v| html! {
                            <button class={classes!("btn", "btn-ghost", (snap.speed == v).then_some("on"))}
                                onclick={cmd(ClientMsg::Speed { speed: v })}>{ format!("×{v}") }</button>
                        }) }
                    </div>
                    <ol class="rows">
                        { for order.iter().take(10).map(|c| html! {
                            <li class="row">
                                <span class="pos">{ c.position }</span>
                                <span class="team-bar" style={format!("background:#{}", c.colour)}></span>
                                <div class="row-main"><span class="row-title">{ &c.name }</span></div>
                                <span class="muted">{ if c.in_pit { "PIT".to_string() } else if c.position == 1 { t("Leader", "Leader").to_string() } else { c.gap.clone() } }</span>
                            </li>
                        }) }
                    </ol>
                </>
            }
        }
        _ => {
            html! { <p class="muted">{ st.loading.clone().unwrap_or_else(|| t("Chargement du replay…", "Loading replay…").into()) }</p> }
        }
    };
    html! {
        <section class="card">
            <h2>{ t("Replay : course, qualifs, sprint", "Replay: race, quali, sprint") }</h2>
            if weekend.len() > 1 {
                <div class="segmented">
                    { for weekend.iter().map(|(name, k)| {
                        let (chosen, k) = (chosen.clone(), *k);
                        html! {
                            <button class={classes!("seg", (Some(k) == key).then_some("seg-active"))}
                                onclick={move |_| chosen.set(Some(k))}>{ label(name) }</button>
                        }
                    }) }
                </div>
            }
            { body }
            <p class="muted">{ t(
                "Démarre tout seul à ×30. Positions de chaque pilote d'après son avancement dans le tour (données OpenF1).",
                "Starts on its own at ×30. Each driver's position from their progress through the lap (OpenF1 data).",
            ) }</p>
        </section>
    }
}

#[derive(Properties, PartialEq)]
pub struct LapReplayProps {
    /// Télémétrie du tour (une entrée de /api/of1/telemetry).
    pub tel: Rc<Value>,
    pub colour: AttrValue,
}

/// Replay d'un tour : la voiture parcourt le circuit au rythme réel du tour, tableau de bord
/// synchronisé (vitesse, rapport, régime, accélérateur, frein, DRS).
#[function_component]
pub fn LapReplay(p: &LapReplayProps) -> Html {
    let nums = |k: &str| -> Vec<f64> {
        p.tel
            .get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default()
    };
    let circuit = s(&p.tel, "circuit_id");
    let track = use_json::<f1x_protocol::TrackMap>(
        (!circuit.is_empty()).then(|| format!("/api/track/{circuit}")),
    );
    let playing = use_state(|| true);
    let speed = use_state(|| 1.0f64);
    let elapsed = use_mut_ref(|| 0.0f64);
    let redraw = use_force_update();
    let times = nums("time");
    let duration = num(&p.tel, "lap_duration")
        .or_else(|| times.last().copied())
        .unwrap_or(1.0)
        .max(1.0);
    {
        let (elapsed, redraw) = (elapsed.clone(), redraw.clone());
        use_effect_with((*playing, *speed, duration), move |&(on, k, dur)| {
            let timer = on.then(|| {
                gloo_timers::callback::Interval::new(50, move || {
                    let mut e = elapsed.borrow_mut();
                    *e = (*e + 0.05 * k) % dur;
                    drop(e);
                    redraw.force_update();
                })
            });
            move || drop(timer)
        });
    }
    if times.is_empty() {
        return html! { <p class="muted">{ t("Replay indisponible pour ce tour.", "Replay unavailable for this lap.") }</p> };
    }
    let e = *elapsed.borrow();
    let i = times.partition_point(|&x| x < e).min(times.len() - 1);
    let at = |k: &str| nums(k).get(i).copied().unwrap_or(0.0);
    let (kmh, gear, rpm, throttle, brake, drs) = (
        at("speed"),
        at("gear"),
        at("rpm"),
        at("throttle"),
        at("brake"),
        at("drs"),
    );
    let map = match &track {
        Some(Ok(m)) if m.points.len() > 1 => {
            let pad = 40.0;
            let pts = &m.points;
            let mut cum = vec![0.0f64];
            for w in pts.windows(2) {
                let d = ((w[1].x - w[0].x) as f64).hypot((w[1].y - w[0].y) as f64);
                cum.push(cum.last().copied().unwrap_or(0.0) + d);
            }
            let dist = nums("distance");
            let frac = match (dist.get(i), dist.last()) {
                (Some(&d), Some(&total)) if total > 0.0 => d / total,
                _ => 0.0,
            };
            let target = frac * cum.last().copied().unwrap_or(0.0);
            let k = cum.partition_point(|&c| c < target).min(pts.len() - 1);
            let line = pts
                .iter()
                .map(|q| format!("{:.0},{:.0}", q.x as f64 + pad, q.y as f64 + pad))
                .collect::<Vec<_>>()
                .join(" ");
            let (cx, cy) = (pts[k].x as f64 + pad, pts[k].y as f64 + pad);
            html! {
                <svg class="outline lap-replay-map" viewBox={format!("0 0 {:.0} {:.0}", m.width + 2.0 * pad, m.height + 2.0 * pad)}
                     role="img" aria-label={t("Position de la voiture sur le circuit", "Car position on the circuit")}>
                    <polyline class="outline-base" points={line.clone()} />
                    <polyline class="outline-line" points={line} />
                    <circle cx={format!("{cx:.0}")} cy={format!("{cy:.0}")} r="34" fill={format!("#{}", p.colour)} stroke="#fff" stroke-width="10" />
                </svg>
            }
        }
        _ => html! {},
    };
    let bar = |label: &'static str, text: String, ratio: f64, class: &'static str| {
        html! {
            <div class="lap-bar">
                <span class="lap-bar-head"><span class="muted">{ label }</span><span>{ text }</span></span>
                <span class="lap-bar-track"><span class={class} style={format!("width:{:.0}%", (ratio.clamp(0.0, 1.0) * 100.0))}></span></span>
            </div>
        }
    };
    let toggle = {
        let playing = playing.clone();
        Callback::from(move |_: MouseEvent| playing.set(!*playing))
    };
    let restart = {
        let (elapsed, redraw) = (elapsed.clone(), redraw.clone());
        Callback::from(move |_: MouseEvent| {
            *elapsed.borrow_mut() = 0.0;
            redraw.force_update();
        })
    };
    html! {
        <div class="lap-replay">
            { map }
            <div class="lap-dash">
                <span class="lap-speed"><b>{ format!("{kmh:.0}") }</b>{ " km/h" }</span>
                <span class="lap-gear"><b>{ if gear < 1.0 { "N".to_string() } else { format!("{gear:.0}") } }</b>{ t(" rapport", " gear") }</span>
                <span class="lap-clock">{ format!("{} / {}", lap_str(e), lap_str(duration)) }</span>
            </div>
            { bar(t("Régime", "RPM"), format!("{rpm:.0} tr/min"), rpm / 13_000.0, "fill-rpm") }
            { bar(t("Accélérateur", "Throttle"), format!("{throttle:.0} %"), throttle / 100.0, "fill-throttle") }
            <div class="lap-flags">
                <span class={classes!("lap-flag", (brake > 0.0).then_some("brake-on"))}>{ t("Frein", "Brake") }</span>
                if !nums("drs").is_empty() {
                    <span class={classes!("lap-flag", (drs >= 100.0).then_some("drs-open"), (50.0..100.0).contains(&drs).then_some("drs-armed"))}>
                        { if drs >= 100.0 { t("DRS ouvert", "DRS open") } else if drs >= 50.0 { t("DRS autorisé", "DRS armed") } else { "DRS" } }
                    </span>
                }
            </div>
            <div class="replay-controls">
                <button class="btn" onclick={toggle}>{ if *playing { t("⏸ Pause", "⏸ Pause") } else { t("▶ Lecture", "▶ Play") } }</button>
                <button class="btn btn-ghost" onclick={restart}>{ t("⏮ Début", "⏮ Start") }</button>
                { for [1.0f64, 2.0, 4.0].into_iter().map(|v| {
                    let speed = speed.clone();
                    html! { <button class={classes!("btn", "btn-ghost", (*speed == v).then_some("on"))} onclick={move |_| speed.set(v)}>{ format!("×{v:.0}") }</button> }
                }) }
            </div>
        </div>
    }
}

/// Couleur d'une gomme (rouge tendre, jaune medium, blanc dur, vert intermédiaire, bleu pluie).
fn tyre_colour(c: &str) -> &'static str {
    match c {
        "SOFT" => "#ef4444",
        "MEDIUM" => "#facc15",
        "HARD" => "#e5e7eb",
        "INTERMEDIATE" => "#22c55e",
        "WET" => "#3b82f6",
        _ => "#9a9aad",
    }
}

fn tyre_name(c: &str) -> &'static str {
    match c {
        "SOFT" => t("Tendre", "Soft"),
        "MEDIUM" => t("Medium", "Medium"),
        "HARD" => t("Dur", "Hard"),
        "INTERMEDIATE" => t("Intermédiaire", "Intermediate"),
        "WET" => t("Pluie", "Wet"),
        _ => "?",
    }
}

#[derive(Properties, PartialEq)]
pub struct PitDetailProps {
    /// `session_key=…` ou `year=…&date=…` (course retrouvée par le serveur).
    pub query: AttrValue,
}

/// Détail de chaque arrêt : tour, temps dans la voie et à l'arrêt, pneus retirés et montés,
/// position avant et après (sources OpenF1 pit, stints, position, drivers).
#[function_component]
pub fn PitDetail(p: &PitDetailProps) -> Html {
    let data = use_json::<Value>(Some(format!("/api/of1/pitdetail?{}", p.query)));
    if let Some(v) = state_view(&data) {
        return html! { <section class="card"><h2>{ t("Détail de chaque arrêt", "Every pit stop in detail") }</h2>{ v }</section> };
    }
    let Some(Ok(v)) = &data else { return html! {} };
    let stops = v
        .get("stops")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    if stops.is_empty() {
        return html! {};
    }
    let fastest = stops
        .iter()
        .filter_map(|x| num(x, "stop_duration"))
        .fold(f64::MAX, f64::min);
    let tyre = |x: &Value, after: bool| -> Html {
        let Some(t0) = x.as_object().map(|_| x) else {
            return html! { <span class="muted">{ "?" }</span> };
        };
        let c = s(t0, "compound");
        let detail = if after {
            match int(t0, "age_at_start").unwrap_or(0) {
                0 => t("neufs", "new").to_string(),
                n => tr!("usagés ({n} t.)", "used ({n} laps)"),
            }
        } else {
            tr!("{} tours", "{} laps", int(t0, "age_end").unwrap_or(0))
        };
        html! {
            <span class="pit-tyre"><i style={format!("background:{}", tyre_colour(&c))}></i>{ format!("{} {}", tyre_name(&c), detail) }</span>
        }
    };
    html! {
        <section class="card">
            <h2>{ tr!("Détail de chaque arrêt ({})", "Every pit stop in detail ({})", stops.len()) }</h2>
            <ol class="rows pit-detail">
                { for stops.iter().map(|x| {
                    let (pb, pa) = (int(x, "position_before"), int(x, "position_after"));
                    let stop = num(x, "stop_duration");
                    let lane = num(x, "lane_duration");
                    let best = stop.is_some_and(|v| (v - fastest).abs() < 1e-6);
                    let moved = match (pb, pa) {
                        (Some(b), Some(a)) => html! {
                            <span class={classes!("pit-pos", (a > b).then_some("lost"), (a < b).then_some("won"))}>{ format!("P{b} → P{a}") }</span>
                        },
                        _ => html! {},
                    };
                    html! {
                        <li class="row pit-row" style={format!("--team:#{}", s(x, "colour"))}>
                            <span class="pos">{ format!("T{}", s(x, "lap")) }</span>
                            <div class="row-main">
                                <span class="row-title">{ s(x, "name") }<small class="muted">{ format!(" · {}", s(x, "team")) }</small></span>
                                <span class="pit-tyres">
                                    { tyre(x.get("tyre_before").unwrap_or(&Value::Null), false) }
                                    <span aria-hidden="true">{ " → " }</span>
                                    { tyre(x.get("tyre_after").unwrap_or(&Value::Null), true) }
                                </span>
                                <span class="row-sub">
                                    { lane.map(|v| tr!("Voie des stands {:.1} s", "Pit lane {:.1} s", v)).unwrap_or_default() }
                                    { stop.map(|v| tr!(" · immobilisé {:.1} s", " · stationary {:.1} s", v)).unwrap_or_default() }
                                    if best { <strong class="pit-best">{ t(" · le plus rapide", " · fastest") }</strong> }
                                </span>
                            </div>
                            { moved }
                        </li>
                    }
                }) }
            </ol>
            <p class="muted">{ t(
                "Voie des stands : de l'entrée à la sortie. Immobilisé : voiture à l'arrêt pendant le changement de pneus. Positions juste avant l'entrée et juste après la ressortie. Données OpenF1.",
                "Pit lane: entry to exit. Stationary: car stopped for the tyre change. Positions just before entry and just after exit. OpenF1 data.",
            ) }</p>
        </section>
    }
}
