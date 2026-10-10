//! « Devine le pilote » : statistiques anonymes, 4 propositions.

use std::collections::HashMap;
use std::rc::Rc;

use active::prelude::*;

use super::stats::{Champion, tally};
use crate::api::{Fetch, Json, all, use_f1, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::models::RaceResult;
use crate::tr;
use crate::util::{flag_nationality, load, random, store};
use crate::{Route, link};

#[derive(Clone, PartialEq)]
struct Profile {
    id: String,
    name: String,
    nationality: Option<String>,
    wins: u32,
    podiums: u32,
    poles: u32,
    titles: u32,
    first_win: String,
    last_win: String,
    team: String,
}

#[derive(Clone, PartialEq)]
struct Question {
    answer: usize,
    options: Vec<usize>,
}

fn new_question(pool: usize) -> Question {
    let answer = random(pool);
    let mut options = vec![answer];
    while options.len() < 4.min(pool) {
        let o = random(pool);
        if !options.contains(&o) {
            options.push(o);
        }
    }
    // Mélange (Fisher-Yates).
    for i in (1..options.len()).rev() {
        options.swap(i, random(i + 1));
    }
    Question { answer, options }
}

/// Pilotes à deviner (au moins 3 victoires), une fois les 4 listes de résultats chargées.
fn profiles(
    [wins, seconds, thirds, poles]: [State<Fetch>; 4],
    champions: State<Json<Vec<Champion>>>,
) -> Vec<Profile> {
    let ready = [wins, seconds, thirds, poles]
        .iter()
        .all(|f| f.with(|f| f.done().is_some()));
    if !ready {
        return Vec::new();
    }
    let firsts = |f: State<Fetch>| -> Vec<(String, RaceResult)> {
        f.with(|f| {
            f.done()
                .map(|d| {
                    d.races()
                        .iter()
                        .filter_map(|r| {
                            Some((r.season.clone(), r.results.as_ref()?.first()?.clone()))
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
    };
    let w = firsts(wins);
    let count = |list: &[(String, RaceResult)]| -> HashMap<String, u32> {
        let mut m = HashMap::new();
        for (_, r) in list {
            *m.entry(r.driver.driver_id.clone()).or_insert(0) += 1;
        }
        m
    };
    let (p2, p3, pl) = (
        count(&firsts(seconds)),
        count(&firsts(thirds)),
        count(&firsts(poles)),
    );
    let champs: Vec<Champion> = champions.with(|c| match c {
        Some(Ok(c)) => c.iter().filter(|c| !c.in_progress).cloned().collect(),
        _ => Vec::new(),
    });
    tally(
        w.iter()
            .map(|(_, r)| (r.driver.driver_id.clone(), r.driver.full_name())),
    )
    .into_iter()
    .filter(|(_, _, n)| *n >= 3)
    .map(|(id, name, n)| {
        let mine: Vec<&(String, RaceResult)> =
            w.iter().filter(|(_, r)| r.driver.driver_id == id).collect();
        let team = tally(mine.iter().map(|(_, r)| {
            (
                r.constructor.constructor_id.clone(),
                r.constructor.name.clone(),
            )
        }))
        .first()
        .map(|t| t.1.clone())
        .unwrap_or_default();
        Profile {
            nationality: mine.first().and_then(|(_, r)| r.driver.nationality.clone()),
            wins: n,
            podiums: n + p2.get(&id).copied().unwrap_or(0) + p3.get(&id).copied().unwrap_or(0),
            poles: pl.get(&id).copied().unwrap_or(0),
            titles: champs.iter().filter(|c| c.driver.driver_id == id).count() as u32,
            first_win: mine.first().map(|m| m.0.clone()).unwrap_or_default(),
            last_win: mine.last().map(|m| m.0.clone()).unwrap_or_default(),
            team,
            id,
            name,
        }
    })
    .collect()
}

/// Valeur d'une case de statistiques, recalculée quand les états qu'elle lit changent.
type Value = Rc<dyn Fn() -> String>;

/// Comme `stat_grid`, avec des valeurs qui suivent les états (mises à jour sur place).
fn stat_grid_dyn(items: Vec<(&'static str, Value)>) -> Node {
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

pub fn quiz_page() -> Node {
    let wins = use_f1(all("results/1.json"));
    let seconds = use_f1(all("results/2.json"));
    let thirds = use_f1(all("results/3.json"));
    let poles = use_f1(all("grid/1/results.json"));
    let champions = use_json::<Vec<Champion>>(Some("/api/champions".into()));
    let question = use_state(None::<Question>);
    let picked = use_state(None::<usize>);
    let score = use_state((0u32, 0u32)); // (bonnes réponses, questions)
    let streak = use_state(0u32);
    let best = use_state(load::<u32>("f1x-quiz-best").unwrap_or(0));

    let ready = memo(move || {
        [wins, seconds, thirds, poles]
            .iter()
            .all(|f| f.with(|f| f.done().is_some()))
    });
    let pool = memo(move || profiles([wins, seconds, thirds, poles], champions));
    let size = memo(move || pool.with(Vec::len));

    // Première question dès que les données sont prêtes.
    effect(move || {
        let n = size.get();
        untrack(|| {
            if question.with(Option::is_none) && n >= 4 {
                question.set(Some(new_question(n)));
            }
        });
    });

    let answer = move || question.with(|q| q.as_ref().map(|q| q.answer));
    // Le pilote à deviner (suit la question, et ses titres quand les champions arrivent).
    let target = memo(move || answer().and_then(|i| pool.with(|p| p.get(i).cloned())));
    let shown = memo(move || answer().is_some_and(|a| a < size.get()));

    // Construit une fois la première question prête : les questions suivantes et les réponses
    // ne changent que les textes et les classes.
    let body = dynamic(move || match (shown.get(), ready.get()) {
        (_, false) => section()
            .class("card")
            .child(p().class("muted").text(t(
                "Chargement de 75 ans de résultats…",
                "Loading 75 years of results…",
            )))
            .child(loading())
            .into(),
        (true, true) => {
            let stat = move |f: fn(&Profile) -> String| -> Value {
                Rc::new(move || target.with(|p| p.as_ref().map(f).unwrap_or_default()))
            };
            let titles: Value = Rc::new(move || {
                if champions.with(|c| c.as_ref().is_some_and(|c| c.is_ok())) {
                    target.with(|p| p.as_ref().map(|p| p.titles.to_string()).unwrap_or_default())
                } else {
                    "?".into()
                }
            });
            let hero = section()
                .class("card hero")
                .child(p().class("eyebrow").text(t("Qui suis-je ?", "Who am I?")))
                .child(stat_grid_dyn(vec![
                    (t("Titres", "Titles"), titles),
                    (t("Victoires", "Wins"), stat(|p| p.wins.to_string())),
                    (t("Podiums", "Podiums"), stat(|p| p.podiums.to_string())),
                    ("Poles", stat(|p| p.poles.to_string())),
                    (
                        t("1re victoire", "First win"),
                        stat(|p| p.first_win.clone()),
                    ),
                    (t("Dernière", "Last win"), stat(|p| p.last_win.clone())),
                ]))
                .child(p().class("muted").text_dyn(move || {
                    let team =
                        target.with(|p| p.as_ref().map(|p| p.team.clone()).unwrap_or_default());
                    tr!(
                        "Écurie de la plupart de ses victoires : {}",
                        "Team for most of the wins: {}",
                        team
                    )
                }));
            let count = untrack(|| question.with(|q| q.as_ref().map_or(0, |q| q.options.len())));
            let options = div().class("quiz-options").children((0..count).map(|i| {
                let choice =
                    move || question.with(|q| q.as_ref().and_then(|q| q.options.get(i).copied()));
                let state = move || {
                    let o = choice();
                    match picked.get() {
                        None => "",
                        Some(_) if o == answer() => "quiz-right",
                        Some(x) if Some(x) == o => "quiz-wrong",
                        _ => "quiz-dim",
                    }
                };
                let on_click = move |_: Event| {
                    if picked.with(Option::is_some) {
                        return;
                    }
                    let (Some(o), Some(a)) = (choice(), answer()) else {
                        return;
                    };
                    let right = o == a;
                    picked.set(Some(o));
                    score.update(|s| {
                        s.0 += u32::from(right);
                        s.1 += 1;
                    });
                    let s = if right { streak.get() + 1 } else { 0 };
                    streak.set(s);
                    if s > best.get() {
                        best.set(s);
                        store("f1x-quiz-best", &s);
                    }
                };
                button()
                    .class("quiz-option")
                    .class_if("quiz-right", move || state() == "quiz-right")
                    .class_if("quiz-wrong", move || state() == "quiz-wrong")
                    .class_if("quiz-dim", move || state() == "quiz-dim")
                    .on_click(on_click)
                    .text_dyn(move || {
                        let Some(o) = choice() else {
                            return String::new();
                        };
                        pool.with(|p| {
                            let Some(p) = p.get(o) else {
                                return String::new();
                            };
                            let mark = if picked.with(Option::is_some) {
                                flag_nationality(p.nationality.as_deref())
                            } else {
                                "🏎️"
                            };
                            format!("{mark} {}", p.name)
                        })
                    })
            }));
            let result = dynamic(move || {
                let Some(x) = picked.get() else {
                    return Node::Empty;
                };
                let (right, name, id) = untrack(|| {
                    let right = Some(x) == answer();
                    target.with(|p| {
                        let (name, id) = p
                            .as_ref()
                            .map(|p| (p.name.clone(), p.id.clone()))
                            .unwrap_or_default();
                        (right, name, id)
                    })
                });
                section()
                    .class("card")
                    .child(p().class("projection").text(if right {
                        t("✅ Bonne réponse !", "✅ Correct!")
                    } else {
                        t("❌ Raté…", "❌ Wrong…")
                    }))
                    .child(p().text(tr!("C'était {}.", "It was {}.", name)))
                    .child(
                        link(Route::driver(&id), "link")
                            .text(t("Voir sa fiche →", "See the driver page →")),
                    )
                    .child(
                        button()
                            .class("btn")
                            .on_click(move |_| {
                                picked.set(None);
                                question.set(Some(new_question(size.get())));
                            })
                            .text(t("Question suivante", "Next question")),
                    )
                    .into()
            });
            fragment([Node::from(hero), options.into(), result])
        }
        _ => loading(),
    });

    layout(
        t("Devine le pilote", "Guess the driver"),
        Some(Tab::Archives),
        fragment([
            div()
                .class("live-bar")
                .child(span().text_dyn(move || {
                    let (right, total) = score.get();
                    tr!("Score : {}/{}", "Score: {}/{}", right, total)
                }))
                .child(span().class("live-viewers").text_dyn(move || {
                    tr!(
                        "Série : {} · record {}",
                        "Streak: {} · best {}",
                        streak.get(),
                        best.get()
                    )
                }))
                .into(),
            body,
        ]),
    )
}
