//! Page d'un Grand Prix : en-tête, données OpenF1, circuit, programme, qualifications, météo,
//! résultats (course, sprint), meilleurs tours, arrêts aux stands, analyse tour par tour
//! (chargée à la demande) et bilan de la course.
//!
//! Les requêtes arrivent chacune à son heure : chaque carte ne suit que les données qu'elle
//! affiche, et les cartes qui ont leur propre état (météo, analyse tour par tour) ne sont
//! jamais reconstruites par l'arrivée d'autres données.

use std::collections::HashMap;
use std::rc::Rc;

use active::prelude::*;
use wasm_bindgen::JsCast;

use super::season_label;
use crate::api::{Fetch, all, f1, use_f1_dyn};
use crate::components::*;
use crate::i18n::t;
use crate::models::{
    MrData, PitStop, QualifyingResult, Race, RaceResult, Status, translate_status,
};
use crate::tr;
use crate::util::{flag_country, now_ms, team_color, team_style};
use crate::{Route, link};

const DAY_MS: f64 = 86_400_000.0;

/// Le week-end a commencé : rien à chercher plus de 3 jours avant la course.
fn started(race: &Race, now: f64) -> bool {
    now > race.start_ms() - 3.0 * DAY_MS
}

fn year_of(race: &Race) -> u32 {
    race.season.parse().unwrap_or(0)
}

/// Les états de la page dont dépendent ses cartes (tous `Copy`).
#[derive(Clone, Copy)]
struct RaceData {
    results: State<Fetch>,
    results_list: State<Rc<Vec<RaceResult>>>,
    sprint_list: State<Rc<Vec<RaceResult>>>,
    quali_list: State<Rc<Vec<QualifyingResult>>>,
    sprint_quali: State<Rc<Vec<QualifyingResult>>>,
    stops: State<Rc<Vec<PitStop>>>,
    statuses: State<Rc<Vec<Status>>>,
    laps: State<Fetch>,
    show_laps: State<bool>,
}

/// Une liste de la course chargée (`pick` choisit laquelle), vide tant qu'elle n'est pas là.
/// Elle ne change qu'une fois, à l'arrivée des données.
fn race_list<T: Clone + PartialEq + 'static>(
    fetch: State<Fetch>,
    pick: fn(&Race) -> &Option<Vec<T>>,
) -> State<Rc<Vec<T>>> {
    memo(move || {
        Rc::new(fetch.with(|f| {
            f.done()
                .and_then(|d| d.race())
                .and_then(|r| pick(r).clone())
                .unwrap_or_default()
        }))
    })
}

