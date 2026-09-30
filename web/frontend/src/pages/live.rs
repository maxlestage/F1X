use std::rc::Rc;

use f1x_protocol::{Car, ClientMsg, Mode, SessionSummary, Snapshot, TrackStatus, format_lap};
use gloo_net::http::Request;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

use crate::api::{f1, use_f1};
use crate::components::*;
use crate::live::use_live;
use crate::util::{current_year, flag_country, local_date, now_ms};

const SPEEDS: [u32; 6] = [1, 2, 5, 10, 30, 60];

fn session_fr(name: &str) -> String {
    match name {
        "Race" => "Course".into(),
        "Qualifying" => "Qualifications".into(),
        "Sprint" => "Sprint".into(),
        "Sprint Qualifying" | "Sprint Shootout" => "Qualifs sprint".into(),
        n if n.starts_with("Practice ") => format!("Essais libres {}", &n[9..]),
        n => n.to_string(),
    }
}

fn clock_local(iso: &str) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    if d.get_time().is_nan() {
        return String::new();
    }
    format!(
        "{:02}:{:02}:{:02}",
        d.get_hours(),
        d.get_minutes(),
        d.get_seconds()
    )
}

/// Sessions rejouables d'une année.
#[hook]
fn use_sessions(year: u32) -> Option<Result<Vec<SessionSummary>, String>> {
    let state = use_state(|| None);
    {
        let state = state.clone();
        use_effect_with(year, move |year| {
            state.set(None);
            let url = format!("/api/live/sessions/{year}");
            wasm_bindgen_futures::spawn_local(async move {
                let result = match Request::get(&url).send().await {
                    Ok(r) if r.ok() => r
                        .json::<Vec<SessionSummary>>()
                        .await
                        .map_err(|e| e.to_string()),
                    Ok(r) => Err(format!("Sessions indisponibles ({})", r.status())),
                    Err(e) => Err(e.to_string()),
                };
                state.set(Some(result));
            });
        });
    }
    (*state).clone()
}

#[function_component]
pub fn LivePage() -> Html {
    let live = use_live();
    let s = &*live.state;

    let body = match &s.snapshot {
        Some(snap) => html! { <Board snapshot={Rc::clone(snap)} send={live.send.clone()} /> },
        None => html! { <Lobby send={live.send.clone()} /> },
    };

    html! {
        <Layout title="Direct" tab={Tab::Live}>
            <div class="live-bar">
                <span class={classes!("dot", s.connected.then_some("dot-on"))} aria-hidden="true"></span>
                <span>{ if s.connected { "Connecté en temps réel" } else { "Connexion…" } }</span>
                <span class="live-viewers">{ format!("👥 {} en ligne", s.viewers) }</span>
            </div>
            if s.snapshot.is_none() {
                <section class={classes!("card", s.live_active.then_some("hero"))}>
                    <h2>{ if s.live_active { "🔴 Session en cours" } else { "Direct" } }</h2>
                    <p class="muted">{ &s.status }</p>
                    if s.live_active {
                        <button class="btn" onclick={let send = live.send.clone(); move |_| send.emit(ClientMsg::Live)}>{ "Suivre le direct" }</button>
                    }
                </section>
            }
            if let Some(message) = &s.loading { <section class="card"><p class="muted">{ message }</p>{ loading() }</section> }
            if let Some(message) = &s.error { <ErrorCard message={message.clone()} /> }
            { body }
        </Layout>
    }
}

#[derive(Properties, PartialEq)]
struct LobbyProps {
    send: Callback<ClientMsg>,
}

