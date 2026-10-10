//! Fiche écurie.

use std::rc::Rc;

use active::prelude::*;

use super::stats::{Champion, team_titles};
use crate::api::{Fetch, Json, f1, use_f1_dyn, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Constructor, Driver, MrData};
use crate::tr;
use crate::util::{flag_country, flag_nationality, team_style};
use crate::{Route, link};

/// Requêtes de la fiche qui arrivent après les informations de l'écurie.
#[derive(Clone, Copy)]
struct Live {
    champions: State<Json<Vec<Champion>>>,
    wins: State<Fetch>,
    seconds: State<Fetch>,
    thirds: State<Fetch>,
    poles: State<Fetch>,
    seasons: State<Fetch>,
    chosen: State<Option<String>>,
    season: State<Option<String>>,
    /// Une saison est connue (la carte « Saison par saison » peut s'afficher).
    has_season: State<bool>,
    standing: State<Fetch>,
    results: State<Fetch>,
}

/// Valeur d'une statistique, relue quand ses données arrivent.
type Value = Rc<dyn Fn() -> String>;

/// Fiche écurie : palmarès complet et détail saison par saison. La page reste affichée quand
/// on passe à une autre écurie : seules ses données changent.
pub fn team_page(id: State<String>) -> Node {
    let path = move |suffix: &'static str, limit: u32| {
        use_f1_dyn(move || f1(format!("constructors/{}{suffix}.json", id.get()), limit))
    };

    let info = path("", 1);
    let champions = use_json::<Vec<Champion>>(Some("/api/champions".into()));
    let wins = path("/results/1", 1);
    let seconds = path("/results/2", 1);
    let thirds = path("/results/3", 1);
    // Poles = départs en tête de grille (fonctionne pour toutes les époques).
    let poles = path("/grid/1/results", 1);
    let seasons = path("/seasons", 100);

    // Saison choisie dans la liste, sinon la dernière saison de l'écurie (choix oublié quand
    // on passe à une autre écurie).
    let chosen = use_state(None::<String>);
    effect(move || {
        id.with(|_| ());
        untrack(|| chosen.set(None));
    });
    let season = memo(move || {
        chosen.get().or_else(|| {
            seasons.with(|f| {
                f.done()
                    .and_then(|d| d.seasons().last().map(|s| s.season.clone()))
            })
        })
    });
    let base = |file: &'static str, limit: u32| {
        move || {
            season
                .get()
                .and_then(|s| f1(format!("{s}/constructors/{}/{file}.json", id.get()), limit))
        }
    };
    let has_season = memo(move || season.with(Option::is_some));
    let standing = use_f1_dyn(base("constructorStandings", 1));
    let results = use_f1_dyn(base("results", 100));

    let live = Live {
        champions,
        wins,
        seconds,
        thirds,
        poles,
        seasons,
        chosen,
        season,
        has_season,
        standing,
        results,
    };
    // Informations chargées mais vides : écurie inconnue.
    let missing =
        memo(move || info.with(|f| f.done().is_some_and(|d| d.constructors().is_empty())));

    // Le titre suit le chargement : la mise en page n'est pas reconstruite à l'arrivée des données.
    let title = move || {
        info.with(|f| {
            f.done()
                .and_then(|d| d.constructors().first().map(|c| c.name.clone()))
        })
        .unwrap_or_else(|| t("Écurie", "Team").into())
    };
    let page = move || {
        layout_dyn(
            title,
            Some(Tab::Standings),
            fetch_view(info, move |data| body(data, live)),
        )
    };
    dynamic(move || {
        if missing.get() {
            untrack(super::not_found)
        } else {
            untrack(page)
        }
    })
}

