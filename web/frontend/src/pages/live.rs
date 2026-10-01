use std::rc::Rc;

use f1x_protocol::{
    Car, ClientMsg, EventKind, Mode, SessionSummary, Snapshot, TrackMap, TrackStatus, format_lap,
};
use gloo_net::http::Request;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

use crate::api::{f1, use_f1};
use crate::components::*;
use crate::i18n::t;
use crate::live::use_live;
use crate::tr;
use crate::util::{current_year, flag_country, local_date, now_ms};

const SPEEDS: [u32; 6] = [1, 2, 5, 10, 30, 60];

fn session_fr(name: &str) -> String {
    if !crate::i18n::is_fr() {
        return name.to_string();
    }
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
                    Ok(r) => Err(tr!(
                        "Sessions indisponibles ({})",
                        "Sessions unavailable ({})",
                        r.status()
                    )),
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
    // Texte d'état dans la langue choisie (le serveur n'envoie que des indicateurs utiles).
    let status_text = if !s.live_available {
        t(
            "Le direct nécessite un accès OpenF1 (abonnement). En attendant, rejoue n'importe quelle session depuis 2023.",
            "Live timing requires OpenF1 access (subscription). Meanwhile, replay any session since 2023.",
        )
        .to_string()
    } else if s.live_active {
        s.status.clone()
    } else {
        t(
            "Aucune session en cours pour le moment.",
            "No session running right now.",
        )
        .to_string()
    };

    let body = match &s.snapshot {
        Some(snap) => {
            html! { <Board snapshot={Rc::clone(snap)} track={s.track.clone()} send={live.send.clone()} /> }
        }
        None => html! { <Lobby send={live.send.clone()} /> },
    };

    html! {
        <Layout title={t("Direct", "Live")} tab={Tab::Live}>
            <div class="live-bar">
                <span class={classes!("dot", s.connected.then_some("dot-on"))} aria-hidden="true"></span>
                <span>{ if s.connected { t("Connecté en temps réel", "Connected in real time") } else { t("Connexion…", "Connecting…") } }</span>
                <span class="live-viewers">{ tr!("👥 {} en ligne", "👥 {} online", s.viewers) }</span>
            </div>
            if s.snapshot.is_none() {
                <section class={classes!("card", s.live_active.then_some("hero"))}>
                    <h2>{ if s.live_active { t("🔴 Session en cours", "🔴 Session in progress") } else { t("Direct", "Live") } }</h2>
                    <p class="muted">{ status_text }</p>
                    if s.live_active {
                        <button class="btn" onclick={let send = live.send.clone(); move |_| send.emit(ClientMsg::Live)}>{ t("Suivre le direct", "Follow live") }</button>
                    }
                </section>
            }
            if s.loading.is_some() {
                <section class="card"><p class="muted">{ t("Chargement des données de la session…", "Loading session data…") }</p>{ loading() }</section>
            }
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
                    <p class="eyebrow">{ t("Prochaine session", "Next session") }</p>
                    <h2>{ format!("{} {} — {name}", flag_country(&race.circuit.location.country), race.race_name) }</h2>
                    <p class="muted">{ local_date(&iso, true) }</p>
                    <Countdown target_ms={crate::util::parse_ms(&iso)} />
                </section>
            }

            <section class="card">
                <h2>{ t("Rejouer une session", "Replay a session") }</h2>
                <p class="muted">{ t("Revis n'importe quelle session depuis 2023 comme en direct : classement, écarts, pneus, arrêts, drapeaux, météo et direction de course, en temps réel ou en accéléré.", "Relive any session since 2023 as if it were live: order, gaps, tyres, pit stops, flags, weather and race control, in real time or sped up.") }</p>
                <label class="select">
                    <span class="select-label">{ t("Année", "Year") }</span>
                    <select onchange={on_year} aria-label={t("Année", "Year")}>
                        { for (2023..=this_year).rev().map(|y| html! { <option value={y.to_string()} selected={y == *year}>{ y }</option> }) }
                    </select>
                </label>
                { match &sessions {
                    None => loading(),
                    Some(Err(e)) => html! { <p class="muted">{ e }</p> },
                    Some(Ok(_)) if list.is_empty() => html! { <p class="muted">{ t("Aucune session disponible pour cette année.", "No session available for this year.") }</p> },
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
                <p class="select-label">{ t("Vitesse", "Speed") }</p>
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
                <button class="btn" onclick={start} disabled={selected.is_none()}>{ t("▶ Lancer le replay", "▶ Start the replay") }</button>
            </section>
        </>
    }
}

#[derive(Properties, PartialEq)]
struct BoardProps {
    snapshot: Rc<Snapshot>,
    track: Option<Rc<TrackMap>>,
    send: Callback<ClientMsg>,
}

fn track_banner(status: TrackStatus) -> (&'static str, &'static str) {
    match status {
        TrackStatus::Green => ("ts-green", t("Piste dégagée", "Track clear")),
        TrackStatus::Yellow => ("ts-yellow", t("Drapeau jaune", "Yellow flag")),
        TrackStatus::SafetyCar => ("ts-sc", t("Voiture de sécurité", "Safety car")),
        TrackStatus::VirtualSafetyCar => (
            "ts-sc",
            t("Voiture de sécurité virtuelle", "Virtual safety car"),
        ),
        TrackStatus::Red => ("ts-red", t("Drapeau rouge", "Red flag")),
        TrackStatus::Chequered => ("ts-chequered", t("Drapeau à damier", "Chequered flag")),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Order,
    Map,
    Strategy,
    Timeline,
}

// ---------- Alertes (préférences mémorisées sur l'appareil) ----------

#[derive(Clone, Copy, PartialEq)]
enum AlertKind {
    Flags,
    Overtakes,
    Pits,
    FastestLaps,
    Retirements,
}

impl AlertKind {
    const ALL: [AlertKind; 5] = [
        Self::Flags,
        Self::Overtakes,
        Self::Pits,
        Self::FastestLaps,
        Self::Retirements,
    ];
    fn key(self) -> &'static str {
        match self {
            Self::Flags => "flags",
            Self::Overtakes => "overtakes",
            Self::Pits => "pits",
            Self::FastestLaps => "fastest",
            Self::Retirements => "retired",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Flags => t("Drapeaux et voiture de sécurité", "Flags and safety car"),
            Self::Overtakes => t("Dépassements (top 3)", "Overtakes (top 3)"),
            Self::Pits => t("Arrêts aux stands", "Pit stops"),
            Self::FastestLaps => t("Meilleurs tours", "Fastest laps"),
            Self::Retirements => t("Abandons", "Retirements"),
        }
    }
    fn of(kind: &EventKind) -> Option<Self> {
        match kind {
            EventKind::Status { .. } | EventKind::Penalty { .. } => Some(Self::Flags),
            EventKind::Overtake { position, .. } if *position <= 3 => Some(Self::Overtakes),
            EventKind::Overtake { .. } => None,
            EventKind::Pit { .. } => Some(Self::Pits),
            EventKind::FastestLap { .. } => Some(Self::FastestLaps),
            EventKind::Retired { .. } => Some(Self::Retirements),
        }
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn alert_enabled(kind: AlertKind) -> bool {
    storage()
        .and_then(|s| {
            s.get_item(&format!("f1x-alert-{}", kind.key()))
                .ok()
                .flatten()
        })
        .map(|v| v != "0")
        // Par défaut, seulement l'important : drapeaux / voiture de sécurité et abandons.
        .unwrap_or(matches!(kind, AlertKind::Flags | AlertKind::Retirements))
}

fn set_alert(kind: AlertKind, on: bool) {
    if let Some(s) = storage() {
        let _ = s.set_item(
            &format!("f1x-alert-{}", kind.key()),
            if on { "1" } else { "0" },
        );
    }
}

// ---------- Textes des événements ----------

fn status_text(status: TrackStatus) -> &'static str {
    track_banner(status).1
}

fn event_icon(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Overtake { .. } => "⚔️",
        EventKind::Pit { .. } => "🔧",
        EventKind::FastestLap { .. } => "⏱",
        EventKind::Status {
            status: TrackStatus::Green,
        } => "🟢",
        EventKind::Status {
            status: TrackStatus::Yellow,
        } => "🟡",
        EventKind::Status {
            status: TrackStatus::SafetyCar | TrackStatus::VirtualSafetyCar,
        } => "🚨",
        EventKind::Status {
            status: TrackStatus::Red,
        } => "🔴",
        EventKind::Status {
            status: TrackStatus::Chequered,
        } => "🏁",
        EventKind::Penalty { .. } => "⚖️",
        EventKind::Retired { .. } => "❌",
    }
}

fn event_text(kind: &EventKind) -> String {
    match kind {
        EventKind::Overtake {
            driver,
            passed,
            position,
        } => {
            tr!(
                "{driver} dépasse {passed} pour la P{position}",
                "{driver} passes {passed} for P{position}"
            )
        }
        EventKind::Pit { driver, duration } => match duration {
            Some(d) => tr!(
                "{driver} s'arrête aux stands ({d:.1} s dans la voie)",
                "{driver} pits ({d:.1} s in the pit lane)"
            ),
            None => tr!("{driver} s'arrête aux stands", "{driver} pits"),
        },
        EventKind::FastestLap { driver, time } => {
            tr!(
                "Meilleur tour pour {driver} : {}",
                "Fastest lap for {driver}: {}",
                format_lap(*time)
            )
        }
        EventKind::Status { status } => status_text(*status).to_string(),
        EventKind::Penalty { message } => message.clone(),
        EventKind::Retired { driver } => tr!("Abandon de {driver}", "{driver} retires"),
    }
}

// ---------- Tableau de bord ----------

/// Race Center : direct ou replay.
#[function_component]
fn Board(props: &BoardProps) -> Html {
    let snap = &props.snapshot;
    let view = use_state(|| View::Order);
    let banner = use_state(|| None::<(u32, String)>);
    let seen = use_mut_ref(std::collections::HashSet::<String>::new);
    let first = use_mut_ref(|| true);
    let banner_id = use_mut_ref(|| 0u32);
    let last_banner = use_mut_ref(|| 0.0f64);

    // Alerte : au plus une à la fois, une toutes les 8 s, jamais au premier affichage.
    {
        let banner = banner.clone();
        let snap = Rc::clone(snap);
        use_effect_with(snap.clock.clone(), move |_| {
            let mut fresh = Vec::new();
            for e in snap.events.iter() {
                let key = format!("{}{:?}", e.date, e.kind);
                if seen.borrow_mut().insert(key) {
                    fresh.push(e.clone());
                }
            }
            if std::mem::replace(&mut *first.borrow_mut(), false) {
                return;
            }
            let now = now_ms();
            if now - *last_banner.borrow() < 8_000.0 {
                return;
            }
            // Le plus important des nouveaux événements activés.
            let rank = |k: &EventKind| match k {
                EventKind::Status { .. } => 0,
                EventKind::Retired { .. } => 1,
                EventKind::Penalty { .. } => 2,
                EventKind::Overtake { .. } => 3,
                EventKind::Pit { .. } => 4,
                EventKind::FastestLap { .. } => 5,
            };
            let Some(e) = fresh
                .iter()
                .filter(|e| AlertKind::of(&e.kind).is_some_and(alert_enabled))
                .min_by_key(|e| rank(&e.kind))
            else {
                return;
            };
            *last_banner.borrow_mut() = now;
            *banner_id.borrow_mut() += 1;
            let id = *banner_id.borrow();
            banner.set(Some((
                id,
                format!("{} {}", event_icon(&e.kind), event_text(&e.kind)),
            )));
            let banner = banner.clone();
            gloo_timers::callback::Timeout::new(4_000, move || {
                if banner.as_ref().is_some_and(|b| b.0 == id) {
                    banner.set(None);
                }
            })
            .forget();
        });
    }

    let send = props.send.clone();
    let cmd = |m: ClientMsg| {
        let send = send.clone();
        Callback::from(move |_| send.emit(m.clone()))
    };
    let (ts_class, ts_label) = track_banner(snap.track_status);
    let is_replay = snap.mode == Mode::Replay;
    let lap = match snap.total_laps {
        Some(total) if snap.lap > 0 => {
            tr!("Tour {}/{total}", "Lap {}/{total}", snap.lap.min(total))
        }
        _ if snap.lap > 0 => tr!("Tour {}", "Lap {}", snap.lap),
        _ => t("Avant le départ", "Before the start").into(),
    };
    let tab = |v: View, label: &'static str| {
        let view = view.clone();
        html! {
            <button class={classes!("seg", (*view == v).then_some("seg-active"))} onclick={move |_| view.set(v)}>{ label }</button>
        }
    };

    html! {
        <>
            if let Some((id, text)) = (*banner).clone() {
                <button class="banner" key={id} role="status" aria-live="polite"
                        onclick={let banner = banner.clone(); move |_| banner.set(None)}
                        aria-label={t("Fermer l'alerte", "Dismiss alert")}>
                    <span class="banner-text">{ text }</span><span class="banner-x" aria-hidden="true">{ "✕" }</span>
                </button>
            }
            <section class="card hero">
                <div class="card-head">
                    <span class={classes!("badge", if is_replay { "badge" } else { "badge-live" })}>
                        { if is_replay { format!("Replay ×{}", snap.speed) } else { t("● En direct", "● Live").into() } }
                    </span>
                    <span class="muted">{ clock_local(&snap.clock) }</span>
                </div>
                <h2 class="hero-title">{ format!("{} {} — {}", flag_country(&snap.session.country), snap.session.location, session_fr(&snap.session.session_name)) }</h2>
                <p class="lap-counter">{ lap }{ if snap.finished { t(" · Terminé", " · Finished") } else if snap.paused { t(" · En pause", " · Paused") } else { "" } }</p>
                <div class={classes!("track-status", ts_class)} role="status">{ ts_label }</div>
                if let Some(e) = snap.events.first() {
                    <p class="ticker" key={e.date.clone()}>
                        <span aria-hidden="true">{ event_icon(&e.kind) }</span>
                        <span class="ticker-text">{ event_text(&e.kind) }</span>
                        if e.lap > 0 { <span class="ticker-lap">{ tr!("T{}", "L{}", e.lap) }</span> }
                    </p>
                }
                if is_replay {
                    <div class="progress" aria-label={t("Avancement du replay", "Replay progress")}>
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
                        <button class="btn btn-ghost" onclick={cmd(ClientMsg::Seek { seconds: -120 })} aria-label={t("Reculer de 2 minutes", "Back 2 minutes")}>{ "−2 min" }</button>
                        if snap.paused {
                            <button class="btn" onclick={cmd(ClientMsg::Resume)}>{ "▶" }</button>
                        } else {
                            <button class="btn" onclick={cmd(ClientMsg::Pause)} aria-label="Pause">{ "❚❚" }</button>
                        }
                        <button class="btn btn-ghost" onclick={cmd(ClientMsg::Seek { seconds: 120 })} aria-label={t("Avancer de 2 minutes", "Forward 2 minutes")}>{ "+2 min" }</button>
                    </div>
                }
                <button class="btn btn-ghost" onclick={cmd(ClientMsg::Stop)}>{ if is_replay { t("Quitter le replay", "Leave the replay") } else { t("Quitter le direct", "Leave live") } }</button>
            </section>

            <div class="segmented segmented-4" role="tablist">
                { tab(View::Order, t("Ordre", "Order")) }
                { tab(View::Map, t("Carte", "Map")) }
                { tab(View::Strategy, t("Stratégie", "Strategy")) }
                { tab(View::Timeline, t("Chrono", "Timeline")) }
            </div>

            { match *view {
                View::Order => html! {
                    <>
                        <ol class="rows rows-card live-board">{ for snap.cars.iter().map(car_row) }</ol>
                        { legend() }
                    </>
                },
                View::Map => html! { <LiveMap snapshot={Rc::clone(snap)} track={props.track.clone()} /> },
                View::Strategy => html! { <Strategy snapshot={Rc::clone(snap)} /> },
                View::Timeline => timeline(snap),
            } }
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

fn compound_name(compound: &str) -> &'static str {
    match compound {
        "SOFT" => t("tendre", "soft"),
        "MEDIUM" => t("medium", "medium"),
        "HARD" => t("dur", "hard"),
        "INTERMEDIATE" => t("intermédiaire", "intermediate"),
        "WET" => t("pluie", "wet"),
        _ => "?",
    }
}

fn sector_class(flag: u8) -> &'static str {
    match flag {
        3 => "sec sec-purple",
        2 => "sec sec-green",
        1 => "sec sec-yellow",
        _ => "sec",
    }
}

