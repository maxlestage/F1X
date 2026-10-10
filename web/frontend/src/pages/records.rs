//! Records de tous les temps (depuis 1950).

use std::rc::Rc;

use active::prelude::*;

use super::stats::{Champion, age_at, tally};
use crate::api::{Fetch, all, use_f1, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Race, RaceResult};
use crate::util::{flag_nationality, team_style};
use crate::{Route, link, tr};

/// Premier résultat de chaque course d'une réponse filtrée (ex. vainqueurs).
fn firsts(f: &Fetch) -> Vec<(Race, RaceResult)> {
    f.done()
        .map(|d| {
            d.races()
                .iter()
                .filter_map(|r| Some((r.clone(), r.results.as_ref()?.first()?.clone())))
                .collect()
        })
        .unwrap_or_default()
}

/// État d'une section : chargement, échec ou prête.
#[derive(Clone, Copy, PartialEq)]
enum Status {
    Loading,
    Failed,
    Ready,
}

fn status(fetches: &[State<Fetch>]) -> Status {
    if fetches
        .iter()
        .any(|f| f.with(|f| matches!(f, Fetch::Failed(_))))
    {
        Status::Failed
    } else if fetches.iter().all(|f| f.with(|f| f.done().is_some())) {
        Status::Ready
    } else {
        Status::Loading
    }
}

/// Classement (clé, libellé, nombre), du plus grand au plus petit.
type Ranking = Vec<(String, String, u32)>;

/// Section « top N » avec barres. La carte est construite une fois : seuls la note, le
/// chargement et la liste suivent leurs données.
fn top_section(
    title: &str,
    note: impl Fn() -> Option<&'static str> + 'static,
    rows: State<Ranking>,
    route: fn(&str) -> Route,
    unit: (&'static str, &'static str),
    st: impl Fn() -> Status + 'static,
) -> Node {
    section()
        .class("card")
        .child(h2().text(title.to_string()))
        .child(dynamic(move || {
            note().map(|n| p().class("muted").text(n)).into()
        }))
        .child(dynamic(move || match st() {
            Status::Loading if rows.with(Vec::is_empty) => loading(),
            Status::Failed => p()
                .class("muted")
                .text(t(
                    "Données momentanément indisponibles, recharge la page dans un instant.",
                    "Data temporarily unavailable, reload the page in a moment.",
                ))
                .into(),
            _ => Node::Empty,
        }))
        .child(ol().class("rows").children_dyn(move || {
            rows.with(|rows| {
                let max = rows.first().map(|r| r.2).unwrap_or(1).max(1) as f64;
                rows.iter()
                    .take(10)
                    .enumerate()
                    .map(|(i, (id, name, n))| {
                        li().class("row row-plain")
                            .child(span().class("pos").text((i + 1).to_string()))
                            .child(
                                link(route(id), "row-main")
                                    .child(span().class("row-title").text(name.clone()))
                                    .child(span().class("bar").attr("aria-hidden", "true").child(
                                        span().class("bar-fill bar-red").style(format!(
                                            "width:{:.1}%",
                                            *n as f64 / max * 100.0
                                        )),
                                    )),
                            )
                            .child(
                                span()
                                    .class("pts")
                                    .text(n.to_string())
                                    .child(small().text(if *n > 1 { unit.1 } else { unit.0 })),
                            )
                            .into()
                    })
                    .collect()
            })
        }))
        .into()
}

fn driver_key(r: &RaceResult) -> (String, String) {
    (r.driver.driver_id.clone(), r.driver.full_name())
}

fn team_key(r: &RaceResult) -> (String, String) {
    (
        r.constructor.constructor_id.clone(),
        r.constructor.name.clone(),
    )
}

/// Âges des vainqueurs, du plus jeune au plus âgé.
fn ages(winners: &[(Race, RaceResult)]) -> Vec<(f64, &Race, &RaceResult)> {
    let mut ages: Vec<(f64, &Race, &RaceResult)> = winners
        .iter()
        .filter_map(|(race, r)| {
            Some((
                age_at(r.driver.date_of_birth.as_deref()?, &race.date)?,
                race,
                r,
            ))
        })
        .collect();
    ages.sort_by(|a, b| a.0.total_cmp(&b.0));
    ages
}

