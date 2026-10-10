use std::collections::HashMap;
use std::rc::Rc;

use active::prelude::*;

use crate::api::{Fetch, all, f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Circuit, Constructor, Driver, is_classified, translate_status};
use crate::util::{country_fr, flag_country, flag_nationality, fold, format_birth};
use crate::{Route, link, tr};

/// Plateau des archives : toute l'histoire de la F1 depuis 1950.
pub fn archives_page() -> Node {
    let seasons = use_f1(f1("seasons.json", 1));
    let races = use_f1(f1("races.json", 1));
    let drivers = use_f1(f1("drivers.json", 1));
    let teams = use_f1(f1("constructors.json", 1));
    let circuits = use_f1(f1("circuits.json", 1));
    let status = use_f1(all("status.json"));

    // Chaque compteur suit sa requête : « – » est remplacé sur place à l'arrivée.
    let total = |fetch: State<Fetch>| -> CountFn { Rc::new(move || fetch.with(total_of)) };
    let arrow = |route: Route, title: &'static str, sub: &'static str| {
        nav_row(
            route,
            strong().text(title),
            sub.to_string(),
            Some("→".into()),
        )
    };

    let hero = section()
        .class("card hero")
        .child(p().class("eyebrow").text(t("Depuis 1950", "Since 1950")))
        .child(
            h2().class("hero-title")
                .text(t("Toute l'histoire de la F1", "The whole history of F1")),
        )
        .child(stat_grid_dyn(vec![
            (t("Saisons", "Seasons"), total(seasons)),
            ("Grands Prix", total(races)),
            (t("Pilotes", "Drivers"), total(drivers)),
        ]))
        .child(season_select(String::new, || SeasonTarget::Calendar, None));

    let presentation = if crate::i18n::is_fr() {
        "/presentation?lang=fr"
    } else {
        "/presentation?lang=en"
    };
    let tools = ol()
        .class("rows rows-card")
        // Page servie par le serveur (hors application) : lien classique.
        .child(
            li().class("row row-plain")
                .child(
                    server_link(presentation)
                        .class("row-main")
                        .child(
                            span()
                                .class("row-title")
                                .child(strong().text(t("ℹ️ Présentation de F1X", "ℹ️ About F1X"))),
                        )
                        .child(span().class("row-sub").text(t(
                            "Le site de présentation, à partager",
                            "The presentation site, to share",
                        ))),
                )
                .child(span().class("pts").text("→")),
        )
        .child(arrow(
            Route::News,
            t("📰 Actualités", "📰 News"),
            t("Les derniers titres de la presse F1", "Latest F1 headlines"),
        ))
        .child(arrow(
            Route::Records,
            t("🏅 Records", "🏅 Records"),
            t(
                "Titres, victoires, poles, séries, âges…",
                "Titles, wins, poles, streaks, ages…",
            ),
        ))
        .child(arrow(
            Route::Compare,
            t("⚖️ Comparateur", "⚖️ Compare"),
            t("Deux pilotes face à face", "Two drivers head to head"),
        ))
        .child(arrow(
            Route::Predict,
            t("🔮 Pronostics", "🔮 Predictions"),
            t(
                "Pronostique chaque Grand Prix, gagne des points",
                "Predict every Grand Prix, score points",
            ),
        ))
        .child(arrow(
            Route::Fantasy,
            t("🏎️ Fantasy F1", "🏎️ Fantasy F1"),
            t(
                "Ton équipe, 100 M€, points réels",
                "Your team, €100M, real points",
            ),
        ))
        .child(arrow(
            Route::Quiz,
            t("🧩 Devine le pilote", "🧩 Guess the driver"),
            t(
                "Quiz sur 75 ans de statistiques",
                "Quiz on 75 years of stats",
            ),
        ))
        .child(arrow(
            Route::Glossary,
            t("📚 Lexique", "📚 Glossary"),
            t(
                "Drapeaux, pneus, stratégie, règlement…",
                "Flags, tyres, strategy, rules…",
            ),
        ));

    let archive = ol()
        .class("rows rows-card")
        .child(arrow(
            Route::Data,
            t("📊 Données OpenF1", "📊 OpenF1 data"),
            t(
                "Chaque séance depuis 2023 : télémétrie, pneus, écarts, radios…",
                "Every session since 2023: telemetry, tyres, gaps, radio…",
            ),
        ))
        .child(counted_row(
            Route::AllSeasons,
            t("Saisons", "Seasons"),
            t(
                "Calendriers, vainqueurs et classements",
                "Calendars, winners and standings",
            ),
            total(seasons),
        ))
        .child(counted_row(
            Route::AllDrivers,
            t("Pilotes", "Drivers"),
            t(
                "Tous les pilotes, avec recherche",
                "Every driver, searchable",
            ),
            total(drivers),
        ))
        .child(counted_row(
            Route::AllTeams,
            t("Écuries", "Teams"),
            t("Tous les constructeurs", "Every constructor"),
            total(teams),
        ))
        .child(counted_row(
            Route::AllCircuits,
            "Circuits",
            t(
                "Tous les circuits, GP disputés, recherche et tri",
                "Every circuit, races held, search and sort",
            ),
            total(circuits),
        ));

    layout(
        t("Explorer", "Explore"),
        Some(Tab::Archives),
        fragment([
            Node::from(hero),
            h2().class("section-title")
                .text(t("Outils et jeux", "Tools and games"))
                .into(),
            tools.into(),
            h2().class("section-title")
                .text(t("Archives", "Archive"))
                .into(),
            archive.into(),
            dynamic(move || status.with(causes_card)),
        ]),
    )
}