/// Page d'un Grand Prix (`params` : saison, manche). Elle reste affichée quand on passe au
/// Grand Prix précédent ou suivant : le calendrier n'est pas rechargé, seules les données de
/// la manche le sont.
pub fn race_page(params: State<(String, u32)>) -> Node {
    // Mémorisées : passer à la manche suivante ne relance pas ce qui ne dépend que de la saison.
    let season_memo = memo(move || params.with(|(season, _)| season.clone()));
    let round_memo = memo(move || params.with(|(_, round)| *round));
    let season = move || season_memo.get();
    let round = move || round_memo.get();
    let base = move || format!("{}/{}", season(), round());
    let now = now_ms();
    let show_laps = use_state(false);

    let schedule = use_f1_dyn(move || f1(format!("{}.json", season()), 100));
    // La manche de la page, une fois le calendrier chargé : les requêtes de la course
    // partent de là.
    let race = memo(move || {
        let round = round();
        schedule.with(|f| {
            f.done()
                .and_then(|d| d.races().iter().find(|r| r.round_num() == round).cloned())
        })
    });
    // La manche connue est celle de l'adresse (pas encore la précédente).
    let current = move |r: &Race| r.round_num() == round();
    // Un fichier de la manche, demandé seulement quand `wanted` le permet pour la course.
    let file = |name: &'static str, wanted: fn(&Race, f64) -> bool| {
        use_f1_dyn(move || {
            let ok = race.with(|r| r.as_ref().is_some_and(|r| current(r) && wanted(r, now)));
            if ok {
                f1(format!("{}/{name}.json", base()), 100)
            } else {
                None
            }
        })
    };
    let results = file("results", |r, now| r.is_over(now));
    let sprint = file("sprint", |r, now| started(r, now) && r.is_sprint_weekend());
    let qualifying = file("qualifying", |r, now| started(r, now) && year_of(r) >= 1994);
    // Arrêts aux stands disponibles depuis 2011, tours depuis 1996.
    let pit_stops = file("pitstops", |r, now| r.is_over(now) && year_of(r) >= 2011);
    let status = file("status", |r, now| r.is_over(now));
    // Tours : chargés à la demande (bouton « Charger l'analyse »).
    let laps = use_f1_dyn(move || {
        let ok = show_laps.get()
            && race.with(|r| {
                r.as_ref()
                    .is_some_and(|r| current(r) && r.is_over(now) && year_of(r) >= 1996)
            });
        if ok {
            all(format!("{}/laps.json", base()))
        } else {
            None
        }
    });

    let data = RaceData {
        results,
        results_list: race_list(results, |r| &r.results),
        sprint_list: race_list(sprint, |r| &r.sprint_results),
        quali_list: race_list(qualifying, |r| &r.qualifying_results),
        sprint_quali: race_list(qualifying, |r| &r.sprint_qualifying_results),
        stops: memo(move || {
            Rc::new(pit_stops.with(|f| f.done().map(MrData::pit_stops).unwrap_or_default()))
        }),
        statuses: memo(move || {
            Rc::new(status.with(|f| f.done().map(|d| d.statuses().to_vec()).unwrap_or_default()))
        }),
        laps,
        show_laps,
    };

    // Calendrier chargé sans cette manche : page introuvable.
    let missing = memo(move || schedule.with(|f| f.done().is_some()) && race.with(Option::is_none));
    // Chargement (`Some(None)`) ou erreur du calendrier ; rien une fois chargé.
    let schedule_state = memo(move || {
        schedule.with(|f| match f {
            Fetch::Loading => Some(None),
            Fetch::Failed(e) => Some(Some(e.clone())),
            _ => None,
        })
    });
    let title = move || {
        race.with(|r| r.as_ref().map(|r| r.race_name.clone()))
            .unwrap_or_else(|| format!("Grand Prix · {}", season_label(&season())))
    };

    dynamic(move || {
        if missing.get() {
            return untrack(super::not_found);
        }
        untrack(move || {
            let pending = dynamic(move || match schedule_state.get() {
                Some(None) => loading(),
                Some(Some(e)) => error_card(&e),
                None => Node::Empty,
            });
            // Construit une seule fois, à l'arrivée du calendrier.
            let body = dynamic(move || match race.get() {
                Some(r) => untrack(|| {
                    let total =
                        schedule.with(|f| f.done().map(|d| d.races().len() as u32).unwrap_or(0));
                    race_body(r, &season(), round(), total, now, data)
                }),
                None => Node::Empty,
            });
            layout_dyn(title, Some(Tab::Calendar), fragment([pending, body]))
        })
    })
}

