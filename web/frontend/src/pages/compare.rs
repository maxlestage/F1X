//! Comparateur de pilotes : statistiques de carrière côte à côte et face-à-face.

use std::collections::HashMap;
use std::rc::Rc;

use active::prelude::*;

use super::stats::{Career, Champion, career, driver_titles, entries};
use crate::api::{Fetch, Json, all, use_f1, use_f1_dyn, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Driver, MrData, Race, RaceResult};
use crate::util::{flag_nationality, fold};
use crate::{Route, link, tr};

/// Couleurs validées (contraste et daltonisme) sur fond sombre.
const COLOR_A: &str = "#e5483f";
const COLOR_B: &str = "#3b9fd8";
/// Côté encore vide dans l'adresse (`/comparer/hamilton/_`).
const NONE: &str = "_";

/// Recherche d'un pilote (suggestions sous le champ, liste verticale).
fn picker(
    label: &'static str,
    color: &'static str,
    drivers: State<Fetch>,
    current: State<Option<Driver>>,
    on_pick: impl Fn(String) + 'static,
) -> Node {
    let query = use_state(String::new());
    let on_pick = Rc::new(on_pick);
    // Six suggestions au plus, à partir de deux lettres.
    let matches = memo(move || {
        let q = fold(&query.get());
        if q.len() < 2 {
            return Vec::new();
        }
        drivers.with(|f| {
            f.done()
                .map(|d| {
                    d.drivers()
                        .iter()
                        .filter(|d| {
                            fold(&d.full_name()).contains(&q)
                                || d.code.as_deref().is_some_and(|c| fold(c) == q)
                        })
                        .take(6)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        })
    });
    let open = memo(move || matches.with(|m| !m.is_empty()));
    let suggestion = Rc::new(move |d: &Driver| -> Node {
        let id = d.driver_id.clone();
        let on_pick = on_pick.clone();
        li().child(
            button()
                .on_click(move |_| {
                    query.set(String::new());
                    on_pick(id.clone());
                })
                .text(format!(
                    "{} {}",
                    flag_nationality(d.nationality.as_deref()),
                    d.full_name()
                ))
                .child(
                    small().class("muted").text(
                        d.date_of_birth
                            .as_deref()
                            .map(|b| format!(" · {}", b.get(..4).unwrap_or(b)))
                            .unwrap_or_default(),
                    ),
                ),
        )
        .into()
    });
    div()
        .class("picker")
        .style(format!("--pick:{color}"))
        .child(span().class("picker-label").text(label))
        .child(dynamic(move || {
            current.with(|d| match d {
                Some(d) => p()
                    .class("picker-current")
                    .text(format!("{} ", flag_nationality(d.nationality.as_deref())))
                    .child(strong().text(d.full_name()))
                    .into(),
                None => Node::Empty,
            })
        }))
        // Hors des parties dynamiques : le champ garde le focus pendant la frappe.
        .child(
            input()
                .class("search")
                .attr("type", "search")
                .attr_dyn("value", move || query.get())
                .on_input(move |e| query.set(e.value()))
                .attr(
                    "placeholder",
                    t("Rechercher un pilote…", "Search a driver…"),
                )
                .attr("aria-label", label)
                .attr("autocomplete", "off"),
        )
        .child(dynamic(move || {
            if !open.get() {
                return Node::Empty;
            }
            let row = suggestion.clone();
            ul().class("suggestions")
                .children_keyed(
                    move || matches.get(),
                    |d| d.driver_id.clone(),
                    move |d| row(d),
                )
                .into()
        }))
        .into()
}

/// Ligne de statistique : deux barres (A rouge, B bleu), le meilleur en gras.
fn stat_row(
    label: &str,
    a: f64,
    b: f64,
    fmt: impl Fn(f64) -> String,
    higher_is_better: bool,
) -> Node {
    let max = a.max(b).max(f64::EPSILON);
    let a_best = if higher_is_better {
        a > b
    } else {
        a < b && a > 0.0
    };
    let b_best = if higher_is_better {
        b > a
    } else {
        b < a && b > 0.0
    };
    let bar = |v: f64, color: &str, best: bool| {
        let pct = if higher_is_better {
            v / max * 100.0
        } else if v > 0.0 {
            a.min(b).max(f64::EPSILON) / v * 100.0
        } else {
            0.0
        };
        span()
            .class("cmp-bar")
            .child(span().class("cmp-fill").style(format!(
                "width:{:.1}%;background:{color}",
                pct.clamp(0.0, 100.0)
            )))
            .child(
                span()
                    .class("cmp-val")
                    .class(when(best, "cmp-best"))
                    .text(fmt(v)),
            )
    };
    li().class("cmp-row")
        .child(span().class("cmp-label").text(label.to_string()))
        .child(bar(a, COLOR_A, a_best))
        .child(bar(b, COLOR_B, b_best))
        .into()
}

/// Bouton « Exporter en CSV » (comme [`export_csv`]), dont les lignes sont calculées au clic :
/// les titres peuvent arriver après le reste. Il y a toujours des lignes : jamais désactivé.
fn export_on_click(filename: String, rows: impl Fn() -> Vec<Vec<String>> + 'static) -> Node {
    button()
        .class("btn btn-ghost btn-small")
        .on_click(move |_| crate::util::download_csv(&filename, &rows()))
        .text(t("⬇ Exporter (CSV)", "⬇ Export (CSV)"))
        .into()
}

/// Comparateur (`pair` : pilotes A et B de l'adresse). La page reste affichée quand on choisit
/// un autre pilote : la liste des pilotes et les titres ne sont pas rechargés, et seule la
/// carrière du pilote qui change l'est.
pub fn compare_page(pair: State<(Option<String>, Option<String>)>) -> Node {
    // Pilote A (`first`) ou B ; « _ » dans l'adresse : pas encore choisi.
    let side = |first: bool| {
        memo(move || {
            pair.with(|(a, b)| if first { a } else { b }.clone())
                .filter(|id| id != NONE)
        })
    };
    let (a, b) = (side(true), side(false));
    let list = use_f1(all("drivers.json"));
    let champions = use_json::<Vec<Champion>>(Some("/api/champions".into()));
    let results = |id: State<Option<String>>| {
        use_f1_dyn(move || {
            id.get()
                .and_then(|id| all(format!("drivers/{id}/results.json")))
        })
    };
    let races_a = results(a);
    let races_b = results(b);

    // Pilotes choisis, retrouvés dans la liste quand elle arrive.
    let find = |id: State<Option<String>>| {
        memo(move || -> Option<Driver> {
            let id = id.get()?;
            list.with(|f| {
                f.done()?
                    .drivers()
                    .iter()
                    .find(|d| d.driver_id == id)
                    .cloned()
            })
        })
    };
    let (da, db) = (find(a), find(b));
    let pick = |side: u8| {
        move |id: String| {
            let other = |s: State<Option<String>>| s.get().unwrap_or_else(|| NONE.into());
            let (na, nb) = if side == 0 {
                (id, other(b))
            } else {
                (other(a), id)
            };
            active::navigate(&Route::CompareWith { a: na, b: nb }.href());
        }
    };

    let body = dynamic(move || {
        let (ra, rb) = (races_a.with(Fetch::data), races_b.with(Fetch::data));
        match (ra, rb, da.get(), db.get()) {
            (Some(ra), Some(rb), Some(da), Some(db)) => comparison(&ra, &rb, &da, &db, champions),
            (_, _, Some(_), Some(_)) => loading(),
            _ => p()
                .class("section-intro")
                .text(t(
                    "Choisis deux pilotes pour les comparer.",
                    "Pick two drivers to compare them.",
                ))
                .into(),
        }
    });

    let chips = div().class("chips").children(
        [
            ("hamilton", "max_verstappen"),
            ("senna", "prost"),
            ("michael_schumacher", "alonso"),
            ("leclerc", "norris"),
        ]
        .iter()
        .map(|(x, y)| {
            let to = Route::CompareWith {
                a: x.to_string(),
                b: y.to_string(),
            };
            let label = format!(
                "{} / {}",
                x.split('_').next_back().unwrap_or(x),
                y.split('_').next_back().unwrap_or(y)
            );
            link(to, "chip").text(label)
        }),
    );

    layout(
        t("Comparateur", "Compare"),
        Some(Tab::Archives),
        fragment([
            Node::from(
                section()
                    .class("card")
                    .child(dynamic(move || {
                        if list.with(Fetch::is_loading) {
                            loading()
                        } else {
                            Node::Empty
                        }
                    }))
                    .child(picker(
                        t("Pilote A", "Driver A"),
                        COLOR_A,
                        list,
                        da,
                        pick(0),
                    ))
                    .child(picker(
                        t("Pilote B", "Driver B"),
                        COLOR_B,
                        list,
                        db,
                        pick(1),
                    ))
                    .child(chips),
            ),
            body,
        ]),
    )
}

/// Statistiques côte à côte, face-à-face et export, une fois les deux carrières chargées.
/// Seuls les titres suivent `champions`, qui peut arriver après.
fn comparison(
    ra: &MrData,
    rb: &MrData,
    da: &Driver,
    db: &Driver,
    champions: State<Json<Vec<Champion>>>,
) -> Node {
    let (ca, cb): (Career, Career) = (career(ra.races()), career(rb.races()));
    let titles = move |id: &str| {
        champions.with(|c| match c {
            Some(Ok(c)) => driver_titles(c, id).len() as f64,
            _ => 0.0,
        })
    };
    // Face-à-face sur les courses disputées ensemble.
    let map_b: HashMap<(String, String), (&Race, &RaceResult)> = entries(rb.races())
        .into_iter()
        .map(|(race, r)| ((race.season.clone(), race.round.clone()), (race, r)))
        .collect();
    let (mut common, mut ahead_a, mut ahead_b, mut mates, mut mates_a, mut grid_a, mut grid_b) =
        (0, 0, 0, 0, 0, 0, 0);
    for (race, r) in entries(ra.races()) {
        let Some((_, rb)) = map_b.get(&(race.season.clone(), race.round.clone())) else {
            continue;
        };
        common += 1;
        let (pa, pb): (u32, u32) = (
            r.position.parse().unwrap_or(99),
            rb.position.parse().unwrap_or(99),
        );
        if pa < pb {
            ahead_a += 1
        } else {
            ahead_b += 1
        }
        if r.constructor.constructor_id == rb.constructor.constructor_id {
            mates += 1;
            if pa < pb {
                mates_a += 1
            }
            let (ga, gb): (u32, u32) = (
                r.grid
                    .as_deref()
                    .and_then(|g| g.parse().ok())
                    .filter(|g| *g > 0)
                    .unwrap_or(99),
                rb.grid
                    .as_deref()
                    .and_then(|g| g.parse().ok())
                    .filter(|g| *g > 0)
                    .unwrap_or(99),
            );
            if ga < gb {
                grid_a += 1
            } else if gb < ga {
                grid_b += 1
            }
        }
    }
    let int = |v: f64| format!("{v:.0}");
    let (ida, idb) = (da.driver_id.clone(), db.driver_id.clone());
    let csv = {
        let (ida, idb) = (ida.clone(), idb.clone());
        let (na, nb) = (da.full_name(), db.full_name());
        let (ca, cb) = (ca.clone(), cb.clone());
        move || -> Vec<Vec<String>> {
            vec![
                vec![t("Statistique", "Statistic").into(), na.clone(), nb.clone()],
                vec![
                    t("Titres", "Titles").into(),
                    int(titles(&ida)),
                    int(titles(&idb)),
                ],
                vec![
                    t("Départs", "Starts").into(),
                    ca.starts.to_string(),
                    cb.starts.to_string(),
                ],
                vec![
                    t("Victoires", "Wins").into(),
                    ca.wins.to_string(),
                    cb.wins.to_string(),
                ],
                vec![
                    t("Podiums", "Podiums").into(),
                    ca.podiums.to_string(),
                    cb.podiums.to_string(),
                ],
                vec!["Poles".into(), ca.poles.to_string(), cb.poles.to_string()],
                vec![
                    t("Meilleurs tours", "Fastest laps").into(),
                    ca.fastest.to_string(),
                    cb.fastest.to_string(),
                ],
                vec![
                    "Points".into(),
                    format!("{:.0}", ca.points),
                    format!("{:.0}", cb.points),
                ],
                vec![
                    t("Abandons", "Retirements").into(),
                    ca.retirements.to_string(),
                    cb.retirements.to_string(),
                ],
            ]
        }
    };
    // Les titres arrivent à part : seule leur ligne est reconstruite.
    let titles_row =
        dynamic(move || stat_row(t("Titres", "Titles"), titles(&ida), titles(&idb), int, true));

    let stats = section()
        .class("card")
        .child(
            div()
                .class("cmp-legend")
                .child(
                    span()
                        .child(span().class("dot-a").style(format!("background:{COLOR_A}")))
                        .text(da.full_name()),
                )
                .child(
                    span()
                        .child(span().class("dot-a").style(format!("background:{COLOR_B}")))
                        .text(db.full_name()),
                ),
        )
        .child(
            ul().class("cmp")
                .child(titles_row)
                .child(stat_row(
                    t("Départs", "Starts"),
                    ca.starts as f64,
                    cb.starts as f64,
                    int,
                    true,
                ))
                .child(stat_row(
                    t("Victoires", "Wins"),
                    ca.wins as f64,
                    cb.wins as f64,
                    int,
                    true,
                ))
                .child(stat_row(
                    t("% de victoires", "Win rate"),
                    ca.win_rate(),
                    cb.win_rate(),
                    |v| format!("{v:.1}%"),
                    true,
                ))
                .child(stat_row(
                    t("Podiums", "Podiums"),
                    ca.podiums as f64,
                    cb.podiums as f64,
                    int,
                    true,
                ))
                .child(stat_row(
                    "Poles",
                    ca.poles as f64,
                    cb.poles as f64,
                    int,
                    true,
                ))
                .child(stat_row(
                    t("Meilleurs tours", "Fastest laps"),
                    ca.fastest as f64,
                    cb.fastest as f64,
                    int,
                    true,
                ))
                .child(stat_row("Points", ca.points, cb.points, int, true))
                .child(stat_row(
                    t("Place moyenne à l'arrivée", "Average finish"),
                    ca.avg_finish().unwrap_or(0.0),
                    cb.avg_finish().unwrap_or(0.0),
                    |v| format!("{v:.1}"),
                    false,
                ))
                .child(stat_row(
                    t("Abandons", "Retirements"),
                    ca.retirements as f64,
                    cb.retirements as f64,
                    int,
                    false,
                ))
                .child(stat_row(
                    t("Saisons", "Seasons"),
                    ca.seasons.len() as f64,
                    cb.seasons.len() as f64,
                    int,
                    true,
                )),
        )
        .child(p().class("muted").text(t(
            "Barre la plus longue = meilleur (pour la place moyenne et les abandons, le plus bas l'emporte). Valeur en gras = avantage.",
            "Longest bar = better (for average finish and retirements, lower wins). Bold value = advantage.",
        )));

    let session = |name: String, time: String| {
        li().class("session")
            .child(span().class("session-name").text(name))
            .child(span().class("session-time").text(time))
    };
    let head_to_head: Node = if common == 0 {
        p().class("muted")
            .text(t(
                "Ils n'ont jamais couru la même course.",
                "They never raced in the same Grand Prix.",
            ))
            .into()
    } else {
        fragment([
            Node::from(
                ul().class("sessions")
                    .child(session(
                        tr!("Courses ensemble ({common})", "Races together ({common})"),
                        format!(
                            "{} {ahead_a} – {ahead_b} {}",
                            da.family_name, db.family_name
                        ),
                    ))
                    .child((mates > 0).then(|| {
                        fragment([
                            Node::from(session(
                                tr!("Coéquipiers, course ({mates})", "Teammates, race ({mates})"),
                                format!(
                                    "{} {mates_a} – {} {}",
                                    da.family_name,
                                    mates - mates_a,
                                    db.family_name
                                ),
                            )),
                            session(
                                t("Coéquipiers, grille", "Teammates, grid").into(),
                                format!(
                                    "{} {grid_a} – {grid_b} {}",
                                    da.family_name, db.family_name
                                ),
                            )
                            .into(),
                        ])
                    })),
            ),
            p().class("muted")
                .text(t(
                    "Qui a terminé devant l'autre quand ils étaient tous les deux au départ.",
                    "Who finished ahead when both started the race.",
                ))
                .into(),
        ])
    };

    fragment([
        Node::from(stats),
        section()
            .class("card")
            .child(h2().text(t("Face-à-face", "Head to head")))
            .child(head_to_head)
            .into(),
        export_on_click(format!("f1x-{}-vs-{}.csv", da.driver_id, db.driver_id), csv),
    ])
}
