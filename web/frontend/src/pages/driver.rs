//! Fiche pilote.

use std::rc::Rc;

use active::prelude::*;

use super::stats::{Champion, driver_titles, entries};
use crate::api::{Fetch, Json, all, f1, use_f1_dyn, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Constructor, MrData, Race, RaceResult};
use crate::tr;
use crate::util::{age, current_year, flag_country, flag_nationality, format_birth, team_style};
use crate::{Route, link};

/// États partagés par les parties de la fiche qui suivent des données arrivées après coup.
#[derive(Clone, Copy)]
struct Live {
    champions: State<Json<Vec<Champion>>>,
    chosen: State<Option<String>>,
    season: State<Option<String>>,
    standing: State<Fetch>,
}

/// Fiche pilote : carrière complète (1950 → aujourd'hui) et détail saison par saison. La
/// page reste affichée quand on passe à un autre pilote : seules ses données changent.
pub fn driver_page(id: State<String>) -> Node {
    // Toute la carrière en une seule requête paginée : départs, victoires, podiums,
    // poles, meilleurs tours, saisons et écuries en sont déduits (économise le quota Jolpica).
    let career = use_f1_dyn(move || all(format!("drivers/{}/results.json", id.get())));
    let champions = use_json::<Vec<Champion>>(Some("/api/champions".into()));
    // Saison choisie dans la liste, sinon la dernière saison du pilote (choix oublié quand
    // on passe à un autre pilote).
    let chosen = use_state(None::<String>);
    effect(move || {
        id.with(|_| ());
        untrack(|| chosen.set(None));
    });
    let season = memo(move || {
        chosen.get().or_else(|| {
            career.with(|f| {
                f.done()
                    .and_then(|d| entries(d.races()).last().map(|(r, _)| r.season.clone()))
            })
        })
    });
    let standing = use_f1_dyn(move || {
        season
            .get()
            .and_then(|s| f1(format!("{s}/drivers/{}/driverStandings.json", id.get()), 1))
    });
    let live = Live {
        champions,
        chosen,
        season,
        standing,
    };
    // Carrière chargée mais vide : pilote inconnu.
    let missing =
        memo(move || career.with(|f| f.done().is_some_and(|d| entries(d.races()).is_empty())));

    // Le titre suit le chargement : la mise en page n'est pas reconstruite à l'arrivée des données.
    let title = move || {
        career
            .with(|f| {
                f.done().and_then(|d| {
                    entries(d.races())
                        .first()
                        .map(|(_, r)| r.driver.full_name())
                })
            })
            .unwrap_or_else(|| t("Pilote", "Driver").into())
    };
    let page = move || {
        layout_dyn(
            title,
            Some(Tab::Standings),
            fetch_view(career, move |data| body(data, live)),
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

/// Contenu de la fiche, construit une seule fois quand la carrière est chargée ; les parties
/// qui dépendent d'autres requêtes (titres, saison choisie) les suivent chacune de leur côté.
fn body(data: &MrData, live: Live) -> Node {
    let races: Rc<Vec<Race>> = Rc::new(data.races().to_vec());
    let entries = entries(&races);
    let Some(driver) = entries.first().map(|(_, r)| r.driver.clone()) else {
        return Node::Empty;
    };

    let mut season_list: Vec<String> = entries.iter().map(|(r, _)| r.season.clone()).collect();
    season_list.dedup();

    let count =
        |f: &dyn Fn(&RaceResult) -> bool| entries.iter().filter(|(_, r)| f(r)).count().to_string();
    let starts = entries.len().to_string();
    let wins = count(&|r| r.position == "1");
    let podiums = count(&|r| matches!(r.position.as_str(), "1" | "2" | "3"));
    let poles = count(&|r| r.grid.as_deref() == Some("1"));
    let fastest = count(&|r| r.has_fastest_lap());
    let mut teams: Vec<Constructor> = Vec::new();
    for (_, r) in &entries {
        if !teams
            .iter()
            .any(|t| t.constructor_id == r.constructor.constructor_id)
        {
            teams.push(r.constructor.clone());
        }
    }
    let latest_team = entries.last().map(|(_, r)| r.constructor.clone());
    let recent = season_list
        .last()
        .and_then(|s| s.parse::<u32>().ok())
        .is_some_and(|y| y + 2 >= current_year());

    let mut eyebrow = Vec::new();
    if let Some(n) = &driver.permanent_number {
        eyebrow.push(format!("#{n}"));
    }
    if let Some(c) = &driver.code {
        eyebrow.push(c.clone());
    }
    if let Some(n) = &driver.nationality {
        eyebrow.push(n.clone());
    }

    let titles = {
        let id = driver.driver_id.clone();
        let champions = live.champions;
        dynamic(move || {
            let Some(Ok(c)) = champions.get() else {
                return Node::Empty;
            };
            match driver_titles(&c, &id).as_slice() {
                titles @ [_, ..] => p()
                    .class("titles")
                    .text(tr!(
                        "🏆 Champion du monde ×{} : {}",
                        "🏆 World champion ×{}: {}",
                        titles.len(),
                        titles.join(", ")
                    ))
                    .into(),
                [] => Node::Empty,
            }
        })
    };

    let hero = section().class("card hero");
    let hero = match &latest_team {
        Some(team) => hero.style(team_style(&team.constructor_id)),
        None => hero,
    };
    let hero = hero
        .child(
            driver
                .url
                .as_ref()
                .map(|url| crate::photo::wiki_photo(url, &driver.full_name(), false)),
        )
        .child(p().class("eyebrow").text(eyebrow.join(" · ")))
        .child(h2().class("hero-title").text(format!(
            "{} {}",
            flag_nationality(driver.nationality.as_deref()),
            driver.full_name()
        )))
        .child(driver.date_of_birth.as_ref().map(|dob| {
            p().class("muted")
                .text(tr!("Né le {}", "Born {}", format_birth(dob)))
                // L'API ne donne pas de date de décès : âge affiché pour les pilotes récents seulement.
                .child(
                    age(dob)
                        .filter(|_| recent)
                        .map(|a| tr!(" · {a} ans", " · {a} years old")),
                )
        }))
        .child(stat_grid(vec![
            (t("Départs", "Starts"), starts),
            (t("Victoires", "Wins"), wins),
            ("Podiums", podiums),
            ("Poles", poles),
            (t("Meilleurs tours", "Fastest laps"), fastest),
            (t("Saisons", "Seasons"), season_list.len().to_string()),
        ]))
        .child(titles)
        .child(fav_button("driver", &driver.driver_id))
        .child(driver.url.as_ref().map(|url| {
            a().class("link")
                .href(url.clone())
                .attr("target", "_blank")
                .attr("rel", "noopener")
                .text(t("Wikipédia ↗", "Wikipedia ↗"))
        }));

    let teams_card = (!teams.is_empty()).then(|| {
        section()
            .class("card")
            .child(h2().text(t("Écuries", "Teams")))
            .child(ol().class("rows").children(teams.iter().map(|c| {
                li().class("row")
                    .style(team_style(&c.constructor_id))
                    .child(link(Route::team(&c.constructor_id), "row-main").child(
                        span().class("row-title").text(format!(
                            "{} {}",
                            flag_nationality(c.nationality.as_deref()),
                            c.name
                        )),
                    ))
            })))
    });

    fragment([
        Node::from(hero),
        driver
            .url
            .as_ref()
            .map(|url| wiki_bio(url, t("À propos", "About")))
            .into(),
        // Monoplace actuelle uniquement (une F1 moderne n'aurait pas de sens pour les pilotes d'antan).
        latest_team
            .as_ref()
            .filter(|_| recent)
            .map(|team| crate::gl3d::car_card(&team.constructor_id, &team.name))
            .into(),
        teams_card.into(),
        season_card(races.clone(), &season_list, live),
    ])
}

/// Carte « Saison par saison » : la liste des saisons est construite une fois, seuls le
/// classement, les courses et le lien suivent la saison choisie.
fn season_card(races: Rc<Vec<Race>>, season_list: &[String], live: Live) -> Node {
    let Live {
        chosen,
        season,
        standing,
        ..
    } = live;
    let initial = untrack(|| chosen.get()).or_else(|| season_list.last().cloned());

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
                .and_then(|l| l.driver_standings.as_ref()?.first().cloned())
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

    // Gardées par position : changer de saison met les lignes à jour sur place.
    let rows = ol().class("rows").children_indexed(
        move || {
            let Some(current) = season.get() else {
                return Vec::new();
            };
            entries(&races)
                .into_iter()
                .rev()
                .filter(|(race, _)| race.season == current)
                .map(|(race, r)| (race.clone(), r.clone()))
                .collect()
        },
        race_row,
    );

    let standings_link = dynamic(move || {
        season
            .get()
            .map(|season| {
                link(
                    Route::DriverStandings {
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
        .child(rows)
        .child(standings_link)
        .into()
}

/// Une course de la saison : position, Grand Prix, écurie, départ, temps ou abandon, points.
/// La ligne suit `item` : elle est mise à jour sur place quand la saison change.
fn race_row(item: State<(Race, RaceResult)>) -> Node {
    let get = move |f: fn(&Race, &RaceResult) -> String| move || item.with(|(race, r)| f(race, r));
    let sub = |_: &Race, r: &RaceResult| {
        let outcome = r.outcome();
        let mut sub = vec![
            r.constructor.name.clone(),
            tr!(
                "départ P{}",
                "started P{}",
                r.grid.as_deref().unwrap_or("-")
            ),
        ];
        if !outcome.is_empty() {
            sub.push(outcome);
        }
        sub.join(" · ")
    };
    li().class("row")
        .attr_dyn(
            "style",
            get(|_, r| team_style(&r.constructor.constructor_id)),
        )
        .child(
            span()
                .class("pos pos-sm")
                .text_dyn(get(|_, r| r.position_text.clone())),
        )
        .child(
            a().class("row-main")
                .attr_dyn(
                    "href",
                    get(|race, _| Route::race(&race.season, race.round_num()).href()),
                )
                .child(span().class("row-title").text_dyn(get(|race, _| {
                    format!(
                        "{} {}",
                        flag_country(&race.circuit.location.country),
                        race.race_name
                    )
                })))
                .child(span().class("row-sub").text_dyn(get(sub))),
        )
        .child(dynamic(move || {
            let points = item.with(|(_, r)| r.points.clone());
            (points.parse::<f64>().unwrap_or(0.0) > 0.0)
                .then(|| span().class("pts").text(format!("+{points}")))
                .into()
        }))
        .into()
}
