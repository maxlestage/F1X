//! Pronostics avant chaque Grand Prix, notés automatiquement après la course.
//! Enregistrés sur l'appareil (aucun compte, aucun serveur).

use std::collections::BTreeMap;
use std::rc::Rc;

use active::prelude::*;
use serde::{Deserialize, Serialize};

use crate::api::{all, f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Driver, Race, is_classified};
use crate::tr;
use crate::util::{flag_country, load, local_date, now_ms, parse_ms, store};

const KEY: &str = "f1x-predictions";

/// Course notée : (course, points, détail ✅/❌).
type Scored = (Race, u32, Vec<(String, bool)>);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    pub season: String,
    pub pole: String,
    pub winner: String,
    pub second: String,
    pub third: String,
    pub fastest: String,
    pub dnf: u32,
}

/// Barème : (points, détail) pour un pronostic et le résultat de la course.
fn score(p: &Prediction, race: &Race) -> Option<(u32, Vec<(String, bool)>)> {
    let results = race.results.as_ref()?;
    let by_pos = |n: &str| {
        results
            .iter()
            .find(|r| r.position == n)
            .map(|r| r.driver.driver_id.clone())
            .unwrap_or_default()
    };
    let pole = results
        .iter()
        .find(|r| r.grid.as_deref() == Some("1"))
        .map(|r| r.driver.driver_id.clone())
        .unwrap_or_default();
    let fastest = results
        .iter()
        .find(|r| r.has_fastest_lap())
        .map(|r| r.driver.driver_id.clone());
    let podium = [by_pos("1"), by_pos("2"), by_pos("3")];
    let dnf = results
        .iter()
        .filter(|r| !r.status.as_deref().is_none_or(is_classified) && r.time.is_none())
        .count() as u32;
    let mut pts = 0;
    let mut detail = Vec::new();
    let mut check = |label: String, ok: bool, value: u32| {
        if ok {
            pts += value;
        }
        detail.push((label, ok));
    };
    check(t("Pole (5)", "Pole (5)").into(), p.pole == pole, 5);
    check(
        t("Vainqueur (10)", "Winner (10)").into(),
        p.winner == podium[0],
        10,
    );
    for (i, pick) in [&p.second, &p.third].iter().enumerate() {
        let exact = **pick == podium[i + 1];
        let on_podium = podium.contains(pick);
        let label = if i == 0 {
            t(
                "2e (6, ou 2 si sur le podium)",
                "2nd (6, or 2 if on the podium)",
            )
        } else {
            t(
                "3e (6, ou 2 si sur le podium)",
                "3rd (6, or 2 if on the podium)",
            )
        };
        check(label.into(), exact || on_podium, if exact { 6 } else { 2 });
    }
    if let Some(f) = fastest {
        check(
            t("Meilleur tour (5)", "Fastest lap (5)").into(),
            p.fastest == f,
            5,
        );
    }
    let diff = p.dnf.abs_diff(dnf);
    check(
        tr!(
            "Abandons : {dnf} (4 si exact, 2 à ±1)",
            "Retirements: {dnf} (4 if exact, 2 if ±1)"
        ),
        diff <= 1,
        if diff == 0 { 4 } else { 2 },
    );
    Some((pts, detail))
}