/// Contenu de la fiche, construit une seule fois quand l'écurie est chargée ; les compteurs,
/// les titres et la saison choisie suivent leurs propres requêtes.
fn body(data: &MrData, live: Live) -> Node {
    let Some(team) = data.constructors().first().cloned() else {
        return Node::Empty;
    };
    let Live {
        champions,
        wins,
        seconds,
        thirds,
        poles,
        seasons,
        has_season,
        ..
    } = live;

    // « Française · 1950 – 2026 », la période une fois les saisons chargées.
    let period = text_dyn(move || {
        seasons.with(|f| {
            let list = f.done().map(|d| d.seasons()).unwrap_or_default();
            match (list.first(), list.last()) {
                (Some(first), Some(last)) => format!(" · {} – {}", first.season, last.season),
                _ => String::new(),
            }
        })
    });
    let podiums: Value = Rc::new(move || {
        [wins, seconds, thirds]
            .iter()
            .map(|f| f.with(|f| f.done().map(|d| d.total())))
            .sum::<Option<u32>>()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "–".into())
    });
    let titles = {
        let id = team.constructor_id.clone();
        dynamic(move || {
            let Some(Ok(c)) = champions.get() else {
                return Node::Empty;
            };
            match team_titles(&c, &id).as_slice() {
                titles @ [_, ..] => p()
                    .class("titles")
                    .text(tr!(
                        "🏆 Champion constructeurs ×{} : {}",
                        "🏆 Constructors' champion ×{}: {}",
                        titles.len(),
                        titles.join(", ")
                    ))
                    .into(),
                [] => Node::Empty,
            }
        })
    };

    let hero = section()
        .class("card hero")
        .style(team_style(&team.constructor_id))
        .child(
            p().class("eyebrow")
                .text(team.nationality.clone().unwrap_or_default())
                .child(period),
        )
        .child(h2().class("hero-title").text(format!(
            "{} {}",
            flag_nationality(team.nationality.as_deref()),
            team.name
        )))
        .child(live_stat_grid(vec![
            (
                t("Victoires", "Wins"),
                Rc::new(move || wins.with(total_of)) as Value,
            ),
            ("Podiums", podiums),
            ("Poles", Rc::new(move || poles.with(total_of))),
        ]))
        .child(titles)
        .child(fav_button("team", &team.constructor_id))
        .child(team.url.as_ref().map(|url| {
            a().class("link")
                .href(url.clone())
                .attr("target", "_blank")
                .attr("rel", "noopener")
                .text(t("Wikipédia ↗", "Wikipedia ↗"))
        }));

    // La carte « Saison par saison » apparaît quand la liste des saisons arrive, une seule fois.
    let season_part = {
        let team = team.clone();
        dynamic(move || {
            if has_season.get() {
                untrack(|| season_card(&team, live))
            } else {
                Node::Empty
            }
        })
    };

    fragment([
        Node::from(hero),
        team.url
            .as_ref()
            .map(|url| wiki_bio(url, t("À propos", "About")))
            .into(),
        crate::gl3d::car_card(&team.constructor_id, &team.name),
        season_part,
    ])
}

