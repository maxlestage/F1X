//! Pronostics avant chaque Grand Prix, notés automatiquement après la course.
//! Enregistrés sur l'appareil (aucun compte, aucun serveur).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use web_sys::HtmlSelectElement;
use yew::prelude::*;

use crate::api::{all, f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::models::{Race, is_classified};
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

#[function_component]
pub fn PredictPage() -> Html {
    let schedule = use_f1(f1("current.json", 100));
    let drivers = use_f1(f1("current/drivers.json", 100));
    let results = use_f1(all("current/results.json"));
    let saved = use_state(|| load::<BTreeMap<String, Prediction>>(KEY).unwrap_or_default());
    let draft = use_state(|| None::<Prediction>);
    let flash = use_state(|| false);

    let now = now_ms();
    let races = schedule
        .done()
        .map(|d| d.races().to_vec())
        .unwrap_or_default();
    let next = races.iter().find(|r| !r.is_over(now)).cloned();
    let season = races.first().map(|r| r.season.clone()).unwrap_or_default();
    let mut list = drivers
        .done()
        .map(|d| d.drivers().to_vec())
        .unwrap_or_default();
    list.sort_by(|a, b| a.family_name.cmp(&b.family_name));

    let body = match &next {
        None => fetch_view(&schedule, |_| {
            empty_card(t(
                "Pas de prochain Grand Prix cette saison.",
                "No upcoming Grand Prix this season.",
            ))
        }),
        Some(race) => {
            let key = format!("{}-{}", race.season, race.round);
            let deadline_iso = race
                .qualifying
                .as_ref()
                .map(|q| q.iso())
                .unwrap_or_else(|| race.start_iso());
            let locked = now >= parse_ms(&deadline_iso);
            let current = (*draft)
                .clone()
                .or_else(|| saved.get(&key).cloned())
                .unwrap_or(Prediction {
                    season: race.season.clone(),
                    dnf: 2,
                    ..Default::default()
                });
            let field = |label: &'static str, value: String, set: fn(&mut Prediction, String)| {
                let draft = draft.clone();
                let base = current.clone();
                let onchange = Callback::from(move |e: Event| {
                    let mut p = (*draft).clone().unwrap_or_else(|| base.clone());
                    set(
                        &mut p,
                        e.target_unchecked_into::<HtmlSelectElement>().value(),
                    );
                    draft.set(Some(p));
                });
                html! {
                    <label class="select">
                        <span class="select-label predict-label">{ label }</span>
                        <select {onchange} disabled={locked} aria-label={label}>
                            <option value="" selected={value.is_empty()}>{ "—" }</option>
                            { for list.iter().map(|d| html! {
                                <option value={d.driver_id.clone()} selected={d.driver_id == value}>{ d.full_name() }</option>
                            }) }
                        </select>
                    </label>
                }
            };
            let save = {
                let saved = saved.clone();
                let draft = draft.clone();
                let flash = flash.clone();
                let key = key.clone();
                let current = current.clone();
                Callback::from(move |_| {
                    let mut all = (*saved).clone();
                    all.insert(key.clone(), current.clone());
                    store(KEY, &all);
                    saved.set(all);
                    draft.set(None);
                    flash.set(true);
                })
            };
            let on_dnf = {
                let draft = draft.clone();
                let base = current.clone();
                Callback::from(move |e: Event| {
                    let mut p = (*draft).clone().unwrap_or_else(|| base.clone());
                    p.dnf = e
                        .target_unchecked_into::<HtmlSelectElement>()
                        .value()
                        .parse()
                        .unwrap_or(0);
                    draft.set(Some(p));
                })
            };
            html! {
                <section class="card hero">
                    <p class="eyebrow">{ tr!("Manche {}", "Round {}", race.round) }</p>
                    <h2 class="hero-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</h2>
                    <p class="muted">{ if locked {
                        t("🔒 Pronostics fermés (les qualifications ont commencé).", "🔒 Predictions closed (qualifying has started).").to_string()
                    } else {
                        tr!("Jusqu'au début des qualifications : {}", "Until qualifying starts: {}", local_date(&deadline_iso, true))
                    } }</p>
                    { field(t("Pole", "Pole"), current.pole.clone(), |p, v| p.pole = v) }
                    { field(t("Vainqueur", "Winner"), current.winner.clone(), |p, v| p.winner = v) }
                    { field(t("2e", "2nd"), current.second.clone(), |p, v| p.second = v) }
                    { field(t("3e", "3rd"), current.third.clone(), |p, v| p.third = v) }
                    { field(t("Meilleur tour", "Fastest lap"), current.fastest.clone(), |p, v| p.fastest = v) }
                    <label class="select">
                        <span class="select-label predict-label">{ t("Abandons", "Retirements") }</span>
                        <select onchange={on_dnf} disabled={locked}>
                            { for (0..=10u32).map(|n| html! { <option value={n.to_string()} selected={n == current.dnf}>{ n }</option> }) }
                        </select>
                    </label>
                    if !locked {
                        <button class="btn" onclick={save}>{ if *flash && draft.is_none() { t("Enregistré ✓", "Saved ✓") } else { t("Enregistrer mon pronostic", "Save my prediction") } }</button>
                    }
                </section>
            }
        }
    };

    // Historique et points (saison en cours).
    let finished: Vec<Race> = results
        .done()
        .map(|d| d.races().to_vec())
        .unwrap_or_default();
    let history: Vec<Scored> = finished
        .iter()
        .filter_map(|race| {
            let p = saved.get(&format!("{}-{}", race.season, race.round))?;
            let (pts, detail) = score(p, race)?;
            Some((race.clone(), pts, detail))
        })
        .collect();
    let total: u32 = history.iter().map(|h| h.1).sum();

    html! {
        <Layout title={t("Pronostics", "Predictions")} tab={Tab::Archives}>
            { body }
            <section class="card">
                <h2>{ tr!("Mes points {} : {total}", "My {} points: {total}", season) }</h2>
                if history.is_empty() {
                    <p class="muted">{ t("Tes pronostics notés apparaîtront ici après chaque course.", "Your scored predictions will show up here after each race.") }</p>
                }
                <ol class="rows">
                    { for history.iter().rev().map(|(race, pts, detail)| html! {
                        <li class="row row-plain">
                            <span class="row-main">
                                <span class="row-title">{ format!("{} {}", flag_country(&race.circuit.location.country), race.race_name) }</span>
                                <span class="row-sub">{ detail.iter().map(|(l, ok)| format!("{} {l}", if *ok { "✅" } else { "❌" })).collect::<Vec<_>>().join(" · ") }</span>
                            </span>
                            <span class="pts">{ format!("+{pts}") }</span>
                        </li>
                    }) }
                </ol>
            </section>
            <section class="card">
                <h2>{ t("Barème", "Scoring") }</h2>
                <p class="muted">{ t(
                    "Pole 5 · Vainqueur 10 · 2e et 3e : 6 si exact, 2 si le pilote est sur le podium · Meilleur tour 5 · Abandons : 4 si exact, 2 à ±1. Pole = pilote parti en tête de la grille. Pronostics enregistrés sur ce téléphone.",
                    "Pole 5 · Winner 10 · 2nd and 3rd: 6 if exact, 2 if the driver is on the podium · Fastest lap 5 · Retirements: 4 if exact, 2 if ±1. Pole = driver starting first on the grid. Predictions are stored on this phone.",
                ) }</p>
            </section>
        </Layout>
    }
}
