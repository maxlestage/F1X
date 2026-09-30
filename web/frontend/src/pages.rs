//! Les écrans de l'application.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::api::{Fetch, use_api};
use crate::components::*;
use crate::models::{ConstructorStanding, QualifyingResult, Race, RaceResult, StandingsList};
use crate::util::{flag_country, flag_nationality, local_date, now_ms, team_style, wins_label};

fn next_race(races: &[Race], now: f64) -> Option<&Race> {
    races.iter().find(|r| !r.is_over(now))
}

fn first_list(lists: &[StandingsList]) -> Option<&StandingsList> {
    lists.first()
}

// ---------- Accueil ----------

#[function_component]
pub fn Home() -> Html {
    let schedule = use_api::<Vec<Race>>("schedule".into());
    let last = use_api::<Vec<Race>>("last".into());
    let drivers = use_api::<Vec<StandingsList>>("drivers".into());
    let teams = use_api::<Vec<StandingsList>>("teams".into());

    let body = fetch_view(&schedule, |races| {
        let now = now_ms();
        let season = races.first().map(|r| r.season.clone()).unwrap_or_default();
        let last_race = last.done().and_then(|v| v.first());
        let drivers = drivers
            .done()
            .and_then(|l| first_list(l))
            .and_then(|l| l.driver_standings.clone())
            .unwrap_or_default();
        let teams = teams
            .done()
            .and_then(|l| first_list(l))
            .and_then(|l| l.constructor_standings.clone())
            .unwrap_or_default();
        html! {
            <>
                if let Some(race) = next_race(races, now) {
                    <section class="card hero">
                        <p class="eyebrow">{ format!("Prochain Grand Prix · Manche {}", race.round) }</p>
                        <h2 class="hero-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</h2>
                        <p class="muted">{ format!("{} — {}", race.circuit.circuit_name, race.circuit.location.locality) }</p>
                        <Countdown target_ms={race.start_ms()} />
                        <SessionsList race={race.clone()} />
                        <Link<Route> to={Route::Race { round: race.round_num() }} classes="btn">{ "Voir le Grand Prix" }</Link<Route>>
                    </section>
                } else {
                    <section class="card">
                        <h2>{ format!("Saison {season} terminée") }</h2>
                        <p class="muted">{ "Rendez-vous la saison prochaine !" }</p>
                    </section>
                }

                if let Some(race) = last_race {
                    <section class="card">
                        <div class="card-head">
                            <h2>{ "Dernier résultat" }</h2>
                            <Link<Route> to={Route::Race { round: race.round_num() }} classes="link">{ "Détails" }</Link<Route>>
                        </div>
                        <p class="muted">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</p>
                        <ol class="podium">
                            { for race.results.iter().flatten().take(3).map(|r| html! {
                                <li class="podium-step" style={team_style(&r.constructor.constructor_id)}>
                                    <span class="podium-pos">{ &r.position }</span>
                                    <span class="podium-name">{ &r.driver.family_name }</span>
                                    <span class="podium-team">{ &r.constructor.name }</span>
                                </li>
                            }) }
                        </ol>
                    </section>
                }

                if !drivers.is_empty() {
                    <section class="card">
                        <div class="card-head">
                            <h2>{ "Pilotes" }</h2>
                            <Link<Route> to={Route::Drivers} classes="link">{ "Tout voir" }</Link<Route>>
                        </div>
                        <ol class="rows">{ for drivers.iter().take(5).map(driver_standing_row) }</ol>
                    </section>
                }

                if !teams.is_empty() {
                    <section class="card">
                        <div class="card-head">
                            <h2>{ "Écuries" }</h2>
                            <Link<Route> to={Route::Teams} classes="link">{ "Tout voir" }</Link<Route>>
                        </div>
                        <ol class="rows">{ for teams.iter().take(3).map(team_standing_row) }</ol>
                    </section>
                }
            </>
        }
    });

    html! { <Layout tab={Tab::Home}>{ body }</Layout> }
}

// ---------- Calendrier ----------

#[function_component]
pub fn Calendar() -> Html {
    let schedule = use_api::<Vec<Race>>("schedule".into());
    let body = fetch_view(&schedule, |races| {
        let now = now_ms();
        let next_round = next_race(races, now).map(|r| r.round.clone());
        let season = races.first().map(|r| r.season.as_str()).unwrap_or("");
        html! {
            <>
                <p class="section-intro">{ format!("Saison {season} · {} Grands Prix", races.len()) }</p>
                <ol class="race-list">
                    { for races.iter().map(|race| {
                        let done = race.is_over(now);
                        let is_next = next_round.as_deref() == Some(race.round.as_str());
                        html! {
                            <li>
                                <Link<Route> to={Route::Race { round: race.round_num() }}
                                    classes={classes!("race-item", done.then_some("done"), is_next.then_some("next"))}>
                                    <span class="race-round">{ format!("R{}", race.round) }</span>
                                    <span class="race-main">
                                        <span class="race-name">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</span>
                                        <span class="race-meta">
                                            { local_date(&race.start_iso(), false) }
                                            if race.is_sprint_weekend() { { " · " }<span class="tag">{ "Sprint" }</span> }
                                        </span>
                                    </span>
                                    if is_next { <span class="badge badge-live">{ "Prochain" }</span> }
                                    else if done { <span class="badge">{ "Terminé" }</span> }
                                </Link<Route>>
                            </li>
                        }
                    }) }
                </ol>
            </>
        }
    });
    html! { <Layout title="Calendrier" tab={Tab::Calendar}>{ body }</Layout> }
}