/// Grille de statistiques (comme `stat_grid`) dont les valeurs arrivent après coup : « – »
/// pendant le chargement, puis le chiffre, écrit sur place (le moteur d'animation le fait
/// alors compter, comme les autres chiffres).
fn live_stat_grid(items: Vec<(&'static str, Value)>) -> Node {
    dl().class("stats")
        .children(items.into_iter().map(|(label, value)| {
            let long = value.clone();
            div().child(dt().text(label)).child(
                dd().class_if("dd-long", move || long().chars().count() > 5)
                    .text_dyn(move || value()),
            )
        }))
        .into()
}

/// Carte « Saison par saison » : la liste des saisons est construite une fois, le classement,
/// les pilotes, les résultats et le lien suivent la saison choisie.
fn season_card(team: &Constructor, live: Live) -> Node {
    let Live {
        seasons,
        chosen,
        season,
        standing,
        results,
        ..
    } = live;
    let season_list: Vec<String> = seasons.with(|f| {
        f.done()
            .map(|d| d.seasons().iter().map(|s| s.season.clone()).collect())
            .unwrap_or_default()
    });
    let initial = chosen.get().or_else(|| season_list.last().cloned());

    let options = season_list.iter().rev().map(|s| {
        let o = option().attr("value", s.clone());
        let o = if initial.as_deref() == Some(s.as_str()) {
            o.attr("selected", "")
        } else {
            o
        };
        o.text(s.clone())
    });

    let season_standing = dynamic(move || {
        let s = standing.with(|f| {
            f.done()
                .and_then(|d| d.standings())
                .and_then(|l| l.constructor_standings.as_ref()?.first().cloned())
        });
        s.map(|s| {
            stat_grid(vec![
                (t("Classement", "Standings"), format!("P{}", s.rank())),
                ("Points", s.points.clone()),
                (t("Victoires", "Wins"), s.wins.clone()),
            ])
        })
        .into()
    });

    // Pilotes de la saison, déduits des résultats (une requête de moins).
    let drivers = {
        let team_id = team.constructor_id.clone();
        dynamic(move || {
            let mut season_drivers: Vec<Driver> = Vec::new();
            results.with(|f| {
                for r in f
                    .done()
                    .map(|d| d.races())
                    .unwrap_or_default()
                    .iter()
                    .flat_map(|r| r.results.iter().flatten())
                {
                    if !season_drivers
                        .iter()
                        .any(|d| d.driver_id == r.driver.driver_id)
                    {
                        season_drivers.push(r.driver.clone());
                    }
                }
            });
            if season_drivers.is_empty() {
                return Node::Empty;
            }
            fragment([
                Node::from(h3().class("subhead").text(t("Pilotes", "Drivers"))),
                ol().class("rows")
                    .children(season_drivers.iter().map(|drv| {
                        li().class("row").style(team_style(&team_id)).child(
                            link(Route::driver(&drv.driver_id), "row-main").child(
                                span()
                                    .class("row-title")
                                    .text(format!(
                                        "{} {} ",
                                        flag_nationality(drv.nationality.as_deref()),
                                        drv.given_name
                                    ))
                                    .child(strong().text(drv.family_name.clone())),
                            ),
                        )
                    }))
                    .into(),
            ])
        })
    };

    let race_rows = {
        let team_id = team.constructor_id.clone();
        fetch_view(results, move |d| {
            ol().class("rows")
                .children(d.races().iter().rev().map(|race| {
                    let rows = race.results.clone().unwrap_or_default();
                    let best = rows
                        .iter()
                        .filter_map(|r| r.position.parse::<u32>().ok())
                        .min();
                    let points: f64 = rows
                        .iter()
                        .filter_map(|r| r.points.parse::<f64>().ok())
                        .sum();
                    let sub = rows
                        .iter()
                        .map(|r| format!("{} {}", r.driver.family_name, r.position_text))
                        .collect::<Vec<_>>()
                        .join(" · ");
                    li().class("row")
                        .style(team_style(&team_id))
                        .child(
                            span()
                                .class("pos pos-sm")
                                .text(best.map(|b| b.to_string()).unwrap_or_else(|| "–".into())),
                        )
                        .child(
                            link(Route::race(&race.season, race.round_num()), "row-main")
                                .child(span().class("row-title").text(format!(
                                    "{} {}",
                                    flag_country(&race.circuit.location.country),
                                    race.race_name
                                )))
                                .child(span().class("row-sub").text(sub)),
                        )
                        .child(
                            (points > 0.0).then(|| span().class("pts").text(format!("+{points}"))),
                        )
                }))
                .into()
        })
    };

    let standings_link = dynamic(move || {
        season
            .get()
            .map(|season| {
                link(
                    Route::TeamStandings {
                        season: season.clone(),
                    },
                    "btn btn-ghost",
                )
                .text(tr!("Classement {season}", "{season} standings"))
            })
            .into()
    });

    section()
        .class("card")
        .child(h2().text(t("Saison par saison", "Season by season")))
        .child(
            label()
                .class("select")
                .child(span().class("select-label").text(t("Saison", "Season")))
                .child(
                    select()
                        .attr("aria-label", t("Choisir une saison", "Choose a season"))
                        .on("change", move |e| chosen.set(Some(e.value())))
                        .children(options),
                ),
        )
        .child(season_standing)
        .child(drivers)
        .child(h3().class("subhead").text(t("Résultats", "Results")))
        .child(race_rows)
        .child(standings_link)
        .into()
}
