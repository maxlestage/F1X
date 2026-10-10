//! Fantasy F1 : 5 pilotes + 1 écurie, budget 100 M€, points = résultats réels.
//! Équipe enregistrée sur l'appareil ; chaque modification s'applique à partir du prochain GP.

use std::collections::HashMap;
use std::rc::Rc;

use active::prelude::*;
use serde::{Deserialize, Serialize};

use crate::api::{Fetch, all, f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::models::{ConstructorStanding, DriverStanding};
use crate::tr;
use crate::util::{load, now_ms, store, team_style};

const KEY: &str = "f1x-fantasy";
const BUDGET: f64 = 100.0;
const DRIVERS: usize = 5;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Version {
    season: String,
    from_round: u32,
    drivers: Vec<String>,
    team: String,
}

/// Classement et prix (en M€) par identifiant.
type Market<T> = (Vec<T>, HashMap<String, f64>);
/// Points marqués par manche, par pilote (ou par écurie).
type RoundPoints = HashMap<u32, HashMap<String, f64>>;

/// Prix d'après les points au championnat : de 5 à 30 M€ (pilotes), 8 à 30 M€ (écuries).
fn price(points: f64, leader: f64, min: f64) -> f64 {
    let ratio = if leader > 0.0 {
        (points / leader).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((min + (30.0 - min) * ratio.powf(0.8)) * 2.0).round() / 2.0
}

/// Points du premier du classement (0 si inconnus).
fn leader(points: Option<&str>) -> f64 {
    points.and_then(|p| p.parse().ok()).unwrap_or(0.0)
}

/// Valeur d'une case de statistiques, recalculée quand les états qu'elle lit changent.
type Value = Rc<dyn Fn() -> String>;

/// Comme `stat_grid`, avec des valeurs qui suivent les états (mises à jour sur place).
fn stat_grid_dyn(items: Vec<(&'static str, Value)>) -> Node {
    dl().class("stats")
        .children(items.into_iter().map(|(label, value)| {
            // Valeurs longues (temps au tour, unités) : police adaptée à la largeur de la case.
            let long = value.clone();
            div().child(dt().text(label)).child(
                dd().class_if("dd-long", move || long().chars().count() > 5)
                    .text_dyn(move || value()),
            )
        }))
        .into()
}

pub fn fantasy_page() -> Node {
    let schedule = use_f1(f1("current.json", 100));
    let standings = use_f1(f1("current/driverStandings.json", 100));
    let teams = use_f1(f1("current/constructorStandings.json", 100));
    let results = use_f1(all("current/results.json"));
    let sprints = use_f1(all("current/sprint.json"));
    let versions = use_state(load::<Vec<Version>>(KEY).unwrap_or_default());
    let draft = use_state(None::<(Vec<String>, String)>);

    // Saison et prochaine manche (celle à partir de laquelle une modification compte).
    let calendar = memo(move || {
        let now = now_ms();
        schedule.with(|f| {
            let races = f.done().map(|d| d.races()).unwrap_or_default();
            let season = races.first().map(|r| r.season.clone()).unwrap_or_default();
            let next_round = races
                .iter()
                .find(|r| !r.is_over(now))
                .map(|r| r.round_num());
            (season, next_round)
        })
    });
    let next_round = move || calendar.with(|c| c.1);
    let latest = move || {
        let season = calendar.with(|c| c.0.clone());
        versions.with(|all| all.iter().rfind(|v| v.season == season).cloned())
    };
    // Sélection affichée : le brouillon, sinon la dernière équipe enregistrée.
    let selection = memo(move || {
        draft
            .get()
            .unwrap_or_else(|| latest().map(|v| (v.drivers, v.team)).unwrap_or_default())
    });

    let drivers: State<Market<DriverStanding>> = memo(move || {
        let ds = standings.with(|f| {
            f.done()
                .and_then(|d| d.standings())
                .and_then(|l| l.driver_standings.clone())
                .unwrap_or_default()
        });
        let top = leader(ds.first().map(|s| s.points.as_str()));
        let prices = ds
            .iter()
            .map(|s| {
                (
                    s.driver.driver_id.clone(),
                    price(s.points.parse().unwrap_or(0.0), top, 5.0),
                )
            })
            .collect();
        (ds, prices)
    });
    let constructors: State<Market<ConstructorStanding>> = memo(move || {
        let cs = teams.with(|f| {
            f.done()
                .and_then(|d| d.standings())
                .and_then(|l| l.constructor_standings.clone())
                .unwrap_or_default()
        });
        let top = leader(cs.first().map(|s| s.points.as_str()));
        let prices = cs
            .iter()
            .map(|s| {
                (
                    s.constructor.constructor_id.clone(),
                    price(s.points.parse().unwrap_or(0.0), top, 8.0),
                )
            })
            .collect();
        (cs, prices)
    });

    let left = move || {
        let (picked, team) = selection.get();
        let spent = drivers.with(|(_, p)| picked.iter().filter_map(|d| p.get(d)).sum::<f64>())
            + constructors.with(|(_, p)| p.get(&team).copied().unwrap_or(0.0));
        BUDGET - spent
    };
    let over_budget = memo(move || left() < 0.0);
    let picked_count = move || selection.with(|(p, _)| p.len());
    let valid =
        move || selection.with(|(p, team)| p.len() == DRIVERS && !team.is_empty()) && left() >= 0.0;
    let changed = move || {
        let latest = latest();
        selection.with(|(p, team)| latest.is_none_or(|v| v.drivers != *p || v.team != *team))
    };

    // Points par manche : pilotes (course + sprint) + écurie (moitié des points de ses pilotes).
    let points = memo(move || {
        let mut round_points: RoundPoints = HashMap::new();
        let mut team_round: RoundPoints = HashMap::new();
        for f in [results, sprints] {
            f.with(|f: &Fetch| {
                for race in f.done().map(|d| d.races()).unwrap_or_default() {
                    let rows = race.results.as_ref().or(race.sprint_results.as_ref());
                    for r in rows.into_iter().flatten() {
                        let p: f64 = r.points.parse().unwrap_or(0.0);
                        *round_points
                            .entry(race.round_num())
                            .or_default()
                            .entry(r.driver.driver_id.clone())
                            .or_default() += p;
                        *team_round
                            .entry(race.round_num())
                            .or_default()
                            .entry(r.constructor.constructor_id.clone())
                            .or_default() += p;
                    }
                }
            });
        }
        (round_points, team_round)
    });
    let history = memo(move || {
        let season = calendar.with(|c| c.0.clone());
        let mine: Vec<Version> =
            versions.with(|all| all.iter().filter(|v| v.season == season).cloned().collect());
        points.with(|(round_points, team_round)| {
            let mut rounds: Vec<u32> = round_points.keys().copied().collect();
            rounds.sort_unstable();
            rounds
                .iter()
                .filter_map(|r| {
                    let v = mine.iter().rev().find(|v| v.from_round <= *r)?;
                    let drivers: f64 = v
                        .drivers
                        .iter()
                        .filter_map(|d| round_points[r].get(d))
                        .sum();
                    let team = team_round
                        .get(r)
                        .and_then(|m| m.get(&v.team))
                        .copied()
                        .unwrap_or(0.0)
                        / 2.0;
                    Some((*r, drivers + team))
                })
                .collect::<Vec<(u32, f64)>>()
        })
    });

    let save = move |_: Event| {
        let Some(from_round) = next_round() else {
            return;
        };
        let season = calendar.with(|c| c.0.clone());
        let (picked, team) = selection.get();
        let mut all = versions.get();
        all.retain(|v| !(v.season == season && v.from_round == from_round));
        all.push(Version {
            season,
            from_round,
            drivers: picked,
            team,
        });
        store(KEY, &all);
        versions.set(all);
        draft.set(None);
    };

    let ready =
        memo(move || standings.with(|f| f.done().is_some()) && teams.with(|f| f.done().is_some()));

    let hero = section()
        .class("card hero")
        .child(p().class("eyebrow").text_dyn(move || {
            let season = calendar.with(|c| c.0.clone());
            tr!("Saison {season}", "Season {season}")
        }))
        .child(h2().class("hero-title").text_dyn(move || {
            // « + 0.0 » : évite l'affichage « -0 »
            let total: f64 = history.with(|h| h.iter().map(|h| h.1).sum::<f64>()) + 0.0;
            tr!("{total:.0} points", "{total:.0} points")
        }))
        .child(stat_grid_dyn(vec![
            (
                t("Budget restant", "Budget left"),
                Rc::new(move || format!("{:.1} M€", left())),
            ),
            (
                t("Pilotes", "Drivers"),
                Rc::new(move || format!("{}/{DRIVERS}", picked_count())),
            ),
            (
                t("Écurie", "Team"),
                Rc::new(move || {
                    if selection.with(|(_, team)| team.is_empty()) {
                        "0/1".into()
                    } else {
                        "1/1".into()
                    }
                }),
            ),
        ]))
        .child(dynamic(move || {
            over_budget
                .get()
                .then(|| {
                    p().class("muted").text(t(
                        "⚠️ Budget dépassé : retire un pilote.",
                        "⚠️ Over budget: drop a driver.",
                    ))
                })
                .into()
        }))
        .child(
            button()
                .class("btn")
                .on_click(save)
                .bool_attr("disabled", move || {
                    !valid() || !changed() || next_round().is_none()
                })
                .text_dyn(move || match next_round() {
                    Some(r) if changed() => tr!(
                        "Valider (compte à partir de la manche {r})",
                        "Confirm (counts from round {r})"
                    ),
                    Some(_) => t("Équipe enregistrée ✓", "Team saved ✓").to_string(),
                    None => t("Saison terminée", "Season over").to_string(),
                }),
        );

    // Listes construites une fois les classements arrivés : un choix ne met à jour que les
    // classes et les boutons, sans reconstruire les lignes.
    let driver_rows = ol().class("rows").children_dyn(move || {
        drivers.with(|(ds, prices)| {
            ds.iter()
                .map(|s| {
                    let id = s.driver.driver_id.clone();
                    let cost = prices.get(&id).copied().unwrap_or(5.0);
                    let on = {
                        let id = id.clone();
                        move || selection.with(|(p, _)| p.contains(&id))
                    };
                    let affordable = {
                        let on = on.clone();
                        move || on() || (picked_count() < DRIVERS && cost <= left())
                    };
                    let toggle = move |_: Event| {
                        let (mut p, team) = selection.get();
                        if let Some(i) = p.iter().position(|x| *x == id) {
                            p.remove(i);
                        } else if p.len() < DRIVERS {
                            p.push(id.clone());
                        }
                        draft.set(Some((p, team)));
                    };
                    let row = li().class("row").class_if("row-picked", on.clone());
                    let row = match s.constructors.last() {
                        Some(c) => row.style(team_style(&c.constructor_id)),
                        None => row,
                    };
                    row.child(
                        span()
                            .class("row-main")
                            .child(
                                span()
                                    .class("row-title")
                                    .text(format!("{} ", s.driver.given_name))
                                    .child(strong().text(s.driver.family_name.clone())),
                            )
                            .child(
                                span()
                                    .class("row-sub")
                                    .text(format!("{cost:.1} M€ · {} pts", s.points)),
                            ),
                    )
                    .child(pick_button(on, toggle).bool_attr("disabled", move || !affordable()))
                    .into()
                })
                .collect()
        })
    });
    let team_rows = ol().class("rows").children_dyn(move || {
        constructors.with(|(cs, prices)| {
            cs.iter()
                .map(|s| {
                    let id = s.constructor.constructor_id.clone();
                    let cost = prices.get(&id).copied().unwrap_or(8.0);
                    let on = {
                        let id = id.clone();
                        move || selection.with(|(_, team)| *team == id)
                    };
                    let style = team_style(&id);
                    let choose = move |_: Event| {
                        let (p, _) = selection.get();
                        draft.set(Some((p, id.clone())));
                    };
                    li().class("row")
                        .class_if("row-picked", on.clone())
                        .style(style)
                        .child(
                            span()
                                .class("row-main")
                                .child(span().class("row-title").text(s.constructor.name.clone()))
                                .child(
                                    span()
                                        .class("row-sub")
                                        .text(format!("{cost:.1} M€ · {} pts", s.points)),
                                ),
                        )
                        .child(pick_button(on, choose))
                        .into()
                })
                .collect()
        })
    });

    let history_card = dynamic(move || {
        history.with(|history| {
            if history.is_empty() {
                return Node::Empty;
            }
            section()
                .class("card")
                .child(h2().text(t("Points par manche", "Points per round")))
                .child(
                    ul().class("sessions")
                        .children(history.iter().rev().map(|(r, p)| {
                            li().class("session")
                                .child(
                                    span()
                                        .class("session-name")
                                        .text(tr!("Manche {r}", "Round {r}")),
                                )
                                .child(span().class("session-time").text(format!("+{p:.0}")))
                        })),
                )
                .into()
        })
    });

    layout(
        t("Fantasy F1", "Fantasy F1"),
        Some(Tab::Archives),
        fragment([
            Node::from(hero),
            dynamic(move || if ready.get() { Node::Empty } else { loading() }),
            section()
                .class("card")
                .child(h2().text(t("Pilotes", "Drivers")))
                .child(driver_rows)
                .into(),
            section()
                .class("card")
                .child(h2().text(t("Écurie", "Team")))
                .child(team_rows)
                .into(),
            history_card,
            section()
                .class("card")
                .child(h2().text(t("Règles", "Rules")))
                .child(p().class("muted").text(t(
                    "5 pilotes + 1 écurie, 100 M€. Prix selon le classement actuel. Points : ceux marqués en vrai par tes pilotes (course + sprint), plus la moitié des points de ton écurie. Chaque modification compte à partir du prochain Grand Prix. Équipe enregistrée sur ce téléphone.",
                    "5 drivers + 1 team, €100M. Prices follow the current standings. Points: what your drivers really score (race + sprint), plus half of your team's points. Every change counts from the next Grand Prix. Team stored on this phone.",
                )))
                .into(),
        ]),
    )
}

/// Bouton « + » / « ✓ » d'une ligne, qui suit la sélection.
fn pick_button(
    on: impl Fn() -> bool + Clone + 'static,
    click: impl Fn(Event) + 'static,
) -> Element {
    let pressed = on.clone();
    let mark = on.clone();
    button()
        .class("pick-btn")
        .class_if("pick-on", on)
        .on_click(click)
        .attr_dyn("aria-pressed", move || pressed().to_string())
        .text_dyn(move || if mark() { "✓" } else { "+" }.to_string())
}
