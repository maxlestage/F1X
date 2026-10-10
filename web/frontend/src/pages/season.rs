use active::prelude::*;

use super::season_label;
use crate::api::{f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::models::RaceResult;
use crate::tr;
use crate::util::{flag_country, local_date, now_ms};
use crate::{Route, link};

/// Calendrier d'une saison, avec le vainqueur de chaque Grand Prix disputé.
pub fn season_page(season: &str) -> Node {
    let season = season.to_string();
    let schedule = use_f1(f1(format!("{season}.json"), 100));
    let winners = use_f1(f1(format!("{season}/results/1.json"), 100));

    let body = {
        let season = season.clone();
        fetch_view(schedule, move |data| {
            let races = data.races();
            let now = now_ms();
            let next_round = races
                .iter()
                .find(|r| !r.is_over(now))
                .map(|r| r.round.clone());
            let year = races.first().map(|r| r.season.clone()).unwrap_or_default();
            if races.is_empty() {
                return empty_card(t(
                    "Aucun Grand Prix pour cette saison.",
                    "No Grand Prix this season.",
                ));
            }
            // La course qui arrive, avec sa date, son heure et le compte à rebours, tout en haut.
            let next_card = races.iter().find(|r| !r.is_over(now)).map(|race| {
                link(Route::race(&season, race.round_num()), "card next-race")
                    .child(
                        span()
                            .class("eyebrow")
                            .text(t("Prochaine course", "Next race")),
                    )
                    .child(span().class("next-race-name").text(format!(
                        "{} {}",
                        flag_country(&race.circuit.location.country),
                        race.race_name
                    )))
                    .child(
                        span()
                            .class("next-race-date")
                            .text(format!(
                                "📅 {}",
                                local_date(&race.start_iso(), race.has_time())
                            ))
                            .child(race.is_sprint_weekend().then(sprint_tag)),
                    )
                    .child(span().class("muted").text(format!(
                        "{} · {}",
                        race.circuit.circuit_name, race.circuit.location.locality
                    )))
                    .child(race.has_time().then(|| countdown(race.start_ms())))
            });
            let rows =
                races.iter().map(|race| {
                    let done = race.is_over(now);
                    let is_next = next_round.as_deref() == Some(race.round.as_str());
                    let round = race.round.clone();
                    // Le vainqueur arrive par une autre requête : seule cette ligne le suit.
                    let winner = text_dyn(move || {
                        winners.with(|w| {
                            w.done()
                                .and_then(|d| d.races().iter().find(|r| r.round == round).cloned())
                                .and_then(|r| r.winner().map(winner_label))
                                .unwrap_or_default()
                        })
                    });
                    li().child(
                        link(Route::race(&season, race.round_num()), "race-item")
                            .class(when(done && season == crate::CURRENT, "done"))
                            .class(when(is_next, "next"))
                            .child(span().class("race-round").text(format!("R{}", race.round)))
                            .child(
                                span()
                                    .class("race-main")
                                    .child(span().class("race-name").text(format!(
                                        "{} {}",
                                        flag_country(&race.circuit.location.country),
                                        race.race_name
                                    )))
                                    // Une seule ligne : date, sprint, vainqueur.
                                    .child(
                                        span()
                                            .class("race-meta")
                                            .text(local_date(&race.start_iso(), false))
                                            .child(race.is_sprint_weekend().then(sprint_tag))
                                            .child(winner),
                                    ),
                            )
                            .child(is_next.then(|| {
                                span().class("badge badge-live").text(t("Prochain", "Next"))
                            }))
                            .child((done && season == crate::CURRENT).then(|| {
                                span()
                                    .class("race-check")
                                    .attr("aria-label", t("Terminé", "Done"))
                                    .text("✓")
                            })),
                    )
                });
            fragment([
                Node::from(next_card),
                p().class("section-intro")
                    .text(tr!(
                        "Saison {year} · {} Grands Prix",
                        "Season {year} · {} Grands Prix",
                        races.len()
                    ))
                    .into(),
                ol().class("race-list").children(rows).into(),
            ])
        })
    };

    layout(
        &tr!("Calendrier · {}", "Calendar · {}", season_label(&season)),
        Some(Tab::Calendar),
        fragment([
            season_select(&season, SeasonTarget::Calendar, None),
            // Abonnement au calendrier de la saison (iPhone, Mac, Google Agenda, Outlook…).
            a().class("link cal-sub")
                .href(crate::util::webcal_url())
                .text(t(
                    "📅 Ajouter les séances à mon calendrier",
                    "📅 Add sessions to my calendar",
                ))
                .into(),
            div()
                .class("segmented")
                .child(
                    link(
                        Route::DriverStandings {
                            season: season.clone(),
                        },
                        "seg",
                    )
                    .text(t("Classement pilotes", "Driver standings")),
                )
                .child(
                    link(
                        Route::TeamStandings {
                            season: season.clone(),
                        },
                        "seg",
                    )
                    .text(t("Classement écuries", "Team standings")),
                )
                .into(),
            body,
        ]),
    )
}

fn sprint_tag() -> Node {
    fragment([Node::from(" · "), span().class("tag").text("Sprint").into()])
}

/// « · 🏆 L. Hamilton ».
fn winner_label(w: &RaceResult) -> String {
    format!(
        " · 🏆 {}. {}",
        w.driver.given_name.chars().next().unwrap_or(' '),
        w.driver.family_name
    )
}

#[derive(Clone, Copy, PartialEq)]
pub enum StandingsKind {
    Drivers,
    Teams,
}

pub fn standings_page(season: &str, kind: StandingsKind) -> Node {
    let season = season.to_string();
    let file = match kind {
        StandingsKind::Drivers => "driverStandings",
        StandingsKind::Teams => "constructorStandings",
    };
    let fetch = use_f1(f1(format!("{season}/{file}.json"), 100));

    let body = {
        let season = season.clone();
        fetch_view(fetch, move |data| {
            let list = data.standings();
            let after = list
                .and_then(|l| l.round.clone())
                .map(|r| tr!("Après la manche {r}", "After round {r}"))
                .unwrap_or_default();
            match kind {
                StandingsKind::Drivers => {
                    let rows = list
                        .and_then(|l| l.driver_standings.clone())
                        .unwrap_or_default();
                    if rows.is_empty() {
                        return empty_card(t(
                            "Le classement n'est pas encore disponible.",
                            "Standings are not available yet.",
                        ));
                    }
                    let csv: Vec<Vec<String>> = std::iter::once(vec![
                        t("Pos", "Pos").into(),
                        t("Pilote", "Driver").into(),
                        t("Écurie", "Team").into(),
                        "Points".into(),
                        t("Victoires", "Wins").into(),
                    ])
                    .chain(rows.iter().map(|s| {
                        vec![
                            s.rank(),
                            s.driver.full_name(),
                            s.constructors
                                .last()
                                .map(|c| c.name.clone())
                                .unwrap_or_default(),
                            s.points.clone(),
                            s.wins.clone(),
                        ]
                    }))
                    .collect();
                    fragment([
                        p().class("section-intro").text(after).into(),
                        ol().class("rows rows-card")
                            .children(rows.iter().map(driver_standing_row))
                            .into(),
                        export_csv(format!("f1x-pilotes-{season}.csv"), csv),
                    ])
                }
                StandingsKind::Teams => {
                    let rows = list
                        .and_then(|l| l.constructor_standings.clone())
                        .unwrap_or_default();
                    if rows.is_empty() {
                        return empty_card(t(
                            "Pas de classement des constructeurs pour cette saison (créé en 1958).",
                            "No constructors' championship this season (created in 1958).",
                        ));
                    }
                    let leader = rows
                        .first()
                        .and_then(|t| t.points.parse().ok())
                        .unwrap_or(0.0);
                    let csv: Vec<Vec<String>> = std::iter::once(vec![
                        "Pos".into(),
                        t("Écurie", "Team").into(),
                        "Points".into(),
                        t("Victoires", "Wins").into(),
                    ])
                    .chain(rows.iter().map(|s| {
                        vec![
                            s.rank(),
                            s.constructor.name.clone(),
                            s.points.clone(),
                            s.wins.clone(),
                        ]
                    }))
                    .collect();
                    fragment([
                        p().class("section-intro").text(after).into(),
                        ol().class("rows rows-card")
                            .children(rows.iter().map(|tm| team_standing_row(tm, leader)))
                            .into(),
                        export_csv(format!("f1x-ecuries-{season}.csv"), csv),
                    ])
                }
            }
        })
    };

    let (title, target) = match kind {
        StandingsKind::Drivers => (t("Pilotes", "Drivers"), SeasonTarget::DriverStandings),
        StandingsKind::Teams => (t("Écuries", "Teams"), SeasonTarget::TeamStandings),
    };
    let seg = |k: StandingsKind, route: Route, label: &'static str| {
        link(route, "seg")
            .class(when(kind == k, "seg-active"))
            .text(label)
    };
    layout(
        &format!("{title} · {}", season_label(&season)),
        Some(Tab::Standings),
        fragment([
            season_select(&season, target, None),
            div()
                .class("segmented")
                .child(seg(
                    StandingsKind::Drivers,
                    Route::DriverStandings {
                        season: season.clone(),
                    },
                    t("Pilotes", "Drivers"),
                ))
                .child(seg(
                    StandingsKind::Teams,
                    Route::TeamStandings {
                        season: season.clone(),
                    },
                    t("Écuries", "Teams"),
                ))
                .into(),
            body,
        ]),
    )
}
