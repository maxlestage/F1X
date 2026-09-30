use std::collections::HashMap;

use yew::prelude::*;
use yew_router::prelude::*;

use super::season_label;
use crate::Route;
use crate::api::{f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::models::RaceResult;
use crate::tr;
use crate::util::{flag_country, local_date, now_ms};

#[derive(Properties, PartialEq)]
pub struct SeasonProps {
    pub season: AttrValue,
}

/// Calendrier d'une saison, avec le vainqueur de chaque Grand Prix disputé.
#[function_component]
pub fn SeasonPage(props: &SeasonProps) -> Html {
    let season = props.season.to_string();
    let schedule = use_f1(f1(format!("{season}.json"), 100));
    let winners = use_f1(f1(format!("{season}/results/1.json"), 100));

    let winner_by_round: HashMap<String, RaceResult> = winners
        .done()
        .map(|d| {
            d.races()
                .iter()
                .filter_map(|r| Some((r.round.clone(), r.winner()?.clone())))
                .collect()
        })
        .unwrap_or_default();

    let body = fetch_view(&schedule, |data| {
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
        html! {
            <>
                <p class="section-intro">{ tr!("Saison {year} · {} Grands Prix", "Season {year} · {} Grands Prix", races.len()) }</p>
                <ol class="race-list">
                    { for races.iter().map(|race| {
                        let done = race.is_over(now);
                        let is_next = next_round.as_deref() == Some(race.round.as_str());
                        let winner = winner_by_round.get(&race.round);
                        html! {
                            <li>
                                <Link<Route> to={Route::race(&season, race.round_num())}
                                    classes={classes!("race-item", (done && season == crate::CURRENT).then_some("done"), is_next.then_some("next"))}>
                                    <span class="race-round">{ format!("R{}", race.round) }</span>
                                    <span class="race-main">
                                        <span class="race-name">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</span>
                                        <span class="race-meta">
                                            { local_date(&race.start_iso(), false) }
                                            if race.is_sprint_weekend() { { " · " }<span class="tag">{ "Sprint" }</span> }
                                        </span>
                                        if let Some(w) = winner {
                                            <span class="race-meta">{ format!("🏆 {} ({})", w.driver.full_name(), w.constructor.name) }</span>
                                        }
                                    </span>
                                    if is_next { <span class="badge badge-live">{ t("Prochain", "Next") }</span> }
                                </Link<Route>>
                            </li>
                        }
                    }) }
                </ol>
            </>
        }
    });

    html! {
        <Layout title={tr!("Calendrier · {}", "Calendar · {}", season_label(&season))} tab={Tab::Calendar}>
            <SeasonSelect season={props.season.clone()} target={SeasonTarget::Calendar} />
            <div class="segmented">
                <Link<Route> to={Route::DriverStandings { season: season.clone() }} classes="seg">{ t("Classement pilotes", "Driver standings") }</Link<Route>>
                <Link<Route> to={Route::TeamStandings { season: season.clone() }} classes="seg">{ t("Classement écuries", "Team standings") }</Link<Route>>
            </div>
            { body }
        </Layout>
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum StandingsKind {
    Drivers,
    Teams,
}

#[derive(Properties, PartialEq)]
pub struct StandingsProps {
    pub season: AttrValue,
    pub kind: StandingsKind,
}

#[function_component]
pub fn StandingsPage(props: &StandingsProps) -> Html {
    let season = props.season.to_string();
    let file = match props.kind {
        StandingsKind::Drivers => "driverStandings",
        StandingsKind::Teams => "constructorStandings",
    };
    let fetch = use_f1(f1(format!("{season}/{file}.json"), 100));

    let body = fetch_view(&fetch, |data| {
        let list = data.standings();
        let after = list
            .and_then(|l| l.round.clone())
            .map(|r| tr!("Après la manche {r}", "After round {r}"))
            .unwrap_or_default();
        match props.kind {
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
                html! {
                    <>
                        <p class="section-intro">{ after }</p>
                        <ol class="rows rows-card">{ for rows.iter().map(driver_standing_row) }</ol>
                        <ExportCsv filename={format!("f1x-pilotes-{season}.csv")} rows={csv} />
                    </>
                }
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
                html! {
                    <>
                        <p class="section-intro">{ after }</p>
                        <ol class="rows rows-card">{ for rows.iter().map(|t| team_standing_row(t, leader)) }</ol>
                        <ExportCsv filename={format!("f1x-ecuries-{season}.csv")} rows={csv} />
                    </>
                }
            }
        }
    });

    let (title, target) = match props.kind {
        StandingsKind::Drivers => (t("Pilotes", "Drivers"), SeasonTarget::DriverStandings),
        StandingsKind::Teams => (t("Écuries", "Teams"), SeasonTarget::TeamStandings),
    };
    let is = |k| {
        if props.kind == k {
            "seg seg-active"
        } else {
            "seg"
        }
    };
    html! {
        <Layout title={format!("{title} · {}", season_label(&season))} tab={Tab::Standings}>
            <SeasonSelect season={props.season.clone()} {target} />
            <div class="segmented">
                <Link<Route> to={Route::DriverStandings { season: season.clone() }} classes={is(StandingsKind::Drivers)}>{ t("Pilotes", "Drivers") }</Link<Route>>
                <Link<Route> to={Route::TeamStandings { season: season.clone() }} classes={is(StandingsKind::Teams)}>{ t("Écuries", "Teams") }</Link<Route>>
            </div>
            { body }
        </Layout>
    }
}
