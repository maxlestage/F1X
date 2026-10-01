use web_sys::HtmlSelectElement;
use yew::prelude::*;
use yew_router::prelude::*;

use super::IdProps;
use crate::Route;
use crate::api::{f1, use_f1, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::models::Driver;
use crate::tr;
use crate::util::{flag_country, flag_nationality, team_style};

/// Fiche écurie : palmarès complet et détail saison par saison.
#[function_component]
pub fn TeamPage(props: &IdProps) -> Html {
    let id = props.id.to_string();
    let p = |suffix: &str| format!("constructors/{id}{suffix}.json");

    let info = use_f1(f1(p(""), 1));
    let champions = use_json::<Vec<super::stats::Champion>>(Some("/api/champions".into()));
    let wins = use_f1(f1(p("/results/1"), 1));
    let seconds = use_f1(f1(p("/results/2"), 1));
    let thirds = use_f1(f1(p("/results/3"), 1));
    // Poles = départs en tête de grille (fonctionne pour toutes les époques).
    let poles = use_f1(f1(p("/grid/1/results"), 1));
    let seasons = use_f1(f1(p("/seasons"), 100));

    let season_list: Vec<String> = seasons
        .done()
        .map(|d| d.seasons().iter().map(|s| s.season.clone()).collect())
        .unwrap_or_default();
    let chosen = use_state(|| None::<String>);
    let season = (*chosen).clone().or_else(|| season_list.last().cloned());
    let base = |file: &str| {
        season
            .as_ref()
            .map(|s| format!("{s}/constructors/{id}/{file}.json"))
    };
    let standing = use_f1(base("constructorStandings").and_then(|p| f1(p, 1)));
    let results = use_f1(base("results").and_then(|p| f1(p, 100)));

    let Some(team) = info.done().and_then(|d| d.constructors().first().cloned()) else {
        return match info.done() {
            Some(_) => html! { <super::NotFound /> },
            None => {
                html! { <Layout title={t("Écurie", "Team")} tab={Tab::Standings}>{ fetch_view(&info, |_| html! {}) }</Layout> }
            }
        };
    };

    let podiums = [&wins, &seconds, &thirds]
        .iter()
        .map(|f| f.done().map(|d| d.total()))
        .sum::<Option<u32>>()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "–".into());
    let onchange = {
        let chosen = chosen.clone();
        Callback::from(move |e: Event| {
            chosen.set(Some(e.target_unchecked_into::<HtmlSelectElement>().value()))
        })
    };
    let season_standing = standing
        .done()
        .and_then(|d| d.standings())
        .and_then(|l| l.constructor_standings.clone())
        .and_then(|v| v.into_iter().next());
    // Pilotes de la saison, déduits des résultats (une requête de moins).
    let mut season_drivers: Vec<Driver> = Vec::new();
    for r in results
        .done()
        .map(|d| d.races())
        .unwrap_or_default()
        .iter()
        .flat_map(|r| r.results.iter().flatten())
    {
        if !season_drivers
            .iter()
            .any(|d| d.driver_id == r.driver.driver_id)
        {
            season_drivers.push(r.driver.clone());
        }
    }
    let first_season = season_list.first().cloned().unwrap_or_default();
    let last_season = season_list.last().cloned().unwrap_or_default();

    html! {
        <Layout title={team.name.clone()} tab={Tab::Standings}>
            <section class="card hero" style={team_style(&team.constructor_id)}>
                <p class="eyebrow">
                    { team.nationality.clone().unwrap_or_default() }
                    if !first_season.is_empty() { { format!(" · {first_season} – {last_season}") } }
                </p>
                <h2 class="hero-title">{ format!("{} {}", flag_nationality(team.nationality.as_deref()), team.name) }</h2>
                { stat_grid(vec![
                    (t("Victoires", "Wins"), total_of(&wins)),
                    ("Podiums", podiums),
                    ("Poles", total_of(&poles)),
                ]) }
                if let Some(Ok(c)) = &champions {
                    if let titles @ [_, ..] = super::stats::team_titles(c, &team.constructor_id).as_slice() {
                        <p class="titles">{ tr!("🏆 Champion constructeurs ×{} : {}", "🏆 Constructors' champion ×{}: {}", titles.len(), titles.join(", ")) }</p>
                    }
                }
                <FavButton kind="team" id={team.constructor_id.clone()} />
                if let Some(url) = &team.url {
                    <a class="link" href={url.clone()} target="_blank" rel="noopener">{ t("Wikipédia ↗", "Wikipedia ↗") }</a>
                }
            </section>

            { crate::gl3d::car_card(crate::util::team_color(&team.constructor_id), &team.name) }

            if let Some(season) = &season {
                <section class="card">
                    <h2>{ t("Saison par saison", "Season by season") }</h2>
                    <label class="select">
                        <span class="select-label">{ t("Saison", "Season") }</span>
                        <select {onchange} aria-label={t("Choisir une saison", "Choose a season")}>
                            { for season_list.iter().rev().map(|s| html! {
                                <option value={s.clone()} selected={s == season}>{ s }</option>
                            }) }
                        </select>
                    </label>
                    if let Some(s) = &season_standing {
                        { stat_grid(vec![
                            (t("Classement", "Standings"), format!("P{}", s.rank())),
                            ("Points", s.points.clone()),
                            (t("Victoires", "Wins"), s.wins.clone()),
                        ]) }
                    }
                    if !season_drivers.is_empty() {
                        <h3 class="subhead">{ t("Pilotes", "Drivers") }</h3>
                        <ol class="rows">
                            { for season_drivers.iter().map(|drv| html! {
                                <li class="row" style={team_style(&team.constructor_id)}>
                                    <Link<Route> to={Route::driver(&drv.driver_id)} classes="row-main">
                                        <span class="row-title">
                                            { flag_nationality(drv.nationality.as_deref()) }{ " " }
                                            { &drv.given_name }{ " " }<strong>{ &drv.family_name }</strong>
                                        </span>
                                    </Link<Route>>
                                </li>
                            }) }
                        </ol>
                    }
                    <h3 class="subhead">{ t("Résultats", "Results") }</h3>
                    { fetch_view(&results, |d| html! {
                        <ol class="rows">
                            { for d.races().iter().rev().map(|race| {
                                let rows = race.results.clone().unwrap_or_default();
                                let best = rows.iter().filter_map(|r| r.position.parse::<u32>().ok()).min();
                                let points: f64 = rows.iter().filter_map(|r| r.points.parse::<f64>().ok()).sum();
                                let sub = rows.iter().map(|r| format!("{} {}", r.driver.family_name, r.position_text)).collect::<Vec<_>>().join(" · ");
                                html! {
                                    <li class="row" style={team_style(&team.constructor_id)}>
                                        <span class="pos pos-sm">{ best.map(|b| b.to_string()).unwrap_or_else(|| "–".into()) }</span>
                                        <Link<Route> to={Route::race(&race.season, race.round_num())} classes="row-main">
                                            <span class="row-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</span>
                                            <span class="row-sub">{ sub }</span>
                                        </Link<Route>>
                                        if points > 0.0 { <span class="pts">{ format!("+{points}") }</span> }
                                    </li>
                                }
                            }) }
                        </ol>
                    }) }
                    <Link<Route> to={Route::TeamStandings { season: season.clone() }} classes="btn btn-ghost">
                        { tr!("Classement {season}", "{season} standings") }
                    </Link<Route>>
                </section>
            }
        </Layout>
    }
}