pub fn predict_page() -> Node {
    let schedule = use_f1(f1("current.json", 100));
    let drivers = use_f1(f1("current/drivers.json", 100));
    let results = use_f1(all("current/results.json"));
    let saved = use_state(load::<BTreeMap<String, Prediction>>(KEY).unwrap_or_default());
    let draft = use_state(None::<Prediction>);
    let flash = use_state(false);

    // Pilotes de la saison, par nom de famille (arrivent après le calendrier).
    let list = memo(move || {
        let mut list = drivers.with(|f| f.done().map(|d| d.drivers().to_vec()).unwrap_or_default());
        list.sort_by(|a, b| a.family_name.cmp(&b.family_name));
        list
    });

    // Reconstruit seulement à l'arrivée du calendrier : un choix ne touche que la valeur.
    let body = fetch_view(schedule, move |data| {
        let now = now_ms();
        match data.races().iter().find(|r| !r.is_over(now)) {
            None => empty_card(t(
                "Pas de prochain Grand Prix cette saison.",
                "No upcoming Grand Prix this season.",
            )),
            Some(race) => prediction_card(race, list, saved, draft, flash),
        }
    });

    // Historique et points (saison en cours).
    let history = memo(move || {
        results.with(|f| {
            f.done()
                .map(|d| {
                    d.races()
                        .iter()
                        .filter_map(|race| {
                            let key = format!("{}-{}", race.season, race.round);
                            let p = saved.with(|s| s.get(&key).cloned())?;
                            let (pts, detail) = score(&p, race)?;
                            Some((race.clone(), pts, detail))
                        })
                        .collect::<Vec<Scored>>()
                })
                .unwrap_or_default()
        })
    });

    layout(
        t("Pronostics", "Predictions"),
        Some(Tab::Archives),
        fragment([
            body,
            section()
                .class("card")
                .child(h2().text_dyn(move || {
                    let season = schedule.with(|f| {
                        f.done()
                            .and_then(|d| d.races().first().map(|r| r.season.clone()))
                            .unwrap_or_default()
                    });
                    let total: u32 = history.with(|h| h.iter().map(|h| h.1).sum());
                    tr!("Mes points {} : {total}", "My {} points: {total}", season)
                }))
                .child(dynamic(move || {
                    history
                        .with(Vec::is_empty)
                        .then(|| {
                            p().class("muted").text(t(
                                "Tes pronostics notés apparaîtront ici après chaque course.",
                                "Your scored predictions will show up here after each race.",
                            ))
                        })
                        .into()
                }))
                .child(ol().class("rows").children_dyn(move || {
                    history.with(|h| h.iter().rev().map(history_row).collect())
                }))
                .into(),
            section()
                .class("card")
                .child(h2().text(t("Barème", "Scoring")))
                .child(p().class("muted").text(t(
                    "Pole 5 · Vainqueur 10 · 2e et 3e : 6 si exact, 2 si le pilote est sur le podium · Meilleur tour 5 · Abandons : 4 si exact, 2 à ±1. Pole = pilote parti en tête de la grille. Pronostics enregistrés sur ce téléphone.",
                    "Pole 5 · Winner 10 · 2nd and 3rd: 6 if exact, 2 if the driver is on the podium · Fastest lap 5 · Retirements: 4 if exact, 2 if ±1. Pole = driver starting first on the grid. Predictions are stored on this phone.",
                )))
                .into(),
        ]),
    )
}

