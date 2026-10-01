use std::collections::HashMap;

use yew::prelude::*;
use yew_router::prelude::*;

use super::IdProps;
use super::{TrackPanel, osm_embed};
use crate::api::{all, f1, use_f1, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::models::Race;
use crate::tr;
use crate::util::{country_fr, flag_country, local_date, now_ms, team_style};
use crate::{CURRENT, Route};
use f1x_protocol::TrackMap;

/// « 1:21.046 » → secondes (pour comparer des temps au tour).
fn lap_seconds(t: &str) -> Option<f64> {
    match t.split_once(':') {
        Some((m, s)) => Some(m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?),
        None => t.parse().ok(),
    }
}

/// Classement « nom → nombre » trié décroissant.
fn tally(items: impl Iterator<Item = (String, String)>) -> Vec<(String, String, u32)> {
    let mut map: HashMap<String, (String, u32)> = HashMap::new();
    for (id, name) in items {
        map.entry(id).or_insert((name, 0)).1 += 1;
    }
    let mut v: Vec<(String, String, u32)> = map
        .into_iter()
        .map(|(id, (name, n))| (id, name, n))
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));
    v
}

fn plural(n: u32, word: &str) -> String {
    format!("{n} {word}{}", if n > 1 { "s" } else { "" })
}