fn car_row(c: &Car) -> Html {
    // Écarts toujours étiquetés : « devant » = voiture juste devant, « leader » = 1er.
    let gaps = if c.position == 1 {
        t("Leader", "Leader").to_string()
    } else if c.gap.is_empty() {
        String::new()
    } else if c.interval.is_empty() || c.interval == c.gap {
        tr!("leader {}", "leader {}", c.gap)
    } else {
        tr!(
            "devant {} · leader {}",
            "ahead {} · leader {}",
            c.interval,
            c.gap
        )
    };
    html! {
        <li class={classes!("row", c.retired.then_some("row-out"))} style={format!("--team:#{}", c.colour)}>
            <span class="pos">{ c.position }</span>
            <span class="row-main">
                <span class="row-title">
                    <strong>{ &c.code }</strong>{ " " }<span class="muted">{ &c.team }</span>
                    if c.fastest { { " " }<span class="tag tag-purple" title={t("Meilleur tour de la session", "Session fastest lap")}>{ "⏱" }</span> }
                    if c.in_pit { { " " }<span class="tag">{ t("Stand", "Pit") }</span> }
                    if c.retired { { " " }<span class="tag">{ t("Abandon", "Out") }</span> }
                </span>
                if !gaps.is_empty() { <span class="row-sub">{ gaps }</span> }
                <span class="row-sub lap-line">
                    if let Some(l) = c.last_lap { { tr!("dernier tour {}", "last lap {}", format_lap(l)) } }
                    <span class="sectors" aria-label={t("Secteurs du dernier tour", "Last lap sectors")}>
                        { for c.sector_flags.iter().map(|f| html! { <span class={sector_class(*f)}></span> }) }
                    </span>
                </span>
            </span>
            if let Some(compound) = &c.compound {
                <span class="tyre-cell" title={tr!("Pneu {}, {} tours, {} arrêt(s)", "{} tyre, {} laps, {} stop(s)", compound_name(compound), c.tyre_age.unwrap_or(0), c.pits)}>
                    <span class={classes!("tyre", tyre(compound).1)}>{ tyre(compound).0 }</span>
                    <small>{ tr!("{} t.", "{} l.", c.tyre_age.unwrap_or(0)) }</small>
                </span>
            }
        </li>
    }
}

