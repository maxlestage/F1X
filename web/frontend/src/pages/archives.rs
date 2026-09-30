use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::Route;
use crate::api::{all, f1, use_f1};
use crate::components::*;
use crate::models::translate_status;
use crate::util::{flag_country, flag_nationality, fold, format_birth};

/// Plateau des archives : toute l'histoire de la F1 depuis 1950.
#[function_component]
pub fn ArchivesPage() -> Html {
    let seasons = use_f1(f1("seasons.json", 1));
    let races = use_f1(f1("races.json", 1));
    let drivers = use_f1(f1("drivers.json", 1));
    let teams = use_f1(f1("constructors.json", 1));
    let circuits = use_f1(f1("circuits.json", 1));
    let status = use_f1(f1("status.json", 100));

    let mut statuses = status
        .done()
        .map(|d| d.statuses().to_vec())
        .unwrap_or_default();
    statuses.sort_by_key(|s| std::cmp::Reverse(s.count.parse::<u64>().unwrap_or(0)));
    let top: Vec<_> = statuses
        .iter()
        .filter(|s| s.status != "Finished" && !s.status.starts_with('+'))
        .take(8)
        .collect();
    let max = top
        .first()
        .and_then(|s| s.count.parse::<f64>().ok())
        .unwrap_or(1.0);

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
                { entry(Route::AllCircuits, "Circuits", "Où la F1 a couru", total_of(&circuits)) }
            </ol>

            if !top.is_empty() {
                <section class="card">
                    <h2>{ "Causes d'abandon" }</h2>
                    <p class="muted">{ "Toutes saisons confondues." }</p>
                    <ol class="rows">
                        { for top.iter().map(|s| {
                            let n: f64 = s.count.parse().unwrap_or(0.0);
                            html! {
                                <li class="row row-plain">
                                    <span class="row-main">
                                        <span class="row-title">{ translate_status(&s.status) }</span>
                                        <span class="bar" aria-hidden="true"><span class="bar-fill bar-red" style={format!("width:{:.1}%", n / max * 100.0)}></span></span>
                                    </span>
                                    <span class="pts">{ &s.count }</span>
                                </li>
                            }
                        }) }
                    </ol>
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

#[function_component]
pub fn AllCircuitsPage() -> Html {
    let circuits = use_f1(f1("circuits.json", 100));
    let body = fetch_view(&circuits, |d| {
        let mut list: Vec<_> = d.circuits().iter().collect();
        list.sort_by(|a, b| {
            a.location
                .country
                .cmp(&b.location.country)
                .then(a.circuit_name.cmp(&b.circuit_name))
        });
        html! {
            <ol class="rows rows-card">
                { for list.iter().map(|c| nav_row(
                    Route::circuit(&c.circuit_id),
                    html! { <>{ flag_country(&c.location.country) }{ " " }{ &c.circuit_name }</> },
                    format!("{}, {}", c.location.locality, c.location.country),
                    None,
                )) }
            </ol>
        }
    });
    html! { <Layout title="Circuits" tab={Tab::Archives}>{ body }</Layout> }
}
