use std::collections::HashMap;

use yew::prelude::*;
use yew_router::prelude::*;

use super::IdProps;
use crate::Route;
use crate::api::{all, f1, use_f1};
use crate::components::*;
use crate::util::{flag_country, team_style};

/// Fiche circuit : localisation, Grands Prix disputés et palmarès.
#[function_component]
pub fn CircuitPage(props: &IdProps) -> Html {
    let id = props.id.to_string();
    let races = use_f1(all(format!("circuits/{id}/races.json")));
    let winners = use_f1(f1(format!("circuits/{id}/results/1.json"), 100));

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
    let first = race_list.first().map(|r| r.season.clone());
    let last = race_list.last().map(|r| r.season.clone());

    // Recordmen des victoires sur ce circuit.
    let winner_rows = winners
        .done()
        .map(|d| d.races().to_vec())
        .unwrap_or_default();
    let mut tally: HashMap<String, (String, u32)> = HashMap::new();
    for r in winner_rows.iter().filter_map(|r| r.winner()) {
        let e = tally
            .entry(r.driver.driver_id.clone())
            .or_insert((r.driver.full_name(), 0));
        e.1 += 1;
    }
    let mut top: Vec<(String, String, u32)> = tally
        .into_iter()
        .map(|(id, (name, n))| (id, name, n))
        .collect();
    top.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));

    html! {
        <Layout title={circuit.circuit_name.clone()} tab={Tab::Archives}>
            <section class="card hero">
                <p class="eyebrow">{ format!("{}, {}", loc.locality, loc.country) }</p>
                <h2 class="hero-title">{ format!("{} {}", flag_country(&loc.country), circuit.circuit_name) }</h2>
                { stat_grid(vec![
                    ("Grands Prix", total_of(&races)),
                    ("Premier", first.unwrap_or_else(|| "–".into())),
                    ("Dernier", last.unwrap_or_else(|| "–".into())),
                ]) }
                if let Some(url) = map_url {
                    <a class="btn btn-ghost" href={url} target="_blank" rel="noopener">{ "Voir sur la carte ↗" }</a>
                }
                if let Some(url) = &circuit.url {
                    <a class="link" href={url.clone()} target="_blank" rel="noopener">{ "Wikipédia ↗" }</a>
                }
            </section>

            if !top.is_empty() {
                <section class="card">
                    <h2>{ "Rois du circuit" }</h2>
                    <ol class="rows">
                        { for top.iter().take(5).enumerate().map(|(i, (id, name, n))| html! {
                            <li class="row row-plain">
                                <span class="pos">{ i + 1 }</span>
                                <Link<Route> to={Route::driver(id)} classes="row-main">
                                    <span class="row-title">{ name }</span>
                                </Link<Route>>
                                <span class="pts">{ n }<small>{ if *n > 1 { " victoires" } else { " victoire" } }</small></span>
                            </li>
                        }) }
                    </ol>
                </section>
            }

            <section class="card">
                <h2>{ "Palmarès" }</h2>
                { fetch_view(&winners, |_| html! {
                    <ol class="rows">
                        { for winner_rows.iter().rev().filter_map(|race| {
                            let w = race.winner()?;
                            Some(html! {
                                <li class="row" style={team_style(&w.constructor.constructor_id)}>
                                    <span class="pos pos-sm">{ &race.season }</span>
                                    <Link<Route> to={Route::race(&race.season, race.round_num())} classes="row-main">
                                        <span class="row-title">{ w.driver.full_name() }</span>
                                        <span class="row-sub">{ format!("{} · {}", w.constructor.name, race.race_name) }</span>
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