/// Légende du classement (toujours accessible, repliable).
fn legend() -> Html {
    let tyre_item = |c: &str| {
        let (letter, class) = tyre(c);
        html! { <li><span class={classes!("tyre", class)}>{ letter }</span>{ compound_name(c) }</li> }
    };
    html! {
        <details class="card legend" open=true>
            <summary><strong>{ t("Légende", "Legend") }</strong></summary>
            <ul class="legend-list">
                <li><span class="legend-key">{ t("devant +0.738", "ahead +0.738") }</span>{ t("écart en secondes avec la voiture juste devant", "gap in seconds to the car just ahead") }</li>
                <li><span class="legend-key">{ t("leader +5.089", "leader +5.089") }</span>{ t("écart avec le premier", "gap to the race leader") }</li>
                <li><span class="legend-key">{ t("+1 TOUR", "+1 LAP") }</span>{ t("doublé par le leader", "lapped by the leader") }</li>
                <li><span class="legend-key">{ t("dernier tour", "last lap") }</span>{ t("temps de son dernier tour complet", "time of the last completed lap") }</li>
                <li>
                    <span class="sectors"><span class="sec sec-purple"></span><span class="sec sec-green"></span><span class="sec sec-yellow"></span></span>
                    { t("secteurs du dernier tour : violet = meilleur de tous, vert = record perso, jaune = plus lent", "last lap sectors: purple = overall best, green = personal best, yellow = slower") }
                </li>
                <li><span class="tag tag-purple">{ "⏱" }</span>{ t("détient le meilleur tour de la session", "holds the session's fastest lap") }</li>
                <li><span class="tag">{ t("Stand", "Pit") }</span>{ t("dans la voie des stands", "in the pit lane") }</li>
                <li><span class="legend-bar"></span>{ t("bande de couleur = écurie", "colour strip = team") }</li>
                <li>{ t("Pneu actuel et nombre de tours effectués avec (« 12 t. ») :", "Current tyre and laps done on it (“12 l.”):") }</li>
            </ul>
            <ul class="legend-tyres">
                { tyre_item("SOFT") }{ tyre_item("MEDIUM") }{ tyre_item("HARD") }{ tyre_item("INTERMEDIATE") }{ tyre_item("WET") }
            </ul>
            <p class="muted">{ t("Alertes (une petite bannière en bas de l'écran, 8 s minimum entre deux) :", "Alerts (a small banner at the bottom, at least 8 s apart):") }</p>
            <AlertSettings />
        </details>
    }
}

