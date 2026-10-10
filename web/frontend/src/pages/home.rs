use yew::prelude::*;
use yew_router::prelude::*;

use crate::api::{f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::tr;
use crate::util::{flag_country, now_ms, team_style};
use crate::{CURRENT, Route};

#[function_component]
pub fn Home() -> Html {
    let schedule = use_f1(f1("current.json", 100));
    let last = use_f1(f1("current/last/results.json", 100));
    let drivers = use_f1(f1("current/driverStandings.json", 100));
    let teams = use_f1(f1("current/constructorStandings.json", 100));

    let fav_driver = crate::util::fav_driver();
    let fav_team = crate::util::fav_team();
    let body = fetch_view(&schedule, |data| {
        let races = data.races();
        let now = now_ms();
        let season = races.first().map(|r| r.season.clone()).unwrap_or_default();
        let next = races.iter().find(|r| !r.is_over(now));
        let last_race = last.done().and_then(|d| d.race());
        let drivers = drivers
            .done()
            .and_then(|d| d.standings())
            .and_then(|l| l.driver_standings.clone())
            .unwrap_or_default();
        let teams = teams
            .done()
            .and_then(|d| d.standings())
            .and_then(|l| l.constructor_standings.clone())
            .unwrap_or_default();
        let fav_d = fav_driver
            .as_ref()
            .and_then(|id| drivers.iter().find(|s| &s.driver.driver_id == id));
        let fav_t = fav_team
            .as_ref()
            .and_then(|id| teams.iter().find(|s| &s.constructor.constructor_id == id));
        let leader = teams
            .first()
            .and_then(|t| t.points.parse().ok())
            .unwrap_or(0.0);
        html! {
            <>
                if let Some(race) = next {
                    <section class="card hero">
                        <p class="eyebrow">{ tr!("Prochain Grand Prix · Manche {}", "Next Grand Prix · Round {}", race.round) }</p>
                        <h2 class="hero-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</h2>
                        <p class="muted">
                            <Link<Route> to={Route::circuit(&race.circuit.circuit_id)} classes="link-inline">{ &race.circuit.circuit_name }</Link<Route>>
                            { format!(" — {}", race.circuit.location.locality) }
                        </p>
                        <Link<Route> to={Route::circuit(&race.circuit.circuit_id)} classes="outline-link">
                            <super::TrackOutline circuit_id={race.circuit.circuit_id.clone()} />
                        </Link<Route>>
                        <Countdown target_ms={race.start_ms()} />
                        <SessionsList race={race.clone()} />
                        <Link<Route> to={Route::race(CURRENT, race.round_num())} classes="btn">{ t("Voir le Grand Prix", "View the Grand Prix") }</Link<Route>>
                    </section>
                } else {
                    <section class="card">
                        <h2>{ tr!("Saison {season} terminée", "Season {season} is over") }</h2>
                        <p class="muted">{ t("Rendez-vous la saison prochaine !", "See you next season!") }</p>
                    </section>
                }

                // Ordre des qualifications (qualifs, sinon qualifs sprint) dès la fin de la séance.
                if let Some(race) = next.filter(|r| r.start_ms() - now < 4.0 * 86_400_000.0) {
                    <QualiOrder round={race.round_num()} />
                }

                if let Some(race) = next {
                    <super::WeekendWeather race={race.clone()} full={false} />
                }

                if let Some(race) = last_race {
                    <section class="card">
                        <div class="card-head">
                            <h2>{ t("Dernier résultat", "Latest result") }</h2>
                            <Link<Route> to={Route::race(CURRENT, race.round_num())} classes="link">{ t("Détails", "Details") }</Link<Route>>
                        </div>
                        <p class="muted">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</p>
                        <ol class="podium">
                            { for race.results.iter().flatten().take(3).map(|r| html! {
                                <li class="podium-step" style={team_style(&r.constructor.constructor_id)}>
                                    <Link<Route> to={Route::driver(&r.driver.driver_id)} classes="podium-link">
                                        <crate::photo::Avatar name={r.driver.full_name()} url={r.driver.url.clone().map(AttrValue::from)}
                                            colour={crate::util::team_color(&r.constructor.constructor_id)} size={52} />
                                        <span class="podium-pos">{ &r.position }</span>
                                        <span class="podium-name">{ &r.driver.family_name }</span>
                                        <span class="podium-team">{ &r.constructor.name }</span>
                                    </Link<Route>>
                                </li>
                            }) }
                        </ol>
                    </section>
                }

                if fav_d.is_some() || fav_t.is_some() {
                    <section class="card">
                        <h2>{ t("⭐ Mes favoris", "⭐ My favourites") }</h2>
                        <ol class="rows">
                            if let Some(d) = fav_d { { driver_standing_row(d) } }
                            if let Some(tm) = fav_t { { team_standing_row(tm, leader) } }
                        </ol>
                    </section>
                }

                if !drivers.is_empty() {
                    <section class="card">
                        <div class="card-head">
                            <h2>{ t("Pilotes", "Drivers") }</h2>
                            <Link<Route> to={Route::DriverStandings { season: CURRENT.into() }} classes="link">{ t("Tout voir", "See all") }</Link<Route>>
                        </div>
                        <ol class="rows">{ for drivers.iter().take(5).map(driver_standing_row) }</ol>
                    </section>
                }

                if !teams.is_empty() {
                    <section class="card">
                        <div class="card-head">
                            <h2>{ t("Écuries", "Teams") }</h2>
                            <Link<Route> to={Route::TeamStandings { season: CURRENT.into() }} classes="link">{ t("Tout voir", "See all") }</Link<Route>>
                        </div>
                        <ol class="rows">{ for teams.iter().take(3).map(|t| team_standing_row(t, leader)) }</ol>
                    </section>
                }

                <crate::pwa::InstallCard />

                <Link<Route> to={Route::Predict} classes="btn btn-ghost">{ t("🔮 Pronostiquer le prochain GP", "🔮 Predict the next GP") }</Link<Route>>
                <Link<Route> to={Route::Live} classes="btn">{ t("● Direct & replays en temps réel", "● Live & real-time replays") }</Link<Route>>
                <Link<Route> to={Route::Archives} classes="btn btn-ghost">{ t("Explorer 75 ans d'archives →", "Explore 75 years of history →") }</Link<Route>>
            </>
        }
    });

    html! { <Layout tab={Tab::Home}>{ body }</Layout> }
}

#[derive(Properties, PartialEq)]
struct QualiOrderProps {
    round: u32,
}

/// Ordre des qualifications du prochain Grand Prix : qualifs (grille du Grand Prix) et
/// qualifs sprint, publiés par le serveur dès la fin de la séance. Rien tant qu'il n'y en a pas.
#[function_component]
fn QualiOrder(p: &QualiOrderProps) -> Html {
    let data = use_f1(f1(format!("{CURRENT}/{}/qualifying.json", p.round), 100));
    // Onglet choisi (sinon : les qualifs si elles ont eu lieu, sinon les qualifs sprint).
    let sprint_tab = use_state(|| None::<bool>);
    let all = use_state(|| false);
    let race = data.done().and_then(|d| d.race());
    let main = race
        .and_then(|r| r.qualifying_results.clone())
        .unwrap_or_default();
    let sprint = race
        .and_then(|r| r.sprint_qualifying_results.clone())
        .unwrap_or_default();
    if main.is_empty() && sprint.is_empty() {
        return html! {};
    }
    let show_sprint = (*sprint_tab).unwrap_or(main.is_empty());
    let list = if show_sprint { &sprint } else { &main };
    let shown = if *all { list.len() } else { list.len().min(10) };
    let tab = |on: bool, label: &'static str| {
        let sprint_tab = sprint_tab.clone();
        html! {
            <button class={classes!("seg", (show_sprint == on).then_some("seg-active"))}
                    onclick={Callback::from(move |_| sprint_tab.set(Some(on)))}>{ label }</button>
        }
    };
    let toggle = {
        let all = all.clone();
        Callback::from(move |_| all.set(!*all))
    };
    html! {
        <section class="card">
            <div class="card-head">
                <h2>{ t("Ordre des qualifications", "Qualifying order") }</h2>
                <Link<Route> to={Route::race(CURRENT, p.round)} classes="link">{ t("Détails", "Details") }</Link<Route>>
            </div>
            if !main.is_empty() && !sprint.is_empty() {
                <div class="segmented">{ tab(false, t("Qualifications", "Qualifying")) }{ tab(true, t("Qualifs sprint", "Sprint quali")) }</div>
            } else {
                <p class="eyebrow">{ if show_sprint { t("Qualifs sprint", "Sprint qualifying") } else { t("Qualifications", "Qualifying") } }</p>
            }
            <ol class="rows">
                { for list.iter().take(shown).map(|q| if show_sprint { sprint_qualifying_row(q) } else { qualifying_row(q) }) }
            </ol>
            if list.len() > 10 {
                <button class="btn btn-ghost btn-small" onclick={toggle}>
                    { if *all { t("Voir le top 10", "Show top 10").to_string() } else { tr!("Voir les {} pilotes", "See all {} drivers", list.len()) } }
                </button>
            }
        </section>
    }
}
