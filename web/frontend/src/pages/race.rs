use std::collections::HashMap;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlSelectElement};
use yew::prelude::*;
use yew_router::prelude::*;

use super::season_label;
use crate::Route;
use crate::api::{Fetch, all, f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::models::{MrData, PitStop, RaceResult, translate_status};
use crate::tr;
use crate::util::{flag_country, now_ms, team_color, team_style};

#[derive(Properties, PartialEq)]
pub struct RacePageProps {
    pub season: AttrValue,
    pub round: u32,
}

#[function_component]
pub fn RacePage(props: &RacePageProps) -> Html {
    let season = props.season.to_string();
    let round = props.round;
    let base = format!("{season}/{round}");
    let show_laps = use_state(|| false);

    let schedule = use_f1(f1(format!("{season}.json"), 100));
    let race = schedule
        .done()
        .and_then(|d| d.races().iter().find(|r| r.round_num() == round).cloned());
    let now = now_ms();
    // Rien à chercher plus de 3 jours avant la course.
    let started = race
        .as_ref()
        .is_some_and(|r| now > r.start_ms() - 3.0 * 86_400_000.0);
    let over = race.as_ref().is_some_and(|r| r.is_over(now));
    let year: u32 = race
        .as_ref()
        .and_then(|r| r.season.parse().ok())
        .unwrap_or(0);
    let when = |cond: bool, file: &str, limit| {
        if cond {
            f1(format!("{base}/{file}.json"), limit)
        } else {
            None
        }
    };

    let results = use_f1(when(over, "results", 100));
    let sprint = use_f1(when(
        started && race.as_ref().is_some_and(|r| r.is_sprint_weekend()),
        "sprint",
        100,
    ));
    let qualifying = use_f1(when(started && year >= 1994, "qualifying", 100));
    // Arrêts aux stands disponibles depuis 2011, tours depuis 1996.
    let pit_stops = use_f1(when(over && year >= 2011, "pitstops", 100));
    let status = use_f1(when(over, "status", 100));
    let laps = use_f1(if *show_laps && over && year >= 1996 {
        all(format!("{base}/laps.json"))
    } else {
        None
    });

    let Some(race) = race else {
        let title = format!("Grand Prix · {}", season_label(&season));
        return match schedule {
            Fetch::Done(_) => html! { <super::NotFound /> },
            _ => {
                html! { <Layout title={title} tab={Tab::Calendar}>{ fetch_view(&schedule, |_| html! {}) }</Layout> }
            }
        };
    };
    let total = schedule.done().map(|d| d.races().len() as u32).unwrap_or(0);
    let results_list: Vec<RaceResult> = results
        .done()
        .and_then(|d| d.race())
        .and_then(|r| r.results.clone())
        .unwrap_or_default();
    let sprint_list = sprint
        .done()
        .and_then(|d| d.race())
        .and_then(|r| r.sprint_results.clone())
        .unwrap_or_default();
    let quali_list = qualifying
        .done()
        .and_then(|d| d.race())
        .and_then(|r| r.qualifying_results.clone())
        .unwrap_or_default();
    let stops: Vec<PitStop> = pit_stops.done().map(MrData::pit_stops).unwrap_or_default();
    let statuses = status
        .done()
        .map(|d| d.statuses().to_vec())
        .unwrap_or_default();

    let mut fastest: Vec<&RaceResult> = results_list
        .iter()
        .filter(|r| {
            r.fastest_lap
                .as_ref()
                .is_some_and(|f| f.time.is_some() && f.rank.is_some())
        })
        .collect();
    fastest.sort_by_key(|r| {
        r.fastest_lap
            .as_ref()
            .and_then(|f| f.rank.as_ref()?.parse::<u32>().ok())
            .unwrap_or(99)
    });

    let show_laps_cb = {
        let show_laps = show_laps.clone();
        Callback::from(move |_| show_laps.set(true))
    };

    html! {
        <Layout title={race.race_name.clone()} tab={Tab::Calendar}>
            <section class="card hero">
                <p class="eyebrow">{ tr!("{} · Manche {} / {total}", "{} · Round {} / {total}", race.season, race.round) }</p>
                <h2 class="hero-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</h2>
                <p class="muted">
                    <Link<Route> to={Route::circuit(&race.circuit.circuit_id)} classes="link-inline">{ &race.circuit.circuit_name }</Link<Route>>
                </p>
                <p class="muted">{ format!("{}, {}", race.circuit.location.locality, race.circuit.location.country) }</p>
                if !over && race.has_time() { <Countdown target_ms={race.start_ms()} /> }
                if let Some(url) = &race.url {
                    <a class="link" href={url.clone()} target="_blank" rel="noopener">{ t("Wikipédia ↗", "Wikipedia ↗") }</a>
                }
            </section>

            if over {
                if let Ok(year) = race.season.parse::<u32>() {
                    <super::RaceDataLinks {year} date={race.date.clone()} />
                }
            }

            if over {
                if let Ok(year) = race.season.parse::<u32>() {
                    <super::RaceReplay {year} date={race.date.clone()} />
                }
            }

            <super::CircuitTrack circuit_id={race.circuit.circuit_id.clone()} name={race.circuit.circuit_name.clone()} />

            <section class="card">
                <h2>{ t("Programme", "Schedule") }</h2>
                <SessionsList race={race.clone()} />
            </section>

            if !over { <super::WeekendWeather race={race.clone()} /> }

            if results.is_loading() { { loading() } }
            if !results_list.is_empty() {
                <section class="card">
                    <h2>{ t("Course", "Race") }</h2>
                    <ol class="rows">{ for results_list.iter().map(result_row) }</ol>
                    <ExportCsv filename={format!("f1x-{}-{}.csv", race.season, race.round)} rows={
                        std::iter::once(vec!["Pos".into(), t("Pilote", "Driver").into(), t("Écurie", "Team").into(), t("Grille", "Grid").into(), t("Temps / statut", "Time / status").into(), "Points".into()])
                            .chain(results_list.iter().map(|r| vec![r.position_text.clone(), r.driver.full_name(), r.constructor.name.clone(), r.grid.clone().unwrap_or_default(), r.outcome(), r.points.clone()]))
                            .collect::<Vec<Vec<String>>>()
                    } />
                </section>
            }
            if !sprint_list.is_empty() {
                <section class="card"><h2>{ "Sprint" }</h2><ol class="rows">{ for sprint_list.iter().map(result_row) }</ol></section>
            }
            if !quali_list.is_empty() {
                <section class="card"><h2>{ t("Qualifications", "Qualifying") }</h2><ol class="rows">{ for quali_list.iter().map(qualifying_row) }</ol></section>
            }

            if !fastest.is_empty() {
                <section class="card">
                    <h2>{ t("Meilleurs tours", "Fastest laps") }</h2>
                    <ol class="rows">
                        { for fastest.iter().take(10).map(|r| {
                            let f = r.fastest_lap.as_ref().unwrap();
                            let mut sub = vec![r.constructor.name.clone()];
                            if let Some(l) = &f.lap { sub.push(tr!("tour {l}", "lap {l}")); }
                            if let Some(s) = &f.average_speed { sub.push(format!("{} {}", s.speed, s.units.replace("kph", "km/h"))); }
                            html! {
                                <li class="row" style={team_style(&r.constructor.constructor_id)}>
                                    <span class="pos">{ f.rank.clone().unwrap_or_default() }</span>
                                    <Link<Route> to={Route::driver(&r.driver.driver_id)} classes="row-main">
                                        <span class="row-title">{ &r.driver.given_name }{ " " }<strong>{ &r.driver.family_name }</strong></span>
                                        <span class="row-sub">{ sub.join(" · ") }</span>
                                    </Link<Route>>
                                    <span class="pts pts-time">{ f.time.as_ref().map(|t| t.time.clone()).unwrap_or_default() }</span>
                                </li>
                            }
                        }) }
                    </ol>
                </section>
            }

            if !stops.is_empty() {
                { pit_stops_card(&stops, &results_list) }
            }

            if over && year >= 2023 {
                <super::PitDetail query={format!("year={year}&date={}", race.date)} />
            }

            if over && year >= 1996 && !results_list.is_empty() {
                <section class="card">
                    <h2>{ t("Tour par tour", "Lap by lap") }</h2>
                    { match &laps {
                        Fetch::Idle => html! {
                            <>
                                <p class="muted">{ t("Position de chaque pilote à chaque tour, et tours passés en tête.", "Each driver's position on every lap, and laps led.") }</p>
                                <button class="btn btn-ghost" onclick={show_laps_cb}>{ t("Charger l'analyse", "Load the analysis") }</button>
                            </>
                        },
                        Fetch::Done(data) => html! {
                            <LapAnalysis laps={Rc::clone(data)} results={Rc::new(results_list.clone())} stops={Rc::new(stops.clone())} />
                        },
                        other => fetch_view(other, |_| html! {}),
                    } }
                </section>
            }

            if !statuses.is_empty() {
                <section class="card">
                    <h2>{ t("Bilan de la course", "Race summary") }</h2>
                    <ul class="sessions">
                        { for statuses.iter().map(|s| {
                            let label = translate_status(&s.status);
                            html! {
                                <li class="session">
                                    <span class="session-name">{ if label.is_empty() { t("Arrivés", "Finished").to_string() } else { label } }</span>
                                    <span class="session-time">{ &s.count }</span>
                                </li>
                            }
                        }) }
                    </ul>
                </section>
            }

            if over && results_list.is_empty() && !results.is_loading() {
                { empty_card(t("Les résultats ne sont pas encore disponibles.", "Results are not available yet.")) }
            }

            if over {
                if let Ok(year) = race.season.parse::<u32>() {
                    <div class="of1-link"><super::MeetingLink {year} date={race.date.clone()} /></div>
                }
            }

            <nav class="pager" aria-label="Grands Prix">
                if round > 1 {
                    <Link<Route> to={Route::race(&season, round - 1)} classes="btn btn-ghost">{ t("← Précédent", "← Previous") }</Link<Route>>
                } else { <span></span> }
                if round < total {
                    <Link<Route> to={Route::race(&season, round + 1)} classes="btn btn-ghost">{ t("Suivant →", "Next →") }</Link<Route>>
                }
            </nav>
        </Layout>
    }
}

fn format_duration(d: &str) -> String {
    if d.contains(':') {
        d.to_string()
    } else {
        format!("{} s", d.replace('.', ","))
    }
}

fn pit_stops_card(stops: &[PitStop], results: &[RaceResult]) -> Html {
    let mut by_driver: HashMap<&str, Vec<&PitStop>> = HashMap::new();
    for s in stops {
        by_driver.entry(s.driver_id.as_str()).or_default().push(s);
    }
    let fastest = stops
        .iter()
        .filter_map(|s| Some((s, s.duration.as_deref()?.parse::<f64>().ok()?)))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let name_of = |id: &str| {
        results
            .iter()
            .find(|r| r.driver.driver_id == id)
            .map(|r| r.driver.full_name())
            .unwrap_or_else(|| id.to_string())
    };
    html! {
        <section class="card">
            <h2>{ t("Arrêts aux stands", "Pit stops") }</h2>
            if let Some((s, _)) = fastest {
                <p class="muted">{ tr!("Le plus rapide : {} — {} (tour {})", "Fastest: {} — {} (lap {})", name_of(&s.driver_id), format_duration(s.duration.as_deref().unwrap_or("")), s.lap) }</p>
            }
            <ol class="rows">
                { for results.iter().filter_map(|r| {
                    let driver_stops = by_driver.get(r.driver.driver_id.as_str())?;
                    let detail = driver_stops
                        .iter()
                        .map(|s| format!("T{} · {}", s.lap, s.duration.as_deref().map(format_duration).unwrap_or_default()))
                        .collect::<Vec<_>>()
                        .join(" — ");
                    let n = driver_stops.len();
                    Some(html! {
                        <li class="row" style={team_style(&r.constructor.constructor_id)}>
                            <span class="pos">{ &r.position_text }</span>
                            <Link<Route> to={Route::driver(&r.driver.driver_id)} classes="row-main">
                                <span class="row-title">{ &r.driver.given_name }{ " " }<strong>{ &r.driver.family_name }</strong></span>
                                <span class="row-sub">{ detail }</span>
                            </Link<Route>>
                            <span class="pts">{ n }<small>{ if n > 1 { t(" arrêts", " stops") } else { t(" arrêt", " stop") } }</small></span>
                        </li>
                    })
                }) }
            </ol>
        </section>
    }
}

#[derive(Properties, PartialEq)]
pub struct LapAnalysisProps {
    pub laps: Rc<MrData>,
    pub results: Rc<Vec<RaceResult>>,
    pub stops: Rc<Vec<PitStop>>,
}

const W: f64 = 320.0;
const H: f64 = 200.0;
const ML: f64 = 30.0;
const MR: f64 = 10.0;
const MT: f64 = 10.0;
const MB: f64 = 24.0;

/// Tours en tête (barres) + position tour par tour d'un pilote choisi (courbe unique).
#[function_component]
pub fn LapAnalysis(props: &LapAnalysisProps) -> Html {
    let default_driver = props
        .results
        .first()
        .map(|r| r.driver.driver_id.clone())
        .unwrap_or_default();
    let selected = use_state(|| default_driver);
    let hover = use_state(|| None::<u32>);

    let laps = props.laps.laps();
    let n_laps = laps.keys().max().copied().unwrap_or(0);
    if n_laps == 0 {
        return empty_card(t(
            "Pas de données tour par tour pour cette course.",
            "No lap-by-lap data for this race.",
        ));
    }
    let name_of = |id: &str| {
        props
            .results
            .iter()
            .find(|r| r.driver.driver_id == id)
            .map(|r| r.driver.family_name.clone())
            .unwrap_or_else(|| id.to_string())
    };

    // Tours en tête.
    let mut led: HashMap<&str, u32> = HashMap::new();
    for timings in laps.values() {
        if let Some(t) = timings.iter().find(|t| t.position == "1") {
            *led.entry(t.driver_id.as_str()).or_default() += 1;
        }
    }
    let mut led: Vec<(&str, u32)> = led.into_iter().collect();
    led.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

    // Série du pilote choisi.
    let driver_id = (*selected).clone();
    let series: Vec<(u32, u32, String)> = laps
        .iter()
        .filter_map(|(lap, timings)| {
            let t = timings.iter().find(|t| t.driver_id == driver_id)?;
            Some((*lap, t.position.parse().ok()?, t.time.clone()))
        })
        .collect();
    let max_pos = laps
        .values()
        .flat_map(|v| v.iter().filter_map(|t| t.position.parse::<u32>().ok()))
        .max()
        .unwrap_or(20)
        .max(2);
    let team = props
        .results
        .iter()
        .find(|r| r.driver.driver_id == driver_id)
        .map(|r| r.constructor.constructor_id.clone())
        .unwrap_or_default();
    let color = team_color(&team);
    let x =
        |lap: u32| ML + (lap.saturating_sub(1)) as f64 / (n_laps.max(2) - 1) as f64 * (W - ML - MR);
    let y = |pos: u32| MT + (pos - 1) as f64 / (max_pos - 1) as f64 * (H - MT - MB);
    let path = series
        .iter()
        .enumerate()
        .map(|(i, (lap, pos, _))| {
            format!(
                "{}{:.1},{:.1}",
                if i == 0 { "M" } else { "L" },
                x(*lap),
                y(*pos)
            )
        })
        .collect::<String>();
    let pit_laps: Vec<u32> = props
        .stops
        .iter()
        .filter(|s| s.driver_id == driver_id)
        .filter_map(|s| s.lap.parse().ok())
        .collect();
    let grid_positions: Vec<u32> = [1, 5, 10, 15, 20, 25]
        .into_iter()
        .filter(|p| *p <= max_pos)
        .collect();
    let hovered = hover.and_then(|l| series.iter().find(|(lap, _, _)| *lap == l));

    let readout = match hovered.or(series.last()) {
        Some((lap, pos, time)) => tr!(
            "Tour {lap} · P{pos} · {time}",
            "Lap {lap} · P{pos} · {time}"
        ),
        None => t("Pas de données pour ce pilote.", "No data for this driver.").into(),
    };

    let onchange = {
        let selected = selected.clone();
        let hover = hover.clone();
        Callback::from(move |e: Event| {
            selected.set(e.target_unchecked_into::<HtmlSelectElement>().value());
            hover.set(None);
        })
    };
    let onpointermove = {
        let hover = hover.clone();
        Callback::from(move |e: PointerEvent| {
            let Some(el) = e
                .current_target()
                .and_then(|t| t.dyn_into::<Element>().ok())
            else {
                return;
            };
            let rect = el.get_bounding_client_rect();
            if rect.width() <= 0.0 {
                return;
            }
            let vx = (e.client_x() as f64 - rect.left()) / rect.width() * W;
            let ratio = ((vx - ML) / (W - ML - MR)).clamp(0.0, 1.0);
            hover.set(Some(
                1 + (ratio * (n_laps.max(2) - 1) as f64).round() as u32,
            ));
        })
    };
    let onpointerleave = {
        let hover = hover.clone();
        Callback::from(move |_: PointerEvent| hover.set(None))
    };

    let max_led = led.first().map(|l| l.1).unwrap_or(1).max(1);

    html! {
        <>
            <h3 class="subhead">{ tr!("Tours en tête ({n_laps} tours)", "Laps led ({n_laps} laps)") }</h3>
            <ol class="rows">
                { for led.iter().map(|(id, n)| {
                    let pct = *n as f64 / max_led as f64 * 100.0;
                    let team = props.results.iter().find(|r| r.driver.driver_id == *id).map(|r| r.constructor.constructor_id.clone()).unwrap_or_default();
                    html! {
                        <li class="row" style={team_style(&team)}>
                            <span class="row-main">
                                <span class="row-title">{ name_of(id) }</span>
                                <span class="bar" aria-hidden="true"><span class="bar-fill" style={format!("width:{pct:.1}%")}></span></span>
                            </span>
                            <span class="pts">{ n }<small>{ if *n > 1 { t(" tours", " laps") } else { t(" tour", " lap") } }</small></span>
                        </li>
                    }
                }) }
            </ol>

            <h3 class="subhead">{ t("Position tour par tour", "Position lap by lap") }</h3>
            <label class="select">
                <span class="select-label">{ t("Pilote", "Driver") }</span>
                <select {onchange} aria-label={t("Choisir un pilote", "Choose a driver")}>
                    { for props.results.iter().map(|r| html! {
                        <option value={r.driver.driver_id.clone()} selected={r.driver.driver_id == driver_id}>
                            { format!("P{} · {}", r.position_text, r.driver.full_name()) }
                        </option>
                    }) }
                </select>
            </label>
            <p class="chart-readout" aria-live="polite">{ readout }</p>
            <svg class="chart" viewBox={format!("0 0 {W} {H}")} role="img"
                 aria-label={tr!("Position de {} à chaque tour", "{}'s position on every lap", name_of(&driver_id))}
                 {onpointermove} {onpointerleave}>
                { for grid_positions.iter().map(|p| html! {
                    <g>
                        <line class="chart-grid" x1={ML.to_string()} x2={(W - MR).to_string()} y1={y(*p).to_string()} y2={y(*p).to_string()} />
                        <text class="chart-axis" x={(ML - 6.0).to_string()} y={(y(*p) + 3.5).to_string()} text-anchor="end">{ format!("P{p}") }</text>
                    </g>
                }) }
                { for [1, n_laps.div_ceil(2), n_laps].into_iter().map(|l| html! {
                    <text class="chart-axis" x={x(l).to_string()} y={(H - 6.0).to_string()} text-anchor="middle">{ format!("T{l}") }</text>
                }) }
                if let Some((lap, _, _)) = hovered {
                    <line class="chart-crosshair" x1={x(*lap).to_string()} x2={x(*lap).to_string()} y1={MT.to_string()} y2={(H - MB).to_string()} />
                }
                <path d={path} fill="none" stroke={color} stroke-width="2" stroke-linejoin="round" stroke-linecap="round" />
                { for pit_laps.iter().filter_map(|l| series.iter().find(|(lap, _, _)| lap == l)).map(|(lap, pos, _)| html! {
                    <circle class="chart-pit" cx={x(*lap).to_string()} cy={y(*pos).to_string()} r="4" stroke={color} />
                }) }
                if let Some((lap, pos, _)) = hovered {
                    <circle class="chart-dot" cx={x(*lap).to_string()} cy={y(*pos).to_string()} r="4.5" fill={color} />
                }
            </svg>
            if !pit_laps.is_empty() {
                <p class="muted chart-legend">
                    <span class="legend-line" style={format!("background:{color}")}></span>{ name_of(&driver_id) }
                    <span class="legend-pit" style={format!("border-color:{color}")}></span>{ t("arrêt aux stands", "pit stop") }
                </p>
            }
            <details class="details">
                <summary>{ t("Voir les données", "Show the data") }</summary>
                <ul class="sessions">
                    { for series.iter().map(|(lap, pos, time)| html! {
                        <li class="session">
                            <span class="session-name">{ tr!("Tour {lap}", "Lap {lap}") }</span>
                            <span class="session-time">{ format!("P{pos} · {time}") }</span>
                        </li>
                    }) }
                </ul>
            </details>
        </>
    }
}