/// Valeur d'une case ou d'un compteur, relue quand les états qu'elle lit changent.
type CountFn = Rc<dyn Fn() -> String>;

/// Comme [`stat_grid`], avec des valeurs qui suivent des états : « – » pendant le chargement,
/// remplacé sur place à l'arrivée (le chiffre compte alors depuis 0).
fn stat_grid_dyn(items: Vec<(&'static str, CountFn)>) -> Node {
    dl().class("stats")
        .children(items.into_iter().map(|(label, value)| {
            // Valeurs longues (temps au tour, unités) : police adaptée à la largeur de la case.
            let long = value.clone();
            div().child(dt().text(label)).child(
                dd().class_if("dd-long", move || long().chars().count() > 5)
                    .text_dyn(move || value()),
            )
        }))
        .into()
}

/// Comme [`nav_row`], avec un compteur à droite qui suit sa requête.
fn counted_row(route: Route, title: &'static str, sub: &'static str, count: CountFn) -> Node {
    li().class("row row-plain")
        .child(
            link(route, "row-main")
                .child(span().class("row-title").child(strong().text(title)))
                .child(span().class("row-sub").text(sub)),
        )
        .child(span().class("pts").text_dyn(move || count()))
        .into()
}

/// Causes d'abandon : statuts non classés, regroupés par libellé traduit (rien tant qu'ils ne
/// sont pas chargés).
fn causes_card(status: &Fetch) -> Node {
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
    if causes.is_empty() {
        return Node::Empty;
    }
    causes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let max = causes.first().map(|c| c.1 as f64).unwrap_or(1.0);
    let total_causes: u64 = causes.iter().map(|c| c.1).sum();
    let cause_row = |(label, n): &(String, u64)| {
        li().class("row row-plain")
            .child(
                span()
                    .class("row-main")
                    .child(span().class("row-title").text(label.clone()))
                    .child(
                        span().class("bar").attr("aria-hidden", "true").child(
                            span()
                                .class("bar-fill bar-red")
                                .style(format!("width:{:.1}%", *n as f64 / max * 100.0)),
                        ),
                    ),
            )
            .child(span().class("pts").text(n.to_string()))
    };
    section()
        .class("card")
        .child(h2().text(t("Causes d'abandon", "Causes of retirement")))
        .child(p().class("muted").text(tr!(
            "{total_causes} abandons, disqualifications et non-partants depuis 1950, en {} causes.",
            "{total_causes} retirements, disqualifications and non-starters since 1950, across {} causes.",
            causes.len()
        )))
        .child(ol().class("rows").children(causes.iter().take(10).map(cause_row)))
        .child((causes.len() > 10).then(|| {
            details()
                .class("details")
                .child(summary().text(tr!(
                    "Voir les {} autres causes",
                    "Show the other {} causes",
                    causes.len() - 10
                )))
                .child(ol().class("rows").children(causes.iter().skip(10).map(cause_row)))
        }))
        .into()
}

pub fn all_seasons_page() -> Node {
    let seasons = use_f1(f1("seasons.json", 100));
    let body = fetch_view(seasons, |d| {
        ol().class("rows rows-card")
            .children(d.seasons().iter().rev().map(|s| {
                nav_row(
                    Route::season(&s.season),
                    strong().text(s.season.clone()),
                    String::new(),
                    Some("→".into()),
                )
            }))
            .into()
    });
    layout(t("Saisons", "Seasons"), Some(Tab::Archives), body)
}

/// Champ de recherche. Il reste en dehors des parties qui affichent les résultats : il n'est
/// jamais reconstruit et garde le focus pendant la frappe.
fn search_box(query: State<String>, placeholder: &'static str) -> Node {
    input()
        .class("search")
        .attr("type", "search")
        .attr_dyn("value", move || query.get())
        .on_input(move |e| query.set(e.value()))
        .attr("placeholder", placeholder)
        .attr("aria-label", placeholder)
        .attr("autocomplete", "off")
        .into()
}

const MAX_ROWS: usize = 80;

/// Résultats d'une recherche : le nombre trouvé, les [`MAX_ROWS`] premières lignes et le reste
/// à affiner. `found` : positions des éléments retenus dans la liste complète. Les lignes sont
/// gardées d'une frappe à l'autre : seules celles qui apparaissent sont créées.
fn search_results(
    found: State<Vec<usize>>,
    count: fn(usize) -> String,
    row: impl Fn(usize) -> Node + 'static,
) -> Node {
    let more = memo(move || found.with(Vec::len).saturating_sub(MAX_ROWS));
    let has_more = memo(move || more.get() > 0);
    fragment([
        Node::from(
            p().class("section-intro")
                .text_dyn(move || count(found.with(Vec::len))),
        ),
        ol().class("rows rows-card")
            .children_keyed(
                move || found.with(|f| f.iter().take(MAX_ROWS).copied().collect()),
                |i| *i,
                move |i| row(*i),
            )
            .into(),
        dynamic(move || {
            if !has_more.get() {
                return Node::Empty;
            }
            p().class("muted")
                .text_dyn(move || {
                    tr!(
                        "… et {} autres : affine ta recherche.",
                        "… and {} more: refine your search.",
                        more.get()
                    )
                })
                .into()
        }),
    ])
}

pub fn all_drivers_page() -> Node {
    let drivers = use_f1(all("drivers.json"));
    let query = use_state(String::new());
    let body = fetch_view(drivers, move |d| {
        // Triés une fois par nom de famille, avec leurs clés de recherche : la frappe ne fait
        // que filtrer.
        let mut list: Vec<(String, Option<String>, Driver)> = d
            .drivers()
            .iter()
            .map(|drv| {
                (
                    fold(&drv.full_name()),
                    drv.code.as_deref().map(fold),
                    drv.clone(),
                )
            })
            .collect();
        list.sort_by_cached_key(|(_, _, a)| fold(&a.family_name));
        let list = Rc::new(list);
        let found = {
            let list = list.clone();
            memo(move || {
                let q = fold(&query.get());
                list.iter()
                    .enumerate()
                    .filter(|(_, (name, code, _))| {
                        q.is_empty() || name.contains(&q) || code.as_deref() == Some(q.as_str())
                    })
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>()
            })
        };
        search_results(
            found,
            |count| {
                tr!(
                    "{count} pilote{}",
                    "{count} driver{}",
                    if count > 1 { "s" } else { "" }
                )
            },
            move |i| {
                let drv = &list[i].2;
                nav_row(
                    Route::driver(&drv.driver_id),
                    fragment([
                        Node::from(format!(
                            "{} {} ",
                            flag_nationality(drv.nationality.as_deref()),
                            drv.given_name
                        )),
                        strong().text(drv.family_name.clone()).into(),
                    ]),
                    drv.date_of_birth
                        .as_deref()
                        .map(|d| tr!("Né le {}", "Born {}", format_birth(d)))
                        .unwrap_or_default(),
                    None,
                )
            },
        )
    });
    layout(
        t("Tous les pilotes", "All drivers"),
        Some(Tab::Archives),
        fragment([
            search_box(
                query,
                t(
                    "Rechercher un pilote (nom, code…)",
                    "Search a driver (name, code…)",
                ),
            ),
            body,
        ]),
    )
}

pub fn all_teams_page() -> Node {
    let teams = use_f1(all("constructors.json"));
    let query = use_state(String::new());
    let body = fetch_view(teams, move |d| {
        // Triées une fois par nom, avec leur clé de recherche.
        let mut list: Vec<(String, Constructor)> = d
            .constructors()
            .iter()
            .map(|c| (fold(&c.name), c.clone()))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        let list = Rc::new(list);
        let found = {
            let list = list.clone();
            memo(move || {
                let q = fold(&query.get());
                list.iter()
                    .enumerate()
                    .filter(|(_, (name, _))| q.is_empty() || name.contains(&q))
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>()
            })
        };
        search_results(
            found,
            |count| {
                tr!(
                    "{count} écurie{}",
                    "{count} team{}",
                    if count > 1 { "s" } else { "" }
                )
            },
            move |i| {
                let c = &list[i].1;
                nav_row(
                    Route::team(&c.constructor_id),
                    format!("{} {}", flag_nationality(c.nationality.as_deref()), c.name),
                    c.nationality.clone().unwrap_or_default(),
                    None,
                )
            },
        )
    });
    layout(
        t("Toutes les écuries", "All teams"),
        Some(Tab::Archives),
        fragment([
            search_box(query, t("Rechercher une écurie", "Search a team")),
            body,
        ]),
    )
}

#[derive(Clone, Copy, PartialEq)]
enum CircuitSort {
    Races,
    Country,
    Recent,
}

#[derive(Clone, PartialEq)]
struct CircuitStats {
    circuit: Circuit,
    /// Grands Prix disputés (`None` tant que l'historique n'est pas chargé).
    races: Option<u32>,
    first: String,
    last: String,
    on_calendar: bool,
}

/// Statistiques de chaque circuit à partir des réponses arrivées (vide sans la liste).
fn circuit_stats(
    circuits: State<Fetch>,
    races: State<Fetch>,
    current: State<Fetch>,
) -> Vec<CircuitStats> {
    let Some(circuits) = circuits.with(Fetch::data) else {
        return Vec::new();
    };
    let races = races.with(Fetch::data);
    let current = current.with(Fetch::data);
    let calendar: Vec<&str> = current
        .as_ref()
        .map(|c| {
            c.races()
                .iter()
                .map(|r| r.circuit.circuit_id.as_str())
                .collect()
        })
        .unwrap_or_default();
    // Grands Prix de chaque circuit (courses dans l'ordre) : nombre, première et dernière saison.
    let mut held: HashMap<&str, (u32, &str, &str)> = HashMap::new();
    for r in races.as_ref().map(|d| d.races()).unwrap_or_default() {
        let entry = held
            .entry(r.circuit.circuit_id.as_str())
            .or_insert((0, &r.season, &r.season));
        entry.0 += 1;
        entry.2 = &r.season;
    }
    circuits
        .circuits()
        .iter()
        .map(|c| {
            let (n, first, last) = held
                .get(c.circuit_id.as_str())
                .copied()
                .unwrap_or((0, "", ""));
            CircuitStats {
                circuit: c.clone(),
                races: races.is_some().then_some(n),
                first: first.to_string(),
                last: last.to_string(),
                on_calendar: calendar.contains(&c.circuit_id.as_str()),
            }
        })
        .collect()
}

fn circuit_row(c: &CircuitStats) -> Node {
    let loc = &c.circuit.location;
    let years = match (c.first.as_str(), c.last.as_str()) {
        ("", _) => String::new(),
        (f, l) if f == l => format!(" · {f}"),
        (f, l) => format!(" · {f}–{l}"),
    };
    nav_row(
        Route::circuit(&c.circuit.circuit_id),
        fragment([
            Node::from(format!(
                "{} {}",
                flag_country(&loc.country),
                c.circuit.circuit_name
            )),
            c.on_calendar
                .then(|| {
                    fragment([
                        Node::from(" "),
                        span()
                            .class("tag")
                            .text(t("Au calendrier", "On the calendar"))
                            .into(),
                    ])
                })
                .into(),
        ]),
        format!("{}, {}{years}", loc.locality, country_fr(&loc.country)),
        c.races.map(|n| format!("{n} GP")),
    )
}

/// Tous les circuits : Grands Prix disputés, années d'utilisation, présence au calendrier.
pub fn all_circuits_page() -> Node {
    let circuits = use_f1(f1("circuits.json", 100));
    // Tous les Grands Prix depuis 1950 (pagination côté serveur) → stats par circuit.
    let races = use_f1(all("races.json"));
    let current = use_f1(f1("current.json", 100));
    let query = use_state(String::new());
    let sort = use_state(CircuitSort::Races);

    // Recalculées seulement quand une réponse arrive, pas à chaque frappe.
    let stats = memo(move || circuit_stats(circuits, races, current));
    // Circuits retenus par la recherche, dans l'ordre choisi.
    let shown = memo(move || {
        let q = fold(&query.get());
        let mut list: Vec<CircuitStats> = stats.with(|s| {
            s.iter()
                .filter(|c| {
                    let loc = &c.circuit.location;
                    q.is_empty()
                        || fold(&c.circuit.circuit_name).contains(&q)
                        || fold(&loc.locality).contains(&q)
                        || fold(&loc.country).contains(&q)
                        || fold(country_fr(&loc.country)).contains(&q)
                })
                .cloned()
                .collect()
        });
        match sort.get() {
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
        list
    });

    let body = fetch_view(circuits, move |d| {
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
        let count = d.circuits().len().to_string();
        let countries = countries.to_string();
        let on_calendar: CountFn = Rc::new(move || {
            if current.with(|f| f.done().is_some()) {
                stats
                    .with(|s| s.iter().filter(|c| c.on_calendar).count())
                    .to_string()
            } else {
                "–".into()
            }
        });
        fragment([
            stat_grid_dyn(vec![
                ("Circuits", Rc::new(move || count.clone())),
                (t("Pays", "Country"), Rc::new(move || countries.clone())),
                (t("Au calendrier", "On the calendar"), on_calendar),
            ]),
            p().class("section-intro")
                .text_dyn(move || {
                    let n = shown.with(Vec::len);
                    format!("{n} circuit{}", if n > 1 { "s" } else { "" })
                })
                .into(),
            ol().class("rows rows-card")
                .children_keyed(
                    move || shown.get(),
                    |c| c.circuit.circuit_id.clone(),
                    circuit_row,
                )
                .into(),
        ])
    });

    let sort_btn = |value: CircuitSort, label: &'static str| {
        button()
            .class("seg")
            .class_if("seg-active", move || sort.get() == value)
            .on_click(move |_| sort.set(value))
            .text(label)
    };

    layout(
        "Circuits",
        Some(Tab::Archives),
        fragment([
            search_box(
                query,
                t(
                    "Rechercher (circuit, ville, pays…)",
                    "Search (circuit, city, country…)",
                ),
            ),
            div()
                .class("segmented segmented-3")
                .child(sort_btn(CircuitSort::Races, t("Plus de GP", "Most GPs")))
                .child(sort_btn(CircuitSort::Recent, t("Récents", "Recent")))
                .child(sort_btn(CircuitSort::Country, t("Pays", "Country")))
                .into(),
            body,
        ]),
    )
}