/// Écran d'accueil du direct : prochaine session + choix d'un replay.
#[function_component]
fn Lobby(props: &LobbyProps) -> Html {
    let this_year = current_year();
    let year = use_state(|| this_year);
    let chosen = use_state(|| None::<u32>);
    let speed = use_state(|| 30u32);
    let sessions = use_sessions(*year);
    let schedule = use_f1(f1("current.json", 100));

    // Prochaine session du week-end (Jolpica).
    let now = now_ms();
    let next_session = schedule.done().and_then(|d| {
        d.races().iter().find_map(|race| {
            race.sessions()
                .into_iter()
                .find(|(_, iso)| crate::util::parse_ms(iso) > now)
                .map(|(name, iso)| (race.clone(), name, iso))
        })
    });

    let list: Vec<SessionSummary> = match &sessions {
        Some(Ok(list)) => list.iter().rev().cloned().collect(),
        _ => Vec::new(),
    };
    let selected = (*chosen).or_else(|| list.first().map(|s| s.session_key));

    let on_year = {
        let year = year.clone();
        let chosen = chosen.clone();
        Callback::from(move |e: Event| {
            if let Ok(y) = e
                .target_unchecked_into::<HtmlSelectElement>()
                .value()
                .parse()
            {
                year.set(y);
                chosen.set(None);
            }
        })
    };
    let on_session = {
        let chosen = chosen.clone();
        Callback::from(move |e: Event| {
            chosen.set(
                e.target_unchecked_into::<HtmlSelectElement>()
                    .value()
                    .parse()
                    .ok(),
            )
        })
    };
    let start = {
        let send = props.send.clone();
        let speed = *speed;
        Callback::from(move |_| {
            if let Some(session_key) = selected {
                send.emit(ClientMsg::Replay { session_key, speed });
            }
        })
    };

    html! {
        <>
            if let Some((race, name, iso)) = next_session {
                <section class="card">
                    <p class="eyebrow">{ "Prochaine session" }</p>
                    <h2>{ format!("{} {} — {name}", flag_country(&race.circuit.location.country), race.race_name) }</h2>
                    <p class="muted">{ local_date(&iso, true) }</p>
                    <Countdown target_ms={crate::util::parse_ms(&iso)} />
                </section>
            }

            <section class="card">
                <h2>{ "Rejouer une session" }</h2>
                <p class="muted">{ "Revis n'importe quelle session depuis 2023 comme en direct : classement, écarts, pneus, arrêts, drapeaux, météo et direction de course, en temps réel ou en accéléré." }</p>
                <label class="select">
                    <span class="select-label">{ "Année" }</span>
                    <select onchange={on_year} aria-label="Année">
                        { for (2023..=this_year).rev().map(|y| html! { <option value={y.to_string()} selected={y == *year}>{ y }</option> }) }
                    </select>
                </label>
                { match &sessions {
                    None => loading(),
                    Some(Err(e)) => html! { <p class="muted">{ e }</p> },
                    Some(Ok(_)) if list.is_empty() => html! { <p class="muted">{ "Aucune session disponible pour cette année." }</p> },
                    Some(Ok(_)) => html! {
                        <label class="select">
                            <span class="select-label">{ "Session" }</span>
                            <select onchange={on_session} aria-label="Session">
                                { for list.iter().map(|s| html! {
                                    <option value={s.session_key.to_string()} selected={Some(s.session_key) == selected}>
                                        { format!("{} · {} — {}", local_date(&s.date_start, false), s.location, session_fr(&s.session_name)) }
                                    </option>
                                }) }
                            </select>
                        </label>
                    },
                } }
                <p class="select-label">{ "Vitesse" }</p>
                <div class="speed-grid">
                    { for SPEEDS.iter().map(|v| {
                        let speed = speed.clone();
                        let v = *v;
                        html! {
                            <button class={classes!("seg", (*speed == v).then_some("seg-active"))} onclick={move |_| speed.set(v)}>
                                { format!("×{v}") }
                            </button>
                        }
                    }) }
                </div>
                <button class="btn" onclick={start} disabled={selected.is_none()}>{ "▶ Lancer le replay" }</button>
            </section>
        </>
    }
}

#[derive(Properties, PartialEq)]
struct BoardProps {
    snapshot: Rc<Snapshot>,
    send: Callback<ClientMsg>,
}

fn track_banner(status: TrackStatus) -> (&'static str, &'static str) {
    match status {
        TrackStatus::Green => ("ts-green", "Piste dégagée"),
        TrackStatus::Yellow => ("ts-yellow", "Drapeau jaune"),
        TrackStatus::SafetyCar => ("ts-sc", "Voiture de sécurité"),
        TrackStatus::VirtualSafetyCar => ("ts-sc", "Voiture de sécurité virtuelle"),
        TrackStatus::Red => ("ts-red", "Drapeau rouge"),
        TrackStatus::Chequered => ("ts-chequered", "Drapeau à damier"),
    }
}