/// Contenu de la page une fois la manche connue.
fn race_body(race: Race, season: &str, round: u32, total: u32, now: f64, d: RaceData) -> Node {
    let over = race.is_over(now);
    let year = year_of(&race);
    // Liens OpenF1 : course terminée et saison lisible.
    let of1_year = race.season.parse::<u32>().ok().filter(|_| over);

    // Qualifications et qualifs sprint : après le programme avant la course, après les résultats ensuite.
    let quali_cards = || {
        fragment([
            quali_card(
                d.quali_list,
                t("Qualifications", "Qualifying"),
                qualifying_row,
            ),
            quali_card(
                d.sprint_quali,
                t("Qualifs sprint", "Sprint qualifying"),
                sprint_qualifying_row,
            ),
        ])
    };

    let hero = section()
        .class("card hero")
        .child(p().class("eyebrow").text(tr!(
            "{} · Manche {} / {total}",
            "{} · Round {} / {total}",
            race.season,
            race.round
        )))
        .child(h2().class("hero-title").text(format!(
            "{} {}",
            flag_country(&race.circuit.location.country),
            race.race_name
        )))
        .child(
            p().class("muted").child(
                link(Route::circuit(&race.circuit.circuit_id), "link-inline")
                    .text(race.circuit.circuit_name.clone()),
            ),
        )
        .child(p().class("muted").text(format!(
            "{}, {}",
            race.circuit.location.locality, race.circuit.location.country
        )))
        .child((!over && race.has_time()).then(|| countdown(race.start_ms())))
        .child(race.url.as_ref().map(|url| {
            a().class("link")
                .href(url.clone())
                .attr("target", "_blank")
                .attr("rel", "noopener")
                .text(t("Wikipédia ↗", "Wikipedia ↗"))
        }));

    // Résultats en cours de chargement.
    let results_loading = memo(move || d.results.with(Fetch::is_loading));
    // Course terminée sans résultats publiés.
    let no_results =
        memo(move || d.results_list.with(|l| l.is_empty()) && !d.results.with(Fetch::is_loading));

    let pager = nav()
        .class("pager")
        .attr("aria-label", "Grands Prix")
        .child(if round > 1 {
            Node::from(
                link(Route::race(season, round - 1), "btn btn-ghost")
                    .text(t("← Précédent", "← Previous")),
            )
        } else {
            span().into()
        })
        .child((round < total).then(|| {
            link(Route::race(season, round + 1), "btn btn-ghost").text(t("Suivant →", "Next →"))
        }));

    fragment([
        Node::from(hero),
        of1_year
            .map(|y| super::race_data_links(y, &race.date))
            .into(),
        of1_year.map(|y| super::race_replay(y, &race.date)).into(),
        super::circuit_track(&race.circuit.circuit_id, &race.circuit.circuit_name),
        section()
            .class("card")
            .child(h2().text(t("Programme", "Schedule")))
            .child(sessions_list(&race))
            .into(),
        // Avant la course, l'ordre des qualifications (la grille) suit directement le programme.
        (!over).then(quali_cards).into(),
        (!over)
            .then(|| super::weekend_weather(race.clone(), true))
            .into(),
        dynamic(move || {
            if results_loading.get() {
                loading()
            } else {
                Node::Empty
            }
        }),
        results_card(&race, d.results_list),
        sprint_card(d.sprint_list),
        over.then(quali_cards).into(),
        fastest_laps_card(d.results_list),
        pit_stops_card(d.stops, d.results_list),
        (over && year >= 2023)
            .then(|| super::pit_detail(&format!("year={year}&date={}", race.date)))
            .into(),
        (over && year >= 1996).then(|| lap_section(d)).into(),
        race_summary_card(d.statuses),
        over.then(|| {
            dynamic(move || {
                if no_results.get() {
                    empty_card(t(
                        "Les résultats ne sont pas encore disponibles.",
                        "Results are not available yet.",
                    ))
                } else {
                    Node::Empty
                }
            })
        })
        .into(),
        of1_year
            .map(|y| {
                div()
                    .class("of1-link")
                    .child(super::meeting_link(y, &race.date))
            })
            .into(),
        pager.into(),
    ])
}

/// Carte de qualifications, affichée dès que sa liste arrive.
fn quali_card(
    list: State<Rc<Vec<QualifyingResult>>>,
    title: &'static str,
    row: fn(&QualifyingResult) -> Node,
) -> Node {
    dynamic(move || {
        let list = list.get();
        if list.is_empty() {
            return Node::Empty;
        }
        section()
            .class("card")
            .child(h2().text(title))
            .child(ol().class("rows").children(list.iter().map(row)))
            .into()
    })
}

fn results_card(race: &Race, list: State<Rc<Vec<RaceResult>>>) -> Node {
    let filename = format!("f1x-{}-{}.csv", race.season, race.round);
    dynamic(move || {
        let list = list.get();
        if list.is_empty() {
            return Node::Empty;
        }
        let rows: Vec<Vec<String>> = std::iter::once(vec![
            "Pos".into(),
            t("Pilote", "Driver").into(),
            t("Écurie", "Team").into(),
            t("Grille", "Grid").into(),
            t("Temps / statut", "Time / status").into(),
            "Points".into(),
        ])
        .chain(list.iter().map(|r| {
            vec![
                r.position_text.clone(),
                r.driver.full_name(),
                r.constructor.name.clone(),
                r.grid.clone().unwrap_or_default(),
                r.outcome(),
                r.points.clone(),
            ]
        }))
        .collect();
        section()
            .class("card")
            .child(h2().text(t("Course", "Race")))
            .child(ol().class("rows").children(list.iter().map(result_row)))
            .child(export_csv(filename.clone(), rows))
            .into()
    })
}

