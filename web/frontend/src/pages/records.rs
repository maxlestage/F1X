//! Records de tous les temps (depuis 1950).

use yew::prelude::*;
use yew_router::prelude::*;

use super::stats::{Champion, age_at, tally};
use crate::Route;
use crate::api::{Fetch, all, use_f1, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Race, RaceResult};
use crate::tr;
use crate::util::{flag_nationality, team_style};

/// Premier résultat de chaque course d'une réponse filtrée (ex. vainqueurs).
fn firsts(f: &Fetch) -> Vec<(Race, RaceResult)> {
    f.done()
        .map(|d| {
            d.races()
                .iter()
                .filter_map(|r| Some((r.clone(), r.results.as_ref()?.first()?.clone())))
                .collect()
        })
        .unwrap_or_default()
}

/// Section « top N » avec barres.
fn top_section(
    title: &str,
    note: Option<&str>,
    rows: &[(String, String, u32)],
    route: fn(&str) -> Route,
    unit: (&str, &str),
    loading_: bool,
) -> Html {
    let max = rows.first().map(|r| r.2).unwrap_or(1).max(1) as f64;
    html! {
        <section class="card">
            <h2>{ title.to_string() }</h2>
            if let Some(n) = note { <p class="muted">{ n.to_string() }</p> }
            if loading_ && rows.is_empty() { { loading() } }
            <ol class="rows">
                { for rows.iter().take(10).enumerate().map(|(i, (id, name, n))| html! {
                    <li class="row row-plain">
                        <span class="pos">{ i + 1 }</span>
                        <Link<Route> to={route(id)} classes="row-main">
                            <span class="row-title">{ name }</span>
                            <span class="bar" aria-hidden="true"><span class="bar-fill bar-red" style={format!("width:{:.1}%", *n as f64 / max * 100.0)}></span></span>
                        </Link<Route>>
                        <span class="pts">{ n }<small>{ if *n > 1 { unit.1.to_string() } else { unit.0.to_string() } }</small></span>
                    </li>
                }) }
            </ol>
        </section>
    }
}