/// Tableau de bord temps réel (direct ou replay).
#[function_component]
fn Board(props: &BoardProps) -> Html {
    let snap = &props.snapshot;
    let send = props.send.clone();
    let cmd = |m: ClientMsg| {
        let send = send.clone();
        Callback::from(move |_| send.emit(m.clone()))
    };
    let (ts_class, ts_label) = track_banner(snap.track_status);
    let is_replay = snap.mode == Mode::Replay;
    let lap = match snap.total_laps {
        Some(total) if snap.lap > 0 => format!("Tour {}/{total}", snap.lap.min(total)),
        _ if snap.lap > 0 => format!("Tour {}", snap.lap),
        _ => "Avant le départ".into(),
    };

    html! {
        <>
            <section class="card hero">
                <div class="card-head">
                    <span class={classes!("badge", if is_replay { "badge" } else { "badge-live" })}>
                        { if is_replay { format!("Replay ×{}", snap.speed) } else { "● En direct".into() } }
                    </span>
                    <span class="muted">{ clock_local(&snap.clock) }</span>
                </div>
                <h2 class="hero-title">{ format!("{} {} — {}", flag_country(&snap.session.country), snap.session.location, session_fr(&snap.session.session_name)) }</h2>
                <p class="lap-counter">{ lap }{ if snap.finished { " · Terminé" } else if snap.paused { " · En pause" } else { "" } }</p>
                <div class={classes!("track-status", ts_class)} role="status">{ ts_label }</div>
                if is_replay {
                    <div class="progress" aria-label="Avancement du replay">
                        <span style={format!("width:{:.1}%", snap.progress * 100.0)}></span>
                    </div>
                    <div class="speed-grid">
                        { for SPEEDS.iter().map(|v| html! {
                            <button class={classes!("seg", (snap.speed == *v).then_some("seg-active"))} onclick={cmd(ClientMsg::Speed { speed: *v })}>
                                { format!("×{v}") }
                            </button>
                        }) }
                    </div>
                    <div class="controls">
                        <button class="btn btn-ghost" onclick={cmd(ClientMsg::Seek { seconds: -120 })} aria-label="Reculer de 2 minutes">{ "−2 min" }</button>
                        if snap.paused {
                            <button class="btn" onclick={cmd(ClientMsg::Resume)}>{ "▶" }</button>
                        } else {
                            <button class="btn" onclick={cmd(ClientMsg::Pause)} aria-label="Pause">{ "❚❚" }</button>
                        }
                        <button class="btn btn-ghost" onclick={cmd(ClientMsg::Seek { seconds: 120 })} aria-label="Avancer de 2 minutes">{ "+2 min" }</button>
                    </div>
                }
                <button class="btn btn-ghost" onclick={cmd(ClientMsg::Stop)}>{ if is_replay { "Quitter le replay" } else { "Quitter le direct" } }</button>
            </section>

            if let Some(w) = &snap.weather {
                <section class="card">
                    { stat_grid(vec![
                        ("Air", format!("{:.0}°", w.air_temperature)),
                        ("Piste", format!("{:.0}°", w.track_temperature)),
                        (if w.rainfall { "Pluie 🌧" } else { "Humidité" }, format!("{:.0}%", w.humidity)),
                    ]) }
                </section>
            }

            <ol class="rows rows-card live-board">
                { for snap.cars.iter().map(car_row) }
            </ol>

            if !snap.race_control.is_empty() {
                <section class="card">
                    <h2>{ "Direction de course" }</h2>
                    <ul class="sessions">
                        { for snap.race_control.iter().map(|m| html! {
                            <li class="session rc">
                                <span class="session-time">
                                    { clock_local(&m.date) }
                                    if let Some(l) = m.lap { { format!(" · T{l}") } }
                                </span>
                                <span class="rc-msg">{ &m.message }</span>
                            </li>
                        }) }
                    </ul>
                </section>
            }
        </>
    }
}

fn tyre(compound: &str) -> (&'static str, &'static str) {
    match compound {
        "SOFT" => ("S", "tyre-soft"),
        "MEDIUM" => ("M", "tyre-medium"),
        "HARD" => ("H", "tyre-hard"),
        "INTERMEDIATE" => ("I", "tyre-inter"),
        "WET" => ("W", "tyre-wet"),
        _ => ("?", "tyre-hard"),
    }
}

fn car_row(c: &Car) -> Html {
    let mut sub = Vec::new();
    if !c.gap.is_empty() {
        sub.push(if c.interval.is_empty() || c.interval == c.gap {
            c.gap.clone()
        } else {
            format!("{} ({})", c.interval, c.gap)
        });
    } else if c.position == 1 {
        sub.push("Leader".into());
    }
    if let Some(l) = c.last_lap {
        sub.push(format!("dernier {}", format_lap(l)));
    }
    html! {
        <li class="row" style={format!("--team:#{}", c.colour)}>
            <span class="pos">{ c.position }</span>
            <span class="row-main">
                <span class="row-title">
                    <strong>{ &c.code }</strong>{ " " }<span class="muted">{ &c.team }</span>
                    if c.fastest { { " " }<span class="tag tag-purple" title="Meilleur tour">{ "⏱" }</span> }
                    if c.in_pit { { " " }<span class="tag">{ "Stand" }</span> }
                </span>
                <span class="row-sub">{ sub.join(" · ") }</span>
            </span>
            if let Some(compound) = &c.compound {
                <span class="tyre-cell" title={format!("{compound}, {} tours, {} arrêt(s)", c.tyre_age.unwrap_or(0), c.pits)}>
                    <span class={classes!("tyre", tyre(compound).1)}>{ tyre(compound).0 }</span>
                    <small>{ format!("{}t", c.tyre_age.unwrap_or(0)) }</small>
                </span>
            }
        </li>
    }
}