fn sprint_card(list: State<Rc<Vec<RaceResult>>>) -> Node {
    dynamic(move || {
        let list = list.get();
        if list.is_empty() {
            return Node::Empty;
        }
        section()
            .class("card")
            .child(h2().text("Sprint"))
            .child(ol().class("rows").children(list.iter().map(result_row)))
            .into()
    })
}

fn fastest_laps_card(list: State<Rc<Vec<RaceResult>>>) -> Node {
    dynamic(move || {
        let list = list.get();
        let mut fastest: Vec<&RaceResult> = list
            .iter()
            .filter(|r| {
                r.fastest_lap
                    .as_ref()
                    .is_some_and(|f| f.time.is_some() && f.rank.is_some())
            })
            .collect();
        if fastest.is_empty() {
            return Node::Empty;
        }
        fastest.sort_by_key(|r| {
            r.fastest_lap
                .as_ref()
                .and_then(|f| f.rank.as_ref()?.parse::<u32>().ok())
                .unwrap_or(99)
        });
        section()
            .class("card")
            .child(h2().text(t("Meilleurs tours", "Fastest laps")))
            .child(
                ol().class("rows")
                    .children(fastest.iter().take(10).filter_map(|r| {
                        let f = r.fastest_lap.as_ref()?;
                        let mut sub = vec![r.constructor.name.clone()];
                        if let Some(l) = &f.lap {
                            sub.push(tr!("tour {l}", "lap {l}"));
                        }
                        if let Some(s) = &f.average_speed {
                            sub.push(format!("{} {}", s.speed, s.units.replace("kph", "km/h")));
                        }
                        Some(
                            li().class("row")
                                .style(team_style(&r.constructor.constructor_id))
                                .child(span().class("pos").text(f.rank.clone().unwrap_or_default()))
                                .child(
                                    link(Route::driver(&r.driver.driver_id), "row-main")
                                        .child(
                                            span()
                                                .class("row-title")
                                                .text(format!("{} ", r.driver.given_name))
                                                .child(strong().text(r.driver.family_name.clone())),
                                        )
                                        .child(span().class("row-sub").text(sub.join(" · "))),
                                )
                                .child(
                                    span().class("pts pts-time").text(
                                        f.time
                                            .as_ref()
                                            .map(|tv| tv.time.clone())
                                            .unwrap_or_default(),
                                    ),
                                ),
                        )
                    })),
            )
            .into()
    })
}

fn format_duration(d: &str) -> String {
    if d.contains(':') {
        d.to_string()
    } else {
        format!("{} s", d.replace('.', ","))
    }
}

fn full_name_of(results: &[RaceResult], id: &str) -> String {
    results
        .iter()
        .find(|r| r.driver.driver_id == id)
        .map(|r| r.driver.full_name())
        .unwrap_or_else(|| id.to_string())
}

