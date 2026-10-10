//! Données OpenF1 complètes (les 18 points d'accès, via le relais mis en cache du serveur) :
//! réunions, séances, résultats, grille, championnat, tours, positions, écarts, pneus, arrêts,
//! dépassements, télémétrie comparée, positions GPS, météo, direction de course, radios.
//!
//! Chaque page crée ses chargements dans son corps, puis ne lit les données que dans des
//! parties dynamiques ciblées (une par carte ou par graphique) : l'arrivée d'une source ne
//! reconstruit que ce qui l'affiche.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use active::prelude::*;
use gloo_timers::callback::Interval;
use serde_json::Value;
use wasm_bindgen::JsCast;

use crate::api::{use_json, use_json_dyn};
use crate::components::{Tab, dynamic, layout, layout_dyn, loading, stat_grid, when};
use crate::i18n::t;
use crate::util::{current_year, local_date};
use crate::{Route, link, tr};

type Json = crate::api::Json<Value>;

fn of1(endpoint: &str, query: &str) -> Option<String> {
    Some(format!("/api/of1/{endpoint}?{query}"))
}
fn list(v: &Json) -> Vec<Value> {
    match v {
        Some(Ok(v)) => v.as_array().cloned().unwrap_or_default(),
        _ => Vec::new(),
    }
}
/// Nombre d'éléments d'une liste chargée (0 sinon).
fn count(v: &Json) -> usize {
    match v {
        Some(Ok(v)) => v.as_array().map_or(0, Vec::len),
        _ => 0,
    }
}
/// Premier élément d'une liste chargée (`Null` sinon).
fn first(v: &Json) -> Value {
    match v {
        Some(Ok(v)) => v
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null),
        _ => Value::Null,
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
/// Les nombres d'un tableau JSON (`[]` sinon).
fn floats(v: &Value, k: &str) -> Vec<f64> {
    v.get(k)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

/// Palette catégorielle validée (fond sombre), dans un ordre fixe.
const SERIES: [&str; 5] = ["#3987e5", "#d95926", "#199e70", "#c98500", "#d55181"];

/// État d'un chargement, sans ses données.
#[derive(Clone, Copy, PartialEq)]
enum Load {
    Pending,
    Missing,
    Unavailable,
    Ready,
}

fn load_of(v: &Json) -> Load {
    match v {
        None => Load::Pending,
        Some(Err((404, _))) => Load::Missing,
        Some(Err(_)) => Load::Unavailable,
        Some(Ok(_)) => Load::Ready,
    }
}

/// Chargement, absence de données ou indisponibilité ; `None` une fois les données là.
fn load_view(l: Load) -> Option<Node> {
    match l {
        Load::Pending => Some(loading()),
        Load::Missing => Some(
            p().class("muted")
                .text(t(
                    "Pas de données pour cette séance.",
                    "No data for this session.",
                ))
                .into(),
        ),
        Load::Unavailable => Some(
            p().class("muted")
                .text(t(
                    "Données OpenF1 momentanément indisponibles (limite de requêtes) — réessaie dans une minute.",
                    "OpenF1 data temporarily unavailable (rate limit) — try again in a minute.",
                ))
                .into(),
        ),
        Load::Ready => None,
    }
}

fn state_view(v: &Json) -> Option<Node> {
    load_view(load_of(v))
}

/// L'état d'un chargement, qui ne change qu'en passant de « en cours » à « chargé » (ou en
/// erreur) : une partie qui ne lit que lui n'est construite qu'une fois les données là.
fn load(v: State<Json>) -> State<Load> {
    memo(move || v.with(load_of))
}

fn card(content: impl Into<Node>) -> Node {
    section().class("card").child(content).into()
}

/// `--team:#xxxxxx` d'un pilote connu, sinon rien.
fn team_var(names: &Names, n: i64) -> String {
    names
        .get(&n)
        .map(|x| format!("--team:#{}", x.2))
        .unwrap_or_default()
}

// ---------- Graphique en lignes générique (un seul axe) ----------

/// Libellé d'une valeur d'un axe.
type Fmt = Rc<dyn Fn(f64) -> String>;

fn fmt(f: impl Fn(f64) -> String + 'static) -> Fmt {
    Rc::new(f)
}

#[derive(Clone, PartialEq)]
struct Series {
    label: String,
    color: String,
    points: Vec<(f64, f64)>,
}

/// Graphique en lignes : `LineChart::new(séries, libellé de x, libellé de y).view()`. Toucher
/// le graphique affiche les valeurs de chaque série à cet endroit.
struct LineChart {
    series: Vec<Series>,
    fmt_x: Fmt,
    fmt_y: Fmt,
    invert: bool,
    step: bool,
    height: f64,
}

impl LineChart {
    fn new(series: Vec<Series>, fmt_x: Fmt, fmt_y: Fmt) -> Self {
        Self {
            series,
            fmt_x,
            fmt_y,
            invert: false,
            step: false,
            height: 180.0,
        }
    }

    /// Petites valeurs en haut (positions).
    fn invert(mut self) -> Self {
        self.invert = true;
        self
    }

    /// En escalier (positions, rapports, frein…).
    fn step(mut self) -> Self {
        self.step = true;
        self
    }

    fn height(mut self, height: f64) -> Self {
        self.height = height;
        self
    }

    fn view(self) -> Node {
        let LineChart {
            series,
            fmt_x,
            fmt_y,
            invert,
            step,
            height: h,
        } = self;
        let all: Vec<(f64, f64)> = series
            .iter()
            .flat_map(|sr| sr.points.iter().copied())
            .collect();
        if all.len() < 2 {
            return p()
                .class("muted")
                .text(t("Pas assez de données.", "Not enough data."))
                .into();
        }
        // Abscisse touchée : seuls la lecture et le réticule la suivent.
        let pick = use_state(None::<f64>);
        let picked = memo(move || pick.with(Option::is_some));
        let (w, ml, mr, mt, mb) = (340.0, 40.0, 10.0, 10.0, 22.0);
        let (x0, x1) = all
            .iter()
            .fold((f64::MAX, f64::MIN), |r, q| (r.0.min(q.0), r.1.max(q.0)));
        let (y0, y1) = all
            .iter()
            .fold((f64::MAX, f64::MIN), |r, q| (r.0.min(q.1), r.1.max(q.1)));
        let pad = ((y1 - y0) * 0.05).max(0.5);
        let (y0, y1) = (y0 - pad, y1 + pad);
        let x = move |v: f64| ml + (v - x0) / (x1 - x0).max(1e-9) * (w - ml - mr);
        let y = move |v: f64| {
            let r = (v - y0) / (y1 - y0).max(1e-9);
            if invert {
                mt + r * (h - mt - mb)
            } else {
                h - mb - r * (h - mt - mb)
            }
        };
        let path_d = |sr: &Series| {
            let mut d = String::new();
            for (k, &(va, vb)) in sr.points.iter().enumerate() {
                if step && k > 0 {
                    d.push_str(&format!(" H{:.1} V{:.1}", x(va), y(vb)));
                } else {
                    d.push_str(&format!(
                        "{}{:.1},{:.1}",
                        if k == 0 { "M" } else { " L" },
                        x(va),
                        y(vb)
                    ));
                }
            }
            d
        };
        let on_pointer = move |e: Event| {
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
            let vx = (f64::from(pe.client_x()) - r.left()) / r.width() * w;
            pick.set(Some(x0 + (vx - ml) / (w - ml - mr) * (x1 - x0)));
        };
        let series = Rc::new(series);
        let readout = {
            let (series, fmt_x, fmt_y) = (series.clone(), fmt_x.clone(), fmt_y.clone());
            move || match pick.get() {
                Some(px) => {
                    let mut parts = vec![fmt_x(px)];
                    for sr in series.iter() {
                        if let Some(&(_, v)) = sr
                            .points
                            .iter()
                            .min_by(|m, n| (m.0 - px).abs().total_cmp(&(n.0 - px).abs()))
                        {
                            parts.push(format!("{} {}", sr.label, fmt_y(v)));
                        }
                    }
                    parts.join(" · ")
                }
                None => t(
                    "Touche le graphique pour lire les valeurs.",
                    "Touch the chart to read the values.",
                )
                .to_string(),
            }
        };
        let ticks_y = [y0 + (y1 - y0) * 0.1, (y0 + y1) / 2.0, y1 - (y1 - y0) * 0.1];
        let ticks_x = [x0, (x0 + x1) / 2.0, x1];
        let grid = ticks_y.iter().map(|v| {
            fragment([
                Node::from(
                    line()
                        .class("chart-grid")
                        .attr("x1", ml.to_string())
                        .attr("x2", (w - mr).to_string())
                        .attr("y1", format!("{:.1}", y(*v)))
                        .attr("y2", format!("{:.1}", y(*v))),
                ),
                text_svg()
                    .class("chart-axis")
                    .attr("x", (ml - 4.0).to_string())
                    .attr("y", format!("{:.1}", y(*v) + 3.5))
                    .attr("text-anchor", "end")
                    .text(fmt_y(*v))
                    .into(),
            ])
        });
        let axis_x = ticks_x.iter().enumerate().map(|(k, v)| {
            text_svg()
                .class("chart-axis")
                .attr("x", format!("{:.1}", x(*v)))
                .attr("y", (h - 6.0).to_string())
                .attr("text-anchor", ["start", "middle", "end"][k])
                .text(fmt_x(*v))
        });
        // Réticule : inséré au premier toucher, puis seule sa position suit.
        let crosshair = dynamic(move || {
            if !picked.get() {
                return Node::Empty;
            }
            let cx = move || {
                pick.get()
                    .map(|px| format!("{:.1}", x(px)))
                    .unwrap_or_default()
            };
            line()
                .class("chart-crosshair")
                .attr_dyn("x1", cx)
                .attr_dyn("x2", cx)
                .attr("y1", mt.to_string())
                .attr("y2", (h - mb).to_string())
                .into()
        });
        let lines = series.iter().map(|sr| {
            path()
                .class("chart-line")
                .attr("pathLength", "1")
                .attr("d", path_d(sr))
                .attr("fill", "none")
                .attr("stroke", sr.color.clone())
                .attr("stroke-width", "2")
                .attr("stroke-linejoin", "round")
        });
        let legend = (series.len() > 1).then(|| {
            div()
                .class("chart-legend")
                .children(series.iter().map(|sr| {
                    span()
                        .child(
                            span()
                                .class("legend-line")
                                .style(format!("background:{}", sr.color)),
                        )
                        .text(format!(" {}", sr.label))
                }))
        });
        div()
            .class("of1-chart")
            .child(
                p().class("chart-readout")
                    .class_if("chart-readout-hint", move || !picked.get())
                    .attr("aria-live", "polite")
                    .text_dyn(readout),
            )
            .child(
                svg()
                    .class("chart")
                    .attr("viewBox", format!("0 0 {w} {h}"))
                    .attr("role", "img")
                    .on("pointerdown", on_pointer)
                    .on("pointermove", on_pointer)
                    .children(grid)
                    .children(axis_x)
                    .child(crosshair)
                    .children(lines),
            )
            .child(legend)
            .into()
    }
}

// ---------- Années et réunions ----------

pub fn data_year_page(year: u32) -> Node {
    let meetings = use_json::<Value>(of1("meetings", &format!("year={year}")));
    let years: Vec<u32> = (2023..=current_year()).rev().collect();
    let body = dynamic(move || {
        let meetings = meetings.get();
        state_view(&meetings).unwrap_or_else(|| {
            let mut items = list(&meetings);
            items.reverse();
            ol().class("rows")
                .children(items.iter().map(|m| {
                    let key = int(m, "meeting_key").unwrap_or(0) as u32;
                    li().class("row").child(
                        link(Route::DataMeeting { key }, "row-main")
                            .child(span().class("row-title").text(s(m, "meeting_name")))
                            .child(span().class("row-sub").text(format!(
                                "{} · {} · {}",
                                s(m, "location"),
                                s(m, "country_name"),
                                local_date(&s(m, "date_start"), false)
                            ))),
                    )
                }))
                .into()
        })
    });
    layout(
        t("Données OpenF1", "OpenF1 data"),
        Some(Tab::Archives),
        fragment([
            section()
                .class("card")
                .child(h2().text(t("Toutes les données OpenF1", "All OpenF1 data")))
                .child(p().class("muted").text(t(
                    "Chaque séance depuis 2023 en détail : résultats, grille, championnat, tours, positions, écarts, pneus, arrêts, dépassements, télémétrie, météo, direction de course et radios. Les 18 sources d'OpenF1, mises en cache par le serveur.",
                    "Every session since 2023 in detail: results, grid, championship, laps, positions, gaps, tyres, stops, overtakes, telemetry, weather, race control and radio. All 18 OpenF1 sources, cached by the server.",
                )))
                .child(div().class("year-pills").children(years.iter().map(|&y| {
                    link(Route::DataYear { year: y }, "pill")
                        .class(when(y == year, "pill-on"))
                        .text(y.to_string())
                }))),
            section()
                .class("card")
                .child(h2().text(tr!("Grands Prix {}", "{} Grands Prix", year)))
                .child(body),
        ]),
    )
}

pub fn data_meeting_page(key: u32) -> Node {
    let meeting = use_json::<Value>(of1("meetings", &format!("meeting_key={key}")));
    let sessions = use_json::<Value>(of1("sessions", &format!("meeting_key={key}")));
    let grid = use_json::<Value>(of1("starting_grid", &format!("meeting_key={key}")));
    let drivers = use_json::<Value>(of1("drivers", &format!("meeting_key={key}")));
    let m = memo(move || meeting.with(first));
    let names = memo(move || Rc::new(driver_map(&drivers.with(list))));
    let hero = section()
        .class("card hero")
        .child(p().class("eyebrow").text_dyn(move || {
            m.with(|m| format!("{} · {}", s(m, "location"), s(m, "country_name")))
        }))
        .child(
            h2().class("hero-title")
                .text_dyn(move || m.with(|m| s(m, "meeting_name"))),
        )
        .child(
            p().class("muted")
                .text_dyn(move || m.with(|m| s(m, "meeting_official_name"))),
        )
        .child(dynamic(move || {
            m.with(|m| {
                stat_grid(vec![
                    (t("Circuit", "Circuit"), s(m, "circuit_short_name")),
                    (t("Type", "Type"), s(m, "circuit_type")),
                    (t("Année", "Year"), s(m, "year")),
                ])
            })
        }));
    let sessions_card = section()
        .class("card")
        .child(h2().text(t("Séances", "Sessions")))
        .child(dynamic(move || {
            let sessions = sessions.get();
            state_view(&sessions).unwrap_or_else(|| {
                ol().class("rows")
                    .children(list(&sessions).iter().map(|x| {
                        let key = int(x, "session_key").unwrap_or(0) as u32;
                        li().class("row")
                            .child(
                                link(Route::DataSession { key }, "row-main")
                                    .child(span().class("row-title").text(s(x, "session_name")))
                                    .child(
                                        span()
                                            .class("row-sub")
                                            .text(local_date(&s(x, "date_start"), true)),
                                    ),
                            )
                            .child(span().class("row-meta").text("→"))
                    }))
                    .into()
            })
        }));
    let grid_card = dynamic(move || {
        let rows = grid.with(list);
        if rows.is_empty() {
            return Node::Empty;
        }
        let names = names.get();
        section()
            .class("card")
            .child(h2().text(t("Grille de départ", "Starting grid")))
            .child(ol().class("rows").children(rows.iter().map(|g| {
                let d = names.get(&int(g, "driver_number").unwrap_or(0));
                li().class("row")
                    .style(d.map(|d| format!("--team:#{}", d.2)).unwrap_or_default())
                    .child(span().class("pos").text(s(g, "position")))
                    .child(
                        div()
                            .class("row-main")
                            .child(
                                span().class("row-title").text(
                                    d.map(|d| d.0.clone())
                                        .unwrap_or_else(|| s(g, "driver_number")),
                                ),
                            )
                            .child(
                                span()
                                    .class("row-sub")
                                    .text(d.map(|d| d.1.clone()).unwrap_or_default()),
                            ),
                    )
                    .child(
                        span()
                            .class("row-meta")
                            .text(num(g, "lap_duration").map(lap_str).unwrap_or_default()),
                    )
            })))
            .into()
    });
    layout_dyn(
        move || m.with(|m| s(m, "meeting_name")),
        Some(Tab::Archives),
        fragment([Node::from(hero), sessions_card.into(), grid_card]),
    )
}

/// numéro → (nom, écurie, couleur, acronyme).
type Names = BTreeMap<i64, (String, String, String, String)>;

fn driver_map(drivers: &[Value]) -> Names {
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

/// La rubrique choisie : ses données ne se chargent qu'à son ouverture.
fn view_content(view: View, key: u32, names: State<Rc<Names>>) -> Node {
    match view {
        View::Results => results_view(key, names),
        View::Driver => driver_view(key, names),
        View::Laps => laps_view(key, names),
        View::Positions => positions_view(key, names),
        View::Tyres => tyres_view(key, names),
        View::Telemetry => telemetry_view(key, names),
        View::Race => race_view(key, names),
        View::Radio => radio_view(key, names),
        View::Weather => weather_view(key),
    }
}

pub fn data_session_page(key: u32) -> Node {
    let view = use_state(View::Results);
    let session = use_json::<Value>(of1("sessions", &format!("session_key={key}")));
    let drivers = use_json::<Value>(of1("drivers", &format!("session_key={key}")));
    let sess = memo(move || session.with(first));
    let names = memo(move || Rc::new(driver_map(&drivers.with(list))));
    let tab = |v: View, label: &'static str| {
        button()
            .class("seg")
            .class_if("seg-active", move || view.get() == v)
            .on_click(move |_| {
                if view.get() != v {
                    view.set(v);
                }
            })
            .text(label)
    };
    let hero = section()
        .class("card hero")
        .child(p().class("eyebrow").text_dyn(move || {
            sess.with(|x| format!("{} · {}", s(x, "country_name"), s(x, "year")))
        }))
        .child(h2().class("hero-title").text_dyn(move || {
            sess.with(|x| format!("{} — {}", s(x, "location"), s(x, "session_name")))
        }))
        .child(
            p().class("muted")
                .text_dyn(move || sess.with(|x| local_date(&s(x, "date_start"), true))),
        )
        .child(dynamic(move || {
            sess.with(|x| int(x, "meeting_key"))
                .map_or(Node::Empty, |mk| {
                    link(Route::DataMeeting { key: mk as u32 }, "link")
                        .text(t(
                            "← Toutes les séances du week-end",
                            "← All weekend sessions",
                        ))
                        .into()
                })
        }));
    let tabs = div()
        .class("segmented segmented-4 of1-tabs")
        .child(tab(View::Results, t("Résultats", "Results")))
        .child(tab(View::Driver, t("Pilote", "Driver")))
        .child(tab(View::Laps, t("Tours", "Laps")))
        .child(tab(View::Positions, t("Positions", "Positions")))
        .child(tab(View::Tyres, t("Pneus", "Tyres")))
        .child(tab(View::Telemetry, t("Télémétrie", "Telemetry")))
        .child(tab(View::Race, t("Course", "Race")))
        .child(tab(View::Radio, t("Radios", "Radio")))
        .child(tab(View::Weather, t("Météo", "Weather")));
    layout_dyn(
        move || sess.with(|x| format!("{} · {}", s(x, "location"), s(x, "session_name"))),
        Some(Tab::Archives),
        fragment([
            Node::from(hero),
            tabs.into(),
            dynamic(move || view_content(view.get(), key, names)),
        ]),
    )
}

fn name(names: &Names, n: i64) -> String {
    names
        .get(&n)
        .map(|d| d.0.clone())
        .unwrap_or_else(|| format!("#{n}"))
}
fn code(names: &Names, n: i64) -> String {
    names
        .get(&n)
        .map(|d| d.3.clone())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| format!("#{n}"))
}

/// « ▲2 » / « ▼1 » entre la place avant et après la séance.
fn move_label(a: Option<i64>, b: Option<i64>) -> String {
    match (a, b) {
        (Some(a), Some(b)) if b < a => format!(" ▲{}", a - b),
        (Some(a), Some(b)) if b > a => format!(" ▼{}", b - a),
        _ => String::new(),
    }
}

/// Résultat de la séance + championnat après la séance.
fn results_view(key: u32, names: State<Rc<Names>>) -> Node {
    let q = format!("session_key={key}");
    let res = use_json::<Value>(of1("session_result", &q));
    let champ_d = use_json::<Value>(of1("championship_drivers", &q));
    let champ_t = use_json::<Value>(of1("championship_teams", &q));
    let classification = dynamic(move || {
        let res = res.get();
        state_view(&res).unwrap_or_else(|| {
            let n = names.get();
            ol().class("rows")
                .children(list(&res).iter().map(|r| {
                    let d = int(r, "driver_number").unwrap_or(0);
                    let flags = [("dnf", "DNF"), ("dns", "DNS"), ("dsq", "DSQ")]
                        .iter()
                        .filter(|(k, _)| r.get(*k).and_then(Value::as_bool).unwrap_or(false))
                        .map(|(_, l)| *l)
                        .collect::<Vec<_>>()
                        .join(" ");
                    let gap = match r.get("gap_to_leader") {
                        Some(Value::Number(x)) if x.as_f64() == Some(0.0) => {
                            num(r, "duration").map(lap_str).unwrap_or_default()
                        }
                        Some(Value::Number(x)) => format!("+{:.3}", x.as_f64().unwrap_or(0.0)),
                        Some(Value::String(x)) => x.clone(),
                        Some(Value::Array(a)) => a
                            .last()
                            .and_then(Value::as_f64)
                            .map(|x| format!("+{x:.3}"))
                            .unwrap_or_default(),
                        _ => String::new(),
                    };
                    let points = num(r, "points")
                        .filter(|x| *x > 0.0)
                        .map(|x| format!(" · {x} pts"))
                        .unwrap_or_default();
                    li().class("row")
                        .style(team_var(&n, d))
                        .child(span().class("pos").text(s(r, "position")))
                        .child(
                            div()
                                .class("row-main")
                                .child(span().class("row-title").text(name(&n, d)))
                                .child(span().class("row-sub").text(format!(
                                    "{}{}{}",
                                    n.get(&d).map(|x| x.1.clone()).unwrap_or_default(),
                                    int(r, "number_of_laps")
                                        .map(|l| tr!(" · {} tours", " · {} laps", l))
                                        .unwrap_or_default(),
                                    if flags.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" · {flags}")
                                    }
                                ))),
                        )
                        .child(span().class("row-meta").text(format!("{gap}{points}")))
                }))
                .into()
        })
    });
    let drivers_card = dynamic(move || {
        let rows = champ_d.with(list);
        if rows.is_empty() {
            return Node::Empty;
        }
        let n = names.get();
        section()
            .class("card")
            .child(h2().text(t(
                "Championnat pilotes après la séance",
                "Drivers' championship after the session",
            )))
            .child(ol().class("rows").children(rows.iter().take(22).map(|c| {
                let d = int(c, "driver_number").unwrap_or(0);
                let gained =
                    num(c, "points_current").unwrap_or(0.0) - num(c, "points_start").unwrap_or(0.0);
                li().class("row")
                    .style(team_var(&n, d))
                    .child(span().class("pos").text(s(c, "position_current")))
                    .child(
                        div()
                            .class("row-main")
                            .child(span().class("row-title").text(format!(
                                "{}{}",
                                name(&n, d),
                                move_label(int(c, "position_start"), int(c, "position_current"))
                            )))
                            .child(span().class("row-sub").text(if gained > 0.0 {
                                tr!("+{} pts sur la séance", "+{} pts this session", gained)
                            } else {
                                String::new()
                            })),
                    )
                    .child(
                        span()
                            .class("row-meta")
                            .text(format!("{} pts", s(c, "points_current"))),
                    )
            })))
            .into()
    });
    let teams_card = dynamic(move || {
        let rows = champ_t.with(list);
        if rows.is_empty() {
            return Node::Empty;
        }
        section()
            .class("card")
            .child(h2().text(t("Championnat constructeurs", "Constructors' championship")))
            .child(ol().class("rows").children(rows.iter().map(|c| {
                li().class("row")
                    .child(span().class("pos").text(s(c, "position_current")))
                    .child(
                        div()
                            .class("row-main")
                            .child(span().class("row-title").text(format!(
                                "{}{}",
                                s(c, "team_name"),
                                move_label(int(c, "position_start"), int(c, "position_current"))
                            ))),
                    )
                    .child(
                        span()
                            .class("row-meta")
                            .text(format!("{} pts", s(c, "points_current"))),
                    )
            })))
            .into()
    });
    fragment([
        Node::from(
            section()
                .class("card")
                .child(h2().text(t("Classement de la séance", "Session classification")))
                .child(classification),
        ),
        drivers_card,
        teams_card,
    ])
}

