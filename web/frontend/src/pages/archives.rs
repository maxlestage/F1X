use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::Route;
use crate::api::{all, f1, use_f1};
use crate::components::*;
use crate::models::{Circuit, Race, is_classified, translate_status};
use crate::util::{country_fr, flag_country, flag_nationality, fold, format_birth};

/// Plateau des archives : toute l'histoire de la F1 depuis 1950.
#[function_component]
pub fn ArchivesPage() -> Html {
    let seasons = use_f1(f1("seasons.json", 1));
    let races = use_f1(f1("races.json", 1));
    let drivers = use_f1(f1("drivers.json", 1));
    let teams = use_f1(f1("constructors.json", 1));
    let circuits = use_f1(f1("circuits.json", 1));
    let status = use_f1(all("status.json"));

    // Causes d'abandon : statuts non classés, regroupés par libellé traduit.
    let mut causes: Vec<(String, u64)> = Vec::new();
    for st in status.done().map(|d| d.statuses()).unwrap_or_default() {
        if is_classified(&st.status) {
            continue;
        }
        let label = translate_status(&st.status);
        let n = st.count.parse::<u64>().unwrap_or(0);
        match causes.iter_mut().find(|(l, _)| *l == label) {
            Some(entry) => entry.1 += n,
            None => causes.push((label, n)),
        }
    }
    causes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let max = causes.first().map(|c| c.1 as f64).unwrap_or(1.0);
    let total_causes: u64 = causes.iter().map(|c| c.1).sum();
    let cause_row = |(label, n): &(String, u64)| {
        html! {
            <li class="row row-plain">
                <span class="row-main">
                    <span class="row-title">{ label }</span>
                    <span class="bar" aria-hidden="true"><span class="bar-fill bar-red" style={format!("width:{:.1}%", *n as f64 / max * 100.0)}></span></span>
                </span>
                <span class="pts">{ n }</span>
            </li>
        }
    };

    let entry = |route: Route, title: &str, sub: &str, count: String| {
        nav_row(
            route,
            html! { <strong>{ title.to_string() }</strong> },
            sub.to_string(),
            Some(count),
        )
    };

    html! {
        <Layout title="Archives" tab={Tab::Archives}>
            <section class="card hero">
                <p class="eyebrow">{ "Depuis 1950" }</p>
                <h2 class="hero-title">{ "Toute l'histoire de la F1" }</h2>
                { stat_grid(vec![
                    ("Saisons", total_of(&seasons)),
                    ("Grands Prix", total_of(&races)),
                    ("Pilotes", total_of(&drivers)),
                ]) }
                <SeasonSelect season="" target={SeasonTarget::Calendar} />
            </section>

            <ol class="rows rows-card">
                { entry(Route::AllSeasons, "Saisons", "Calendriers, vainqueurs et classements", total_of(&seasons)) }
                { entry(Route::AllDrivers, "Pilotes", "Tous les pilotes, avec recherche", total_of(&drivers)) }
                { entry(Route::AllTeams, "Écuries", "Tous les constructeurs", total_of(&teams)) }
                { entry(Route::AllCircuits, "Circuits", "Tous les circuits, GP disputés, recherche et tri", total_of(&circuits)) }
            </ol>

            if !causes.is_empty() {
                <section class="card">
                    <h2>{ "Causes d'abandon" }</h2>
                    <p class="muted">{ format!("{total_causes} abandons, disqualifications et non-partants depuis 1950, en {} causes.", causes.len()) }</p>
                    <ol class="rows">{ for causes.iter().take(10).map(cause_row) }</ol>
                    if causes.len() > 10 {
                        <details class="details">
                            <summary>{ format!("Voir les {} autres causes", causes.len() - 10) }</summary>
                            <ol class="rows">{ for causes.iter().skip(10).map(cause_row) }</ol>
                        </details>
                    }
                </section>
            }
        </Layout>
    }
}

#[function_component]
pub fn AllSeasonsPage() -> Html {
    let seasons = use_f1(f1("seasons.json", 100));
    let body = fetch_view(&seasons, |d| {
        html! {
            <ol class="rows rows-card">
                { for d.seasons().iter().rev().map(|s| nav_row(
                    Route::season(&s.season),
                    html! { <strong>{ &s.season }</strong> },
                    String::new(),
                    Some("→".into()),
                )) }
            </ol>
        }
    });
    html! { <Layout title="Saisons" tab={Tab::Archives}>{ body }</Layout> }
}

