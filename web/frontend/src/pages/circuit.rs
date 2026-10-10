use std::collections::HashMap;
use std::rc::Rc;

use active::prelude::*;
use f1x_protocol::TrackMap;

use super::{apple_map, osm_embed, track_panel, track_unavailable, track_url};
use crate::api::{Fetch, Json, all, f1, use_f1, use_f1_dyn, use_json_dyn};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Circuit, Race};
use crate::tr;
use crate::util::{country_fr, flag_country, local_date, now_ms, team_style};
use crate::{CURRENT, Route, link};

/// « 1:21.046 » → secondes (pour comparer des temps au tour).
fn lap_seconds(t: &str) -> Option<f64> {
    match t.split_once(':') {
        Some((m, s)) => Some(m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?),
        None => t.parse().ok(),
    }
}

/// Classement « nom → nombre » trié décroissant.
fn tally(items: impl Iterator<Item = (String, String)>) -> Vec<(String, String, u32)> {
    let mut map: HashMap<String, (String, u32)> = HashMap::new();
    for (id, name) in items {
        map.entry(id).or_insert((name, 0)).1 += 1;
    }
    let mut v: Vec<(String, String, u32)> = map
        .into_iter()
        .map(|(id, (name, n))| (id, name, n))
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));
    v
}

fn plural(n: u32, word: &str) -> String {
    format!("{n} {word}{}", if n > 1 { "s" } else { "" })
}

/// Palmarès et statistiques tirés de la liste des vainqueurs du circuit.
#[derive(Clone, PartialEq, Default)]
struct Palmares {
    /// La liste des vainqueurs est arrivée.
    loaded: bool,
    /// Nombre de Grands Prix de la liste.
    n_wins: usize,
    /// Pilotes vainqueurs : (id, nom, victoires), du plus titré au moins titré.
    drivers: Vec<(String, String, u32)>,
    /// Écuries victorieuses : (id, nom, victoires).
    teams: Vec<(String, String, u32)>,
    from_pole: usize,
    /// Victoire partie le plus loin sur la grille : (place, pilote).
    lowest_grid: Option<(u32, String)>,
}

fn palmares(winners: &Fetch) -> Palmares {
    let Some(data) = winners.done() else {
        return Palmares::default();
    };
    let rows = data.races();
    let wins = rows.iter().filter_map(|r| r.winner());
    let drivers = tally(
        wins.clone()
            .map(|w| (w.driver.driver_id.clone(), w.driver.full_name())),
    );
    let teams = tally(wins.clone().map(|w| {
        (
            w.constructor.constructor_id.clone(),
            w.constructor.name.clone(),
        )
    }));
    let from_pole = wins
        .clone()
        .filter(|w| w.grid.as_deref() == Some("1"))
        .count();
    let lowest_grid = wins
        .filter_map(|w| {
            Some((
                w.grid.as_deref()?.parse::<u32>().ok().filter(|g| *g > 0)?,
                w,
            ))
        })
        .max_by_key(|(g, _)| *g)
        .map(|(g, w)| (g, w.driver.full_name()));
    Palmares {
        loaded: true,
        n_wins: rows.len(),
        drivers,
        teams,
        from_pole,
        lowest_grid,
    }
}

/// Meilleur tour en course : (secondes, temps, saison, pilote).
type Record = Option<(f64, String, String, String)>;