fn age_row(&(age, race, r): &(f64, &Race, &RaceResult)) -> Node {
    let years = age.floor() as u32;
    let days = ((age - age.floor()) * 365.2425) as u32;
    li().class("row")
        .style(team_style(&r.constructor.constructor_id))
        .child(
            link(Route::driver(&r.driver.driver_id), "row-main")
                .child(span().class("row-title").text(format!(
                    "{} {}",
                    flag_nationality(r.driver.nationality.as_deref()),
                    r.driver.full_name()
                )))
                .child(span().class("row-sub").text(format!(
                    "{} {} · {}",
                    race.season, race.race_name, r.constructor.name
                ))),
        )
        .child(
            span()
                .class("pts")
                .text(tr!("{years} ans {days} j", "{years} y {days} d")),
        )
        .into()
}

/// Plus longues séries de victoires consécutives (3 et plus) : pilote, nom, longueur, saisons.
fn streaks(winners: &[(Race, RaceResult)]) -> Vec<(String, String, u32, String)> {
    let mut out: Vec<(String, String, u32, String)> = Vec::new();
    let mut i = 0;
    while i < winners.len() {
        let id = &winners[i].1.driver.driver_id;
        let mut j = i;
        while j + 1 < winners.len() && winners[j + 1].1.driver.driver_id == *id {
            j += 1;
        }
        let n = (j - i + 1) as u32;
        if n >= 3 {
            let span = if winners[i].0.season == winners[j].0.season {
                winners[i].0.season.clone()
            } else {
                format!("{}–{}", winners[i].0.season, winners[j].0.season)
            };
            out.push((id.clone(), winners[i].1.driver.full_name(), n, span));
        }
        i = j + 1;
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.2));
    out
}

fn champion_row(c: &Champion) -> Node {
    let row = li().class("row");
    let row = match &c.driver_team {
        Some(k) => row.style(team_style(&k.constructor_id)),
        None => row,
    };
    row.child(span().class("pos pos-sm").text(c.season.clone()))
        .child(
            link(Route::driver(&c.driver.driver_id), "row-main")
                .child(span().class("row-title").text(format!(
                    "{} {}",
                    flag_nationality(c.driver.nationality.as_deref()),
                    c.driver.full_name()
                )))
                .child(
                    span()
                        .class("row-sub")
                        .text(
                            c.driver_team
                                .as_ref()
                                .map(|k| k.name.clone())
                                .unwrap_or_default(),
                        )
                        .child(
                            c.constructor.as_ref().map(|k| {
                                tr!(" · constructeurs : {}", " · constructors: {}", k.name)
                            }),
                        ),
                ),
        )
        .into()
}

/// Bouton « Exporter en CSV » (comme [`export_csv`]) qui suit ses lignes : désactivé tant
/// qu'elles ne sont pas chargées, lignes calculées au clic.
fn export_dyn(filename: &'static str, rows: impl Fn() -> Vec<Vec<String>> + 'static) -> Node {
    let rows = Rc::new(rows);
    let check = rows.clone();
    button()
        .class("btn btn-ghost btn-small")
        .bool_attr("disabled", move || check().len() < 2)
        .on_click(move |_| crate::util::download_csv(filename, &rows()))
        .text(t("⬇ Exporter (CSV)", "⬇ Export (CSV)"))
        .into()
}