#[derive(Properties, PartialEq)]
struct SearchProps {
    value: AttrValue,
    oninput: Callback<String>,
    placeholder: AttrValue,
}

#[function_component]
fn SearchBox(props: &SearchProps) -> Html {
    let cb = props.oninput.clone();
    let oninput = Callback::from(move |e: InputEvent| {
        cb.emit(e.target_unchecked_into::<HtmlInputElement>().value())
    });
    html! {
        <input class="search" type="search" value={props.value.clone()} {oninput}
               placeholder={props.placeholder.clone()} aria-label={props.placeholder.clone()} autocomplete="off" />
    }
}

const MAX_ROWS: usize = 80;

#[function_component]
pub fn AllDriversPage() -> Html {
    let drivers = use_f1(all("drivers.json"));
    let query = use_state(String::new);
    let oninput = {
        let query = query.clone();
        Callback::from(move |v: String| query.set(v))
    };
    let q = fold(&query);
    let body = fetch_view(&drivers, |d| {
        let mut list: Vec<_> = d
            .drivers()
            .iter()
            .filter(|drv| {
                q.is_empty()
                    || fold(&drv.full_name()).contains(&q)
                    || drv.code.as_deref().is_some_and(|c| fold(c) == q)
            })
            .collect();
        list.sort_by_cached_key(|a| fold(&a.family_name));
        let count = list.len();
        html! {
            <>
                <p class="section-intro">{ format!("{count} pilote{}", if count > 1 { "s" } else { "" }) }</p>
                <ol class="rows rows-card">
                    { for list.iter().take(MAX_ROWS).map(|drv| nav_row(
                        Route::driver(&drv.driver_id),
                        html! { <>{ flag_nationality(drv.nationality.as_deref()) }{ " " }{ &drv.given_name }{ " " }<strong>{ &drv.family_name }</strong></> },
                        drv.date_of_birth.as_deref().map(|d| format!("Né le {}", format_birth(d))).unwrap_or_default(),
                        None,
                    )) }
                </ol>
                if count > MAX_ROWS { <p class="muted">{ format!("… et {} autres : affine ta recherche.", count - MAX_ROWS) }</p> }
            </>
        }
    });
    html! {
        <Layout title="Tous les pilotes" tab={Tab::Archives}>
            <SearchBox value={(*query).clone()} {oninput} placeholder="Rechercher un pilote (nom, code…)" />
            { body }
        </Layout>
    }
}

#[function_component]
pub fn AllTeamsPage() -> Html {
    let teams = use_f1(all("constructors.json"));
    let query = use_state(String::new);
    let oninput = {
        let query = query.clone();
        Callback::from(move |v: String| query.set(v))
    };
    let q = fold(&query);
    let body = fetch_view(&teams, |d| {
        let mut list: Vec<_> = d
            .constructors()
            .iter()
            .filter(|c| q.is_empty() || fold(&c.name).contains(&q))
            .collect();
        list.sort_by_cached_key(|a| fold(&a.name));
        let count = list.len();
        html! {
            <>
                <p class="section-intro">{ format!("{count} écurie{}", if count > 1 { "s" } else { "" }) }</p>
                <ol class="rows rows-card">
                    { for list.iter().take(MAX_ROWS).map(|c| nav_row(
                        Route::team(&c.constructor_id),
                        html! { <>{ flag_nationality(c.nationality.as_deref()) }{ " " }{ &c.name }</> },
                        c.nationality.clone().unwrap_or_default(),
                        None,
                    )) }
                </ol>
                if count > MAX_ROWS { <p class="muted">{ format!("… et {} autres : affine ta recherche.", count - MAX_ROWS) }</p> }
            </>
        }
    });
    html! {
        <Layout title="Toutes les écuries" tab={Tab::Archives}>
            <SearchBox value={(*query).clone()} {oninput} placeholder="Rechercher une écurie" />
            { body }
        </Layout>
    }
}

#[derive(Clone, Copy, PartialEq)]
enum CircuitSort {
    Races,
    Country,
    Recent,
}

struct CircuitStats {
    circuit: Circuit,
    races: u32,
    first: String,
    last: String,
    on_calendar: bool,
}