/// Le prochain Grand Prix et le formulaire de pronostic.
fn prediction_card(
    race: &Race,
    list: State<Vec<Driver>>,
    saved: State<BTreeMap<String, Prediction>>,
    draft: State<Option<Prediction>>,
    flash: State<bool>,
) -> Node {
    let key = format!("{}-{}", race.season, race.round);
    let deadline_iso = race
        .qualifying
        .as_ref()
        .map(|q| q.iso())
        .unwrap_or_else(|| race.start_iso());
    let deadline = parse_ms(&deadline_iso);
    // Fermé dès le début des qualifications ; revérifié à chaque choix et à l'enregistrement.
    let locked = use_state(now_ms() >= deadline);
    let relock = move || {
        if now_ms() >= deadline && !locked.get() {
            locked.set(true);
        }
    };
    let current: Rc<dyn Fn() -> Prediction> = {
        let key = key.clone();
        let season = race.season.clone();
        Rc::new(move || {
            draft
                .get()
                .or_else(|| saved.with(|s| s.get(&key).cloned()))
                .unwrap_or(Prediction {
                    season: season.clone(),
                    dnf: 2,
                    ..Default::default()
                })
        })
    };
    let field = |caption: &'static str,
                 get: fn(&Prediction) -> &String,
                 set: fn(&mut Prediction, String)| {
        let on_change = {
            let current = current.clone();
            move |e: Event| {
                let mut p = current();
                set(&mut p, e.value());
                draft.set(Some(p));
                relock();
            }
        };
        // Options reconstruites quand la liste des pilotes arrive, le choix courant marqué.
        let options = {
            let current = current.clone();
            move || {
                let value = untrack(|| get(&current()).clone());
                let selected = |o: Element, on: bool| if on { o.attr("selected", "") } else { o };
                std::iter::once(selected(option().attr("value", ""), value.is_empty()).text("—"))
                    .chain(list.with(|l| {
                        l.iter()
                            .map(|d| {
                                selected(
                                    option().attr("value", d.driver_id.clone()),
                                    d.driver_id == value,
                                )
                                .text(d.full_name())
                            })
                            .collect::<Vec<_>>()
                    }))
                    .map(Node::from)
                    .collect()
            }
        };
        label()
            .class("select")
            .child(span().class("select-label predict-label").text(caption))
            .child(
                select()
                    .on("change", on_change)
                    .bool_attr("disabled", move || locked.get())
                    .attr("aria-label", caption)
                    .children_dyn(options),
            )
    };
    let on_dnf = {
        let current = current.clone();
        move |e: Event| {
            let mut p = current();
            p.dnf = e.value().parse().unwrap_or(0);
            draft.set(Some(p));
            relock();
        }
    };
    let save = {
        let current = current.clone();
        move |_: Event| {
            let prediction = current();
            let mut all = saved.get();
            all.insert(key.clone(), prediction);
            store(KEY, &all);
            saved.set(all);
            draft.set(None);
            flash.set(true);
            relock();
        }
    };
    let dnf = untrack(|| current().dnf);
    section()
        .class("card hero")
        .child(
            p().class("eyebrow")
                .text(tr!("Manche {}", "Round {}", race.round)),
        )
        .child(h2().class("hero-title").text(format!(
            "{} {}",
            flag_country(&race.circuit.location.country),
            race.race_name
        )))
        .child(p().class("muted").text_dyn(move || {
            if locked.get() {
                t(
                    "🔒 Pronostics fermés (les qualifications ont commencé).",
                    "🔒 Predictions closed (qualifying has started).",
                )
                .to_string()
            } else {
                tr!(
                    "Jusqu'au début des qualifications : {}",
                    "Until qualifying starts: {}",
                    local_date(&deadline_iso, true)
                )
            }
        }))
        .child(field(t("Pole", "Pole"), |p| &p.pole, |p, v| p.pole = v))
        .child(field(
            t("Vainqueur", "Winner"),
            |p| &p.winner,
            |p, v| p.winner = v,
        ))
        .child(field(t("2e", "2nd"), |p| &p.second, |p, v| p.second = v))
        .child(field(t("3e", "3rd"), |p| &p.third, |p, v| p.third = v))
        .child(field(
            t("Meilleur tour", "Fastest lap"),
            |p| &p.fastest,
            |p, v| p.fastest = v,
        ))
        .child(
            label()
                .class("select")
                .child(
                    span()
                        .class("select-label predict-label")
                        .text(t("Abandons", "Retirements")),
                )
                .child(
                    select()
                        .on("change", on_dnf)
                        .bool_attr("disabled", move || locked.get())
                        .children((0..=10u32).map(|n| {
                            let o = option().attr("value", n.to_string());
                            let o = if n == dnf { o.attr("selected", "") } else { o };
                            o.text(n.to_string())
                        })),
                ),
        )
        .child(dynamic(move || {
            (!locked.get())
                .then(|| {
                    button()
                        .class("btn")
                        .on_click(save.clone())
                        .text_dyn(move || {
                            if flash.get() && draft.with(Option::is_none) {
                                t("Enregistré ✓", "Saved ✓")
                            } else {
                                t("Enregistrer mon pronostic", "Save my prediction")
                            }
                            .to_string()
                        })
                })
                .into()
        }))
        .into()
}

fn history_row((race, pts, detail): &Scored) -> Node {
    li().class("row row-plain")
        .child(
            span()
                .class("row-main")
                .child(span().class("row-title").text(format!(
                    "{} {}",
                    flag_country(&race.circuit.location.country),
                    race.race_name
                )))
                .child(
                    span().class("row-sub").text(
                        detail
                            .iter()
                            .map(|(l, ok)| format!("{} {l}", if *ok { "✅" } else { "❌" }))
                            .collect::<Vec<_>>()
                            .join(" · "),
                    ),
                ),
        )
        .child(span().class("pts").text(format!("+{pts}")))
        .into()
}