// ---------- Grand Prix ----------

#[derive(Properties, PartialEq)]
pub struct RacePageProps {
    pub round: u32,
}

#[function_component]
pub fn RacePage(props: &RacePageProps) -> Html {
    let round = props.round;
    let schedule = use_api::<Vec<Race>>("schedule".into());
    let results = use_api::<Vec<Race>>(format!("race/{round}/results"));
    let sprint = use_api::<Vec<Race>>(format!("race/{round}/sprint"));
    let qualifying = use_api::<Vec<Race>>(format!("race/{round}/qualifying"));

    let Some(races) = schedule.done() else {
        return html! { <Layout title="Grand Prix" tab={Tab::Calendar}>{ fetch_view(&schedule, |_| html! {}) }</Layout> };
    };
    let Some(race) = races.iter().find(|r| r.round_num() == round) else {
        return html! { <NotFound /> };
    };

    let first = |f: &Fetch<Vec<Race>>| f.done().and_then(|v| v.first().cloned());
    let results: Vec<RaceResult> = first(&results).and_then(|r| r.results).unwrap_or_default();
    let sprint: Vec<RaceResult> = first(&sprint)
        .and_then(|r| r.sprint_results)
        .unwrap_or_default();
    let qualifying: Vec<QualifyingResult> = first(&qualifying)
        .and_then(|r| r.qualifying_results)
        .unwrap_or_default();
    let over = race.is_over(now_ms());
    let total = races.len() as u32;

    html! {
        <Layout title={race.race_name.clone()} tab={Tab::Calendar}>
            <section class="card hero">
                <p class="eyebrow">{ format!("Manche {} / {total}", race.round) }</p>
                <h2 class="hero-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</h2>
                <p class="muted">{ &race.circuit.circuit_name }</p>
                <p class="muted">{ format!("{}, {}", race.circuit.location.locality, race.circuit.location.country) }</p>
                if !over { <Countdown target_ms={race.start_ms()} /> }
            </section>

            <section class="card">
                <h2>{ "Programme" }</h2>
                <SessionsList race={race.clone()} />
            </section>

            if !results.is_empty() {
                <section class="card"><h2>{ "Course" }</h2><ol class="rows">{ for results.iter().map(result_row) }</ol></section>
            }
            if !sprint.is_empty() {
                <section class="card"><h2>{ "Sprint" }</h2><ol class="rows">{ for sprint.iter().map(result_row) }</ol></section>
            }
            if !qualifying.is_empty() {
                <section class="card"><h2>{ "Qualifications" }</h2><ol class="rows">{ for qualifying.iter().map(qualifying_row) }</ol></section>
            }
            if over && results.is_empty() {
                <section class="card"><p class="muted">{ "Les résultats ne sont pas encore disponibles." }</p></section>
            }

            <nav class="pager" aria-label="Grands Prix">
                if round > 1 {
                    <Link<Route> to={Route::Race { round: round - 1 }} classes="btn btn-ghost">{ "← Précédent" }</Link<Route>>
                } else { <span></span> }
                if round < total {
                    <Link<Route> to={Route::Race { round: round + 1 }} classes="btn btn-ghost">{ "Suivant →" }</Link<Route>>
                }
            </nav>
        </Layout>
    }
}

// ---------- Classements ----------

#[function_component]
pub fn Drivers() -> Html {
    let lists = use_api::<Vec<StandingsList>>("drivers".into());
    let body = fetch_view(&lists, |lists| {
        let standings = first_list(lists)
            .and_then(|l| l.driver_standings.clone())
            .unwrap_or_default();
        if standings.is_empty() {
            html! { <section class="card"><p class="muted">{ "Le classement n'est pas encore disponible." }</p></section> }
        } else {
            html! { <ol class="rows rows-card">{ for standings.iter().map(driver_standing_row) }</ol> }
        }
    });
    html! { <Layout title="Classement pilotes" tab={Tab::Drivers}>{ body }</Layout> }
}

