use web_sys::HtmlSelectElement;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::api::{all, f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Constructor, Race, RaceResult};
use crate::tr;
use crate::util::{age, current_year, flag_country, flag_nationality, format_birth, team_style};

#[derive(Properties, PartialEq)]
pub struct IdProps {
    pub id: AttrValue,
}

/// Fiche pilote : carrière complète (1950 → aujourd'hui) et détail saison par saison.
#[function_component]
pub fn DriverPage(props: &IdProps) -> Html {
    let id = props.id.to_string();
    let p = |suffix: &str| format!("drivers/{id}{suffix}.json");

    // Toute la carrière en une seule requête paginée : départs, victoires, podiums,
    // poles, meilleurs tours, saisons et écuries en sont déduits (économise le quota Jolpica).
    let career = use_f1(all(p("/results")));
    let races: Vec<Race> = career
        .done()
        .map(|d| d.races().to_vec())
        .unwrap_or_default();
    let entries: Vec<(&Race, &RaceResult)> = races
        .iter()
        .filter_map(|r| Some((r, r.results.as_ref()?.first()?)))
        .collect();

    let mut season_list: Vec<String> = entries.iter().map(|(r, _)| r.season.clone()).collect();
    season_list.dedup();
    let chosen = use_state(|| None::<String>);
    let season = (*chosen).clone().or_else(|| season_list.last().cloned());
    let standing = use_f1(
        season
            .as_ref()
            .and_then(|s| f1(format!("{s}/drivers/{id}/driverStandings.json"), 1)),
    );

    let Some(driver) = entries.first().map(|(_, r)| r.driver.clone()) else {
        return match career.done() {
            Some(_) => html! { <super::NotFound /> },
            None => {
                html! { <Layout title={t("Pilote", "Driver")} tab={Tab::Standings}>{ fetch_view(&career, |_| html! {}) }</Layout> }
            }
        };
    };

    let count =
        |f: &dyn Fn(&RaceResult) -> bool| entries.iter().filter(|(_, r)| f(r)).count().to_string();
    let starts = entries.len().to_string();
    let wins = count(&|r| r.position == "1");
    let podiums = count(&|r| matches!(r.position.as_str(), "1" | "2" | "3"));
    let poles = count(&|r| r.grid.as_deref() == Some("1"));
    let fastest = count(&|r| r.has_fastest_lap());
    let mut teams: Vec<Constructor> = Vec::new();
    for (_, r) in &entries {
        if !teams
            .iter()
            .any(|t| t.constructor_id == r.constructor.constructor_id)
        {
            teams.push(r.constructor.clone());
        }
    }
    let latest_team = entries.last().map(|(_, r)| r.constructor.clone());
    let recent = season_list
        .last()
        .and_then(|s| s.parse::<u32>().ok())
        .is_some_and(|y| y + 2 >= current_year());

    let mut eyebrow = Vec::new();
    if let Some(n) = &driver.permanent_number {
        eyebrow.push(format!("#{n}"));
    }
    if let Some(c) = &driver.code {
        eyebrow.push(c.clone());
    }
    if let Some(n) = &driver.nationality {
        eyebrow.push(n.clone());
    }

    let onchange = {
        let chosen = chosen.clone();
        Callback::from(move |e: Event| {
            chosen.set(Some(e.target_unchecked_into::<HtmlSelectElement>().value()))
        })
    };

    let season_standing = standing
        .done()
        .and_then(|d| d.standings())
        .and_then(|l| l.driver_standings.clone())
        .and_then(|v| v.into_iter().next());

    html! {
        <Layout title={driver.full_name()} tab={Tab::Standings}>
            <section class="card hero" style={latest_team.as_ref().map(|t| team_style(&t.constructor_id))}>
                <p class="eyebrow">{ eyebrow.join(" · ") }</p>
                <h2 class="hero-title">{ format!("{} {}", flag_nationality(driver.nationality.as_deref()), driver.full_name()) }</h2>
                if let Some(dob) = &driver.date_of_birth {
                    <p class="muted">
                        { tr!("Né le {}", "Born {}", format_birth(dob)) }
                        // L'API ne donne pas de date de décès : âge affiché pour les pilotes récents seulement.
                        if let Some(a) = age(dob).filter(|_| recent) { { tr!(" · {a} ans", " · {a} years old") } }
                    </p>
                }
                { stat_grid(vec![
                    (t("Départs", "Starts"), starts),
                    (t("Victoires", "Wins"), wins),
                    ("Podiums", podiums),
                    ("Poles", poles),
                    (t("Meilleurs tours", "Fastest laps"), fastest),
                    (t("Saisons", "Seasons"), season_list.len().to_string()),
                ]) }
                if let Some(url) = &driver.url {
                    <a class="link" href={url.clone()} target="_blank" rel="noopener">{ t("Wikipédia ↗", "Wikipedia ↗") }</a>
                }
            </section>

            if !teams.is_empty() {
                <section class="card">
                    <h2>{ t("Écuries", "Teams") }</h2>
                    <ol class="rows">
                        { for teams.iter().map(|c| html! {
                            <li class="row" style={team_style(&c.constructor_id)}>
                                <Link<Route> to={Route::team(&c.constructor_id)} classes="row-main">
                                    <span class="row-title">{ flag_nationality(c.nationality.as_deref()) }{ " " }{ &c.name }</span>
                                </Link<Route>>
                            </li>
                        }) }
                    </ol>
                </section>
            }

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
                    <ol class="rows">
                        { for entries.iter().rev().filter(|(race, _)| &race.season == season).map(|(race, r)| {
                            let outcome = r.outcome();
                            let mut sub = vec![r.constructor.name.clone(), tr!("départ P{}", "started P{}", r.grid.as_deref().unwrap_or("-"))];
                            if !outcome.is_empty() { sub.push(outcome); }
                            html! {
                                <li class="row" style={team_style(&r.constructor.constructor_id)}>
                                    <span class="pos pos-sm">{ &r.position_text }</span>
                                    <Link<Route> to={Route::race(&race.season, race.round_num())} classes="row-main">
                                        <span class="row-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</span>
                                        <span class="row-sub">{ sub.join(" · ") }</span>
                                    </Link<Route>>
                                    if r.points.parse::<f64>().unwrap_or(0.0) > 0.0 {
                                        <span class="pts">{ format!("+{}", r.points) }</span>
                                    }
                                </li>
                            }
                        }) }
                    </ol>
                    <Link<Route> to={Route::DriverStandings { season: season.clone() }} classes="btn btn-ghost">
                        { tr!("Classement {season}", "{season} standings") }
                    </Link<Route>>
                </section>
            }
        </Layout>
    }
}
