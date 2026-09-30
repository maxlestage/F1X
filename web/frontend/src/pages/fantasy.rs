//! Fantasy F1 : 5 pilotes + 1 écurie, budget 100 M€, points = résultats réels.
//! Équipe enregistrée sur l'appareil ; chaque modification s'applique à partir du prochain GP.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use yew::prelude::*;

use crate::api::{all, f1, use_f1};
use crate::components::*;
use crate::i18n::t;
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

/// Prix d'après les points au championnat : de 5 à 30 M€ (pilotes), 8 à 30 M€ (écuries).
fn price(points: f64, leader: f64, min: f64) -> f64 {
    let ratio = if leader > 0.0 {
        (points / leader).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((min + (30.0 - min) * ratio.powf(0.8)) * 2.0).round() / 2.0
}

#[function_component]
pub fn FantasyPage() -> Html {
    let schedule = use_f1(f1("current.json", 100));
    let standings = use_f1(f1("current/driverStandings.json", 100));
    let teams = use_f1(f1("current/constructorStandings.json", 100));
    let results = use_f1(all("current/results.json"));
    let sprints = use_f1(all("current/sprint.json"));
    let versions = use_state(|| load::<Vec<Version>>(KEY).unwrap_or_default());
    let draft = use_state(|| None::<(Vec<String>, String)>);

    let now = now_ms();
    let races = schedule
        .done()
        .map(|d| d.races().to_vec())
        .unwrap_or_default();
    let season = races.first().map(|r| r.season.clone()).unwrap_or_default();
    let next_round = races
        .iter()
        .find(|r| !r.is_over(now))
        .map(|r| r.round_num());
    let mine: Vec<Version> = versions
        .iter()
        .filter(|v| v.season == season)
        .cloned()
        .collect();
    let latest = mine.last().cloned();
    let (picked, team): (Vec<String>, String) = (*draft).clone().unwrap_or_else(|| {
        latest
            .as_ref()
            .map(|v| (v.drivers.clone(), v.team.clone()))
            .unwrap_or_default()
    });

    let ds = standings
        .done()
        .and_then(|d| d.standings())
        .and_then(|l| l.driver_standings.clone())
        .unwrap_or_default();
    let cs = teams
        .done()
        .and_then(|d| d.standings())
        .and_then(|l| l.constructor_standings.clone())
        .unwrap_or_default();
    let d_leader: f64 = ds
        .first()
        .and_then(|s| s.points.parse().ok())
        .unwrap_or(0.0);
    let c_leader: f64 = cs
        .first()
        .and_then(|s| s.points.parse().ok())
        .unwrap_or(0.0);
    let d_price: HashMap<String, f64> = ds
        .iter()
        .map(|s| {
            (
                s.driver.driver_id.clone(),
                price(s.points.parse().unwrap_or(0.0), d_leader, 5.0),
            )
        })
        .collect();
    let c_price: HashMap<String, f64> = cs
        .iter()
        .map(|s| {
            (
                s.constructor.constructor_id.clone(),
                price(s.points.parse().unwrap_or(0.0), c_leader, 8.0),
            )
        })
        .collect();
    let spent: f64 = picked.iter().filter_map(|d| d_price.get(d)).sum::<f64>()
        + c_price.get(&team).copied().unwrap_or(0.0);
    let left = BUDGET - spent;
    let valid = picked.len() == DRIVERS && !team.is_empty() && left >= 0.0;
    let changed = latest
        .as_ref()
        .is_none_or(|v| v.drivers != picked || v.team != team);

    // Points par manche : pilotes (course + sprint) + écurie (moitié des points de ses pilotes).
    let mut round_points: HashMap<u32, HashMap<String, f64>> = HashMap::new();
    let mut team_round: HashMap<u32, HashMap<String, f64>> = HashMap::new();
    for f in [&results, &sprints] {
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
    }
    let mut rounds: Vec<u32> = round_points.keys().copied().collect();
    rounds.sort_unstable();
    let history: Vec<(u32, f64)> = rounds
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
        .collect();
    let total: f64 = history.iter().map(|h| h.1).sum::<f64>() + 0.0; // « + 0.0 » : évite l'affichage « -0 »

    let toggle = |id: String| {
        let draft = draft.clone();
        let (picked, team) = (picked.clone(), team.clone());
        Callback::from(move |_| {
            let mut p = picked.clone();
            if let Some(i) = p.iter().position(|x| *x == id) {
                p.remove(i);
            } else if p.len() < DRIVERS {
                p.push(id.clone());
            }
            draft.set(Some((p, team.clone())));
        })
    };
    let choose_team = |id: String| {
        let draft = draft.clone();
        let picked = picked.clone();
        Callback::from(move |_| draft.set(Some((picked.clone(), id.clone()))))
    };
    let save = {
        let versions = versions.clone();
        let draft = draft.clone();
        let (picked, team, season) = (picked.clone(), team.clone(), season.clone());
        Callback::from(move |_| {
            let Some(from_round) = next_round else { return };
            let mut all = (*versions).clone();
            all.retain(|v| !(v.season == season && v.from_round == from_round));
            all.push(Version {
                season: season.clone(),
                from_round,
                drivers: picked.clone(),
                team: team.clone(),
            });
            store(KEY, &all);
            versions.set(all);
            draft.set(None);
        })
    };

    let ready = standings.done().is_some() && teams.done().is_some();
    html! {
        <Layout title={t("Fantasy F1", "Fantasy F1")} tab={Tab::Archives}>
            <section class="card hero">
                <p class="eyebrow">{ tr!("Saison {season}", "Season {season}") }</p>
                <h2 class="hero-title">{ tr!("{total:.0} points", "{total:.0} points") }</h2>
                { stat_grid(vec![
                    (t("Budget restant", "Budget left"), format!("{left:.1} M€")),
                    (t("Pilotes", "Drivers"), format!("{}/{DRIVERS}", picked.len())),
                    (t("Écurie", "Team"), if team.is_empty() { "0/1".into() } else { "1/1".into() }),
                ]) }
                if left < 0.0 { <p class="muted">{ t("⚠️ Budget dépassé : retire un pilote.", "⚠️ Over budget: drop a driver.") }</p> }
                <button class="btn" onclick={save} disabled={!valid || !changed || next_round.is_none()}>
                    { match next_round {
                        Some(r) if changed => tr!("Valider (compte à partir de la manche {r})", "Confirm (counts from round {r})"),
                        Some(_) => t("Équipe enregistrée ✓", "Team saved ✓").to_string(),
                        None => t("Saison terminée", "Season over").to_string(),
                    } }
                </button>
            </section>

            if !ready { { loading() } }
            <section class="card">
                <h2>{ t("Pilotes", "Drivers") }</h2>
                <ol class="rows">
                    { for ds.iter().map(|s| {
                        let id = s.driver.driver_id.clone();
                        let on = picked.contains(&id);
                        let cost = d_price.get(&id).copied().unwrap_or(5.0);
                        let affordable = on || (picked.len() < DRIVERS && cost <= left);
                        html! {
                            <li class={classes!("row", on.then_some("row-picked"))} style={s.constructors.last().map(|c| team_style(&c.constructor_id))}>
                                <span class="row-main">
                                    <span class="row-title">{ &s.driver.given_name }{ " " }<strong>{ &s.driver.family_name }</strong></span>
                                    <span class="row-sub">{ format!("{cost:.1} M€ · {} pts", s.points) }</span>
                                </span>
                                <button class={classes!("pick-btn", on.then_some("pick-on"))} onclick={toggle(id)} disabled={!affordable}
                                        aria-pressed={on.to_string()}>{ if on { "✓" } else { "+" } }</button>
                            </li>
                        }
                    }) }
                </ol>
            </section>
            <section class="card">
                <h2>{ t("Écurie", "Team") }</h2>
                <ol class="rows">
                    { for cs.iter().map(|s| {
                        let id = s.constructor.constructor_id.clone();
                        let on = team == id;
                        let cost = c_price.get(&id).copied().unwrap_or(8.0);
                        html! {
                            <li class={classes!("row", on.then_some("row-picked"))} style={team_style(&id)}>
                                <span class="row-main">
                                    <span class="row-title">{ &s.constructor.name }</span>
                                    <span class="row-sub">{ format!("{cost:.1} M€ · {} pts", s.points) }</span>
                                </span>
                                <button class={classes!("pick-btn", on.then_some("pick-on"))} onclick={choose_team(id)} aria-pressed={on.to_string()}>{ if on { "✓" } else { "+" } }</button>
                            </li>
                        }
                    }) }
                </ol>
            </section>
            if !history.is_empty() {
                <section class="card">
                    <h2>{ t("Points par manche", "Points per round") }</h2>
                    <ul class="sessions">
                        { for history.iter().rev().map(|(r, p)| html! {
                            <li class="session"><span class="session-name">{ tr!("Manche {r}", "Round {r}") }</span><span class="session-time">{ format!("+{p:.0}") }</span></li>
                        }) }
                    </ul>
                </section>
            }
            <section class="card">
                <h2>{ t("Règles", "Rules") }</h2>
                <p class="muted">{ t(
                    "5 pilotes + 1 écurie, 100 M€. Prix selon le classement actuel. Points : ceux marqués en vrai par tes pilotes (course + sprint), plus la moitié des points de ton écurie. Chaque modification compte à partir du prochain Grand Prix. Équipe enregistrée sur ce téléphone.",
                    "5 drivers + 1 team, €100M. Prices follow the current standings. Points: what your drivers really score (race + sprint), plus half of your team's points. Every change counts from the next Grand Prix. Team stored on this phone.",
                ) }</p>
            </section>
        </Layout>
    }
}
