use active::prelude::*;

use crate::api::{f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::tr;
use crate::util::{flag_country, now_ms, team_style};
use crate::{CURRENT, Route, link};

pub fn home() -> Node {
    let schedule = use_f1(f1("current.json", 100));
    let last = use_f1(f1("current/last/results.json", 100));
    let drivers = use_f1(f1("current/driverStandings.json", 100));
    let teams = use_f1(f1("current/constructorStandings.json", 100));

    // Chaque carte ne suit que les données qu'elle affiche : l'arrivée d'un classement ne
    // reconstruit pas le compte à rebours ni la météo.
    let body = fetch_view(schedule, move |data| {
        let races = data.races();
        let now = now_ms();
        let season = races.first().map(|r| r.season.clone()).unwrap_or_default();
        let next = races.iter().find(|r| !r.is_over(now));
        let next_card: Node = match next {
            Some(race) => section()
                .class("card hero")
                .child(p().class("eyebrow").text(tr!(
                    "Prochain Grand Prix · Manche {}",
                    "Next Grand Prix · Round {}",
                    race.round
                )))
                .child(h2().class("hero-title").text(format!(
                    "{} {}",
                    flag_country(&race.circuit.location.country),
                    race.race_name
                )))
                .child(
                    p().class("muted")
                        .child(
                            link(Route::circuit(&race.circuit.circuit_id), "link-inline")
                                .text(race.circuit.circuit_name.clone()),
                        )
                        .text(format!(" — {}", race.circuit.location.locality)),
                )
                .child(
                    link(Route::circuit(&race.circuit.circuit_id), "outline-link")
                        .child(super::track_outline(&race.circuit.circuit_id)),
                )
                .child(countdown(race.start_ms()))
                .child(sessions_list(race))
                .child(
                    link(Route::race(CURRENT, race.round_num()), "btn")
                        .text(t("Voir le Grand Prix", "View the Grand Prix")),
                )
                .into(),
            None => section()
                .class("card")
                .child(h2().text(tr!("Saison {season} terminée", "Season {season} is over")))
                .child(p().class("muted").text(t(
                    "Rendez-vous la saison prochaine !",
                    "See you next season!",
                )))
                .into(),
        };
        fragment([
            next_card,
            // Ordre des qualifications (qualifs, sinon qualifs sprint) dès la fin de la séance.
            next.filter(|r| r.start_ms() - now < 4.0 * 86_400_000.0)
                .map(|race| quali_order(race.round_num()))
                .into(),
            next.map(|race| super::weekend_weather(race.clone(), false))
                .into(),
            dynamic(move || last_result(last)),
            dynamic(move || favourites(drivers, teams)),
            dynamic(move || top_drivers(drivers)),
            dynamic(move || top_teams(teams)),
            crate::pwa::install_card(),
            link(Route::Predict, "btn btn-ghost")
                .text(t(
                    "🔮 Pronostiquer le prochain GP",
                    "🔮 Predict the next GP",
                ))
                .into(),
            link(Route::Live, "btn")
                .text(t(
                    "● Direct & replays en temps réel",
                    "● Live & real-time replays",
                ))
                .into(),
            link(Route::Archives, "btn btn-ghost")
                .text(t(
                    "Explorer 75 ans d'archives →",
                    "Explore 75 years of history →",
                ))
                .into(),
        ])
    });

    layout("", Some(Tab::Home), body)
}

fn last_result(last: State<crate::api::Fetch>) -> Node {
    let Some(data) = last.with(|f| f.data()) else {
        return Node::Empty;
    };
    let Some(race) = data.race() else {
        return Node::Empty;
    };
    section()
        .class("card")
        .child(
            div()
                .class("card-head")
                .child(h2().text(t("Dernier résultat", "Latest result")))
                .child(
                    link(Route::race(CURRENT, race.round_num()), "link")
                        .text(t("Détails", "Details")),
                ),
        )
        .child(p().class("muted").text(format!(
            "{} {}",
            flag_country(&race.circuit.location.country),
            race.race_name
        )))
        .child(
            ol().class("podium")
                .children(race.results.iter().flatten().take(3).map(|r| {
                    li().class("podium-step")
                        .style(team_style(&r.constructor.constructor_id))
                        .child(
                            link(Route::driver(&r.driver.driver_id), "podium-link")
                                .child(crate::photo::avatar(
                                    &r.driver.full_name(),
                                    r.driver.url.as_deref(),
                                    crate::util::team_color(&r.constructor.constructor_id),
                                    52,
                                ))
                                .child(span().class("podium-pos").text(r.position.clone()))
                                .child(
                                    span()
                                        .class("podium-name")
                                        .text(r.driver.family_name.clone()),
                                )
                                .child(
                                    span().class("podium-team").text(r.constructor.name.clone()),
                                ),
                        )
                })),
        )
        .into()
}

type Standings = (
    Vec<crate::models::DriverStanding>,
    Vec<crate::models::ConstructorStanding>,
);

/// Classements pilotes et écuries chargés jusqu'ici (listes vides sinon).
fn standings(drivers: State<crate::api::Fetch>, teams: State<crate::api::Fetch>) -> Standings {
    let drivers = drivers.with(|f| {
        f.done()
            .and_then(|d| d.standings())
            .and_then(|l| l.driver_standings.clone())
            .unwrap_or_default()
    });
    let teams = teams.with(|f| {
        f.done()
            .and_then(|d| d.standings())
            .and_then(|l| l.constructor_standings.clone())
            .unwrap_or_default()
    });
    (drivers, teams)
}

fn leader_points(teams: &[crate::models::ConstructorStanding]) -> f64 {
    teams
        .first()
        .and_then(|t| t.points.parse().ok())
        .unwrap_or(0.0)
}

fn favourites(drivers: State<crate::api::Fetch>, teams: State<crate::api::Fetch>) -> Node {
    let (drivers, teams) = standings(drivers, teams);
    let fav_driver = crate::util::fav_driver();
    let fav_team = crate::util::fav_team();
    let fav_d = fav_driver
        .as_ref()
        .and_then(|id| drivers.iter().find(|s| &s.driver.driver_id == id));
    let fav_t = fav_team
        .as_ref()
        .and_then(|id| teams.iter().find(|s| &s.constructor.constructor_id == id));
    if fav_d.is_none() && fav_t.is_none() {
        return Node::Empty;
    }
    section()
        .class("card")
        .child(h2().text(t("⭐ Mes favoris", "⭐ My favourites")))
        .child(
            ol().class("rows")
                .child(fav_d.map(driver_standing_row))
                .child(fav_t.map(|tm| team_standing_row(tm, leader_points(&teams)))),
        )
        .into()
}

fn top_drivers(drivers: State<crate::api::Fetch>) -> Node {
    let list = drivers.with(|f| {
        f.done()
            .and_then(|d| d.standings())
            .and_then(|l| l.driver_standings.clone())
            .unwrap_or_default()
    });
    if list.is_empty() {
        return Node::Empty;
    }
    section()
        .class("card")
        .child(
            div()
                .class("card-head")
                .child(h2().text(t("Pilotes", "Drivers")))
                .child(
                    link(
                        Route::DriverStandings {
                            season: CURRENT.into(),
                        },
                        "link",
                    )
                    .text(t("Tout voir", "See all")),
                ),
        )
        .child(
            ol().class("rows")
                .children(list.iter().take(5).map(driver_standing_row)),
        )
        .into()
}

fn top_teams(teams: State<crate::api::Fetch>) -> Node {
    let list = teams.with(|f| {
        f.done()
            .and_then(|d| d.standings())
            .and_then(|l| l.constructor_standings.clone())
            .unwrap_or_default()
    });
    if list.is_empty() {
        return Node::Empty;
    }
    let leader = leader_points(&list);
    section()
        .class("card")
        .child(
            div()
                .class("card-head")
                .child(h2().text(t("Écuries", "Teams")))
                .child(
                    link(
                        Route::TeamStandings {
                            season: CURRENT.into(),
                        },
                        "link",
                    )
                    .text(t("Tout voir", "See all")),
                ),
        )
        .child(
            ol().class("rows")
                .children(list.iter().take(3).map(|tm| team_standing_row(tm, leader))),
        )
        .into()
}

/// Ordre des qualifications du prochain Grand Prix : qualifs (grille du Grand Prix) et
/// qualifs sprint, publiés par le serveur dès la fin de la séance. Rien tant qu'il n'y en a pas.
fn quali_order(round: u32) -> Node {
    let data = use_f1(f1(format!("{CURRENT}/{round}/qualifying.json"), 100));
    // Onglet choisi (sinon : les qualifs si elles ont eu lieu, sinon les qualifs sprint).
    let sprint_tab = use_state(None::<bool>);
    let all = use_state(false);
    dynamic(move || {
        let race = data.with(|f| f.done().and_then(|d| d.race().cloned()));
        let main = race
            .as_ref()
            .and_then(|r| r.qualifying_results.clone())
            .unwrap_or_default();
        let sprint = race
            .as_ref()
            .and_then(|r| r.sprint_qualifying_results.clone())
            .unwrap_or_default();
        if main.is_empty() && sprint.is_empty() {
            return Node::Empty;
        }
        let show_sprint = sprint_tab.get().unwrap_or(main.is_empty());
        let list = if show_sprint {
            sprint.clone()
        } else {
            main.clone()
        };
        let shown = if all.get() {
            list.len()
        } else {
            list.len().min(10)
        };
        let tab = |on: bool, label: &'static str| {
            button()
                .class("seg")
                .class(when(show_sprint == on, "seg-active"))
                .on_click(move |_| sprint_tab.set(Some(on)))
                .text(label)
        };
        let tabs: Node = if !main.is_empty() && !sprint.is_empty() {
            div()
                .class("segmented")
                .child(tab(false, t("Qualifications", "Qualifying")))
                .child(tab(true, t("Qualifs sprint", "Sprint quali")))
                .into()
        } else {
            p().class("eyebrow")
                .text(if show_sprint {
                    t("Qualifs sprint", "Sprint qualifying")
                } else {
                    t("Qualifications", "Qualifying")
                })
                .into()
        };
        section()
            .class("card")
            .child(
                div()
                    .class("card-head")
                    .child(h2().text(t("Ordre des qualifications", "Qualifying order")))
                    .child(link(Route::race(CURRENT, round), "link").text(t("Détails", "Details"))),
            )
            .child(tabs)
            .child(
                ol().class("rows")
                    .children(list.iter().take(shown).map(|q| {
                        if show_sprint {
                            sprint_qualifying_row(q)
                        } else {
                            qualifying_row(q)
                        }
                    })),
            )
            .child((list.len() > 10).then(|| {
                button()
                    .class("btn btn-ghost btn-small")
                    .on_click(move |_| all.update(|a| *a = !*a))
                    .text(if all.get() {
                        t("Voir le top 10", "Show top 10").to_string()
                    } else {
                        tr!("Voir les {} pilotes", "See all {} drivers", list.len())
                    })
            }))
            .into()
    })
}