#[function_component]
fn AlertSettings() -> Html {
    let version = use_state(|| 0u32);
    html! {
        <ul class="alert-settings">
            { for AlertKind::ALL.iter().map(|k| {
                let on = alert_enabled(*k);
                let k = *k;
                let version = version.clone();
                html! {
                    <li>
                        <label>
                            <input type="checkbox" checked={on} onchange={move |_| { set_alert(k, !on); version.set(*version + 1); }} />
                            { k.label() }
                        </label>
                    </li>
                }
            }) }
        </ul>
    }
}

fn timeline(snap: &Snapshot) -> Html {
    html! {
        <>
            <section class="card">
                <h2>{ t("Chronologie", "Timeline") }</h2>
                if snap.events.is_empty() {
                    <p class="muted">{ t("Pas encore d'événement.", "No event yet.") }</p>
                }
                <ol class="timeline">
                    { for snap.events.iter().map(|e| html! {
                        <li>
                            <span class="tl-icon" aria-hidden="true">{ event_icon(&e.kind) }</span>
                            <span class="tl-body">
                                <span class="tl-meta">{ clock_local(&e.date) }{ if e.lap > 0 { tr!(" · T{}", " · L{}", e.lap) } else { String::new() } }</span>
                                <span>{ event_text(&e.kind) }</span>
                            </span>
                        </li>
                    }) }
                </ol>
            </section>
            if !snap.race_control.is_empty() {
                <section class="card">
                    <h2>{ t("Direction de course", "Race control") }</h2>
                    <ul class="sessions">
                        { for snap.race_control.iter().map(|m| html! {
                            <li class="session rc">
                                <span class="session-time">
                                    { clock_local(&m.date) }
                                    if let Some(l) = m.lap { { tr!(" · T{}", " · L{}", l) } }
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

// ---------- Carte en direct ----------

#[derive(Properties, PartialEq)]
struct MapProps {
    snapshot: Rc<Snapshot>,
    track: Option<Rc<TrackMap>>,
}

/// Position approximative de chaque voiture : avancement dans son tour reporté sur le tracé
/// du meilleur tour de la session.
#[function_component]
fn LiveMap(props: &MapProps) -> Html {
    let snap = &props.snapshot;
    let three = use_state(|| false);
    let weather = snap.weather.as_ref().map(|w| {
        stat_grid(vec![
            ("Air", format!("{:.0}°", w.air_temperature)),
            (t("Piste", "Track"), format!("{:.0}°", w.track_temperature)),
            (
                if w.rainfall {
                    t("Pluie 🌧", "Rain 🌧")
                } else {
                    t("Humidité", "Humidity")
                },
                format!("{:.0}%", w.humidity),
            ),
        ])
    });
    let Some(track) = &props.track else {
        return html! {
            <section class="card">
                <p class="muted">{ t("Carte indisponible pour cette session.", "Map unavailable for this session.") }</p>
                { weather.unwrap_or_default() }
            </section>
        };
    };
    let pts = &track.points;
    const PAD: f64 = 40.0;
    let (w, h) = (track.width + 2.0 * PAD, track.height + 2.0 * PAD);
    let at_fraction = |f: f32| {
        let target = f * track.lap_time as f32;
        let i = pts.partition_point(|p| p.t < target).min(pts.len() - 1);
        (pts[i].x as f64 + PAD, pts[i].y as f64 + PAD)
    };
    let line = pts
        .iter()
        .map(|p| format!("{:.1},{:.1}", p.x as f64 + PAD, p.y as f64 + PAD))
        .collect::<Vec<_>>()
        .join(" ");
    let mut cars: Vec<&Car> = snap
        .cars
        .iter()
        .filter(|c| c.lap_progress.is_some() && !c.retired)
        .collect();
    cars.reverse(); // le leader dessiné en dernier (au-dessus)
    let tab = |on: bool, label: &'static str| {
        let three = three.clone();
        html! {
            <button class={classes!("seg", (*three == on).then_some("seg-active"))} aria-pressed={(*three == on).to_string()}
                onclick={move |_| three.set(on)}>{ label }</button>
        }
    };
    if *three {
        let markers: Vec<crate::gl3d::Marker> = cars
            .iter()
            .map(|c| crate::gl3d::Marker {
                key: c.code.clone(),
                label: c.position.to_string(),
                colour: c.colour.clone(),
                fraction: c.lap_progress.unwrap_or(0.0),
            })
            .collect();
        return html! {
            <section class="card">
                <h2>{ t("Carte en direct", "Live map") }</h2>
                <div class="segmented">{ tab(false, t("Plan 2D", "2D map")) }{ tab(true, t("Relief 3D", "3D relief")) }</div>
                <crate::gl3d::View3D scene={crate::gl3d::Scene::Track { map: track.clone(), ghost: false }} markers={Rc::new(markers)} />
                <p class="muted">{ t(
                    "Voitures aux couleurs de leur écurie (chiffre = position), placées d'après l'avancement de chaque pilote dans son tour. Glisse pour tourner autour du circuit.",
                    "Cars in team colours (number = position), placed from each driver's progress through the lap. Drag to orbit the circuit.",
                ) }</p>
                { weather.unwrap_or_default() }
            </section>
        };
    }
    html! {
        <section class="card">
            <h2>{ t("Carte en direct", "Live map") }</h2>
            <div class="segmented">{ tab(false, t("Plan 2D", "2D map")) }{ tab(true, t("Relief 3D", "3D relief")) }</div>
            <svg class="track" viewBox={format!("0 0 {w:.0} {h:.0}")} role="img"
                 aria-label={t("Position des voitures sur le circuit", "Car positions on the circuit")}>
                <polyline class="track-base" points={line.clone()} />
                <polyline class="track-line" points={line} />
                { for cars.iter().map(|c| {
                    let (x, y) = at_fraction(c.lap_progress.unwrap_or(0.0));
                    html! {
                        <g class="car" transform={format!("translate({x:.1},{y:.1})")}>
                            <circle r="26" fill={format!("#{}", c.colour)} class={classes!(c.in_pit.then_some("car-pit"))} />
                            <text class="car-label" text-anchor="middle" dy="9">{ c.position }</text>
                        </g>
                    }
                }) }
            </svg>
            <p class="muted">{ t(
                "Positions estimées d'après l'avancement de chaque pilote dans son tour (couleur = écurie, chiffre = position).",
                "Positions estimated from each driver's progress through the lap (colour = team, number = position).",
            ) }</p>
            { weather.unwrap_or_default() }
        </section>
    }
}

// ---------- Stratégie ----------

fn parse_gap(s: &str) -> Option<f64> {
    s.trim_start_matches('+').parse().ok()
}

#[derive(Properties, PartialEq)]
struct StrategyProps {
    snapshot: Rc<Snapshot>,
}

#[function_component]
fn Strategy(props: &StrategyProps) -> Html {
    let snap = &props.snapshot;
    let chosen = use_state(|| {
        snap.cars
            .get(3)
            .or(snap.cars.first())
            .map(|c| c.code.clone())
            .unwrap_or_default()
    });
    let total = snap.total_laps.unwrap_or(snap.lap.max(1)).max(1) as f32;
    let loss = snap.pit_loss.unwrap_or(22.0);

    // « Et s'il s'arrêtait maintenant ? » : temps perdu dans les stands ajouté à son écart au leader.
    let projection = snap
        .cars
        .iter()
        .find(|c| c.code == *chosen)
        .and_then(|car| {
            let own = if car.position == 1 {
                0.0
            } else {
                parse_gap(&car.gap)?
            };
            let after = own + loss;
            let mut ahead: Vec<(&Car, f64)> = snap
                .cars
                .iter()
                .filter(|c| c.code != car.code && !c.retired)
                .filter_map(|c| {
                    Some((
                        c,
                        if c.position == 1 {
                            0.0
                        } else {
                            parse_gap(&c.gap)?
                        },
                    ))
                })
                .filter(|(_, g)| *g < after)
                .collect();
            ahead.sort_by(|a, b| a.1.total_cmp(&b.1));
            let new_pos = ahead.len() + 1;
            let in_front = ahead.last().map(|(c, g)| (c.code.clone(), after - g));
            let behind = snap
                .cars
                .iter()
                .filter(|c| c.code != car.code && !c.retired)
                .filter_map(|c| {
                    Some((
                        c,
                        if c.position == 1 {
                            0.0
                        } else {
                            parse_gap(&c.gap)?
                        },
                    ))
                })
                .filter(|(_, g)| *g >= after)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(c, g)| (c.code.clone(), g - after));
            Some((car.position, new_pos, in_front, behind))
        });
    let onchange = {
        let chosen = chosen.clone();
        Callback::from(move |e: Event| {
            chosen.set(e.target_unchecked_into::<HtmlSelectElement>().value())
        })
    };

    html! {
        <>
            <section class="card">
                <h2>{ t("Stratégie des pneus", "Tyre strategy") }</h2>
                <ol class="strategy">
                    { for snap.cars.iter().map(|c| html! {
                        <li>
                            <span class="strat-code">{ &c.code }</span>
                            <span class="strat-bar">
                                { for c.stints.iter().map(|s| {
                                    let left = (s.from.saturating_sub(1)) as f32 / total * 100.0;
                                    let width = ((s.to.max(s.from) - s.from + 1) as f32 / total * 100.0).min(100.0 - left);
                                    html! { <span class={classes!("stint", tyre(&s.compound).1)} title={format!("{} · {}–{}", compound_name(&s.compound), s.from, s.to)}
                                                  style={format!("left:{left:.2}%;width:{width:.2}%")}>{ tyre(&s.compound).0 }</span> }
                                }) }
                                <span class="strat-now" style={format!("left:{:.2}%", (snap.lap as f32 / total * 100.0).min(100.0))}></span>
                            </span>
                        </li>
                    }) }
                </ol>
                <p class="muted">{ t("Chaque barre = la course d'un pilote, découpée en relais (lettre = gomme). Trait blanc = tour actuel.", "Each bar = one driver's race, split into stints (letter = compound). White line = current lap.") }</p>
            </section>

            <section class="card">
                <h2>{ t("Et s'il s'arrêtait maintenant ?", "What if they pit now?") }</h2>
                <label class="select">
                    <span class="select-label">{ t("Pilote", "Driver") }</span>
                    <select {onchange} aria-label={t("Choisir un pilote", "Choose a driver")}>
                        { for snap.cars.iter().map(|c| html! {
                            <option value={c.code.clone()} selected={c.code == *chosen}>{ format!("P{} · {} {}", c.position, c.code, c.team) }</option>
                        }) }
                    </select>
                </label>
                { match &projection {
                    Some((from, to, front, behind)) => html! {
                        <>
                            <p class="projection">{ tr!("P{from} → ressortirait P{to}", "P{from} → would rejoin P{to}") }</p>
                            <ul class="sessions">
                                if let Some((code, gap)) = front {
                                    <li class="session"><span class="session-name">{ t("Derrière", "Behind") }</span><span class="session-time">{ format!("{code} (+{gap:.1} s)") }</span></li>
                                }
                                if let Some((code, gap)) = behind {
                                    <li class="session"><span class="session-name">{ t("Devant", "Ahead of") }</span><span class="session-time">{ format!("{code} ({gap:.1} s)") }</span></li>
                                }
                            </ul>
                        </>
                    },
                    None => html! { <p class="muted">{ t("Écarts indisponibles pour ce pilote (doublé ou pas encore en course).", "Gaps unavailable for this driver (lapped or not racing yet).") }</p> },
                } }
                <p class="muted">{ tr!(
                    "Estimation : temps perdu aux stands ≈ {:.1} s (médiane de la course), écarts actuels figés. Ne tient pas compte des pneus neufs ni du trafic.",
                    "Estimate: pit stop loss ≈ {:.1} s (race median), current gaps frozen. Ignores fresh tyres and traffic.",
                    loss
                ) }</p>
            </section>
        </>
    }
}