#[function_component]
pub fn Teams() -> Html {
    let lists = use_api::<Vec<StandingsList>>("teams".into());
    let body = fetch_view(&lists, |lists| {
        let standings: Vec<ConstructorStanding> = first_list(lists)
            .and_then(|l| l.constructor_standings.clone())
            .unwrap_or_default();
        let leader: f64 = standings
            .first()
            .and_then(|s| s.points.parse().ok())
            .unwrap_or(0.0);
        if standings.is_empty() {
            return html! { <section class="card"><p class="muted">{ "Le classement n'est pas encore disponible." }</p></section> };
        }
        html! {
            <ol class="rows rows-card">
                { for standings.iter().map(|s| {
                    let pts: f64 = s.points.parse().unwrap_or(0.0);
                    let pct = if leader > 0.0 { (pts / leader * 100.0).clamp(0.0, 100.0) } else { 0.0 };
                    let wins = wins_label(&s.wins);
                    html! {
                        <li class="row" style={team_style(&s.constructor.constructor_id)}>
                            <span class="pos">{ s.position.clone().unwrap_or_else(|| s.position_text.clone()) }</span>
                            <span class="row-main">
                                <span class="row-title">{ &s.constructor.name }</span>
                                <span class="bar"><span class="bar-fill" style={format!("width:{pct:.1}%")}></span></span>
                                if !wins.is_empty() { <span class="row-sub">{ wins }</span> }
                            </span>
                            <span class="pts">{ &s.points }<small>{ " pts" }</small></span>
                        </li>
                    }
                }) }
            </ol>
        }
    });
    html! { <Layout title="Classement écuries" tab={Tab::Teams}>{ body }</Layout> }
}

// ---------- Fiche pilote ----------

#[derive(Properties, PartialEq)]
pub struct DriverPageProps {
    pub id: AttrValue,
}

#[function_component]
pub fn DriverPage(props: &DriverPageProps) -> Html {
    let lists = use_api::<Vec<StandingsList>>("drivers".into());
    let races = use_api::<Vec<Race>>(format!("driver/{}", props.id));

    let Some(races) = races.done() else {
        return html! { <Layout title="Pilote" tab={Tab::Drivers}>{ fetch_view(&races, |_| html! {}) }</Layout> };
    };
    let standing = lists
        .done()
        .and_then(|l| first_list(l))
        .and_then(|l| l.driver_standings.as_ref())
        .and_then(|v| {
            v.iter()
                .find(|s| s.driver.driver_id == props.id.as_str())
                .cloned()
        });
    let first_result = |r: &Race| r.results.as_ref().and_then(|v| v.first().cloned());
    let driver = standing
        .as_ref()
        .map(|s| s.driver.clone())
        .or_else(|| races.iter().find_map(first_result).map(|r| r.driver));
    let Some(driver) = driver else {
        return html! { <NotFound /> };
    };
    let team = standing
        .as_ref()
        .and_then(|s| s.constructors.last().cloned())
        .or_else(|| {
            races
                .iter()
                .rev()
                .find_map(first_result)
                .map(|r| r.constructor)
        });

    let mut eyebrow = Vec::new();
    if let Some(n) = &driver.permanent_number {
        eyebrow.push(format!("#{n}"));
    }
    if let Some(t) = &team {
        eyebrow.push(t.name.clone());
    }

    html! {
        <Layout title={driver.full_name()} tab={Tab::Drivers}>
            <section class="card hero" style={team.as_ref().map(|t| team_style(&t.constructor_id))}>
                <p class="eyebrow">{ eyebrow.join(" · ") }</p>
                <h2 class="hero-title">{ format!("{} {}", flag_nationality(driver.nationality.as_deref()), driver.full_name()) }</h2>
                if let Some(s) = &standing {
                    <dl class="stats">
                        <div><dt>{ "Position" }</dt><dd>{ s.position.clone().unwrap_or_else(|| s.position_text.clone()) }</dd></div>
                        <div><dt>{ "Points" }</dt><dd>{ &s.points }</dd></div>
                        <div><dt>{ "Victoires" }</dt><dd>{ &s.wins }</dd></div>
                    </dl>
                }
            </section>
            if !races.is_empty() {
                <section class="card">
                    <h2>{ "Saison" }</h2>
                    <ol class="rows">
                        { for races.iter().rev().filter_map(|race| {
                            let r = first_result(race)?;
                            let outcome = r.outcome();
                            let sub = if outcome.is_empty() {
                                format!("Départ P{}", r.grid.as_deref().unwrap_or("-"))
                            } else {
                                format!("Départ P{} · {outcome}", r.grid.as_deref().unwrap_or("-"))
                            };
                            Some(html! {
                                <li class="row" style={team_style(&r.constructor.constructor_id)}>
                                    <span class="pos pos-sm">{ &r.position_text }</span>
                                    <Link<Route> to={Route::Race { round: race.round_num() }} classes="row-main">
                                        <span class="row-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</span>
                                        <span class="row-sub">{ sub }</span>
                                    </Link<Route>>
                                    <span class="pts">{ format!("+{}", r.points) }</span>
                                </li>
                            })
                        }) }
                    </ol>
                </section>
            }
        </Layout>
    }
}

// ---------- 404 ----------

#[function_component]
pub fn NotFound() -> Html {
    html! {
        <Layout title="Page introuvable">
            <section class="card">
                <h2>{ "Hors piste" }</h2>
                <p class="muted">{ "Cette page n'existe pas." }</p>
                <Link<Route> to={Route::Home} classes="btn">{ "Retour au stand" }</Link<Route>>
            </section>
        </Layout>
    }
}