/// Fiche circuit : localisation, prochain GP, record, statistiques et palmarès complet.
#[function_component]
pub fn CircuitPage(props: &IdProps) -> Html {
    let id = props.id.to_string();
    let races = use_f1(all(format!("circuits/{id}/races.json")));
    let winners = use_f1(f1(format!("circuits/{id}/results/1.json"), 100));
    let fastest = use_f1(f1(format!("circuits/{id}/fastest/1/results.json"), 100));
    let current = use_f1(f1("current.json", 100));
    // Tracé GPS : seulement pour les circuits utilisés depuis 2023 (données OpenF1).
    let recent = races.done().is_some_and(|d| {
        d.races()
            .iter()
            .any(|r| r.season.parse::<u32>().unwrap_or(0) >= 2023 && r.is_over(now_ms()))
    });
    let track = use_json::<TrackMap>(recent.then(|| format!("/api/track/{id}")));

    // Les infos du circuit viennent de la liste de ses Grands Prix (une requête de moins).
    let Some(circuit) = races
        .done()
        .and_then(|d| d.race())
        .map(|r| r.circuit.clone())
    else {
        return match races.done() {
            Some(_) => html! { <super::NotFound /> },
            None => {
                html! { <Layout title="Circuit" tab={Tab::Archives}>{ fetch_view(&races, |_| html! {}) }</Layout> }
            }
        };
    };
    let loc = &circuit.location;
    let map_url = match (&loc.lat, &loc.long) {
        (Some(lat), Some(lon)) => Some(format!(
            "https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=14/{lat}/{lon}"
        )),
        _ => None,
    };
    let race_list = races.done().map(|d| d.races().to_vec()).unwrap_or_default();
    let names: Vec<String> = {
        let mut n: Vec<String> = race_list.iter().map(|r| r.race_name.clone()).collect();
        n.sort();
        n.dedup();
        n
    };

    // Prochain Grand Prix ici (saison en cours).
    let now = now_ms();
    let next: Option<Race> = current.done().and_then(|d| {
        d.races()
            .iter()
            .find(|r| r.circuit.circuit_id == id && !r.is_over(now))
            .cloned()
    });

    // Palmarès et statistiques.
    let winner_rows: Vec<Race> = winners
        .done()
        .map(|d| d.races().to_vec())
        .unwrap_or_default();
    let wins = winner_rows.iter().filter_map(|r| r.winner());
    let drivers = tally(
        wins.clone()
            .map(|w| (w.driver.driver_id.clone(), w.driver.full_name())),
    );
    let teams = tally(wins.clone().map(|w| {
        (
            w.constructor.constructor_id.clone(),
            w.constructor.name.clone(),
        )
    }));
    let from_pole = wins
        .clone()
        .filter(|w| w.grid.as_deref() == Some("1"))
        .count();
    let lowest_grid = wins
        .clone()
        .filter_map(|w| {
            Some((
                w.grid.as_deref()?.parse::<u32>().ok().filter(|g| *g > 0)?,
                w,
            ))
        })
        .max_by_key(|(g, _)| *g);
    let n_wins = winner_rows.len();

    // Meilleur tour en course (données disponibles depuis 2004).
    let record = fastest.done().and_then(|d| {
        d.races()
            .iter()
            .filter_map(|race| {
                let r = race.results.as_ref()?.first()?;
                let t = r.fastest_lap.as_ref()?.time.as_ref()?.time.clone();
                Some((
                    lap_seconds(&t)?,
                    t,
                    race.season.clone(),
                    r.driver.full_name(),
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    });

    html! {
        <Layout title={circuit.circuit_name.clone()} tab={Tab::Archives}>
            <section class="card hero">
                <p class="eyebrow">{ format!("{}, {}", loc.locality, country_fr(&loc.country)) }</p>
                <h2 class="hero-title">{ format!("{} {}", flag_country(&loc.country), circuit.circuit_name) }</h2>
                { stat_grid(vec![
                    ("Grands Prix", total_of(&races)),
                    (t("Premier", "First"), race_list.first().map(|r| r.season.clone()).unwrap_or_else(|| "–".into())),
                    (t("Dernier", "Last"), race_list.last().map(|r| r.season.clone()).unwrap_or_else(|| "–".into())),
                    (t("Pilotes vainqueurs", "Winning drivers"), if winners.done().is_some() { drivers.len().to_string() } else { "–".into() }),
                    (t("Depuis la pole", "From pole"), (from_pole * 100).checked_div(n_wins).map(|p| format!("{p}%")).unwrap_or_else(|| "–".into())),
                    (t("Écuries", "Teams"), if winners.done().is_some() { teams.len().to_string() } else { "–".into() }),
                ]) }
                if let (Some(lat), Some(lon)) = (&loc.lat, &loc.long) {
                    <p class="muted">{ tr!("Coordonnées : {lat}, {lon}", "Coordinates: {lat}, {lon}") }</p>
                }
                if let Some(url) = map_url {
                    <a class="btn btn-ghost" href={url} target="_blank" rel="noopener">{ t("Voir sur la carte ↗", "View on the map ↗") }</a>
                }
                if let Some(url) = &circuit.url {
                    <a class="link" href={url.clone()} target="_blank" rel="noopener">{ t("Wikipédia ↗", "Wikipedia ↗") }</a>
                }
            </section>

            if let Some(url) = &circuit.url {
                <crate::photo::WikiPhoto url={url.clone()} alt={circuit.circuit_name.clone()} wide={true} />
            }

            <section class="card">
                <h2>{ t("Tracé", "Track layout") }</h2>
                { match &track {
                    None if recent => loading(),
                    None => html! {
                        <p class="muted">{ t(
                            "Tracé disponible pour les circuits utilisés depuis 2023 (données GPS OpenF1).",
                            "Layout available for circuits used since 2023 (OpenF1 GPS data).",
                        ) }</p>
                    },
                    Some(Ok(map)) => html! { <TrackPanel track={map.clone()} start_3d=true /> },
                    Some(Err((404, _))) => html! {
                        <p class="muted">{ t(
                            "Tracé disponible pour les circuits utilisés depuis 2023 (données GPS OpenF1).",
                            "Layout available for circuits used since 2023 (OpenF1 GPS data).",
                        ) }</p>
                    },
                    Some(Err(_)) => html! {
                        <p class="muted">{ t("Tracé momentanément indisponible.", "Layout temporarily unavailable.") }</p>
                    },
                } }
            </section>

            if let (Some(lat), Some(lon)) = (&loc.lat, &loc.long) {
                <section class="card">
                    <h2>{ t("Carte", "Map") }</h2>
                    { osm_embed(lat, lon) }
                </section>
            }

            if let Some(race) = &next {
                <section class="card">
                    <p class="eyebrow">{ tr!("Prochain Grand Prix · Manche {}", "Next Grand Prix · Round {}", race.round) }</p>
                    <h2>{ &race.race_name }</h2>
                    <p class="muted">{ local_date(&race.start_iso(), race.has_time()) }</p>
                    <Countdown target_ms={race.start_ms()} />
                    <Link<Route> to={Route::race(CURRENT, race.round_num())} classes="btn">{ t("Voir le programme", "View the schedule") }</Link<Route>>
                </section>
            }

            <section class="card">
                <h2>{ t("En bref", "At a glance") }</h2>
                <ul class="sessions">
                    <li class="session"><span class="session-name">{ t("Ville", "City") }</span><span class="session-time">{ &loc.locality }</span></li>
                    <li class="session"><span class="session-name">{ t("Pays", "Country") }</span><span class="session-time">{ country_fr(&loc.country) }</span></li>
                    <li class="session"><span class="session-name">{ t("Grands Prix disputés", "Grands Prix held") }</span><span class="session-time">{ race_list.len() }</span></li>
                    <li class="session"><span class="session-name">{ t("Victoires depuis la pole", "Wins from pole") }</span>
                        <span class="session-time">{ if n_wins > 0 { tr!("{from_pole} sur {n_wins}", "{from_pole} of {n_wins}") } else { "–".into() } }</span></li>
                    if let Some((grid, w)) = lowest_grid {
                        <li class="session"><span class="session-name">{ t("Victoire parti le plus loin", "Win from furthest back") }</span>
                            <span class="session-time">{ format!("P{grid} · {}", w.driver.full_name()) }</span></li>
                    }
                    if let Some((_, time, season, driver)) = &record {
                        <li class="session"><span class="session-name">{ t("Meilleur tour en course", "Race lap record") }</span>
                            <span class="session-time">{ format!("{time} · {driver} ({season})") }</span></li>
                    }
                </ul>
                if !names.is_empty() {
                    <p class="muted">{ tr!("Épreuves : {}", "Events: {}", names.join(", ")) }</p>
                }
                if record.is_some() {
                    <p class="muted">{ t("Record : meilleur tour en course enregistré depuis 2004, toutes configurations du tracé confondues.", "Record: fastest race lap recorded since 2004, all track layouts combined.") }</p>
                }
            </section>

            if !drivers.is_empty() {
                <section class="card">
                    <h2>{ t("Rois du circuit", "Kings of the circuit") }</h2>
                    <ol class="rows">
                        { for drivers.iter().take(5).enumerate().map(|(i, (id, name, n))| html! {
                            <li class="row row-plain">
                                <span class="pos">{ i + 1 }</span>
                                <Link<Route> to={Route::driver(id)} classes="row-main">
                                    <span class="row-title">{ name }</span>
                                </Link<Route>>
                                <span class="pts">{ plural(*n, t("victoire", "win")) }</span>
                            </li>
                        }) }
                    </ol>
                </section>
            }

            if !teams.is_empty() {
                <section class="card">
                    <h2>{ t("Écuries les plus victorieuses", "Most successful teams") }</h2>
                    <ol class="rows">
                        { for teams.iter().take(5).enumerate().map(|(i, (id, name, n))| html! {
                            <li class="row" style={team_style(id)}>
                                <span class="pos">{ i + 1 }</span>
                                <Link<Route> to={Route::team(id)} classes="row-main">
                                    <span class="row-title">{ name }</span>
                                </Link<Route>>
                                <span class="pts">{ plural(*n, t("victoire", "win")) }</span>
                            </li>
                        }) }
                    </ol>
                </section>
            }

            <section class="card">
                <h2>{ tr!("Palmarès ({})", "Winners ({})", n_wins) }</h2>
                { fetch_view(&winners, |_| html! {
                    <ol class="rows">
                        { for winner_rows.iter().rev().filter_map(|race| {
                            let w = race.winner()?;
                            let mut sub = vec![w.constructor.name.clone()];
                            if let Some(g) = w.grid.as_deref().filter(|g| *g != "0") { sub.push(tr!("parti P{g}", "started P{g}")); }
                            if race.race_name != circuit.circuit_name { sub.push(race.race_name.clone()); }
                            Some(html! {
                                <li class="row" style={team_style(&w.constructor.constructor_id)}>
                                    <span class="pos pos-sm">{ &race.season }</span>
                                    <Link<Route> to={Route::race(&race.season, race.round_num())} classes="row-main">
                                        <span class="row-title">{ w.driver.full_name() }</span>
                                        <span class="row-sub">{ sub.join(" · ") }</span>
                                    </Link<Route>>
                                </li>
                            })
                        }) }
                    </ol>
                }) }
            </section>
        </Layout>
    }
}