/// Arrêts aux stands : la carte apparaît avec les arrêts ; le plus rapide et les lignes par
/// pilote suivent aussi les résultats (qui peuvent arriver après).
fn pit_stops_card(stops: State<Rc<Vec<PitStop>>>, results: State<Rc<Vec<RaceResult>>>) -> Node {
    let has_stops = memo(move || stops.with(|s| !s.is_empty()));
    dynamic(move || {
        if !has_stops.get() {
            return Node::Empty;
        }
        let fastest = dynamic(move || {
            let (stops, results) = (stops.get(), results.get());
            let fastest = stops
                .iter()
                .filter_map(|s| Some((s, s.duration.as_deref()?.parse::<f64>().ok()?)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            Node::from(fastest.map(|(s, _)| {
                p().class("muted").text(tr!(
                    "Le plus rapide : {} — {} (tour {})",
                    "Fastest: {} — {} (lap {})",
                    full_name_of(&results, &s.driver_id),
                    format_duration(s.duration.as_deref().unwrap_or("")),
                    s.lap
                ))
            }))
        });
        let rows = ol().class("rows").children_dyn(move || {
            let (stops, results) = (stops.get(), results.get());
            let mut by_driver: HashMap<&str, Vec<&PitStop>> = HashMap::new();
            for s in stops.iter() {
                by_driver.entry(s.driver_id.as_str()).or_default().push(s);
            }
            results
                .iter()
                .filter_map(|r| {
                    let driver_stops = by_driver.get(r.driver.driver_id.as_str())?;
                    let detail = driver_stops
                        .iter()
                        .map(|s| {
                            format!(
                                "T{} · {}",
                                s.lap,
                                s.duration
                                    .as_deref()
                                    .map(format_duration)
                                    .unwrap_or_default()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" — ");
                    let n = driver_stops.len();
                    Some(
                        li().class("row")
                            .style(team_style(&r.constructor.constructor_id))
                            .child(span().class("pos").text(r.position_text.clone()))
                            .child(
                                link(Route::driver(&r.driver.driver_id), "row-main")
                                    .child(
                                        span()
                                            .class("row-title")
                                            .text(format!("{} ", r.driver.given_name))
                                            .child(strong().text(r.driver.family_name.clone())),
                                    )
                                    .child(span().class("row-sub").text(detail)),
                            )
                            .child(span().class("pts").text(n.to_string()).child(small().text(
                                if n > 1 {
                                    t(" arrêts", " stops")
                                } else {
                                    t(" arrêt", " stop")
                                },
                            )))
                            .into(),
                    )
                })
                .collect()
        });
        section()
            .class("card")
            .child(h2().text(t("Arrêts aux stands", "Pit stops")))
            .child(fastest)
            .child(rows)
            .into()
    })
}

/// « Tour par tour » : la carte apparaît avec les résultats ; son contenu ne suit que le
/// chargement des tours, demandé par le bouton.
fn lap_section(d: RaceData) -> Node {
    let has_results = memo(move || d.results_list.with(|l| !l.is_empty()));
    dynamic(move || {
        if !has_results.get() {
            return Node::Empty;
        }
        let content = dynamic(move || match d.laps.get() {
            Fetch::Idle => fragment([
                Node::from(p().class("muted").text(t(
                    "Position de chaque pilote à chaque tour, et tours passés en tête.",
                    "Each driver's position on every lap, and laps led.",
                ))),
                button()
                    .class("btn btn-ghost")
                    .on_click(move |_| d.show_laps.set(true))
                    .text(t("Charger l'analyse", "Load the analysis"))
                    .into(),
            ]),
            Fetch::Done(data) => untrack(|| lap_analysis(data, d.results_list.get(), d.stops)),
            Fetch::Loading => loading(),
            Fetch::Failed(e) => error_card(&e),
        });
        section()
            .class("card")
            .child(h2().text(t("Tour par tour", "Lap by lap")))
            .child(content)
            .into()
    })
}

fn race_summary_card(statuses: State<Rc<Vec<Status>>>) -> Node {
    dynamic(move || {
        let list = statuses.get();
        if list.is_empty() {
            return Node::Empty;
        }
        section()
            .class("card")
            .child(h2().text(t("Bilan de la course", "Race summary")))
            .child(ul().class("sessions").children(list.iter().map(|s| {
                let label = translate_status(&s.status);
                li().class("session")
                    .child(span().class("session-name").text(if label.is_empty() {
                        t("Arrivés", "Finished").to_string()
                    } else {
                        label
                    }))
                    .child(span().class("session-time").text(s.count.clone()))
            })))
            .into()
    })
}

const W: f64 = 320.0;
const H: f64 = 200.0;
const ML: f64 = 30.0;
const MR: f64 = 10.0;
const MT: f64 = 10.0;
const MB: f64 = 24.0;

/// Tours d'un pilote : (tour, position, temps).
type LapSeries = Rc<Vec<(u32, u32, String)>>;

fn family_name(results: &[RaceResult], id: &str) -> String {
    results
        .iter()
        .find(|r| r.driver.driver_id == id)
        .map(|r| r.driver.family_name.clone())
        .unwrap_or_else(|| id.to_string())
}

fn team_of(results: &[RaceResult], id: &str) -> String {
    results
        .iter()
        .find(|r| r.driver.driver_id == id)
        .map(|r| r.constructor.constructor_id.clone())
        .unwrap_or_default()
}

/// Tours en tête (barres) + position tour par tour d'un pilote choisi (courbe unique).
///
/// Construite une fois, à l'arrivée des tours : changer de pilote ou survoler le graphique
/// met à jour la courbe, les repères et les valeurs en place (le tracé ne se redessine pas).
fn lap_analysis(
    laps: Rc<MrData>,
    results: Rc<Vec<RaceResult>>,
    stops: State<Rc<Vec<PitStop>>>,
) -> Node {
    let laps = Rc::new(laps.laps());
    let n_laps = laps.keys().max().copied().unwrap_or(0);
    if n_laps == 0 {
        return empty_card(t(
            "Pas de données tour par tour pour cette course.",
            "No lap-by-lap data for this race.",
        ));
    }
    let default_driver = results
        .first()
        .map(|r| r.driver.driver_id.clone())
        .unwrap_or_default();
    let selected = use_state(default_driver.clone());
    let hover = use_state(None::<u32>);

    // Tours en tête.
    let mut led: HashMap<&str, u32> = HashMap::new();
    for timings in laps.values() {
        if let Some(tm) = timings.iter().find(|tm| tm.position == "1") {
            *led.entry(tm.driver_id.as_str()).or_default() += 1;
        }
    }
    let mut led: Vec<(&str, u32)> = led.into_iter().collect();
    led.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let max_led = led.first().map(|l| l.1).unwrap_or(1).max(1);

    let max_pos = laps
        .values()
        .flat_map(|v| v.iter().filter_map(|tm| tm.position.parse::<u32>().ok()))
        .max()
        .unwrap_or(20)
        .max(2);
    let x = move |lap: u32| {
        ML + (lap.saturating_sub(1)) as f64 / (n_laps.max(2) - 1) as f64 * (W - ML - MR)
    };
    let y =
        move |pos: u32| MT + pos.saturating_sub(1) as f64 / (max_pos - 1) as f64 * (H - MT - MB);

    // Série du pilote choisi.
    let series: State<LapSeries> = {
        let laps = Rc::clone(&laps);
        memo(move || {
            selected.with(|id| {
                Rc::new(
                    laps.iter()
                        .filter_map(|(lap, timings)| {
                            let tm = timings.iter().find(|tm| tm.driver_id == *id)?;
                            Some((*lap, tm.position.parse().ok()?, tm.time.clone()))
                        })
                        .collect(),
                )
            })
        })
    };
    let color = {
        let results = Rc::clone(&results);
        memo(move || selected.with(|id| team_color(&team_of(&results, id))))
    };
    // Le pilote choisi s'est-il arrêté (tours d'arrêt connus) ?
    let has_pits = memo(move || {
        selected.with(|id| {
            stops.with(|st| {
                st.iter()
                    .any(|s| s.driver_id == *id && s.lap.parse::<u32>().is_ok())
            })
        })
    });
    // Arrêts du pilote choisi placés sur sa courbe : (tour, position).
    let pit_marks = memo(move || {
        let pit_laps: Vec<u32> = selected.with(|id| {
            stops.with(|st| {
                st.iter()
                    .filter(|s| s.driver_id == *id)
                    .filter_map(|s| s.lap.parse().ok())
                    .collect()
            })
        });
        series.with(|s| {
            pit_laps
                .iter()
                .filter_map(|l| s.iter().find(|(lap, _, _)| lap == l))
                .map(|(lap, pos, _)| (*lap, *pos))
                .collect::<Vec<_>>()
        })
    });
    // Le tour survolé, s'il fait partie de la série.
    let hovered = memo(move || {
        hover
            .get()
            .and_then(|l| series.with(|s| s.iter().find(|(lap, _, _)| *lap == l).cloned()))
    });

    let readout = move || match hovered.get().or_else(|| series.with(|s| s.last().cloned())) {
        Some((lap, pos, time)) => tr!(
            "Tour {lap} · P{pos} · {time}",
            "Lap {lap} · P{pos} · {time}"
        ),
        None => t("Pas de données pour ce pilote.", "No data for this driver.").into(),
    };

    let on_pointer_move = move |e: Event| {
        let Some(pe) = e.raw().dyn_ref::<web_sys::PointerEvent>() else {
            return;
        };
        let Some(el) = e.current_target() else {
            return;
        };
        let rect = el.get_bounding_client_rect();
        if rect.width() <= 0.0 {
            return;
        }
        let vx = (pe.client_x() as f64 - rect.left()) / rect.width() * W;
        let ratio = ((vx - ML) / (W - ML - MR)).clamp(0.0, 1.0);
        hover.set(Some(
            1 + (ratio * (n_laps.max(2) - 1) as f64).round() as u32,
        ));
    };

    let led_rows = ol().class("rows").children(led.iter().map(|(id, n)| {
        let pct = *n as f64 / max_led as f64 * 100.0;
        li().class("row")
            .style(team_style(&team_of(&results, id)))
            .child(
                span()
                    .class("row-main")
                    .child(span().class("row-title").text(family_name(&results, id)))
                    .child(
                        span()
                            .class("bar")
                            .attr("aria-hidden", "true")
                            .child(span().class("bar-fill").style(format!("width:{pct:.1}%"))),
                    ),
            )
            .child(
                span()
                    .class("pts")
                    .text(n.to_string())
                    .child(small().text(if *n > 1 {
                        t(" tours", " laps")
                    } else {
                        t(" tour", " lap")
                    })),
            )
    }));

    // Construit une fois : le navigateur garde ensuite le pilote choisi.
    let driver_select = label()
        .class("select")
        .child(span().class("select-label").text(t("Pilote", "Driver")))
        .child(
            select()
                .on("change", move |e| {
                    selected.set(e.value());
                    hover.set(None);
                })
                .attr("aria-label", t("Choisir un pilote", "Choose a driver"))
                .children(results.iter().map(|r| {
                    let o = option().attr("value", r.driver.driver_id.clone());
                    let o = if r.driver.driver_id == default_driver {
                        o.attr("selected", "")
                    } else {
                        o
                    };
                    o.text(format!("P{} · {}", r.position_text, r.driver.full_name()))
                })),
        );

    let grid_positions: Vec<u32> = [1, 5, 10, 15, 20, 25]
        .into_iter()
        .filter(|pos| *pos <= max_pos)
        .collect();
    let chart = svg()
        .class("chart")
        .attr("viewBox", format!("0 0 {W} {H}"))
        .attr("role", "img")
        .attr_dyn("aria-label", {
            let results = Rc::clone(&results);
            move || {
                selected.with(|id| {
                    tr!(
                        "Position de {} à chaque tour",
                        "{}'s position on every lap",
                        family_name(&results, id)
                    )
                })
            }
        })
        .on("pointermove", on_pointer_move)
        .on("pointerleave", move |_| hover.set(None))
        .children(grid_positions.iter().map(|pos| {
            g().child(
                line()
                    .class("chart-grid")
                    .attr("x1", ML.to_string())
                    .attr("x2", (W - MR).to_string())
                    .attr("y1", y(*pos).to_string())
                    .attr("y2", y(*pos).to_string()),
            )
            .child(
                text_svg()
                    .class("chart-axis")
                    .attr("x", (ML - 6.0).to_string())
                    .attr("y", (y(*pos) + 3.5).to_string())
                    .attr("text-anchor", "end")
                    .text(format!("P{pos}")),
            )
        }))
        .children([1, n_laps.div_ceil(2), n_laps].into_iter().map(|l| {
            text_svg()
                .class("chart-axis")
                .attr("x", x(l).to_string())
                .attr("y", (H - 6.0).to_string())
                .attr("text-anchor", "middle")
                .text(format!("T{l}"))
        }))
        .child(dynamic(move || {
            Node::from(hovered.with(|h| {
                h.as_ref().map(|(lap, _, _)| {
                    line()
                        .class("chart-crosshair")
                        .attr("x1", x(*lap).to_string())
                        .attr("x2", x(*lap).to_string())
                        .attr("y1", MT.to_string())
                        .attr("y2", (H - MB).to_string())
                })
            }))
        }))
        .child(
            path()
                .class("chart-line")
                .attr("pathLength", "1")
                .attr_dyn("d", move || {
                    series.with(|s| {
                        s.iter()
                            .enumerate()
                            .map(|(i, (lap, pos, _))| {
                                format!(
                                    "{}{:.1},{:.1}",
                                    if i == 0 { "M" } else { "L" },
                                    x(*lap),
                                    y(*pos)
                                )
                            })
                            .collect::<String>()
                    })
                })
                .attr("fill", "none")
                .attr_dyn("stroke", move || color.get().to_string())
                .attr("stroke-width", "2")
                .attr("stroke-linejoin", "round")
                .attr("stroke-linecap", "round"),
        )
        // Repères gardés d'un pilote à l'autre (ils glissent au lieu de réapparaître).
        .children_keyed(
            move || (0..pit_marks.with(Vec::len)).collect::<Vec<usize>>(),
            |i| *i,
            move |&i| {
                let mark = move || pit_marks.with(|m| m.get(i).copied());
                circle()
                    .class("chart-pit")
                    .attr_dyn("cx", move || {
                        mark()
                            .map(|(lap, _)| x(lap).to_string())
                            .unwrap_or_default()
                    })
                    .attr_dyn("cy", move || {
                        mark()
                            .map(|(_, pos)| y(pos).to_string())
                            .unwrap_or_default()
                    })
                    .attr("r", "4")
                    .attr_dyn("stroke", move || color.get().to_string())
                    .into()
            },
        )
        .child(dynamic(move || {
            let c = color.get();
            Node::from(hovered.with(|h| {
                h.as_ref().map(|(lap, pos, _)| {
                    circle()
                        .class("chart-dot")
                        .attr("cx", x(*lap).to_string())
                        .attr("cy", y(*pos).to_string())
                        .attr("r", "4.5")
                        .attr("fill", c)
                })
            }))
        }));

    let legend = {
        let results = Rc::clone(&results);
        dynamic(move || {
            if !has_pits.get() {
                return Node::Empty;
            }
            let c = color.get();
            p().class("muted chart-legend")
                .child(span().class("legend-line").style(format!("background:{c}")))
                .text(selected.with(|id| family_name(&results, id)))
                .child(
                    span()
                        .class("legend-pit")
                        .style(format!("border-color:{c}")),
                )
                .text(t("arrêt aux stands", "pit stop"))
                .into()
        })
    };

    // Une ligne par tour, gardée d'un pilote à l'autre : seules les valeurs changent.
    let data_rows = ul().class("sessions").children_keyed(
        move || series.with(|s| s.iter().map(|(lap, _, _)| *lap).collect::<Vec<u32>>()),
        |lap| *lap,
        move |&lap| {
            li().class("session")
                .child(
                    span()
                        .class("session-name")
                        .text(tr!("Tour {lap}", "Lap {lap}")),
                )
                .child(span().class("session-time").text_dyn(move || {
                    series.with(|s| {
                        s.iter()
                            .find(|(l, _, _)| *l == lap)
                            .map(|(_, pos, time)| format!("P{pos} · {time}"))
                            .unwrap_or_default()
                    })
                }))
                .into()
        },
    );

    fragment([
        Node::from(h3().class("subhead").text(tr!(
            "Tours en tête ({n_laps} tours)",
            "Laps led ({n_laps} laps)"
        ))),
        led_rows.into(),
        h3().class("subhead")
            .text(t("Position tour par tour", "Position lap by lap"))
            .into(),
        driver_select.into(),
        p().class("chart-readout")
            .attr("aria-live", "polite")
            .text_dyn(readout)
            .into(),
        chart.into(),
        legend,
        details()
            .class("details")
            .child(summary().text(t("Voir les données", "Show the data")))
            .child(data_rows)
            .into(),
    ])
}