#[function_component]
pub fn RecordsPage() -> Html {
    let wins = use_f1(all("results/1.json"));
    let seconds = use_f1(all("results/2.json"));
    let thirds = use_f1(all("results/3.json"));
    let poles = use_f1(all("grid/1/results.json"));
    let fastest = use_f1(all("fastest/1/results.json"));
    let champions = use_json::<Vec<Champion>>(Some("/api/champions".into()));

    let winners = firsts(&wins);
    let name = |r: &RaceResult| (r.driver.driver_id.clone(), r.driver.full_name());
    let team = |r: &RaceResult| {
        (
            r.constructor.constructor_id.clone(),
            r.constructor.name.clone(),
        )
    };
    let driver_wins = tally(winners.iter().map(|(_, r)| name(r)));
    let team_wins = tally(winners.iter().map(|(_, r)| team(r)));
    let podium_rows: Vec<(String, String)> = [&wins, &seconds, &thirds]
        .iter()
        .flat_map(|f| firsts(f))
        .map(|(_, r)| name(&r))
        .collect();
    let podium_ready = seconds.done().is_some() && thirds.done().is_some() && wins.done().is_some();
    let podiums = if podium_ready {
        tally(podium_rows.into_iter())
    } else {
        Vec::new()
    };
    let pole_rows = tally(firsts(&poles).iter().map(|(_, r)| name(r)));
    let fastest_rows = tally(firsts(&fastest).iter().map(|(_, r)| name(r)));

    let champs: Vec<Champion> = match &champions {
        Some(Ok(list)) => list.iter().filter(|c| !c.in_progress).cloned().collect(),
        _ => Vec::new(),
    };
    let driver_titles = tally(
        champs
            .iter()
            .map(|c| (c.driver.driver_id.clone(), c.driver.full_name())),
    );
    let team_titles = tally(
        champs
            .iter()
            .filter_map(|c| c.constructor.as_ref())
            .map(|k| (k.constructor_id.clone(), k.name.clone())),
    );

    // Âges des vainqueurs.
    let mut ages: Vec<(f64, &Race, &RaceResult)> = winners
        .iter()
        .filter_map(|(race, r)| {
            Some((
                age_at(r.driver.date_of_birth.as_deref()?, &race.date)?,
                race,
                r,
            ))
        })
        .collect();
    ages.sort_by(|a, b| a.0.total_cmp(&b.0));
    let age_row = |(age, race, r): &(f64, &Race, &RaceResult)| {
        let years = age.floor() as u32;
        let days = ((age - age.floor()) * 365.2425) as u32;
        html! {
            <li class="row" style={team_style(&r.constructor.constructor_id)}>
                <Link<Route> to={Route::driver(&r.driver.driver_id)} classes="row-main">
                    <span class="row-title">{ flag_nationality(r.driver.nationality.as_deref()) }{ " " }{ r.driver.full_name() }</span>
                    <span class="row-sub">{ format!("{} {} · {}", race.season, race.race_name, r.constructor.name) }</span>
                </Link<Route>>
                <span class="pts">{ tr!("{years} ans {days} j", "{years} y {days} d") }</span>
            </li>
        }
    };

    // Plus de victoires en une saison, et plus longues séries de victoires consécutives.
    let season_wins = {
        let mut v = tally(winners.iter().map(|(race, r)| {
            (
                format!("{}|{}", race.season, r.driver.driver_id),
                format!("{} ({})", r.driver.full_name(), race.season),
            )
        }));
        v.truncate(10);
        v
    };
    let streaks = {
        let mut out: Vec<(String, String, u32, String)> = Vec::new();
        let mut i = 0;
        while i < winners.len() {
            let id = &winners[i].1.driver.driver_id;
            let mut j = i;
            while j + 1 < winners.len() && winners[j + 1].1.driver.driver_id == *id {
                j += 1;
            }
            let n = (j - i + 1) as u32;
            if n >= 3 {
                let span = if winners[i].0.season == winners[j].0.season {
                    winners[i].0.season.clone()
                } else {
                    format!("{}–{}", winners[i].0.season, winners[j].0.season)
                };
                out.push((id.clone(), winners[i].1.driver.full_name(), n, span));
            }
            i = j + 1;
        }
        out.sort_by_key(|a| std::cmp::Reverse(a.2));
        out
    };

    let exported: Vec<Vec<String>> = std::iter::once(vec![
        t("Rang", "Rank").into(),
        t("Pilote", "Driver").into(),
        t("Victoires", "Wins").into(),
    ])
    .chain(
        driver_wins
            .iter()
            .enumerate()
            .map(|(i, (_, n, w))| vec![(i + 1).to_string(), n.clone(), w.to_string()]),
    )
    .collect();

    let champions_note = match &champions {
        None => Some(t("Calcul des champions en cours…", "Computing champions…")),
        Some(Err(_)) => Some(t(
            "Champions momentanément indisponibles.",
            "Champions temporarily unavailable.",
        )),
        _ => None,
    };

    html! {
        <Layout title={t("Records", "Records")} tab={Tab::Archives}>
            <section class="card hero">
                <p class="eyebrow">{ t("Depuis 1950", "Since 1950") }</p>
                <h2 class="hero-title">{ t("Les records de la Formule 1", "Formula 1 records") }</h2>
                <p class="muted">{ t("Mis à jour automatiquement après chaque Grand Prix.", "Updated automatically after every Grand Prix.") }</p>
            </section>
            { top_section(t("Titres de champion du monde", "World championships"), champions_note, &driver_titles, Route::driver, (t(" titre", " title"), t(" titres", " titles")), champions.is_none()) }
            { top_section(t("Titres constructeurs", "Constructors' titles"), Some(t("Depuis 1958.", "Since 1958.")), &team_titles, Route::team, (t(" titre", " title"), t(" titres", " titles")), champions.is_none()) }
            { top_section(t("Victoires", "Wins"), None, &driver_wins, Route::driver, (t(" victoire", " win"), t(" victoires", " wins")), wins.is_loading()) }
            { top_section(t("Victoires des écuries", "Team wins"), None, &team_wins, Route::team, (t(" victoire", " win"), t(" victoires", " wins")), wins.is_loading()) }
            { top_section(t("Podiums", "Podiums"), None, &podiums, Route::driver, (" podium", " podiums"), !podium_ready) }
            { top_section(t("Départs en pole position", "Pole positions"), Some(t("Départs depuis la 1re place de la grille.", "Starts from first on the grid.")), &pole_rows, Route::driver, (" pole", " poles"), poles.is_loading()) }
            { top_section(t("Meilleurs tours en course", "Fastest laps"), Some(t("Données disponibles depuis 2004.", "Data available since 2004.")), &fastest_rows, Route::driver, (t(" record", " lap"), t(" records", " laps")), fastest.is_loading()) }

            <section class="card">
                <h2>{ t("Plus de victoires en une saison", "Most wins in a season") }</h2>
                <ol class="rows">
                    { for season_wins.iter().enumerate().map(|(i, (key, label, n))| {
                        let id = key.split('|').nth(1).unwrap_or_default().to_string();
                        html! {
                            <li class="row row-plain">
                                <span class="pos">{ i + 1 }</span>
                                <Link<Route> to={Route::driver(&id)} classes="row-main"><span class="row-title">{ label }</span></Link<Route>>
                                <span class="pts">{ n }</span>
                            </li>
                        }
                    }) }
                </ol>
            </section>

            <section class="card">
                <h2>{ t("Plus longues séries de victoires", "Longest winning streaks") }</h2>
                <ol class="rows">
                    { for streaks.iter().take(8).enumerate().map(|(i, (id, name, n, span))| html! {
                        <li class="row row-plain">
                            <span class="pos">{ i + 1 }</span>
                            <Link<Route> to={Route::driver(id)} classes="row-main">
                                <span class="row-title">{ name }</span>
                                <span class="row-sub">{ span }</span>
                            </Link<Route>>
                            <span class="pts">{ tr!("{n} de suite", "{n} in a row") }</span>
                        </li>
                    }) }
                </ol>
            </section>

            <section class="card">
                <h2>{ t("Plus jeunes vainqueurs", "Youngest winners") }</h2>
                <ol class="rows">{ for ages.iter().take(5).map(age_row) }</ol>
            </section>
            <section class="card">
                <h2>{ t("Vainqueurs les plus âgés", "Oldest winners") }</h2>
                <ol class="rows">{ for ages.iter().rev().take(5).map(age_row) }</ol>
            </section>

            if !champs.is_empty() {
                <details class="card">
                    <summary><strong>{ tr!("Tous les champions ({})", "Every champion ({})", champs.len()) }</strong></summary>
                    <ol class="rows">
                        { for champs.iter().rev().map(|c| html! {
                            <li class="row" style={c.driver_team.as_ref().map(|k| team_style(&k.constructor_id))}>
                                <span class="pos pos-sm">{ &c.season }</span>
                                <Link<Route> to={Route::driver(&c.driver.driver_id)} classes="row-main">
                                    <span class="row-title">{ flag_nationality(c.driver.nationality.as_deref()) }{ " " }{ c.driver.full_name() }</span>
                                    <span class="row-sub">
                                        { c.driver_team.as_ref().map(|k| k.name.clone()).unwrap_or_default() }
                                        if let Some(k) = &c.constructor { { tr!(" · constructeurs : {}", " · constructors: {}", k.name) } }
                                    </span>
                                </Link<Route>>
                            </li>
                        }) }
                    </ol>
                </details>
            }
            <ExportCsv filename="f1x-victoires.csv" rows={exported} />
        </Layout>
    }
}