/// Premiers du classement (ou des pilotes connus) pour les graphiques.
fn top_drivers(res: &Json, names: &Names, k: usize) -> Vec<i64> {
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
fn laps_view(key: u32, names: State<Rc<Names>>) -> Node {
    let q = format!("session_key={key}");
    let laps = use_json::<Value>(of1("laps", &q));
    let res = use_json::<Value>(of1("session_result", &q));
    let status = load(laps);
    dynamic(move || {
        if let Some(v) = load_view(status.get()) {
            return card(v);
        }
        let all = Rc::new(untrack(|| laps.with(list)));
        let chart = {
            let all = all.clone();
            dynamic(move || {
                let n = names.get();
                let top = top_drivers(&res.get(), &n, 5);
                // Médiane pour écarter les tours anormaux (stands, voiture de sécurité).
                let mut durations: Vec<f64> =
                    all.iter().filter_map(|l| num(l, "lap_duration")).collect();
                durations.sort_by(f64::total_cmp);
                let median = durations.get(durations.len() / 2).copied().unwrap_or(100.0);
                let series: Vec<Series> = top
                    .iter()
                    .enumerate()
                    .map(|(k, d)| Series {
                        label: code(&n, *d),
                        color: SERIES[k % SERIES.len()].into(),
                        points: all
                            .iter()
                            .filter(|l| int(l, "driver_number") == Some(*d))
                            .filter_map(|l| {
                                Some((int(l, "lap_number")? as f64, num(l, "lap_duration")?))
                            })
                            .filter(|(_, v)| *v < median * 1.12)
                            .collect(),
                    })
                    .collect();
                LineChart::new(series, fmt(|v| tr!("T{}", "L{}", v.round())), fmt(lap_str)).view()
            })
        };
        let bests = dynamic(move || {
            let n = names.get();
            let best = |k: &str| {
                all.iter()
                    .filter_map(|l| Some((num(l, k)?, int(l, "driver_number")?)))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            };
            let sector = |k: &str| {
                best(k)
                    .map(|(v, d)| format!("{v:.3} {}", code(&n, d)))
                    .unwrap_or_default()
            };
            let speed_trap = all
                .iter()
                .filter_map(|l| Some((num(l, "st_speed")?, int(l, "driver_number")?)))
                .max_by(|a, b| a.0.total_cmp(&b.0));
            stat_grid(vec![
                (
                    t("Meilleur tour", "Fastest lap"),
                    best("lap_duration")
                        .map(|(v, d)| format!("{} {}", lap_str(v), code(&n, d)))
                        .unwrap_or_default(),
                ),
                (t("Secteur 1", "Sector 1"), sector("duration_sector_1")),
                (t("Secteur 2", "Sector 2"), sector("duration_sector_2")),
                (t("Secteur 3", "Sector 3"), sector("duration_sector_3")),
                (
                    t("Pointe (radar)", "Speed trap"),
                    speed_trap
                        .map(|(v, d)| format!("{v:.0} {}", code(&n, d)))
                        .unwrap_or_default(),
                ),
                (
                    t("Tours", "Laps"),
                    all.iter()
                        .filter_map(|l| int(l, "lap_number"))
                        .max()
                        .unwrap_or(0)
                        .to_string(),
                ),
            ])
        });
        fragment([
            Node::from(
                section()
                    .class("card")
                    .child(h2().text(t("Temps au tour", "Lap times")))
                    .child(chart)
                    .child(p().class("muted").text(t(
                        "5 premiers de la séance ; tours lents (stands, neutralisations) écartés.",
                        "Top 5 of the session; slow laps (pits, neutralisations) left out.",
                    ))),
            ),
            section()
                .class("card")
                .child(h2().text(t("Records de la séance", "Session bests")))
                .child(bests)
                .into(),
        ])
    })
}

/// Positions au fil de la séance et écart au leader.
fn positions_view(key: u32, names: State<Rc<Names>>) -> Node {
    let q = format!("session_key={key}");
    let pos = use_json::<Value>(of1("position", &q));
    let res = use_json::<Value>(of1("session_result", &q));
    let top = memo(move || top_drivers(&res.get(), &names.get(), 5));
    // Écarts du 2e et du 3e (rechargés seulement si les pilotes concernés changent).
    let second = move |top: &[i64]| {
        let first_driver = top.first().copied().unwrap_or(1);
        top.get(1).copied().unwrap_or(first_driver)
    };
    let int_a = use_json_dyn::<Value>(move || {
        top.with(|top| {
            of1(
                "intervals",
                &format!("session_key={key}&driver_number={}", second(top)),
            )
        })
    });
    let int_b = use_json_dyn::<Value>(move || {
        top.with(|top| {
            of1(
                "intervals",
                &format!(
                    "session_key={key}&driver_number={}",
                    top.get(2).copied().unwrap_or(second(top))
                ),
            )
        })
    });
    let status = load(pos);
    dynamic(move || {
        if let Some(v) = load_view(status.get()) {
            return card(v);
        }
        let all = Rc::new(untrack(|| pos.with(list)));
        let t0 = all.first().map(|x| ms(&s(x, "date"))).unwrap_or(0.0);
        let minutes = move |iso: &str| (ms(iso) - t0) / 60_000.0;
        let positions = {
            let all = all.clone();
            dynamic(move || {
                let (n, top) = (names.get(), top.get());
                let series: Vec<Series> = top
                    .iter()
                    .enumerate()
                    .map(|(k, d)| Series {
                        label: code(&n, *d),
                        color: SERIES[k % SERIES.len()].into(),
                        points: all
                            .iter()
                            .filter(|x| int(x, "driver_number") == Some(*d))
                            .filter_map(|x| Some((minutes(&s(x, "date")), num(x, "position")?)))
                            .collect(),
                    })
                    .collect();
                LineChart::new(
                    series,
                    fmt(|v| format!("{v:.0} min")),
                    fmt(|v| format!("P{}", v.round())),
                )
                .invert()
                .step()
                .view()
            })
        };
        let gaps = dynamic(move || {
            let (n, top) = (names.get(), top.get());
            let (a, b) = (int_a.get(), int_b.get());
            let gaps: Vec<Series> = [(&a, top.get(1)), (&b, top.get(2))]
                .iter()
                .enumerate()
                .filter_map(|(k, (v, d))| {
                    let d = **d.as_ref()?;
                    Some(Series {
                        label: code(&n, d),
                        color: SERIES[(k + 1) % SERIES.len()].into(),
                        points: list(v)
                            .iter()
                            .filter_map(|x| {
                                Some((minutes(&s(x, "date")), num(x, "gap_to_leader")?))
                            })
                            .filter(|(_, g)| *g < 120.0)
                            .collect(),
                    })
                })
                .collect();
            LineChart::new(
                gaps,
                fmt(|v| format!("{v:.0} min")),
                fmt(|v| format!("{v:.1} s")),
            )
            .view()
        });
        fragment([
            Node::from(
                section()
                    .class("card")
                    .child(h2().text(t(
                        "Positions au fil de la séance",
                        "Positions through the session",
                    )))
                    .child(positions),
            ),
            section()
                .class("card")
                .child(h2().text_dyn(move || {
                    let first_driver = top.with(|top| top.first().copied().unwrap_or(1));
                    tr!(
                        "Écart avec {}",
                        "Gap to {}",
                        names.with(|n| code(n, first_driver))
                    )
                }))
                .child(gaps)
                .into(),
        ])
    })
}

/// Relais de pneus et arrêts aux stands.
fn tyres_view(key: u32, names: State<Rc<Names>>) -> Node {
    let q = format!("session_key={key}");
    let stints = use_json::<Value>(of1("stints", &q));
    let res = use_json::<Value>(of1("session_result", &q));
    let status = load(stints);
    dynamic(move || {
        if let Some(v) = load_view(status.get()) {
            return card(v);
        }
        let all = Rc::new(untrack(|| stints.with(list)));
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
        let rows = dynamic(move || {
            let n = names.get();
            let order = top_drivers(&res.get(), &n, 22);
            fragment(order.iter().map(|d| {
                li().class("strat-row")
                    .child(span().class("strat-code").text(code(&n, *d)))
                    .child(
                        span().class("strat-bar").children(
                            all.iter()
                                .filter(|x| int(x, "driver_number") == Some(*d))
                                .map(|x| {
                                    let (a, b) = (
                                        int(x, "lap_start").unwrap_or(1) as f64,
                                        int(x, "lap_end").unwrap_or(1) as f64,
                                    );
                                    span()
                                        .class("strat-stint")
                                        .attr("title", s(x, "compound"))
                                        .style(format!(
                                            "width:{:.2}%;background:{}",
                                            (b - a + 1.0) / total * 100.0,
                                            colour(&s(x, "compound"))
                                        ))
                                }),
                        ),
                    )
            }))
        });
        fragment([
            Node::from(
                section()
                    .class("card")
                    .child(h2().text(t("Stratégie des pneus", "Tyre strategy")))
                    .child(ol().class("strategy").child(rows))
                    .child(p().class("muted").text(t(
                        "Rouge tendre · jaune medium · blanc dur · vert intermédiaire · bleu pluie.",
                        "Red soft · yellow medium · white hard · green intermediate · blue wet.",
                    ))),
            ),
            pit_detail(&format!("session_key={key}")),
        ])
    })
}

/// Liste déroulante des pilotes de la séance ; `current` : pilote affiché.
fn driver_select(
    names: State<Rc<Names>>,
    current: impl Fn() -> Option<i64> + 'static,
    on_pick: impl Fn(Option<i64>) + 'static,
) -> Element {
    label().class("select").child(
        select()
            .on("change", move |e| on_pick(e.value().parse().ok()))
            .children_dyn(move || {
                let current = current();
                names.with(|n| {
                    n.iter()
                        .map(|(num, d)| {
                            let o = option().attr("value", num.to_string());
                            let o = if Some(*num) == current {
                                o.attr("selected", "")
                            } else {
                                o
                            };
                            o.text(d.0.clone()).into()
                        })
                        .collect()
                })
            }),
    )
}

/// Séries d'une télémétrie (une entrée de /api/of1/telemetry) sur la distance, en km.
fn tel_points(d: &Value, field: &str) -> Vec<(f64, f64)> {
    floats(d, "distance")
        .into_iter()
        .zip(floats(d, field))
        .map(|(x, y)| (x / 1000.0, y))
        .collect()
}

/// Télémétrie comparée du meilleur tour (car_data) et tracé GPS (location).
fn telemetry_view(key: u32, names: State<Rc<Names>>) -> Node {
    let res = use_json::<Value>(of1("session_result", &format!("session_key={key}")));
    let a = use_state(None::<i64>);
    let b = use_state(None::<i64>);
    // Les deux pilotes comparés, choisis ensemble : un seul rechargement quand ils changent.
    let pair = memo(move || {
        let top = top_drivers(&res.get(), &names.get(), 2);
        (
            a.get().or(top.first().copied()),
            b.get().or(top.get(1).copied()),
        )
    });
    let tel = use_json_dyn::<Value>(move || match pair.get() {
        (Some(x), Some(y)) => Some(format!(
            "/api/of1/telemetry?session_key={key}&drivers={x},{y}"
        )),
        _ => None,
    });
    let body = dynamic(move || {
        let tel = tel.get();
        state_view(&tel).unwrap_or_else(|| {
            let n = names.get();
            let drivers = list(&tel);
            let make = |field: &str| -> Vec<Series> {
                drivers
                    .iter()
                    .enumerate()
                    .map(|(k, d)| Series {
                        label: code(&n, int(d, "driver_number").unwrap_or(0)),
                        color: SERIES[k % 2].into(),
                        points: tel_points(d, field),
                    })
                    .collect()
            };
            let laps = drivers
                .iter()
                .map(|d| {
                    format!(
                        "{} {} (T{})",
                        code(&n, int(d, "driver_number").unwrap_or(0)),
                        num(d, "lap_duration").map(lap_str).unwrap_or_default(),
                        s(d, "lap_number")
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");
            let km = fmt(|v| format!("{v:.1} km"));
            fragment([
                Node::from(p().class("muted").text(laps)),
                h3().class("wx-sub")
                    .text(t("Vitesse (km/h)", "Speed (km/h)"))
                    .into(),
                LineChart::new(make("speed"), km.clone(), fmt(|v| format!("{v:.0}"))).view(),
                h3().class("wx-sub")
                    .text(t("Accélérateur (%)", "Throttle (%)"))
                    .into(),
                LineChart::new(make("throttle"), km.clone(), fmt(|v| format!("{v:.0}%")))
                    .height(120.0)
                    .view(),
                h3().class("wx-sub")
                    .text(t("Rapport engagé", "Gear"))
                    .into(),
                LineChart::new(make("gear"), km, fmt(|v| format!("{v:.0}")))
                    .height(110.0)
                    .step()
                    .view(),
            ])
        })
    });
    section()
        .class("card")
        .child(h2().text(t(
            "Télémétrie : meilleurs tours comparés",
            "Telemetry: fastest laps compared",
        )))
        .child(
            div()
                .class("tel-pick")
                .child(driver_select(
                    names,
                    move || pair.get().0,
                    move |d| a.set(d),
                ))
                .child(driver_select(
                    names,
                    move || pair.get().1,
                    move |d| b.set(d),
                )),
        )
        .child(body)
        .child(p().class("muted").text(t(
            "Données car_data OpenF1 (~4 mesures/s), alignées sur la distance parcourue.",
            "OpenF1 car_data (~4 samples/s), aligned on distance travelled.",
        )))
        .into()
}

/// Les tours d'une fiche course (`[]` sinon).
fn race_laps(v: &Value) -> &[Value] {
    v.get("laps")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Meilleur tour d'une fiche course, hors sorties des stands.
fn best_lap(laps: &[Value]) -> Option<u32> {
    laps.iter()
        .filter(|l| !l.get("pit_out").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|l| Some((num(l, "time")?, int(l, "lap")? as u32)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|x| x.1)
}

/// Écart d'un tour : « +3.2 s », « — » pour le leader, « +1 LAP »…
fn gap_str(x: &Value, k: &str) -> String {
    match x.get(k) {
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
    }
}

fn tyre_dot(c: &str) -> Element {
    i().class("tyre-dot")
        .style(format!("background:{}", tyre_colour(c)))
}

/// Fiche course d'un pilote : résumé, tour par tour (temps, secteurs, vitesses, position,
/// écarts, pneus, arrêts), télémétrie du tour choisi (vitesse, gaz, frein, rapport, régime,
/// DRS), messages de la direction de course et radios.
fn driver_view(key: u32, names: State<Rc<Names>>) -> Node {
    let res = use_json::<Value>(of1("session_result", &format!("session_key={key}")));
    // Pilote et tour choisis, changés ensemble (un nouveau pilote repart de son meilleur tour).
    let pick = use_state((None::<i64>, None::<u32>));
    let driver = memo(move || {
        pick.with(|x| x.0)
            .or_else(|| top_drivers(&res.get(), &names.get(), 1).first().copied())
    });
    let data = use_json_dyn::<Value>(move || {
        driver
            .get()
            .map(|d| format!("/api/of1/driverrace?session_key={key}&driver={d}"))
    });
    // Tour analysé : celui choisi, sinon le meilleur tour (hors sorties des stands).
    let chosen = memo(move || {
        pick.with(|x| x.1).or_else(|| {
            data.with(|d| match d {
                Some(Ok(v)) => best_lap(race_laps(v)),
                _ => None,
            })
        })
    });
    let tel = use_json_dyn::<Value>(move || match (driver.get(), chosen.get()) {
        (Some(d), Some(n)) => Some(format!(
            "/api/of1/telemetry?session_key={key}&drivers={d}&lap={n}"
        )),
        _ => None,
    });
    let picker = driver_select(names, move || driver.get(), move |d| pick.set((d, None)));
    let summary_part = dynamic(move || {
        let data = data.get();
        let Some(Ok(v)) = &data else {
            return state_view(&data).unwrap_or_else(loading);
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
        fragment([
            Node::from(
                p().class("drv-title")
                    .text(s(v, "name"))
                    .child(small().class("muted").text(format!(" · {}", s(v, "team")))),
            ),
            stat_grid(vec![
                (
                    t("Départ → arrivée", "Start → finish"),
                    format!("{} → {}", pos("grid"), finish),
                ),
                (
                    t("Points", "Points"),
                    num(v, "points")
                        .map(|x| format!("{x:.0}"))
                        .unwrap_or_else(|| "0".into()),
                ),
                (t("Championnat", "Championship"), champ),
                (t("Points au championnat", "Championship points"), champ_pts),
                (
                    t("Dépassements faits / subis", "Overtakes made / lost"),
                    format!("{} / {}", s(v, "overtakes_made"), s(v, "overtakes_lost")),
                ),
                (t("Arrêts aux stands", "Pit stops"), s(v, "pit_count")),
            ]),
        ])
    });
    let head = section()
        .class("card")
        .attr_dyn("style", move || {
            data.with(|d| match d {
                Some(Ok(v)) => format!("--team:#{}", s(v, "colour")),
                _ => String::new(),
            })
        })
        .child(h2().text(t("Fiche course du pilote", "Driver race file")))
        .child(picker)
        .child(summary_part);
    // Le reste de la fiche, une fois les données du pilote arrivées.
    let rest = dynamic(move || {
        let data = data.get();
        let Some(Ok(v)) = &data else {
            return Node::Empty;
        };
        let v = v.as_ref();
        let laps = race_laps(v);
        let best = best_lap(laps);
        let now = untrack(|| pick.with(|x| x.1)).or(best);
        // Graphique des temps au tour (tours lents écartés : arrêts, voiture de sécurité).
        let fastest = laps
            .iter()
            .filter_map(|l| num(l, "time"))
            .fold(f64::MAX, f64::min);
        let times = vec![Series {
            label: s(v, "code"),
            color: format!("#{}", s(v, "colour")),
            points: laps
                .iter()
                .filter_map(|l| {
                    Some((
                        num(l, "lap")?,
                        num(l, "time").filter(|x| *x < fastest * 1.1)?,
                    ))
                })
                .collect(),
        }];
        let lap_rows = laps.iter().map(|l| {
            let sec = |k: &str| {
                num(l, k)
                    .map(|x| format!("{x:.1}"))
                    .unwrap_or_else(|| "–".into())
            };
            let spd = |k: &str| {
                int(l, k)
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "–".into())
            };
            let age = int(l, "tyre_age")
                .map(|a| tr!(" · {a} t.", " · {a} laps"))
                .unwrap_or_default();
            let badge = if l.get("pit_in").and_then(Value::as_bool).unwrap_or(false) {
                num(l, "stop")
                    .map(|x| tr!("🔧 Arrêt {x:.1} s", "🔧 Stop {x:.1} s"))
                    .unwrap_or_else(|| t("🔧 Arrêt", "🔧 Pit").into())
            } else if l.get("pit_out").and_then(Value::as_bool).unwrap_or(false) {
                t("🔧 Sortie", "🔧 Out").into()
            } else {
                String::new()
            };
            let compound = s(l, "compound");
            li().class("row lap-row")
                .child(span().class("pos").text(format!("T{}", s(l, "lap"))))
                .child(
                    div()
                        .class("row-main")
                        .child(
                            span()
                                .class("row-title")
                                .text(num(l, "time").map(lap_str).unwrap_or_else(|| "–".into()))
                                .child(
                                    small()
                                        .class("muted")
                                        .text(format!("  ·  P{}", s(l, "position"))),
                                )
                                .child(
                                    (!badge.is_empty())
                                        .then(|| span().class("lap-badge").text(badge)),
                                ),
                        )
                        .child(span().class("row-sub").text(format!(
                            "S1 {} · S2 {} · S3 {} · {}/{}/{} km/h",
                            sec("s1"),
                            sec("s2"),
                            sec("s3"),
                            spd("i1"),
                            spd("i2"),
                            spd("st")
                        )))
                        .child(
                            span()
                                .class("row-sub")
                                .child(tyre_dot(&compound))
                                .text(format!("{}{}", tyre_name(&compound), age))
                                .text(tr!(
                                    " · leader {} · devant {}",
                                    " · leader {} · ahead {}",
                                    gap_str(l, "gap"),
                                    gap_str(l, "interval")
                                )),
                        ),
                )
        });
        let lap_options = laps.iter().filter_map(|l| int(l, "lap")).map(|n| {
            let o = option().attr("value", n.to_string());
            let o = if Some(n as u32) == now {
                o.attr("selected", "")
            } else {
                o
            };
            o.text(if Some(n as u32) == best {
                tr!("Tour {n} (meilleur)", "Lap {n} (fastest)")
            } else {
                tr!("Tour {n}", "Lap {n}")
            })
        });
        let tel_body = dynamic(move || {
            let tel = tel.get();
            state_view(&tel).unwrap_or_else(|| {
                let first = list(&tel).into_iter().next().unwrap_or(Value::Null);
                let series = |field: &str, color: &str| {
                    vec![Series {
                        label: field.into(),
                        color: color.into(),
                        points: tel_points(&first, field),
                    }]
                };
                let km = fmt(|v| format!("{v:.1} km"));
                let n0 = fmt(|v| format!("{v:.0}"));
                // Pas de DRS depuis 2026 : graphique masqué quand la donnée n'existe pas.
                let has_drs = first
                    .get("drs")
                    .and_then(Value::as_array)
                    .is_some_and(|a| !a.is_empty());
                let sub = |text: &'static str| Node::from(h3().class("wx-sub").text(text));
                fragment([
                    sub(t("Vitesse (km/h)", "Speed (km/h)")),
                    LineChart::new(series("speed", SERIES[0]), km.clone(), n0.clone()).view(),
                    sub(t("Régime moteur (tr/min)", "Engine speed (rpm)")),
                    LineChart::new(series("rpm", SERIES[1]), km.clone(), n0.clone())
                        .height(120.0)
                        .view(),
                    sub(t("Accélérateur (%)", "Throttle (%)")),
                    LineChart::new(
                        series("throttle", SERIES[2]),
                        km.clone(),
                        fmt(|v| format!("{v:.0}%")),
                    )
                    .height(110.0)
                    .view(),
                    sub(t("Freinage", "Braking")),
                    LineChart::new(
                        series("brake", SERIES[4]),
                        km.clone(),
                        fmt(|v| {
                            if v > 50.0 {
                                t("oui", "on").to_string()
                            } else {
                                t("non", "off").to_string()
                            }
                        }),
                    )
                    .height(80.0)
                    .step()
                    .view(),
                    sub(t("Rapport engagé", "Gear")),
                    LineChart::new(series("gear", SERIES[3]), km.clone(), n0)
                        .height(110.0)
                        .step()
                        .view(),
                    has_drs
                        .then(|| {
                            fragment([
                                sub(t("DRS (aileron ouvert)", "DRS (flap open)")),
                                LineChart::new(
                                    series("drs", SERIES[0]),
                                    km,
                                    fmt(|v| {
                                        if v > 75.0 {
                                            t("ouvert", "open").to_string()
                                        } else if v > 25.0 {
                                            t("autorisé", "armed").to_string()
                                        } else {
                                            t("fermé", "closed").to_string()
                                        }
                                    }),
                                )
                                .height(80.0)
                                .step()
                                .view(),
                            ])
                        })
                        .into(),
                ])
            })
        });
        let colour = s(v, "colour");
        let replay = dynamic(move || {
            let Some(first) = tel.with(list).into_iter().next() else {
                return Node::Empty;
            };
            let title = match untrack(|| chosen.get()) {
                Some(n) => tr!("Replay du tour {n}", "Lap {n} replay"),
                None => t("Replay du tour", "Lap replay").to_string(),
            };
            section()
                .class("card")
                .child(h2().text(title))
                .child(lap_replay(Rc::new(first), colour.clone()))
                .into()
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
        fragment([
            Node::from(
                section()
                    .class("card")
                    .child(h2().text(t("Temps au tour", "Lap times")))
                    .child(
                        LineChart::new(times, fmt(|v| tr!("T{v:.0}", "L{v:.0}")), fmt(lap_str))
                            .view(),
                    )
                    .child(p().class("muted").text(t(
                        "Tours lents (arrêts, voiture de sécurité) écartés du graphique.",
                        "Slow laps (pit stops, safety car) left out of the chart.",
                    ))),
            ),
            section()
                .class("card")
                .child(h2().text(tr!("Tour par tour ({})", "Lap by lap ({})", laps.len())))
                .child(ol().class("rows lap-rows").children(lap_rows))
                .into(),
            section()
                .class("card")
                .child(h2().text(t("Télémétrie d'un tour", "Lap telemetry")))
                .child(
                    label().class("select").child(
                        select()
                            .on("change", move |e| {
                                let n = e.value().parse().ok();
                                pick.update(|x| x.1 = n);
                            })
                            .children(lap_options),
                    ),
                )
                .child(tel_body)
                .child(p().class("muted").text(t(
                    "Données car_data OpenF1 (~4 mesures/s) sur la distance du tour. DRS « autorisé » : le pilote pourra l'ouvrir dans la prochaine zone.",
                    "OpenF1 car_data (~4 samples/s) over the lap distance. DRS “armed”: the driver can open it in the next zone.",
                )))
                .into(),
            replay,
            (!messages.is_empty())
                .then(|| {
                    section()
                        .class("card")
                        .child(h2().text(tr!(
                            "Direction de course ({})",
                            "Race control ({})",
                            messages.len()
                        )))
                        .child(ol().class("rows").children(messages.iter().map(|m| {
                            li().class("row").child(
                                div()
                                    .class("row-main")
                                    .child(span().class("row-title").text(s(m, "message")))
                                    .child(
                                        span()
                                            .class("row-sub")
                                            .text(
                                                int(m, "lap")
                                                    .map(|n| tr!("Tour {n}", "Lap {n}"))
                                                    .unwrap_or_default(),
                                            )
                                            .text(format!(
                                                " · {}",
                                                local_date(&s(m, "date"), true)
                                            )),
                                    ),
                            )
                        })))
                })
                .into(),
            (!radios.is_empty())
                .then(|| {
                    section()
                        .class("card")
                        .child(h2().text(tr!("Radios ({})", "Radio ({})", radios.len())))
                        .child(ol().class("rows").children(radios.iter().map(|r| {
                            li().class("row radio-row").child(
                                div()
                                    .class("row-main")
                                    .child(strong().text(radio_label(r)))
                                    .child(span().class("row-sub").text(radio_detail(r)))
                                    .child(
                                        audio()
                                            .attr("controls", "")
                                            .attr("preload", "metadata")
                                            .attr("src", s(r, "url")),
                                    ),
                            )
                        })))
                })
                .into(),
        ])
    });
    fragment([Node::from(head), rest])
}

/// « 1:23:05 » ou « 4:07 ».
fn race_clock(seconds: f64) -> String {
    let t = seconds.round().max(0.0) as u64;
    let (h, m, sec) = (t / 3600, t / 60 % 60, t % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// « Tour 12 · 1:05 dans le tour », « Avant le départ », « Après l'arrivée ».
fn radio_label(r: &Value) -> String {
    if r.get("after_finish")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return t("Après l'arrivée", "After the finish").to_string();
    }
    match num(r, "lap") {
        Some(lap) => {
            let base = tr!("Tour {}", "Lap {}", lap as u64);
            match num(r, "in_lap") {
                Some(x) => tr!(
                    "{} · {} dans le tour",
                    "{} · {} into the lap",
                    base,
                    race_clock(x)
                ),
                None => base,
            }
        }
        None => t("Avant le départ", "Before the start").to_string(),
    }
}

/// « Début 1:23:05 · 14:05 » (temps depuis le départ, puis heure locale).
fn radio_detail(r: &Value) -> String {
    let at = local_date(&s(r, "date"), true);
    match num(r, "elapsed").filter(|x| *x >= 0.0) {
        Some(x) => tr!(
            "Début {} après le départ · {}",
            "Starts {} after the start · {}",
            race_clock(x),
            at
        ),
        None => at,
    }
}

/// Dépassements, direction de course.
fn race_view(key: u32, names: State<Rc<Names>>) -> Node {
    let q = format!("session_key={key}");
    let overtakes = use_json::<Value>(of1("overtakes", &q));
    let rc = use_json::<Value>(of1("race_control", &q));
    let overtakes_body = dynamic(move || {
        let overtakes = overtakes.get();
        state_view(&overtakes).unwrap_or_else(|| {
            let n = names.get();
            let ov = list(&overtakes);
            let mut by_driver: BTreeMap<i64, usize> = BTreeMap::new();
            for o in &ov {
                if let Some(d) = int(o, "overtaking_driver_number") {
                    *by_driver.entry(d).or_default() += 1;
                }
            }
            let mut ranking: Vec<(i64, usize)> = by_driver.into_iter().collect();
            ranking.sort_by_key(|x| std::cmp::Reverse(x.1));
            fragment([
                Node::from(
                    ol().class("rows")
                        .children(ranking.iter().take(8).map(|(d, c)| {
                            li().class("row")
                                .child(
                                    div()
                                        .class("row-main")
                                        .child(span().class("row-title").text(name(&n, *d))),
                                )
                                .child(span().class("row-meta").text(tr!(
                                    "{} dépassements",
                                    "{} overtakes",
                                    c
                                )))
                        })),
                ),
                details()
                    .class("of1-details")
                    .child(summary().text(t("Tous les dépassements", "Every overtake")))
                    .child(ol().class("rows").children(ov.iter().map(|o| {
                        li().class("row")
                            .child(
                                div()
                                    .class("row-main")
                                    .child(span().class("row-title").text(format!(
                                        "{} → {}",
                                        code(&n, int(o, "overtaking_driver_number").unwrap_or(0)),
                                        code(&n, int(o, "overtaken_driver_number").unwrap_or(0))
                                    )))
                                    .child(
                                        span()
                                            .class("row-sub")
                                            .text(local_date(&s(o, "date"), true)),
                                    ),
                            )
                            .child(
                                span()
                                    .class("row-meta")
                                    .text(format!("P{}", s(o, "position"))),
                            )
                    })))
                    .into(),
            ])
        })
    });
    let rc_body = dynamic(move || {
        let rc = rc.get();
        state_view(&rc).unwrap_or_else(|| {
            ol().class("rows")
                .children(list(&rc).iter().rev().map(|m| {
                    li().class("row").child(
                        div()
                            .class("row-main")
                            .child(span().class("row-title").text(s(m, "message")))
                            .child(
                                span().class("row-sub").text(
                                    [
                                        s(m, "category"),
                                        s(m, "flag"),
                                        int(m, "lap_number")
                                            .map(|l| tr!("tour {}", "lap {}", l))
                                            .unwrap_or_default(),
                                    ]
                                    .into_iter()
                                    .filter(|x| !x.is_empty())
                                    .collect::<Vec<_>>()
                                    .join(" · "),
                                ),
                            ),
                    )
                }))
                .into()
        })
    });
    fragment([
        Node::from(
            section()
                .class("card")
                .child(h2().text_dyn(move || {
                    tr!("Dépassements ({})", "Overtakes ({})", overtakes.with(count))
                }))
                .child(overtakes_body),
        ),
        section()
            .class("card")
            .child(h2().text(t("Direction de course", "Race control")))
            .child(rc_body)
            .into(),
    ])
}

/// Radios d'équipe (fichiers audio publiés par la F1, liés tels quels).
fn radio_view(key: u32, names: State<Rc<Names>>) -> Node {
    let radio = use_json::<Value>(of1("team_radio", &format!("session_key={key}")));
    let body = dynamic(move || {
        let radio = radio.get();
        state_view(&radio).unwrap_or_else(|| {
            let n = names.get();
            ol().class("rows")
                .children(list(&radio).iter().rev().take(60).map(|r| {
                    let d = int(r, "driver_number").unwrap_or(0);
                    li().class("row radio-row").style(team_var(&n, d)).child(
                        div()
                            .class("row-main")
                            .child(span().class("row-title").text(name(&n, d)))
                            .child(
                                span()
                                    .class("row-sub")
                                    .text(local_date(&s(r, "date"), true)),
                            )
                            .child(
                                audio()
                                    .attr("controls", "")
                                    .attr("preload", "none")
                                    .attr("src", s(r, "recording_url")),
                            ),
                    )
                }))
                .into()
        })
    });
    section()
        .class("card")
        .child(
            h2().text_dyn(move || {
                tr!("Radios d'équipe ({})", "Team radio ({})", radio.with(count))
            }),
        )
        .child(body)
        .child(p().class("muted").text(t(
            "Enregistrements publiés par la Formula 1 et référencés par OpenF1 (60 plus récents).",
            "Recordings published by Formula 1 and referenced by OpenF1 (latest 60).",
        )))
        .into()
}

/// Météo de la séance (air, piste, vent, pluie).
fn weather_view(key: u32) -> Node {
    let w = use_json::<Value>(of1("weather", &format!("session_key={key}")));
    dynamic(move || {
        let w = w.get();
        if let Some(v) = state_view(&w) {
            return card(v);
        }
        let all = list(&w);
        let t0 = all.first().map(|x| ms(&s(x, "date"))).unwrap_or(0.0);
        let pts = |k: &str| {
            all.iter()
                .filter_map(|x| Some(((ms(&s(x, "date")) - t0) / 60_000.0, num(x, k)?)))
                .collect::<Vec<_>>()
        };
        let temps = vec![
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
        ];
        let wind = vec![Series {
            label: t("Vent", "Wind").into(),
            color: SERIES[2].into(),
            points: pts("wind_speed")
                .into_iter()
                .map(|(a, b)| (a, b * 3.6))
                .collect(),
        }];
        let rain = all.iter().any(|x| num(x, "rainfall").unwrap_or(0.0) > 0.0);
        let last = all.last().cloned().unwrap_or(Value::Null);
        let min = fmt(|v| format!("{v:.0} min"));
        section()
            .class("card")
            .child(h2().text(t("Météo de la séance", "Session weather")))
            .child(stat_grid(vec![
                (
                    "Air",
                    num(&last, "air_temperature")
                        .map(|v| format!("{v:.1}°"))
                        .unwrap_or_default(),
                ),
                (
                    t("Piste", "Track"),
                    num(&last, "track_temperature")
                        .map(|v| format!("{v:.1}°"))
                        .unwrap_or_default(),
                ),
                (
                    t("Humidité", "Humidity"),
                    num(&last, "humidity")
                        .map(|v| format!("{v:.0}%"))
                        .unwrap_or_default(),
                ),
                (
                    t("Pression", "Pressure"),
                    num(&last, "pressure")
                        .map(|v| format!("{v:.0} hPa"))
                        .unwrap_or_default(),
                ),
                (
                    t("Vent", "Wind"),
                    num(&last, "wind_speed")
                        .map(|v| format!("{:.0} km/h", v * 3.6))
                        .unwrap_or_default(),
                ),
                (
                    t("Pluie", "Rain"),
                    if rain {
                        t("oui", "yes").into()
                    } else {
                        t("non", "no").into()
                    },
                ),
            ]))
            .child(
                h3().class("wx-sub")
                    .text(t("Températures (°C)", "Temperatures (°C)")),
            )
            .child(LineChart::new(temps, min.clone(), fmt(|v| format!("{v:.0}°"))).view())
            .child(h3().class("wx-sub").text(t("Vent (km/h)", "Wind (km/h)")))
            .child(
                LineChart::new(wind, min, fmt(|v| format!("{v:.0}")))
                    .height(120.0)
                    .view(),
            )
            .into()
    })
}

// ---------- Depuis la page d'un Grand Prix ----------

/// Instant de référence d'une course (`date` : « 2025-07-06 »).
fn race_ms(date: &str) -> f64 {
    ms(&format!("{date}T12:00:00Z"))
}

/// Lien « Analyse OpenF1 » d'une page de Grand Prix : retrouve la réunion d'après la date
/// de la course (`date` : « 2025-07-06 »).
// Appelée par la page d'un Grand Prix (pages/race.rs).
#[allow(dead_code)]
pub fn meeting_link(year: u32, date: &str) -> Node {
    let meetings =
        use_json::<Value>((year >= 2023).then(|| format!("/api/of1/meetings?year={year}")));
    let race = race_ms(date);
    dynamic(move || {
        let found = meetings.with(|v| {
            list(v).into_iter().find(|m| {
                let start = ms(&s(m, "date_start"));
                race >= start - 86_400_000.0 && race <= start + 5.0 * 86_400_000.0
            })
        });
        match found.and_then(|m| int(&m, "meeting_key")) {
            Some(key) => link(Route::DataMeeting { key: key as u32 }, "btn btn-ghost")
                .text(t(
                    "📊 Analyse détaillée (données OpenF1)",
                    "📊 Detailed analysis (OpenF1 data)",
                ))
                .into(),
            None => Node::Empty,
        }
    })
}

/// « Tout savoir sur la course » en haut de la page d'un Grand Prix terminé : la fiche de
/// chaque pilote, les tours, positions, pneus, télémétrie, direction de course… affichés
/// directement dans la page, un bouton par rubrique.
// Appelée par la page d'un Grand Prix (pages/race.rs).
#[allow(dead_code)]
pub fn race_data_links(year: u32, date: &str) -> Node {
    let view = use_state(View::Driver);
    let sessions = use_json::<Value>(
        (year >= 2023).then(|| format!("/api/of1/sessions?year={year}&session_name=Race")),
    );
    let race = race_ms(date);
    let key = memo(move || {
        sessions.with(|v| {
            list(v)
                .into_iter()
                .find(|x| (ms(&s(x, "date_start")) - race).abs() <= 2.0 * 86_400_000.0)
                .and_then(|x| int(&x, "session_key"))
                .map(|k| k as u32)
        })
    });
    let drivers = use_json_dyn::<Value>(move || {
        key.get()
            .and_then(|k| of1("drivers", &format!("session_key={k}")))
    });
    let names = memo(move || Rc::new(driver_map(&drivers.with(list))));
    dynamic(move || {
        let Some(key) = key.get() else {
            return Node::Empty;
        };
        let tab = |v: View, label: &'static str| {
            button()
                .class("btn btn-ghost race-data-btn")
                .class_if("on", move || view.get() == v)
                .on_click(move |_| {
                    if view.get() != v {
                        view.set(v);
                    }
                })
                .text(label)
        };
        fragment([
            Node::from(
                section()
                    .class("card")
                    .child(h2().text(t("Tout savoir sur la course", "Everything about the race")))
                    .child(p().class("muted").text(t(
                        "Fiche de chaque pilote tour par tour, secteurs, écarts, pneus, arrêts, télémétrie, direction de course et radios.",
                        "Each driver's lap-by-lap file, sectors, gaps, tyres, stops, telemetry, race control and radio.",
                    )))
                    .child(
                        div()
                            .class("race-data-grid")
                            .child(tab(View::Driver, t("👤 Fiche pilote", "👤 Driver file")))
                            .child(tab(View::Results, t("🏁 Résultats", "🏁 Results")))
                            .child(tab(View::Laps, t("⏱ Tours", "⏱ Laps")))
                            .child(tab(View::Positions, t("↕ Positions", "↕ Positions")))
                            .child(tab(View::Tyres, t("🛞 Pneus", "🛞 Tyres")))
                            .child(tab(View::Telemetry, t("📈 Télémétrie", "📈 Telemetry")))
                            .child(tab(
                                View::Race,
                                t("🚩 Direction de course", "🚩 Race control"),
                            ))
                            .child(tab(View::Radio, t("🎧 Radios", "🎧 Radio")))
                            .child(tab(View::Weather, t("🌦 Météo", "🌦 Weather"))),
                    ),
            ),
            dynamic(move || view_content(view.get(), key, names)),
        ])
    })
}

/// Nom court d'une séance rejouable.
fn replay_label(name: &str) -> &'static str {
    match name {
        "Race" => t("Course", "Race"),
        "Qualifying" => t("Qualifs", "Quali"),
        "Sprint" => "Sprint",
        _ => t("Qualifs sprint", "Sprint quali"),
    }
}

/// Une ligne du classement du replay (les 10 premiers).
#[derive(Clone, PartialEq)]
struct ReplayRow {
    position: u32,
    colour: String,
    name: String,
    status: String,
}

/// Replay automatique d'une course terminée (2023 et après) sur la page du Grand Prix :
/// démarre tout seul à ×30, voitures à leurs positions réelles sur le circuit 3D.
// Appelée par la page d'un Grand Prix (pages/race.rs).
#[allow(dead_code)]
pub fn race_replay(year: u32, date: &str) -> Node {
    use f1x_protocol::ClientMsg;
    let sessions =
        use_json::<Value>((year >= 2023).then(|| format!("/api/of1/sessions?year={year}")));
    let race = race_ms(date);
    // Séances du week-end qu'on peut rejouer : course, qualifications, sprint, qualifs sprint.
    let weekend = memo(move || {
        let order = [
            "Race",
            "Qualifying",
            "Sprint",
            "Sprint Qualifying",
            "Sprint Shootout",
        ];
        let mut weekend: Vec<(String, u32)> = sessions
            .with(list)
            .into_iter()
            .filter(|x| {
                let d = race - ms(&s(x, "date_start"));
                d < 4.0 * 86_400_000.0 && d > -2.0 * 86_400_000.0
            })
            .filter(|x| order.contains(&s(x, "session_name").as_str()))
            .filter_map(|x| Some((s(&x, "session_name"), int(&x, "session_key")? as u32)))
            .collect();
        weekend.sort_by_key(|w| order.iter().position(|o| *o == w.0).unwrap_or(9));
        weekend
    });
    let chosen = use_state(None::<u32>);
    let key = memo(move || {
        chosen
            .get()
            .or_else(|| weekend.with(|w| w.first().map(|w| w.1)))
    });
    let racing = move || {
        let key = key.get();
        weekend.with(|w| {
            w.iter()
                .find(|w| Some(w.1) == key)
                .is_none_or(|w| w.0 == "Race" || w.0 == "Sprint")
        })
    };
    let live = crate::live::use_live();
    {
        // Rejoue la séance choisie ; arrête la précédente avant, et le replay en partant.
        let started = Rc::new(Cell::new(false));
        effect(move || {
            let key = key.get();
            untrack(|| {
                if started.replace(true) {
                    live.send(ClientMsg::Stop);
                }
                if let Some(session_key) = key {
                    live.send(ClientMsg::Replay {
                        session_key,
                        speed: 30,
                    });
                }
            });
        });
        on_cleanup(move || live.send(ClientMsg::Stop));
    }
    // Le circuit 3D n'est construit qu'une fois la première image et le tracé reçus (et
    // reconstruit si le tracé change) ; ensuite seules les valeurs suivent le replay.
    let shown_track = memo(move || {
        live.state.with(|st| match (&st.snapshot, &st.track) {
            (Some(_), Some(track)) => Some(Rc::as_ptr(track) as usize),
            _ => None,
        })
    });
    let body = move || {
        dynamic(move || {
            if shown_track.get().is_none() {
                return p()
                    .class("muted")
                    .text_dyn(move || {
                        live.state.with(|st| {
                            st.loading.clone().unwrap_or_else(|| {
                                t("Chargement du replay…", "Loading replay…").into()
                            })
                        })
                    })
                    .into();
            }
            let Some(track) = untrack(|| live.state.with(|st| st.track.clone())) else {
                return Node::Empty;
            };
            let snap_with = move |f: &dyn Fn(&f1x_protocol::Snapshot) -> String| {
                live.state
                    .with(|st| st.snapshot.as_deref().map(f).unwrap_or_default())
            };
            let lap = move || {
                let racing = racing();
                let key = key.get();
                let name = weekend.with(|w| {
                    w.iter()
                        .find(|w| Some(w.1) == key)
                        .map(|w| replay_label(&w.0))
                        .unwrap_or_default()
                });
                snap_with(&|snap| match snap.total_laps {
                    _ if !racing => {
                        // Qualifications : heure de la séance plutôt qu'un compteur de tours.
                        format!("{name} · {}", local_date(&snap.clock, true))
                    }
                    Some(total) => tr!("Tour {}/{total}", "Lap {}/{total}", snap.lap.min(total)),
                    None => tr!("Tour {}", "Lap {}", snap.lap),
                })
            };
            let markers = move || {
                live.state.with(|st| {
                    st.snapshot.as_ref().map(|snap| {
                        Rc::new(
                            snap.cars
                                .iter()
                                .filter(|c| c.lap_progress.is_some() && !c.retired)
                                .map(|c| crate::gl3d::Marker {
                                    key: c.code.clone(),
                                    label: format!("{} {}", c.position, c.code),
                                    colour: c.colour.clone(),
                                    fraction: c.lap_progress.unwrap_or(0.0),
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                })
            };
            // Les 10 premiers : 10 lignes fixes dont seules les valeurs changent.
            let rows = memo(move || {
                live.state.with(|st| {
                    let Some(snap) = &st.snapshot else {
                        return Vec::new();
                    };
                    let mut order: Vec<_> = snap.cars.iter().collect();
                    order.sort_by_key(|c| c.position);
                    order
                        .iter()
                        .take(10)
                        .map(|c| ReplayRow {
                            position: c.position,
                            colour: c.colour.clone(),
                            name: c.name.clone(),
                            status: if c.in_pit {
                                "PIT".to_string()
                            } else if c.position == 1 {
                                t("Leader", "Leader").to_string()
                            } else {
                                c.gap.clone()
                            },
                        })
                        .collect()
                })
            });
            let row_text = move |k: usize, f: fn(&ReplayRow) -> String| {
                move || rows.with(|r| r.get(k).map(f).unwrap_or_default())
            };
            let cmd = move |m: ClientMsg| move |_: Event| live.send(m.clone());
            let paused = move || {
                live.state
                    .with(|st| st.snapshot.as_ref().is_some_and(|x| x.paused))
            };
            let speeds = [1u32, 5, 10, 30, 60];
            fragment([
                Node::from(p().class("replay-lap").child(strong().text_dyn(lap))),
                div()
                    .class("progress")
                    .child(span().attr_dyn("style", move || {
                        snap_with(&|snap| format!("width:{:.1}%", snap.progress * 100.0))
                    }))
                    .into(),
                crate::gl3d::view_3d(
                    crate::gl3d::Scene::Track {
                        map: track,
                        ghost: false,
                    },
                    markers,
                ),
                div()
                    .class("replay-controls")
                    .child(
                        button()
                            .class("btn btn-ghost")
                            .on_click(cmd(ClientMsg::Seek { seconds: -120 }))
                            .text("−2 min"),
                    )
                    .child(
                        button()
                            .class("btn")
                            .on_click(move |_| {
                                live.send(if paused() {
                                    ClientMsg::Resume
                                } else {
                                    ClientMsg::Pause
                                })
                            })
                            .text_dyn(move || {
                                if paused() {
                                    t("▶ Lecture", "▶ Play")
                                } else {
                                    t("⏸ Pause", "⏸ Pause")
                                }
                                .to_string()
                            }),
                    )
                    .child(
                        button()
                            .class("btn btn-ghost")
                            .on_click(cmd(ClientMsg::Seek { seconds: 120 }))
                            .text("+2 min"),
                    )
                    .children(speeds.iter().map(|&v| {
                        button()
                            .class("btn btn-ghost")
                            .class_if("on", move || {
                                live.state
                                    .with(|st| st.snapshot.as_ref().is_some_and(|x| x.speed == v))
                            })
                            .on_click(cmd(ClientMsg::Speed { speed: v }))
                            .text(format!("×{v}"))
                    }))
                    .into(),
                ol().class("rows")
                    .children_keyed(
                        move || (0..rows.with(Vec::len)).collect::<Vec<usize>>(),
                        |k| *k,
                        move |&k| {
                            li().class("row")
                                .child(
                                    span()
                                        .class("pos")
                                        .text_dyn(row_text(k, |r| r.position.to_string())),
                                )
                                .child(span().class("team-bar").attr_dyn(
                                    "style",
                                    row_text(k, |r| format!("background:#{}", r.colour)),
                                ))
                                .child(
                                    div().class("row-main").child(
                                        span()
                                            .class("row-title")
                                            .text_dyn(row_text(k, |r| r.name.clone())),
                                    ),
                                )
                                .child(
                                    span()
                                        .class("muted")
                                        .text_dyn(row_text(k, |r| r.status.clone())),
                                )
                                .into()
                        },
                    )
                    .into(),
            ])
        })
    };
    let has_key = memo(move || key.with(Option::is_some));
    dynamic(move || {
        if !has_key.get() {
            return Node::Empty;
        }
        let tabs = dynamic(move || {
            let weekend = weekend.get();
            if weekend.len() <= 1 {
                return Node::Empty;
            }
            div()
                .class("segmented")
                .children(weekend.into_iter().map(|(name, k)| {
                    button()
                        .class("seg")
                        .class_if("seg-active", move || key.get() == Some(k))
                        .on_click(move |_| chosen.set(Some(k)))
                        .text(replay_label(&name))
                }))
                .into()
        });
        section()
            .class("card")
            .child(h2().text(t(
                "Replay : course, qualifs, sprint",
                "Replay: race, quali, sprint",
            )))
            .child(tabs)
            .child(body())
            .child(p().class("muted").text(t(
                "Démarre tout seul à ×30. Positions de chaque pilote d'après son avancement dans le tour (données OpenF1).",
                "Starts on its own at ×30. Each driver's position from their progress through the lap (OpenF1 data).",
            )))
            .into()
    })
}

/// Replay d'un tour : la voiture parcourt le circuit au rythme réel du tour, tableau de bord
/// synchronisé (vitesse, rapport, régime, accélérateur, frein, DRS). `tel` : télémétrie du
/// tour (une entrée de /api/of1/telemetry).
fn lap_replay(tel: Rc<Value>, colour: String) -> Node {
    let times = Rc::new(floats(&tel, "time"));
    if times.is_empty() {
        return p()
            .class("muted")
            .text(t(
                "Replay indisponible pour ce tour.",
                "Replay unavailable for this lap.",
            ))
            .into();
    }
    let circuit = s(&tel, "circuit_id");
    let track_map = use_json::<f1x_protocol::TrackMap>(
        (!circuit.is_empty()).then(|| format!("/api/track/{circuit}")),
    );
    let playing = use_state(true);
    let speed = use_state(1.0f64);
    let elapsed = use_state(0.0f64);
    let duration = num(&tel, "lap_duration")
        .or_else(|| times.last().copied())
        .unwrap_or(1.0)
        .max(1.0);

    // Horloge du replay : toutes les 50 ms pendant la lecture, arrêtée en quittant la vue.
    let root: Rc<RefCell<Option<web_sys::Element>>> = Rc::default();
    let timer: Rc<RefCell<Option<Interval>>> = Rc::default();
    {
        let (root, timer) = (root.clone(), timer.clone());
        effect(move || {
            let (on, k) = (playing.get(), speed.get());
            let next = on.then(|| {
                let (root, timer) = (root.clone(), timer.clone());
                Interval::new(50, move || {
                    // Vue retirée de la page sans avoir été libérée : l'horloge s'arrête seule.
                    let gone = root.borrow().as_ref().is_some_and(|el| !el.is_connected());
                    if gone || !elapsed.is_alive() {
                        let _ = timer.borrow_mut().take();
                        return;
                    }
                    elapsed.update(|e| *e = (*e + 0.05 * k) % duration);
                })
            });
            untrack(|| *timer.borrow_mut() = next);
        });
    }
    on_cleanup(move || drop(timer.borrow_mut().take()));

    // Mesure affichée : la dernière atteinte au temps écoulé.
    let idx = {
        let times = times.clone();
        memo(move || {
            let e = elapsed.get();
            times.partition_point(|&x| x < e).min(times.len() - 1)
        })
    };
    let at = |k: &str| {
        let values = Rc::new(floats(&tel, k));
        move || values.get(idx.get()).copied().unwrap_or(0.0)
    };
    let (kmh, gear, rpm, throttle, brake, drs) = (
        at("speed"),
        at("gear"),
        at("rpm"),
        at("throttle"),
        at("brake"),
        at("drs"),
    );
    let has_drs = !floats(&tel, "drs").is_empty();
    let dist = Rc::new(floats(&tel, "distance"));
    let map_view = dynamic(move || {
        let track_map = track_map.get();
        let Some(Ok(m)) = track_map else {
            return Node::Empty;
        };
        if m.points.len() < 2 {
            return Node::Empty;
        }
        let pad = 40.0;
        let mut cum = vec![0.0f64];
        for w in m.points.windows(2) {
            let d = f64::from(w[1].x - w[0].x).hypot(f64::from(w[1].y - w[0].y));
            cum.push(cum.last().copied().unwrap_or(0.0) + d);
        }
        let line_pts = m
            .points
            .iter()
            .map(|q| format!("{:.0},{:.0}", f64::from(q.x) + pad, f64::from(q.y) + pad))
            .collect::<Vec<_>>()
            .join(" ");
        // Point du tracé à la même fraction de distance que la voiture.
        let car = {
            let (m, dist) = (m.clone(), dist.clone());
            memo(move || {
                let i = idx.get();
                let frac = match (dist.get(i), dist.last()) {
                    (Some(&d), Some(&total)) if total > 0.0 => d / total,
                    _ => 0.0,
                };
                let target = frac * cum.last().copied().unwrap_or(0.0);
                let k = cum.partition_point(|&c| c < target).min(m.points.len() - 1);
                (
                    format!("{:.0}", f64::from(m.points[k].x) + pad),
                    format!("{:.0}", f64::from(m.points[k].y) + pad),
                )
            })
        };
        svg()
            .class("outline lap-replay-map")
            .attr(
                "viewBox",
                format!("0 0 {:.0} {:.0}", m.width + 2.0 * pad, m.height + 2.0 * pad),
            )
            .attr("role", "img")
            .attr(
                "aria-label",
                t(
                    "Position de la voiture sur le circuit",
                    "Car position on the circuit",
                ),
            )
            .child(
                polyline()
                    .class("outline-base")
                    .attr("points", line_pts.clone()),
            )
            .child(
                polyline()
                    .class("outline-line")
                    .attr("pathLength", "1")
                    .attr("points", line_pts),
            )
            .child(
                circle()
                    .attr_dyn("cx", move || car.with(|c| c.0.clone()))
                    .attr_dyn("cy", move || car.with(|c| c.1.clone()))
                    .attr("r", "34")
                    .attr("fill", format!("#{colour}"))
                    .attr("stroke", "#fff")
                    .attr("stroke-width", "10"),
            )
            .into()
    });
    let bar = |label: &'static str,
               text: Box<dyn Fn() -> String>,
               ratio: Box<dyn Fn() -> f64>,
               class: &'static str| {
        div()
            .class("lap-bar")
            .child(
                span()
                    .class("lap-bar-head")
                    .child(span().class("muted").text(label))
                    .child(span().text_dyn(text)),
            )
            .child(span().class("lap-bar-track").child(
                span().class(class).attr_dyn("style", move || {
                    format!("width:{:.0}%", ratio().clamp(0.0, 1.0) * 100.0)
                }),
            ))
    };
    let (rpm_ratio, throttle_ratio) = (rpm.clone(), throttle.clone());
    let (drs_open, drs_armed) = (drs.clone(), drs.clone());
    div()
        .class("lap-replay")
        .on_mount(move |el| *root.borrow_mut() = Some(el.clone()))
        .child(map_view)
        .child(
            div()
                .class("lap-dash")
                .child(
                    span()
                        .class("lap-speed")
                        .child(b().text_dyn(move || format!("{:.0}", kmh())))
                        .text(" km/h"),
                )
                .child(
                    span()
                        .class("lap-gear")
                        .child(b().text_dyn(move || {
                            let g = gear();
                            if g < 1.0 {
                                "N".to_string()
                            } else {
                                format!("{g:.0}")
                            }
                        }))
                        .text(t(" rapport", " gear")),
                )
                .child(span().class("lap-clock").text_dyn(move || {
                    format!("{} / {}", lap_str(elapsed.get()), lap_str(duration))
                })),
        )
        .child(bar(
            t("Régime", "RPM"),
            Box::new(move || format!("{:.0} tr/min", rpm())),
            Box::new(move || rpm_ratio() / 13_000.0),
            "fill-rpm",
        ))
        .child(bar(
            t("Accélérateur", "Throttle"),
            Box::new(move || format!("{:.0} %", throttle())),
            Box::new(move || throttle_ratio() / 100.0),
            "fill-throttle",
        ))
        .child(
            div()
                .class("lap-flags")
                .child(
                    span()
                        .class("lap-flag")
                        .class_if("brake-on", move || brake() > 0.0)
                        .text(t("Frein", "Brake")),
                )
                .child(has_drs.then(|| {
                    span()
                        .class("lap-flag")
                        .class_if("drs-open", move || drs_open() >= 100.0)
                        .class_if("drs-armed", move || (50.0..100.0).contains(&drs_armed()))
                        .text_dyn(move || {
                            let v = drs();
                            if v >= 100.0 {
                                t("DRS ouvert", "DRS open")
                            } else if v >= 50.0 {
                                t("DRS autorisé", "DRS armed")
                            } else {
                                "DRS"
                            }
                            .to_string()
                        })
                })),
        )
        .child(
            div()
                .class("replay-controls")
                .child(
                    button()
                        .class("btn")
                        .on_click(move |_| playing.update(|on| *on = !*on))
                        .text_dyn(move || {
                            if playing.get() {
                                t("⏸ Pause", "⏸ Pause")
                            } else {
                                t("▶ Lecture", "▶ Play")
                            }
                            .to_string()
                        }),
                )
                .child(
                    button()
                        .class("btn btn-ghost")
                        .on_click(move |_| elapsed.set(0.0))
                        .text(t("⏮ Début", "⏮ Start")),
                )
                .children([1.0f64, 2.0, 4.0].into_iter().map(|v| {
                    button()
                        .class("btn btn-ghost")
                        .class_if("on", move || speed.get() == v)
                        .on_click(move |_| speed.set(v))
                        .text(format!("×{v:.0}"))
                })),
        )
        .into()
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

/// Pneus retirés (`after` faux : « 23 tours ») ou montés (« neufs », « usagés (4 t.) »).
fn pit_tyre(x: &Value, after: bool) -> Node {
    if !x.is_object() {
        return span().class("muted").text("?").into();
    }
    let c = s(x, "compound");
    let detail = if after {
        match int(x, "age_at_start").unwrap_or(0) {
            0 => t("neufs", "new").to_string(),
            n => tr!("usagés ({n} t.)", "used ({n} laps)"),
        }
    } else {
        tr!("{} tours", "{} laps", int(x, "age_end").unwrap_or(0))
    };
    span()
        .class("pit-tyre")
        .child(i().style(format!("background:{}", tyre_colour(&c))))
        .text(format!("{} {}", tyre_name(&c), detail))
        .into()
}

/// Détail de chaque arrêt : tour, temps dans la voie et à l'arrêt, pneus retirés et montés,
/// position avant et après (sources OpenF1 pit, stints, position, drivers). `query` :
/// `session_key=…` ou `year=…&date=…` (course retrouvée par le serveur).
pub fn pit_detail(query: &str) -> Node {
    let data = use_json::<Value>(Some(format!("/api/of1/pitdetail?{query}")));
    dynamic(move || {
        let data = data.get();
        if let Some(v) = state_view(&data) {
            return section()
                .class("card")
                .child(h2().text(t("Détail de chaque arrêt", "Every pit stop in detail")))
                .child(v)
                .into();
        }
        let Some(Ok(v)) = &data else {
            return Node::Empty;
        };
        let stops = v
            .get("stops")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if stops.is_empty() {
            return Node::Empty;
        }
        let fastest = stops
            .iter()
            .filter_map(|x| num(x, "stop_duration"))
            .fold(f64::MAX, f64::min);
        section()
            .class("card")
            .child(h2().text(tr!(
                "Détail de chaque arrêt ({})",
                "Every pit stop in detail ({})",
                stops.len()
            )))
            .child(ol().class("rows pit-detail").children(stops.iter().map(|x| {
                let (pb, pa) = (int(x, "position_before"), int(x, "position_after"));
                let stationary = num(x, "stop_duration");
                let lane = num(x, "lane_duration");
                let best = stationary.is_some_and(|v| (v - fastest).abs() < 1e-6);
                let moved = match (pb, pa) {
                    (Some(before), Some(after)) => Some(
                        span()
                            .class("pit-pos")
                            .class(when(after > before, "lost"))
                            .class(when(after < before, "won"))
                            .text(format!("P{before} → P{after}")),
                    ),
                    _ => None,
                };
                li().class("row pit-row")
                    .style(format!("--team:#{}", s(x, "colour")))
                    .child(span().class("pos").text(format!("T{}", s(x, "lap"))))
                    .child(
                        div()
                            .class("row-main")
                            .child(
                                span()
                                    .class("row-title")
                                    .text(s(x, "name"))
                                    .child(
                                        small()
                                            .class("muted")
                                            .text(format!(" · {}", s(x, "team"))),
                                    ),
                            )
                            .child(
                                span()
                                    .class("pit-tyres")
                                    .child(pit_tyre(
                                        x.get("tyre_before").unwrap_or(&Value::Null),
                                        false,
                                    ))
                                    .child(span().attr("aria-hidden", "true").text(" → "))
                                    .child(pit_tyre(
                                        x.get("tyre_after").unwrap_or(&Value::Null),
                                        true,
                                    )),
                            )
                            .child(
                                span()
                                    .class("row-sub")
                                    .text(
                                        lane.map(|v| {
                                            tr!("Voie des stands {:.1} s", "Pit lane {:.1} s", v)
                                        })
                                        .unwrap_or_default(),
                                    )
                                    .text(
                                        stationary
                                            .map(|v| {
                                                tr!(
                                                    " · immobilisé {:.1} s",
                                                    " · stationary {:.1} s",
                                                    v
                                                )
                                            })
                                            .unwrap_or_default(),
                                    )
                                    .child(best.then(|| {
                                        strong()
                                            .class("pit-best")
                                            .text(t(" · le plus rapide", " · fastest"))
                                    })),
                            ),
                    )
                    .child(moved)
            })))
            .child(p().class("muted").text(t(
                "Voie des stands : de l'entrée à la sortie. Immobilisé : voiture à l'arrêt pendant le changement de pneus. Positions juste avant l'entrée et juste après la ressortie. Données OpenF1.",
                "Pit lane: entry to exit. Stationary: car stopped for the tyre change. Positions just before entry and just after exit. OpenF1 data.",
            )))
            .into()
    })
}
