use yew::prelude::*;
use yew_router::prelude::*;

use crate::api::{f1, use_f1};
use crate::components::*;
use crate::util::{flag_country, now_ms, team_style};
use crate::{CURRENT, Route};

#[function_component]
pub fn Home() -> Html {
    let schedule = use_f1(f1("current.json", 100));
    let last = use_f1(f1("current/last/results.json", 100));
    let drivers = use_f1(f1("current/driverStandings.json", 100));
    let teams = use_f1(f1("current/constructorStandings.json", 100));

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
        let leader = teams
            .first()
            .and_then(|t| t.points.parse().ok())
            .unwrap_or(0.0);
        html! {
            <>
                if let Some(race) = next {
                    <section class="card hero">
                        <p class="eyebrow">{ format!("Prochain Grand Prix · Manche {}", race.round) }</p>
                        <h2 class="hero-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</h2>
                        <p class="muted">
                            <Link<Route> to={Route::circuit(&race.circuit.circuit_id)} classes="link-inline">{ &race.circuit.circuit_name }</Link<Route>>
                            { format!(" — {}", race.circuit.location.locality) }
                        </p>
                        <Countdown target_ms={race.start_ms()} />
                        <SessionsList race={race.clone()} />
                        <Link<Route> to={Route::race(CURRENT, race.round_num())} classes="btn">{ "Voir le Grand Prix" }</Link<Route>>
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
                            <Link<Route> to={Route::race(CURRENT, race.round_num())} classes="link">{ "Détails" }</Link<Route>>
                        </div>
                        <p class="muted">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</p>
                        <ol class="podium">
                            { for race.results.iter().flatten().take(3).map(|r| html! {
                                <li class="podium-step" style={team_style(&r.constructor.constructor_id)}>
                                    <Link<Route> to={Route::driver(&r.driver.driver_id)} classes="podium-link">
                                        <span class="podium-pos">{ &r.position }</span>
                                        <span class="podium-name">{ &r.driver.family_name }</span>
                                        <span class="podium-team">{ &r.constructor.name }</span>
                                    </Link<Route>>
                                </li>
                            }) }
                        </ol>
                    </section>
                }

                if !drivers.is_empty() {
                    <section class="card">
                        <div class="card-head">
                            <h2>{ "Pilotes" }</h2>
                            <Link<Route> to={Route::DriverStandings { season: CURRENT.into() }} classes="link">{ "Tout voir" }</Link<Route>>
                        </div>
                        <ol class="rows">{ for drivers.iter().take(5).map(driver_standing_row) }</ol>
                    </section>
                }

                if !teams.is_empty() {
                    <section class="card">
                        <div class="card-head">
                            <h2>{ "Écuries" }</h2>
                            <Link<Route> to={Route::TeamStandings { season: CURRENT.into() }} classes="link">{ "Tout voir" }</Link<Route>>
                        </div>
                        <ol class="rows">{ for teams.iter().take(3).map(|t| team_standing_row(t, leader)) }</ol>
                    </section>
                }

                <Link<Route> to={Route::Archives} classes="btn btn-ghost">{ "Explorer 75 ans d'archives →" }</Link<Route>>
            </>
        }
    });

    html! { <Layout tab={Tab::Home}>{ body }</Layout> }
}
