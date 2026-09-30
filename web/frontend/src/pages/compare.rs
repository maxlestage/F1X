//! Comparateur de pilotes : statistiques de carrière côte à côte et face-à-face.

use std::collections::HashMap;

use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::*;

use super::stats::{Career, Champion, career, driver_titles, entries};
use crate::Route;
use crate::api::{all, use_f1, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::models::Driver;
use crate::tr;
use crate::util::{flag_nationality, fold};

/// Couleurs validées (contraste et daltonisme) sur fond sombre.
const COLOR_A: &str = "#e5483f";
const COLOR_B: &str = "#3b9fd8";
const NONE: &str = "_";

#[derive(Properties, PartialEq)]
pub struct CompareProps {
    #[prop_or_else(|| NONE.into())]
    pub a: AttrValue,
    #[prop_or_else(|| NONE.into())]
    pub b: AttrValue,
}

#[derive(Properties, PartialEq)]
struct PickerProps {
    label: AttrValue,
    color: AttrValue,
    drivers: std::rc::Rc<Vec<Driver>>,
    current: Option<Driver>,
    on_pick: Callback<String>,
}

/// Recherche d'un pilote (suggestions sous le champ, liste verticale).
#[function_component]
fn Picker(props: &PickerProps) -> Html {
    let query = use_state(String::new);
    let q = fold(&query);
    let oninput = {
        let query = query.clone();
        Callback::from(move |e: InputEvent| {
            query.set(e.target_unchecked_into::<HtmlInputElement>().value())
        })
    };
    let matches: Vec<&Driver> = if q.len() < 2 {
        Vec::new()
    } else {
        props
            .drivers
            .iter()
            .filter(|d| {
                fold(&d.full_name()).contains(&q) || d.code.as_deref().is_some_and(|c| fold(c) == q)
            })
            .take(6)
            .collect()
    };
    html! {
        <div class="picker" style={format!("--pick:{}", props.color)}>
            <span class="picker-label">{ props.label.clone() }</span>
            if let Some(d) = &props.current {
                <p class="picker-current">{ flag_nationality(d.nationality.as_deref()) }{ " " }<strong>{ d.full_name() }</strong></p>
            }
            <input class="search" type="search" value={(*query).clone()} {oninput}
                   placeholder={t("Rechercher un pilote…", "Search a driver…")} aria-label={props.label.clone()} autocomplete="off" />
            if !matches.is_empty() {
                <ul class="suggestions">
                    { for matches.iter().map(|d| {
                        let id = d.driver_id.clone();
                        let on_pick = props.on_pick.clone();
                        let query = query.clone();
                        html! {
                            <li><button onclick={move |_| { query.set(String::new()); on_pick.emit(id.clone()); }}>
                                { flag_nationality(d.nationality.as_deref()) }{ " " }{ d.full_name() }
                                <small class="muted">{ d.date_of_birth.as_deref().map(|b| format!(" · {}", &b[..4])).unwrap_or_default() }</small>
                            </button></li>
                        }
                    }) }
                </ul>
            }
        </div>
    }
}

/// Ligne de statistique : deux barres (A rouge, B bleu), le meilleur en gras.
fn stat_row(
    label: &str,
    a: f64,
    b: f64,
    fmt: impl Fn(f64) -> String,
    higher_is_better: bool,
) -> Html {
    let max = a.max(b).max(f64::EPSILON);
    let a_best = if higher_is_better {
        a > b
    } else {
        a < b && a > 0.0
    };
    let b_best = if higher_is_better {
        b > a
    } else {
        b < a && b > 0.0
    };
    let bar = |v: f64, color: &str, best: bool| {
        let pct = if higher_is_better {
            v / max * 100.0
        } else if v > 0.0 {
            a.min(b).max(f64::EPSILON) / v * 100.0
        } else {
            0.0
        };
        html! {
            <span class="cmp-bar">
                <span class="cmp-fill" style={format!("width:{:.1}%;background:{color}", pct.clamp(0.0, 100.0))}></span>
                <span class={classes!("cmp-val", best.then_some("cmp-best"))}>{ fmt(v) }</span>
            </span>
        }
    };
    html! {
        <li class="cmp-row">
            <span class="cmp-label">{ label.to_string() }</span>
            { bar(a, COLOR_A, a_best) }
            { bar(b, COLOR_B, b_best) }
        </li>
    }
}

#[function_component]
pub fn ComparePage(props: &CompareProps) -> Html {
    let navigator = use_navigator();
    let list = use_f1(all("drivers.json"));
    let champions = use_json::<Vec<Champion>>(Some("/api/champions".into()));
    let a = (props.a != NONE).then(|| props.a.to_string());
    let b = (props.b != NONE).then(|| props.b.to_string());
    let races_a = use_f1(
        a.as_ref()
            .and_then(|id| all(format!("drivers/{id}/results.json"))),
    );
    let races_b = use_f1(
        b.as_ref()
            .and_then(|id| all(format!("drivers/{id}/results.json"))),
    );

    let drivers = std::rc::Rc::new(
        list.done()
            .map(|d| d.drivers().to_vec())
            .unwrap_or_default(),
    );
    let find = |id: &Option<String>| {
        id.as_ref()
            .and_then(|id| drivers.iter().find(|d| &d.driver_id == id).cloned())
    };
    let (da, db) = (find(&a), find(&b));
    let pick = |side: u8| {
        let navigator = navigator.clone();
        let (a, b) = (props.a.to_string(), props.b.to_string());
        Callback::from(move |id: String| {
            let (na, nb) = if side == 0 {
                (id, b.clone())
            } else {
                (a.clone(), id)
            };
            if let Some(nav) = &navigator {
                nav.push(&Route::CompareWith { a: na, b: nb });
            }
        })
    };

    let champs = match &champions {
        Some(Ok(c)) => c.to_vec(),
        _ => Vec::new(),
    };
    let body = match (races_a.done(), races_b.done(), &da, &db) {
        (Some(ra), Some(rb), Some(da), Some(db)) => {
            let (ca, cb): (Career, Career) = (career(ra.races()), career(rb.races()));
            let (ta, tb) = (
                driver_titles(&champs, &da.driver_id).len() as f64,
                driver_titles(&champs, &db.driver_id).len() as f64,
            );
            // Face-à-face sur les courses disputées ensemble.
            let map_b: HashMap<
                (String, String),
                (&crate::models::Race, &crate::models::RaceResult),
            > = entries(rb.races())
                .into_iter()
                .map(|(race, r)| ((race.season.clone(), race.round.clone()), (race, r)))
                .collect();
            let (
                mut common,
                mut ahead_a,
                mut ahead_b,
                mut mates,
                mut mates_a,
                mut grid_a,
                mut grid_b,
            ) = (0, 0, 0, 0, 0, 0, 0);
            for (race, r) in entries(ra.races()) {
                let Some((_, rb)) = map_b.get(&(race.season.clone(), race.round.clone())) else {
                    continue;
                };
                common += 1;
                let (pa, pb): (u32, u32) = (
                    r.position.parse().unwrap_or(99),
                    rb.position.parse().unwrap_or(99),
                );
                if pa < pb {
                    ahead_a += 1
                } else {
                    ahead_b += 1
                }
                if r.constructor.constructor_id == rb.constructor.constructor_id {
                    mates += 1;
                    if pa < pb {
                        mates_a += 1
                    }
                    let (ga, gb): (u32, u32) = (
                        r.grid
                            .as_deref()
                            .and_then(|g| g.parse().ok())
                            .filter(|g| *g > 0)
                            .unwrap_or(99),
                        rb.grid
                            .as_deref()
                            .and_then(|g| g.parse().ok())
                            .filter(|g| *g > 0)
                            .unwrap_or(99),
                    );
                    if ga < gb {
                        grid_a += 1
                    } else if gb < ga {
                        grid_b += 1
                    }
                }
            }
            let int = |v: f64| format!("{v:.0}");
            let rows: Vec<Vec<String>> = vec![
                vec![
                    t("Statistique", "Statistic").into(),
                    da.full_name(),
                    db.full_name(),
                ],
                vec![t("Titres", "Titles").into(), int(ta), int(tb)],
                vec![
                    t("Départs", "Starts").into(),
                    ca.starts.to_string(),
                    cb.starts.to_string(),
                ],
                vec![
                    t("Victoires", "Wins").into(),
                    ca.wins.to_string(),
                    cb.wins.to_string(),
                ],
                vec![
                    t("Podiums", "Podiums").into(),
                    ca.podiums.to_string(),
                    cb.podiums.to_string(),
                ],
                vec!["Poles".into(), ca.poles.to_string(), cb.poles.to_string()],
                vec![
                    t("Meilleurs tours", "Fastest laps").into(),
                    ca.fastest.to_string(),
                    cb.fastest.to_string(),
                ],
                vec![
                    "Points".into(),
                    format!("{:.0}", ca.points),
                    format!("{:.0}", cb.points),
                ],
                vec![
                    t("Abandons", "Retirements").into(),
                    ca.retirements.to_string(),
                    cb.retirements.to_string(),
                ],
            ];
            html! {
                <>
                    <section class="card">
                        <div class="cmp-legend">
                            <span><span class="dot-a" style={format!("background:{COLOR_A}")}></span>{ da.full_name() }</span>
                            <span><span class="dot-a" style={format!("background:{COLOR_B}")}></span>{ db.full_name() }</span>
                        </div>
                        <ul class="cmp">
                            { stat_row(t("Titres", "Titles"), ta, tb, int, true) }
                            { stat_row(t("Départs", "Starts"), ca.starts as f64, cb.starts as f64, int, true) }
                            { stat_row(t("Victoires", "Wins"), ca.wins as f64, cb.wins as f64, int, true) }
                            { stat_row(t("% de victoires", "Win rate"), ca.win_rate(), cb.win_rate(), |v| format!("{v:.1}%"), true) }
                            { stat_row(t("Podiums", "Podiums"), ca.podiums as f64, cb.podiums as f64, int, true) }
                            { stat_row("Poles", ca.poles as f64, cb.poles as f64, int, true) }
                            { stat_row(t("Meilleurs tours", "Fastest laps"), ca.fastest as f64, cb.fastest as f64, int, true) }
                            { stat_row("Points", ca.points, cb.points, int, true) }
                            { stat_row(t("Place moyenne à l'arrivée", "Average finish"), ca.avg_finish().unwrap_or(0.0), cb.avg_finish().unwrap_or(0.0), |v| format!("{v:.1}"), false) }
                            { stat_row(t("Abandons", "Retirements"), ca.retirements as f64, cb.retirements as f64, int, false) }
                            { stat_row(t("Saisons", "Seasons"), ca.seasons.len() as f64, cb.seasons.len() as f64, int, true) }
                        </ul>
                        <p class="muted">{ t("Barre la plus longue = meilleur (pour la place moyenne et les abandons, le plus bas l'emporte). Valeur en gras = avantage.", "Longest bar = better (for average finish and retirements, lower wins). Bold value = advantage.") }</p>
                    </section>
                    <section class="card">
                        <h2>{ t("Face-à-face", "Head to head") }</h2>
                        if common == 0 {
                            <p class="muted">{ t("Ils n'ont jamais couru la même course.", "They never raced in the same Grand Prix.") }</p>
                        } else {
                            <ul class="sessions">
                                <li class="session"><span class="session-name">{ tr!("Courses ensemble ({common})", "Races together ({common})") }</span>
                                    <span class="session-time">{ format!("{} {ahead_a} – {ahead_b} {}", da.family_name, db.family_name) }</span></li>
                                if mates > 0 {
                                    <li class="session"><span class="session-name">{ tr!("Coéquipiers, course ({mates})", "Teammates, race ({mates})") }</span>
                                        <span class="session-time">{ format!("{} {mates_a} – {} {}", da.family_name, mates - mates_a, db.family_name) }</span></li>
                                    <li class="session"><span class="session-name">{ t("Coéquipiers, grille", "Teammates, grid") }</span>
                                        <span class="session-time">{ format!("{} {grid_a} – {grid_b} {}", da.family_name, db.family_name) }</span></li>
                                }
                            </ul>
                            <p class="muted">{ t("Qui a terminé devant l'autre quand ils étaient tous les deux au départ.", "Who finished ahead when both started the race.") }</p>
                        }
                    </section>
                    <ExportCsv filename={format!("f1x-{}-vs-{}.csv", da.driver_id, db.driver_id)} rows={rows} />
                </>
            }
        }
        (_, _, Some(_), Some(_)) => loading(),
        _ => {
            html! { <p class="section-intro">{ t("Choisis deux pilotes pour les comparer.", "Pick two drivers to compare them.") }</p> }
        }
    };

    html! {
        <Layout title={t("Comparateur", "Compare")} tab={Tab::Archives}>
            <section class="card">
                if list.is_loading() { { loading() } }
                <Picker label={t("Pilote A", "Driver A")} color={COLOR_A} drivers={drivers.clone()} current={da.clone()} on_pick={pick(0)} />
                <Picker label={t("Pilote B", "Driver B")} color={COLOR_B} drivers={drivers.clone()} current={db.clone()} on_pick={pick(1)} />
                <div class="chips">
                    { for [("hamilton", "max_verstappen"), ("senna", "prost"), ("michael_schumacher", "alonso"), ("leclerc", "norris")].iter().map(|(x, y)| {
                        let to = Route::CompareWith { a: x.to_string(), b: y.to_string() };
                        let label = format!("{} / {}", x.split('_').next_back().unwrap_or(x), y.split('_').next_back().unwrap_or(y));
                        html! { <Link<Route> to={to} classes="chip">{ label }</Link<Route>> }
                    }) }
                </div>
            </section>
            { body }
        </Layout>
    }
}
