//! « Devine le pilote » : statistiques anonymes, 4 propositions.

use std::collections::HashMap;

use yew::prelude::*;
use yew_router::prelude::*;

use super::stats::{Champion, tally};
use crate::Route;
use crate::api::{all, use_f1, use_json};
use crate::components::*;
use crate::i18n::t;
use crate::tr;
use crate::util::{flag_nationality, load, random, store};

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

#[function_component]
pub fn QuizPage() -> Html {
    let wins = use_f1(all("results/1.json"));
    let seconds = use_f1(all("results/2.json"));
    let thirds = use_f1(all("results/3.json"));
    let poles = use_f1(all("grid/1/results.json"));
    let champions = use_json::<Vec<Champion>>(Some("/api/champions".into()));
    let question = use_state(|| None::<Question>);
    let picked = use_state(|| None::<usize>);
    let score = use_state(|| (0u32, 0u32)); // (bonnes réponses, questions)
    let streak = use_state(|| 0u32);
    let best = use_state(|| load::<u32>("f1x-quiz-best").unwrap_or(0));

    let ready = wins.done().is_some()
        && seconds.done().is_some()
        && thirds.done().is_some()
        && poles.done().is_some();
    let pool: Vec<Profile> = if ready {
        let firsts = |f: &crate::api::Fetch| -> Vec<(String, crate::models::RaceResult)> {
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
        };
        let w = firsts(&wins);
        let count = |list: &[(String, crate::models::RaceResult)]| -> HashMap<String, u32> {
            let mut m = HashMap::new();
            for (_, r) in list {
                *m.entry(r.driver.driver_id.clone()).or_insert(0) += 1;
            }
            m
        };
        let (p2, p3, pl) = (
            count(&firsts(&seconds)),
            count(&firsts(&thirds)),
            count(&firsts(&poles)),
        );
        let champs: Vec<Champion> = match &champions {
            Some(Ok(c)) => c.iter().filter(|c| !c.in_progress).cloned().collect(),
            _ => Vec::new(),
        };
        tally(
            w.iter()
                .map(|(_, r)| (r.driver.driver_id.clone(), r.driver.full_name())),
        )
        .into_iter()
        .filter(|(_, _, n)| *n >= 3)
        .map(|(id, name, n)| {
            let mine: Vec<&(String, crate::models::RaceResult)> =
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
    } else {
        Vec::new()
    };

    // Première question dès que les données sont prêtes.
    if question.is_none() && pool.len() >= 4 {
        question.set(Some(new_question(pool.len())));
    }

    let next = {
        let question = question.clone();
        let picked = picked.clone();
        let n = pool.len();
        Callback::from(move |_| {
            picked.set(None);
            question.set(Some(new_question(n)));
        })
    };

    let body = match (&*question, ready) {
        (_, false) => {
            html! { <section class="card"><p class="muted">{ t("Chargement de 75 ans de résultats…", "Loading 75 years of results…") }</p>{ loading() }</section> }
        }
        (Some(q), true) if q.answer < pool.len() => {
            let target = &pool[q.answer];
            html! {
                <>
                    <section class="card hero">
                        <p class="eyebrow">{ t("Qui suis-je ?", "Who am I?") }</p>
                        { stat_grid(vec![
                            (t("Titres", "Titles"), if champions.as_ref().is_some_and(|c| c.is_ok()) { target.titles.to_string() } else { "?".into() }),
                            (t("Victoires", "Wins"), target.wins.to_string()),
                            (t("Podiums", "Podiums"), target.podiums.to_string()),
                            ("Poles", target.poles.to_string()),
                            (t("1re victoire", "First win"), target.first_win.clone()),
                            (t("Dernière", "Last win"), target.last_win.clone()),
                        ]) }
                        <p class="muted">{ tr!("Écurie de la plupart de ses victoires : {}", "Team for most of the wins: {}", target.team) }</p>
                    </section>
                    <div class="quiz-options">
                        { for q.options.iter().map(|&o| {
                            let p = &pool[o];
                            let state = match *picked {
                                None => "",
                                Some(_) if o == q.answer => "quiz-right",
                                Some(x) if x == o => "quiz-wrong",
                                _ => "quiz-dim",
                            };
                            let onclick = {
                                let picked = picked.clone();
                                let score = score.clone();
                                let streak = streak.clone();
                                let best = best.clone();
                                let right = o == q.answer;
                                let answered = picked.is_some();
                                Callback::from(move |_| {
                                    if answered {
                                        return;
                                    }
                                    picked.set(Some(o));
                                    score.set((score.0 + right as u32, score.1 + 1));
                                    let s = if right { *streak + 1 } else { 0 };
                                    streak.set(s);
                                    if s > *best {
                                        best.set(s);
                                        store("f1x-quiz-best", &s);
                                    }
                                })
                            };
                            html! {
                                <button class={classes!("quiz-option", state)} {onclick}>
                                    { if picked.is_some() { flag_nationality(p.nationality.as_deref()) } else { "🏎️" } }{ " " }{ &p.name }
                                </button>
                            }
                        }) }
                    </div>
                    if let Some(x) = *picked {
                        <section class="card">
                            <p class="projection">{ if x == q.answer { t("✅ Bonne réponse !", "✅ Correct!") } else { t("❌ Raté…", "❌ Wrong…") } }</p>
                            <p>{ tr!("C'était {}.", "It was {}.", target.name) }</p>
                            <Link<Route> to={Route::driver(&target.id)} classes="link">{ t("Voir sa fiche →", "See the driver page →") }</Link<Route>>
                            <button class="btn" onclick={next.clone()}>{ t("Question suivante", "Next question") }</button>
                        </section>
                    }
                </>
            }
        }
        _ => loading(),
    };

    html! {
        <Layout title={t("Devine le pilote", "Guess the driver")} tab={Tab::Archives}>
            <div class="live-bar">
                <span>{ tr!("Score : {}/{}", "Score: {}/{}", score.0, score.1) }</span>
                <span class="live-viewers">{ tr!("Série : {} · record {}", "Streak: {} · best {}", *streak, *best) }</span>
            </div>
            { body }
        </Layout>
    }
}