/// Meilleur tour en course (données disponibles depuis 2004).
fn record_of(fastest: &Fetch) -> Record {
    fastest.done().and_then(|d| {
        d.races()
            .iter()
            .filter_map(|race| {
                let r = race.results.as_ref()?.first()?;
                let lap = r.fastest_lap.as_ref()?.time.as_ref()?.time.clone();
                Some((
                    lap_seconds(&lap)?,
                    lap,
                    race.season.clone(),
                    r.driver.full_name(),
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    })
}

/// Les données de la fiche, chargées une fois par la page et lues par ses différentes parties.
#[derive(Clone, Copy)]
struct Data {
    races: State<Fetch>,
    winners: State<Fetch>,
    current: State<Fetch>,
    track: State<Json<TrackMap>>,
    attempt: State<u32>,
    recent: State<bool>,
    palmares: State<Palmares>,
    record: State<Record>,
}

/// Fiche circuit : localisation, prochain GP, record, statistiques et palmarès complet. La
/// page reste affichée quand on passe à un autre circuit : seules ses données changent.
pub fn circuit_page(id: State<String>) -> Node {
    let races = use_f1_dyn(move || all(format!("circuits/{}/races.json", id.get())));
    let winners = use_f1_dyn(move || f1(format!("circuits/{}/results/1.json", id.get()), 100));
    let fastest =
        use_f1_dyn(move || f1(format!("circuits/{}/fastest/1/results.json", id.get()), 100));
    let current = use_f1(f1("current.json", 100));
    // Tracé GPS : circuits utilisés depuis 2023 (données OpenF1), y compris celui du Grand
    // Prix à venir (tracé embarqué par le serveur ou essais déjà courus).
    let recent = memo(move || {
        races.with(|f| {
            f.done().is_some_and(|d| {
                d.races()
                    .iter()
                    .any(|r| r.season.parse::<u32>().unwrap_or(0) >= 2023)
            })
        })
    });
    let attempt = use_state(0u32);
    effect(move || {
        id.with(|_| ());
        untrack(|| attempt.set(0));
    });
    let track =
        use_json_dyn::<TrackMap>(move || recent.get().then(|| track_url(&id.get(), attempt.get())));
    // Les infos du circuit viennent de la liste de ses Grands Prix (une requête de moins).
    let circuit =
        memo(move || races.with(|f| f.done().and_then(|d| d.race()).map(|r| r.circuit.clone())));
    let missing = memo(move || races.with(|f| f.done().is_some_and(|d| d.race().is_none())));
    let data = Data {
        races,
        winners,
        current,
        track,
        attempt,
        recent,
        palmares: memo(move || winners.with(palmares)),
        record: memo(move || fastest.with(record_of)),
    };

    // Le gabarit n'est remplacé que si le circuit n'existe pas ; sinon son titre suit le nom
    // du circuit une fois chargé, et seul le contenu passe du chargement à la fiche.
    dynamic(move || {
        if missing.get() {
            return super::not_found();
        }
        let content = dynamic(move || match circuit.get() {
            Some(c) => circuit_content(&untrack(|| id.get()), c, data),
            None => fetch_view(races, |_| Node::Empty),
        });
        layout_dyn(
            move || {
                circuit
                    .with(|c| c.as_ref().map(|c| c.circuit_name.clone()))
                    .unwrap_or_else(|| "Circuit".into())
            },
            Some(Tab::Archives),
            content,
        )
    })
}

/// Une valeur de [`live_stats`], relue quand les états qu'elle lit changent.
type StatValue = Rc<dyn Fn() -> String>;

fn fixed(value: String) -> StatValue {
    Rc::new(move || value.clone())
}

/// Comme [`stat_grid`], avec des valeurs suivies : une valeur qui arrive après coup (« – »
/// pendant le chargement) est remplacée en place, sans reconstruire la grille.
fn live_stats(items: Vec<(&'static str, StatValue)>) -> Node {
    dl().class("stats")
        .children(items.into_iter().map(|(label, value)| {
            // Valeurs longues (temps au tour, unités) : police adaptée à la largeur de la case.
            let long = {
                let value = value.clone();
                move || value().chars().count() > 5
            };
            div()
                .child(dt().text(label))
                .child(dd().class_if("dd-long", long).text_dyn(move || value()))
        }))
        .into()
}

/// Ligne « intitulé : valeur » de la carte « En bref ».
fn fact(label: &'static str, value: impl Into<Node>) -> Element {
    li().class("session")
        .child(span().class("session-name").text(label))
        .child(span().class("session-time").child(value))
}

/// La fiche d'un circuit trouvé. Les parties qui dépendent d'autres requêtes (vainqueurs,
/// record, prochain Grand Prix, tracé) suivent chacune leurs données.
fn circuit_content(id: &str, circuit: Circuit, data: Data) -> Node {
    let Data {
        races,
        winners,
        current,
        track,
        attempt,
        recent,
        palmares,
        record,
    } = data;
    let loc = &circuit.location;
    // iPhone, iPad et Mac : Plans (Apple Maps) ; ailleurs : OpenStreetMap.
    let apple = crate::util::is_apple_device();
    let name_q = js_sys::encode_uri_component(&circuit.circuit_name)
        .as_string()
        .unwrap_or_default();
    let map_url = match (&loc.lat, &loc.long) {
        (Some(lat), Some(lon)) if apple => Some(format!(
            "https://maps.apple.com/?ll={lat},{lon}&q={name_q}&z=15&t=k"
        )),
        (Some(lat), Some(lon)) => Some(format!(
            "https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=14/{lat}/{lon}"
        )),
        _ => None,
    };
    // La liste des Grands Prix est déjà là quand la fiche s'affiche.
    let (total, race_list): (String, Vec<Race>) = untrack(|| {
        races.with(|f| {
            (
                total_of(f),
                f.done().map(|d| d.races().to_vec()).unwrap_or_default(),
            )
        })
    });
    let names: Vec<String> = {
        let mut n: Vec<String> = race_list.iter().map(|r| r.race_name.clone()).collect();
        n.sort();
        n.dedup();
        n
    };
    let season_or_dash =
        |r: Option<&Race>| r.map(|r| r.season.clone()).unwrap_or_else(|| "–".into());

    let hero = section()
        .class("card hero")
        .child(
            p().class("eyebrow")
                .text(format!("{}, {}", loc.locality, country_fr(&loc.country))),
        )
        .child(h2().class("hero-title").text(format!(
            "{} {}",
            flag_country(&loc.country),
            circuit.circuit_name
        )))
        .child(live_stats(vec![
            ("Grands Prix", fixed(total)),
            (
                t("Premier", "First"),
                fixed(season_or_dash(race_list.first())),
            ),
            (
                t("Dernier", "Last"),
                fixed(season_or_dash(race_list.last())),
            ),
            (
                t("Pilotes vainqueurs", "Winning drivers"),
                Rc::new(move || {
                    palmares.with(|pm| {
                        if pm.loaded {
                            pm.drivers.len().to_string()
                        } else {
                            "–".into()
                        }
                    })
                }),
            ),
            (
                t("Depuis la pole", "From pole"),
                Rc::new(move || {
                    palmares.with(|pm| {
                        (pm.from_pole * 100)
                            .checked_div(pm.n_wins)
                            .map(|v| format!("{v}%"))
                            .unwrap_or_else(|| "–".into())
                    })
                }),
            ),
            (
                t("Écuries", "Teams"),
                Rc::new(move || {
                    palmares.with(|pm| {
                        if pm.loaded {
                            pm.teams.len().to_string()
                        } else {
                            "–".into()
                        }
                    })
                }),
            ),
        ]))
        .child(match (&loc.lat, &loc.long) {
            (Some(lat), Some(lon)) => Some(p().class("muted").text(tr!(
                "Coordonnées : {lat}, {lon}",
                "Coordinates: {lat}, {lon}"
            ))),
            _ => None,
        })
        .child(map_url.map(|url| {
            a().class("btn btn-ghost")
                .href(url)
                .attr("target", "_blank")
                .attr("rel", "noopener")
                .text(if apple {
                    t("Ouvrir dans Plans ↗", "Open in Apple Maps ↗")
                } else {
                    t("Voir sur la carte ↗", "View on the map ↗")
                })
        }))
        .child(circuit.url.clone().map(|url| {
            a().class("link")
                .href(url)
                .attr("target", "_blank")
                .attr("rel", "noopener")
                .text(t("Wikipédia ↗", "Wikipedia ↗"))
        }));

    let bio = circuit
        .url
        .as_deref()
        .map(|url| wiki_bio(url, t("Histoire du circuit", "About the circuit")));
    let photo = circuit
        .url
        .as_deref()
        .map(|url| crate::photo::wiki_photo(url, &circuit.circuit_name, true));

    let layout_card = section()
        .class("card")
        .child(h2().text(t("Tracé", "Track layout")))
        .child(dynamic(move || {
            let only_recent = || -> Node {
                p().class("muted")
                    .text(t(
                        "Tracé disponible pour les circuits utilisés depuis 2023 (données GPS OpenF1).",
                        "Layout available for circuits used since 2023 (OpenF1 GPS data).",
                    ))
                    .into()
            };
            match track.get() {
                None if recent.get() => loading(),
                None => only_recent(),
                Some(Ok(outline)) => track_panel(outline, true),
                Some(Err((404, _))) => only_recent(),
                Some(Err(_)) => track_unavailable(attempt),
            }
        }));

    let map_card = match (&loc.lat, &loc.long) {
        (Some(lat), Some(lon)) => Some(
            section()
                .class("card")
                .child(h2().text(t("Carte", "Map")))
                .child(if apple {
                    apple_map(lat, lon, &circuit.circuit_name)
                } else {
                    osm_embed(lat, lon)
                })
                .child(
                    div()
                        .class("map-actions")
                        .child(
                            a().class("btn btn-ghost")
                                .href(format!(
                                    "https://maps.apple.com/?ll={lat},{lon}&q={name_q}&z=15&t=k"
                                ))
                                .attr("target", "_blank")
                                .attr("rel", "noopener")
                                .text(t("🗺️ Ouvrir dans Plans", "🗺️ Open in Apple Maps")),
                        )
                        .child(
                            a().class("btn btn-ghost")
                                .href(format!("https://maps.apple.com/?daddr={lat},{lon}"))
                                .attr("target", "_blank")
                                .attr("rel", "noopener")
                                .text(t("🧭 Itinéraire", "🧭 Directions")),
                        ),
                ),
        ),
        _ => None,
    };

    // Prochain Grand Prix ici (saison en cours).
    let next_card = {
        let id = id.to_string();
        dynamic(move || {
            let now = now_ms();
            let next: Option<Race> = current.with(|f| {
                f.done().and_then(|d| {
                    d.races()
                        .iter()
                        .find(|r| r.circuit.circuit_id == id && !r.is_over(now))
                        .cloned()
                })
            });
            let Some(race) = next else {
                return Node::Empty;
            };
            section()
                .class("card")
                .child(p().class("eyebrow").text(tr!(
                    "Prochain Grand Prix · Manche {}",
                    "Next Grand Prix · Round {}",
                    race.round
                )))
                .child(h2().text(race.race_name.clone()))
                .child(
                    p().class("muted")
                        .text(local_date(&race.start_iso(), race.has_time())),
                )
                .child(countdown(race.start_ms()))
                .child(
                    link(Route::race(CURRENT, race.round_num()), "btn")
                        .text(t("Voir le programme", "View the schedule")),
                )
                .into()
        })
    };

    let glance = section()
        .class("card")
        .child(h2().text(t("En bref", "At a glance")))
        .child(
            ul().class("sessions")
                .child(fact(t("Ville", "City"), loc.locality.clone()))
                .child(fact(
                    t("Pays", "Country"),
                    country_fr(&loc.country).to_string(),
                ))
                .child(fact(
                    t("Grands Prix disputés", "Grands Prix held"),
                    race_list.len().to_string(),
                ))
                .child(fact(
                    t("Victoires depuis la pole", "Wins from pole"),
                    text_dyn(move || {
                        palmares.with(|pm| {
                            let (from_pole, n_wins) = (pm.from_pole, pm.n_wins);
                            if n_wins > 0 {
                                tr!("{from_pole} sur {n_wins}", "{from_pole} of {n_wins}")
                            } else {
                                "–".into()
                            }
                        })
                    }),
                ))
                .child(dynamic(move || {
                    palmares
                        .with(|pm| pm.lowest_grid.clone())
                        .map(|(grid, driver)| {
                            fact(
                                t("Victoire parti le plus loin", "Win from furthest back"),
                                format!("P{grid} · {driver}"),
                            )
                        })
                        .into()
                }))
                .child(dynamic(move || {
                    record
                        .get()
                        .map(|(_, lap, season, driver)| {
                            fact(
                                t("Meilleur tour en course", "Race lap record"),
                                format!("{lap} · {driver} ({season})"),
                            )
                        })
                        .into()
                })),
        )
        .child((!names.is_empty()).then(|| {
            p().class("muted")
                .text(tr!("Épreuves : {}", "Events: {}", names.join(", ")))
        }))
        .child(dynamic(move || {
            record
                .with(Option::is_some)
                .then(|| {
                    p().class("muted").text(t(
                        "Record : meilleur tour en course enregistré depuis 2004, toutes configurations du tracé confondues.",
                        "Record: fastest race lap recorded since 2004, all track layouts combined.",
                    ))
                })
                .into()
        }));

    let kings = dynamic(move || {
        palmares.with(|pm| {
            if pm.drivers.is_empty() {
                return Node::Empty;
            }
            section()
                .class("card")
                .child(h2().text(t("Rois du circuit", "Kings of the circuit")))
                .child(
                    ol().class("rows")
                        .children(pm.drivers.iter().take(5).enumerate().map(
                            |(k, (id, name, n))| {
                                li().class("row row-plain")
                                    .child(span().class("pos").text((k + 1).to_string()))
                                    .child(
                                        link(Route::driver(id), "row-main")
                                            .child(span().class("row-title").text(name.clone())),
                                    )
                                    .child(
                                        span().class("pts").text(plural(*n, t("victoire", "win"))),
                                    )
                            },
                        )),
                )
                .into()
        })
    });

    let top_teams =
        dynamic(move || {
            palmares.with(|pm| {
                if pm.teams.is_empty() {
                    return Node::Empty;
                }
                section()
                    .class("card")
                    .child(h2().text(t("Écuries les plus victorieuses", "Most successful teams")))
                    .child(
                        ol().class("rows")
                            .children(pm.teams.iter().take(5).enumerate().map(
                                |(k, (id, name, n))| {
                                    li().class("row")
                                        .style(team_style(id))
                                        .child(span().class("pos").text((k + 1).to_string()))
                                        .child(
                                            link(Route::team(id), "row-main").child(
                                                span().class("row-title").text(name.clone()),
                                            ),
                                        )
                                        .child(
                                            span()
                                                .class("pts")
                                                .text(plural(*n, t("victoire", "win"))),
                                        )
                                },
                            )),
                    )
                    .into()
            })
        });

    let honours = {
        let circuit_name = circuit.circuit_name.clone();
        section()
            .class("card")
            .child(h2().text_dyn(move || {
                tr!(
                    "Palmarès ({})",
                    "Winners ({})",
                    palmares.with(|pm| pm.n_wins)
                )
            }))
            .child(fetch_view(winners, move |d| {
                ol().class("rows")
                    .children(d.races().iter().rev().filter_map(|race| {
                        let w = race.winner()?;
                        let mut meta = vec![w.constructor.name.clone()];
                        if let Some(g) = w.grid.as_deref().filter(|g| *g != "0") {
                            meta.push(tr!("parti P{g}", "started P{g}"));
                        }
                        if race.race_name != circuit_name {
                            meta.push(race.race_name.clone());
                        }
                        Some(
                            li().class("row")
                                .style(team_style(&w.constructor.constructor_id))
                                .child(span().class("pos pos-sm").text(race.season.clone()))
                                .child(
                                    link(Route::race(&race.season, race.round_num()), "row-main")
                                        .child(span().class("row-title").text(w.driver.full_name()))
                                        .child(span().class("row-sub").text(meta.join(" · "))),
                                ),
                        )
                    }))
                    .into()
            }))
    };

    fragment([
        Node::from(hero),
        bio.into(),
        photo.into(),
        layout_card.into(),
        map_card.into(),
        next_card,
        glance.into(),
        kings,
        top_teams,
        honours.into(),
    ])
}