/// Tous les circuits : Grands Prix disputés, années d'utilisation, présence au calendrier.
#[function_component]
pub fn AllCircuitsPage() -> Html {
    let circuits = use_f1(f1("circuits.json", 100));
    // Tous les Grands Prix depuis 1950 (pagination côté serveur) → stats par circuit.
    let races = use_f1(all("races.json"));
    let current = use_f1(f1("current.json", 100));
    let query = use_state(String::new);
    let sort = use_state(|| CircuitSort::Races);
    let oninput = {
        let query = query.clone();
        Callback::from(move |v: String| query.set(v))
    };
    let q = fold(&query);

    let body = fetch_view(&circuits, |d| {
        let all_races = races.done().map(|r| r.races()).unwrap_or_default();
        let calendar: Vec<&str> = current
            .done()
            .map(|c| {
                c.races()
                    .iter()
                    .map(|r| r.circuit.circuit_id.as_str())
                    .collect()
            })
            .unwrap_or_default();
        let mut list: Vec<CircuitStats> = d
            .circuits()
            .iter()
            .map(|c| {
                let held: Vec<&Race> = all_races
                    .iter()
                    .filter(|r| r.circuit.circuit_id == c.circuit_id)
                    .collect();
                CircuitStats {
                    circuit: c.clone(),
                    races: held.len() as u32,
                    first: held.first().map(|r| r.season.clone()).unwrap_or_default(),
                    last: held.last().map(|r| r.season.clone()).unwrap_or_default(),
                    on_calendar: calendar.contains(&c.circuit_id.as_str()),
                }
            })
            .filter(|c| {
                let loc = &c.circuit.location;
                q.is_empty()
                    || fold(&c.circuit.circuit_name).contains(&q)
                    || fold(&loc.locality).contains(&q)
                    || fold(&loc.country).contains(&q)
                    || fold(country_fr(&loc.country)).contains(&q)
            })
            .collect();
        match *sort {
            CircuitSort::Races => list.sort_by(|a, b| {
                b.races
                    .cmp(&a.races)
                    .then(a.circuit.circuit_name.cmp(&b.circuit.circuit_name))
            }),
            CircuitSort::Country => list.sort_by(|a, b| {
                country_fr(&a.circuit.location.country)
                    .cmp(country_fr(&b.circuit.location.country))
                    .then(a.circuit.circuit_name.cmp(&b.circuit.circuit_name))
            }),
            CircuitSort::Recent => {
                list.sort_by(|a, b| b.last.cmp(&a.last).then(b.races.cmp(&a.races)))
            }
        }
        let countries = {
            let mut c: Vec<&str> = d
                .circuits()
                .iter()
                .map(|c| c.location.country.as_str())
                .collect();
            c.sort_unstable();
            c.dedup();
            c.len()
        };
        let on_calendar = d
            .circuits()
            .iter()
            .filter(|c| calendar.contains(&c.circuit_id.as_str()))
            .count();
        html! {
            <>
                { stat_grid(vec![
                    ("Circuits", d.circuits().len().to_string()),
                    ("Pays", countries.to_string()),
                    ("Au calendrier", if current.done().is_some() { on_calendar.to_string() } else { "–".into() }),
                ]) }
                <p class="section-intro">{ format!("{} circuit{}", list.len(), if list.len() > 1 { "s" } else { "" }) }</p>
                <ol class="rows rows-card">
                    { for list.iter().map(|c| {
                        let loc = &c.circuit.location;
                        let years = match (c.first.as_str(), c.last.as_str()) {
                            ("", _) => String::new(),
                            (f, l) if f == l => format!(" · {f}"),
                            (f, l) => format!(" · {f}–{l}"),
                        };
                        nav_row(
                            Route::circuit(&c.circuit.circuit_id),
                            html! {
                                <>
                                    { flag_country(&loc.country) }{ " " }{ &c.circuit.circuit_name }
                                    if c.on_calendar { { " " }<span class="tag">{ "Au calendrier" }</span> }
                                </>
                            },
                            format!("{}, {}{years}", loc.locality, country_fr(&loc.country)),
                            races.done().map(|_| format!("{} GP", c.races)),
                        )
                    }) }
                </ol>
            </>
        }
    });

    let sort_btn = |value: CircuitSort, label: &'static str| {
        let sort = sort.clone();
        let active = *sort == value;
        html! {
            <button class={classes!("seg", active.then_some("seg-active"))} onclick={move |_| sort.set(value)}>{ label }</button>
        }
    };

    html! {
        <Layout title="Circuits" tab={Tab::Archives}>
            <SearchBox value={(*query).clone()} {oninput} placeholder="Rechercher (circuit, ville, pays…)" />
            <div class="segmented segmented-3">
                { sort_btn(CircuitSort::Races, "Plus de GP") }
                { sort_btn(CircuitSort::Recent, "Récents") }
                { sort_btn(CircuitSort::Country, "Pays") }
            </div>
            { body }
        </Layout>
    }
}