pub fn records_page() -> Node {
    let wins = use_f1(all("results/1.json"));
    let seconds = use_f1(all("results/2.json"));
    let thirds = use_f1(all("results/3.json"));
    let poles = use_f1(all("grid/1/results.json"));
    let fastest = use_f1(all("fastest/1/results.json"));
    let champions = use_json::<Vec<Champion>>(Some("/api/champions".into()));

    // Chaque calcul ne suit que les réponses dont il dépend, et chaque section ne relit que le
    // sien : une réponse qui arrive ne reconstruit pas le reste de la page.
    let winners = memo(move || Rc::new(wins.with(firsts)));
    let driver_wins = memo(move || winners.with(|w| tally(w.iter().map(|(_, r)| driver_key(r)))));
    let team_wins = memo(move || winners.with(|w| tally(w.iter().map(|(_, r)| team_key(r)))));
    let podiums = memo(move || {
        let ready = [wins, seconds, thirds]
            .iter()
            .all(|f| f.with(|f| f.done().is_some()));
        if !ready {
            return Vec::new();
        }
        let mut rows: Vec<(String, String)> =
            winners.with(|w| w.iter().map(|(_, r)| driver_key(r)).collect());
        for f in [seconds, thirds] {
            rows.extend(f.with(firsts).iter().map(|(_, r)| driver_key(r)));
        }
        tally(rows.into_iter())
    });
    let pole_rows = memo(move || tally(poles.with(firsts).iter().map(|(_, r)| driver_key(r))));
    let fastest_rows = memo(move || tally(fastest.with(firsts).iter().map(|(_, r)| driver_key(r))));

    let champs = memo(move || -> Vec<Champion> {
        champions.with(|c| match c {
            Some(Ok(list)) => list.iter().filter(|c| !c.in_progress).cloned().collect(),
            _ => Vec::new(),
        })
    });
    let driver_titles = memo(move || {
        champs.with(|c| {
            tally(
                c.iter()
                    .map(|c| (c.driver.driver_id.clone(), c.driver.full_name())),
            )
        })
    });
    let team_titles = memo(move || {
        champs.with(|c| {
            tally(
                c.iter()
                    .filter_map(|c| c.constructor.as_ref())
                    .map(|k| (k.constructor_id.clone(), k.name.clone())),
            )
        })
    });

    // Plus de victoires en une saison.
    let season_wins = memo(move || {
        winners.with(|w| {
            let mut v = tally(w.iter().map(|(race, r)| {
                (
                    format!("{}|{}", race.season, r.driver.driver_id),
                    format!("{} ({})", r.driver.full_name(), race.season),
                )
            }));
            v.truncate(10);
            v
        })
    });

    let exported = move || -> Vec<Vec<String>> {
        std::iter::once(vec![
            t("Rang", "Rank").into(),
            t("Pilote", "Driver").into(),
            t("Victoires", "Wins").into(),
        ])
        .chain(driver_wins.with(|list| {
            list.iter()
                .enumerate()
                .map(|(i, (_, n, w))| vec![(i + 1).to_string(), n.clone(), w.to_string()])
                .collect::<Vec<_>>()
        }))
        .collect()
    };

    let champ_state = move || {
        champions.with(|c| match c {
            None => Status::Loading,
            Some(Err(_)) => Status::Failed,
            Some(Ok(_)) => Status::Ready,
        })
    };
    let champions_note = move || {
        champions.with(|c| match c {
            None => Some(t("Calcul des champions en cours…", "Computing champions…")),
            Some(Err(_)) => Some(t(
                "Champions momentanément indisponibles.",
                "Champions temporarily unavailable.",
            )),
            _ => None,
        })
    };
    let titles_unit = (t(" titre", " title"), t(" titres", " titles"));
    let wins_unit = (t(" victoire", " win"), t(" victoires", " wins"));

    let hero = section()
        .class("card hero")
        .child(p().class("eyebrow").text(t("Depuis 1950", "Since 1950")))
        .child(
            h2().class("hero-title")
                .text(t("Les records de la Formule 1", "Formula 1 records")),
        )
        .child(p().class("muted").text(t(
            "Mis à jour automatiquement après chaque Grand Prix.",
            "Updated automatically after every Grand Prix.",
        )));

    let season_wins_card = section()
        .class("card")
        .child(h2().text(t(
            "Plus de victoires en une saison",
            "Most wins in a season",
        )))
        .child(ol().class("rows").children_dyn(move || {
            season_wins.with(|rows| {
                rows.iter()
                    .enumerate()
                    .map(|(i, (key, label, n))| {
                        let id = key.split('|').nth(1).unwrap_or_default().to_string();
                        li().class("row row-plain")
                            .child(span().class("pos").text((i + 1).to_string()))
                            .child(
                                link(Route::driver(&id), "row-main")
                                    .child(span().class("row-title").text(label.clone())),
                            )
                            .child(span().class("pts").text(n.to_string()))
                            .into()
                    })
                    .collect()
            })
        }));

    let streaks_card = section()
        .class("card")
        .child(h2().text(t(
            "Plus longues séries de victoires",
            "Longest winning streaks",
        )))
        .child(ol().class("rows").children_dyn(move || {
            winners.with(|w| {
                streaks(w)
                    .iter()
                    .take(8)
                    .enumerate()
                    .map(|(i, (id, name, n, span_label))| {
                        li().class("row row-plain")
                            .child(span().class("pos").text((i + 1).to_string()))
                            .child(
                                link(Route::driver(id), "row-main")
                                    .child(span().class("row-title").text(name.clone()))
                                    .child(span().class("row-sub").text(span_label.clone())),
                            )
                            .child(
                                span()
                                    .class("pts")
                                    .text(tr!("{n} de suite", "{n} in a row")),
                            )
                            .into()
                    })
                    .collect()
            })
        }));

    let youngest =
        section()
            .class("card")
            .child(h2().text(t("Plus jeunes vainqueurs", "Youngest winners")))
            .child(ol().class("rows").children_dyn(move || {
                winners.with(|w| ages(w).iter().take(5).map(age_row).collect())
            }));
    let oldest = section()
        .class("card")
        .child(h2().text(t("Vainqueurs les plus âgés", "Oldest winners")))
        .child(ol().class("rows").children_dyn(move || {
            winners.with(|w| ages(w).iter().rev().take(5).map(age_row).collect())
        }));

    let every_champion = dynamic(move || {
        champs.with(|champs| {
            if champs.is_empty() {
                return Node::Empty;
            }
            details()
                .class("card")
                .child(summary().child(strong().text(tr!(
                    "Tous les champions ({})",
                    "Every champion ({})",
                    champs.len()
                ))))
                .child(
                    ol().class("rows")
                        .children(champs.iter().rev().map(champion_row)),
                )
                .into()
        })
    });

    layout(
        t("Records", "Records"),
        Some(Tab::Archives),
        fragment([
            Node::from(hero),
            top_section(
                t("Titres de champion du monde", "World championships"),
                champions_note,
                driver_titles,
                Route::driver,
                titles_unit,
                champ_state,
            ),
            top_section(
                t("Titres constructeurs", "Constructors' titles"),
                || Some(t("Depuis 1958.", "Since 1958.")),
                team_titles,
                Route::team,
                titles_unit,
                champ_state,
            ),
            top_section(
                t("Victoires", "Wins"),
                || None,
                driver_wins,
                Route::driver,
                wins_unit,
                move || status(&[wins]),
            ),
            top_section(
                t("Victoires des écuries", "Team wins"),
                || None,
                team_wins,
                Route::team,
                wins_unit,
                move || status(&[wins]),
            ),
            top_section(
                t("Podiums", "Podiums"),
                || None,
                podiums,
                Route::driver,
                (" podium", " podiums"),
                move || status(&[wins, seconds, thirds]),
            ),
            top_section(
                t("Départs en pole position", "Pole positions"),
                || {
                    Some(t(
                        "Départs depuis la 1re place de la grille.",
                        "Starts from first on the grid.",
                    ))
                },
                pole_rows,
                Route::driver,
                (" pole", " poles"),
                move || status(&[poles]),
            ),
            top_section(
                t("Meilleurs tours en course", "Fastest laps"),
                || {
                    Some(t(
                        "Données disponibles depuis 2004.",
                        "Data available since 2004.",
                    ))
                },
                fastest_rows,
                Route::driver,
                (t(" record", " lap"), t(" records", " laps")),
                move || status(&[fastest]),
            ),
            season_wins_card.into(),
            streaks_card.into(),
            youngest.into(),
            oldest.into(),
            every_champion,
            export_dyn("f1x-victoires.csv", exported),
        ]),
    )
}
