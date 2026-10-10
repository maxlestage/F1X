//! Direct et replays : le « Race Center » (classement, carte, stratégie, chronologie, alertes).
//!
//! Le serveur envoie une photo complète de la course chaque seconde. Rien n'est reconstruit à
//! chaque photo : les lignes (classement, voitures sur la carte, stratégie, chronologie) restent
//! en place et seules leurs valeurs changent — les animations d'entrée ne se rejouent pas et
//! les menus, la légende repliée ou la vue 3D gardent leur état.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use active::prelude::*;
use f1x_protocol::{
    Car, ClientMsg, EventKind, Mode, RaceEvent, SessionSummary, Snapshot, StintInfo, TrackMap,
    TrackStatus, Weather, format_lap,
};
use gloo_net::http::Request;
use gloo_timers::callback::Timeout;

use crate::api::{f1, use_f1};
use crate::components::*;
use crate::gl3d::{Marker, Scene, view_3d};
use crate::i18n::t;
use crate::live::{LiveHandle, use_live};
use crate::tr;
use crate::util::{current_year, flag_country, local_date, now_ms, parse_ms, session_label};

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

// ---------- Lecture de la photo en cours ----------

/// Lit la photo en cours ; valeur par défaut s'il n'y en a pas (arrêt du suivi en cours).
fn read<R: Default>(live: LiveHandle, f: impl FnOnce(&Snapshot) -> R) -> R {
    live.state
        .with(|s| s.snapshot.as_deref().map(f).unwrap_or_default())
}

/// Valeur tirée de chaque photo : ce qui la lit n'est mis à jour que quand elle change.
fn snap_memo<T: Clone + PartialEq + Default + 'static>(
    live: LiveHandle,
    f: impl Fn(&Snapshot) -> T + 'static,
) -> State<T> {
    memo(move || read(live, &f))
}

/// Valeur tirée de l'élément d'une ligne (voiture, ligne du championnat…), suivie seulement
/// quand elle change.
fn pick<S: 'static, T: Clone + PartialEq + Default + 'static>(
    item: State<Option<S>>,
    f: impl Fn(&S) -> T + 'static,
) -> State<T> {
    memo(move || item.with(|x| x.as_ref().map(&f).unwrap_or_default()))
}

/// Texte tiré de l'élément d'une ligne, pour `text_dyn` / `attr_dyn`.
fn shown<S: 'static>(
    item: State<Option<S>>,
    f: impl Fn(&S) -> String + 'static,
) -> impl Fn() -> String + 'static {
    let value = pick(item, f);
    move || value.get()
}

/// Lignes à place fixe : la ligne `i` montre toujours le `i`-ième élément, seules ses valeurs
/// changent (comme un tableau d'affichage). Une ligne déplacée dans la page rejouerait ses
/// animations d'entrée ; ici, aucune ne bouge.
fn slots(count: State<usize>, row: impl Fn(usize) -> Node + 'static) -> Node {
    keyed(
        move || (0..count.get()).collect::<Vec<_>>(),
        |i| *i,
        move |i| row(*i),
    )
}

/// Données partagées comparées par adresse : le tracé du circuit ne change qu'avec la session.
#[derive(Clone)]
struct Same<T>(Rc<T>);

impl<T> PartialEq for Same<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Sessions rejouables d'une année (rechargées quand l'année change).
fn use_sessions(year: State<u32>) -> State<Option<Result<Vec<SessionSummary>, String>>> {
    let state = use_state(None);
    let generation = Rc::new(Cell::new(0u32));
    effect(move || {
        let year = year.get();
        // Une réponse arrivée après un changement d'année est ignorée.
        let current = generation.get().wrapping_add(1);
        generation.set(current);
        untrack(|| state.set(None));
        let generation = generation.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let url = format!("/api/live/sessions/{year}");
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
            if state.is_alive() && generation.get() == current {
                state.set(Some(result));
            }
        });
    });
    state
}

pub fn live_page() -> Node {
    let live = use_live();
    let st = live.state;
    let connected = memo(move || st.with(|s| s.connected));
    let viewers = memo(move || st.with(|s| s.viewers));
    let following = memo(move || st.with(|s| s.snapshot.is_some()));
    let live_active = memo(move || st.with(|s| s.live_active));
    let loading_on = memo(move || st.with(|s| s.loading.is_some()));
    let error = memo(move || st.with(|s| s.error.clone()));
    // Texte d'état dans la langue choisie (le serveur n'envoie que des indicateurs utiles).
    let status_msg = memo(move || {
        st.with(|s| {
            if !s.live_available {
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
            }
        })
    });

    let bar = div()
        .class("live-bar")
        .child(
            span()
                .class("dot")
                .class_if("dot-on", move || connected.get())
                .attr("aria-hidden", "true"),
        )
        .child(span().text_dyn(move || {
            if connected.get() {
                t("Connecté en temps réel", "Connected in real time")
            } else {
                t("Connexion…", "Connecting…")
            }
            .to_string()
        }))
        .child(
            span()
                .class("live-viewers")
                .text_dyn(move || tr!("👥 {} en ligne", "👥 {} online", viewers.get())),
        );

    let intro = dynamic(move || {
        if following.get() {
            return Node::Empty;
        }
        section()
            .class("card")
            .class_if("hero", move || live_active.get())
            .child(h2().text_dyn(move || {
                if live_active.get() {
                    t("🔴 Session en cours", "🔴 Session in progress")
                } else {
                    t("Direct", "Live")
                }
                .to_string()
            }))
            .child(p().class("muted").text_dyn(move || status_msg.get()))
            .child(dynamic(move || {
                if !live_active.get() {
                    return Node::Empty;
                }
                button()
                    .class("btn")
                    .on_click(move |_| live.send(ClientMsg::Live))
                    .text(t("Suivre le direct", "Follow live"))
                    .into()
            }))
            .into()
    });

    let loading_card = dynamic(move || {
        if !loading_on.get() {
            return Node::Empty;
        }
        section()
            .class("card")
            .child(p().class("muted").text(t(
                "Chargement des données de la session…",
                "Loading session data…",
            )))
            .child(loading())
            .into()
    });

    let error_view = dynamic(move || error.get().map(|m| error_card(&m)).unwrap_or_default());

    // L'écran de choix tant qu'aucune session n'est suivie, puis le Race Center.
    let body = dynamic(move || {
        if following.get() {
            board(live)
        } else {
            lobby(live)
        }
    });

    layout(
        t("Direct", "Live"),
        Some(Tab::Live),
        fragment([Node::from(bar), intro, loading_card, error_view, body]),
    )
}

/// Écran d'accueil du direct : prochaine session + choix d'un replay.
fn lobby(live: LiveHandle) -> Node {
    let this_year = current_year();
    let year = use_state(this_year);
    let chosen = use_state(None::<u32>);
    let speed = use_state(30u32);
    let sessions = use_sessions(year);
    let schedule = use_f1(f1("current.json", 100));

    // Session choisie, sinon la plus récente de l'année.
    let selected = move || {
        chosen.get().or_else(|| {
            sessions.with(|r| match r {
                Some(Ok(list)) => list.last().map(|s| s.session_key),
                _ => None,
            })
        })
    };

    // Prochaine session du week-end (Jolpica).
    let next_session = dynamic(move || {
        let now = now_ms();
        let found = schedule.with(|f| {
            f.done().and_then(|d| {
                d.races().iter().find_map(|race| {
                    race.sessions()
                        .into_iter()
                        .find(|(_, iso)| parse_ms(iso) > now)
                        .map(|(name, iso)| {
                            let heading = format!(
                                "{} {} — {}",
                                flag_country(&race.circuit.location.country),
                                race.race_name,
                                session_label(name)
                            );
                            (heading, iso)
                        })
                })
            })
        });
        let Some((heading, iso)) = found else {
            return Node::Empty;
        };
        section()
            .class("card")
            .child(
                p().class("eyebrow")
                    .text(t("Prochaine session", "Next session")),
            )
            .child(h2().text(heading))
            .child(p().class("muted").text(local_date(&iso, true)))
            .child(countdown(parse_ms(&iso)))
            .into()
    });

    let years = select()
        .attr("aria-label", t("Année", "Year"))
        .on("change", move |e| {
            if let Ok(y) = e.value().parse() {
                year.set(y);
                chosen.set(None);
            }
        })
        .children((2023..=this_year).rev().map(|y| {
            let o = option().attr("value", y.to_string());
            let o = if y == this_year {
                o.attr("selected", "")
            } else {
                o
            };
            o.text(y.to_string())
        }));

    // Choix de la session : reconstruit seulement quand la liste de l'année arrive.
    let picker = dynamic(move || {
        let failed = sessions.with(|r| r.as_ref().map(|r| r.as_ref().err().cloned()));
        let list: Vec<SessionSummary> = match failed {
            None => return loading(),
            Some(Some(e)) => return p().class("muted").text(e).into(),
            Some(None) => sessions.with(|r| match r {
                Some(Ok(list)) => list.iter().rev().cloned().collect(),
                _ => Vec::new(),
            }),
        };
        if list.is_empty() {
            return p()
                .class("muted")
                .text(t(
                    "Aucune session disponible pour cette année.",
                    "No session available for this year.",
                ))
                .into();
        }
        let current = untrack(selected);
        label()
            .class("select")
            .child(span().class("select-label").text("Session"))
            .child(
                select()
                    .attr("aria-label", "Session")
                    .on("change", move |e| chosen.set(e.value().parse().ok()))
                    .children(list.iter().map(|s| {
                        let o = option().attr("value", s.session_key.to_string());
                        let o = if Some(s.session_key) == current {
                            o.attr("selected", "")
                        } else {
                            o
                        };
                        o.text(format!(
                            "{} · {} — {}",
                            local_date(&s.date_start, false),
                            s.location,
                            session_fr(&s.session_name)
                        ))
                    })),
            )
            .into()
    });

    let speeds = div().class("speed-grid").children(SPEEDS.iter().map(|&v| {
        button()
            .class("seg")
            .class_if("seg-active", move || speed.get() == v)
            .on_click(move |_| speed.set(v))
            .text(format!("×{v}"))
    }));

    let start = button()
        .class("btn")
        .bool_attr("disabled", move || selected().is_none())
        .on_click(move |_| {
            if let Some(session_key) = selected() {
                live.send(ClientMsg::Replay {
                    session_key,
                    speed: speed.get(),
                });
            }
        })
        .text(t("▶ Lancer le replay", "▶ Start the replay"));

    fragment([
        next_session,
        section()
            .class("card")
            .child(h2().text(t("Rejouer une session", "Replay a session")))
            .child(p().class("muted").text(t(
                "Revis n'importe quelle session depuis 2023 comme en direct : classement, écarts, pneus, arrêts, drapeaux, météo et direction de course, en temps réel ou en accéléré.",
                "Relive any session since 2023 as if it were live: order, gaps, tyres, pit stops, flags, weather and race control, in real time or sped up.",
            )))
            .child(
                label()
                    .class("select")
                    .child(span().class("select-label").text(t("Année", "Year")))
                    .child(years),
            )
            .child(picker)
            .child(p().class("select-label").text(t("Vitesse", "Speed")))
            .child(speeds)
            .child(start)
            .into(),
    ])
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

/// Identité d'un événement de la chronologie (date + nature).
fn event_key(e: &RaceEvent) -> String {
    format!("{}{:?}", e.date, e.kind)
}

// ---------- Tableau de bord ----------

/// Race Center : direct ou replay.
fn board(live: LiveHandle) -> Node {
    let view = use_state(View::Order);
    let banner = use_state(None::<(u32, String)>);
    alerts(live, banner);

    // Valeurs de la photo qui changent rarement : seules les parties qui les montrent suivent.
    let is_replay = snap_memo(live, |s| s.mode == Mode::Replay);
    let speed = snap_memo(live, |s| s.speed);
    let paused = snap_memo(live, |s| s.paused);
    let advance = snap_memo(live, |s| s.progress);
    let clock = snap_memo(live, |s| clock_local(&s.clock));
    let status = snap_memo(live, |s| Some(s.track_status));
    let first_event = snap_memo(live, |s| s.events.first().cloned());
    let heading = snap_memo(live, |s| {
        format!(
            "{} {} — {}",
            flag_country(&s.session.country),
            s.session.location,
            session_fr(&s.session.session_name)
        )
    });
    let lap = snap_memo(live, |s| match s.total_laps {
        Some(total) if s.lap > 0 => {
            tr!("Tour {}/{total}", "Lap {}/{total}", s.lap.min(total))
        }
        _ if s.lap > 0 => tr!("Tour {}", "Lap {}", s.lap),
        _ => t("Avant le départ", "Before the start").into(),
    });
    let state_note = snap_memo(live, |s| {
        if s.finished {
            t(" · Terminé", " · Finished")
        } else if s.paused {
            t(" · En pause", " · Paused")
        } else {
            ""
        }
    });

    // Une nouvelle alerte = une nouvelle bannière (qui glisse à l'écran).
    let banner_view = dynamic(move || {
        let Some((_, message)) = banner.get() else {
            return Node::Empty;
        };
        button()
            .class("banner")
            .attr("role", "status")
            .attr("aria-live", "polite")
            .attr("aria-label", t("Fermer l'alerte", "Dismiss alert"))
            .on_click(move |_| banner.set(None))
            .child(span().class("banner-text").text(message))
            .child(
                span()
                    .class("banner-x")
                    .attr("aria-hidden", "true")
                    .text("✕"),
            )
            .into()
    });

    let track_status = ["ts-green", "ts-yellow", "ts-sc", "ts-red", "ts-chequered"]
        .into_iter()
        .fold(div().class("track-status"), |e, class| {
            e.class_if(class, move || {
                status.get().map(|s| track_banner(s).0) == Some(class)
            })
        })
        .attr("role", "status")
        .text_dyn(move || {
            status
                .get()
                .map(|s| track_banner(s).1)
                .unwrap_or_default()
                .to_string()
        });

    // Dernier événement : un nouveau bandeau à chaque nouvel événement.
    let ticker = dynamic(move || {
        let Some(e) = first_event.get() else {
            return Node::Empty;
        };
        p().class("ticker")
            .child(span().attr("aria-hidden", "true").text(event_icon(&e.kind)))
            .child(span().class("ticker-text").text(event_text(&e.kind)))
            .child((e.lap > 0).then(|| span().class("ticker-lap").text(tr!("T{}", "L{}", e.lap))))
            .into()
    });

    let replay_controls = dynamic(move || {
        if !is_replay.get() {
            return Node::Empty;
        }
        let play_pause = dynamic(move || {
            if paused.get() {
                button()
                    .class("btn")
                    .on_click(move |_| live.send(ClientMsg::Resume))
                    .text("▶")
                    .into()
            } else {
                button()
                    .class("btn")
                    .on_click(move |_| live.send(ClientMsg::Pause))
                    .attr("aria-label", "Pause")
                    .text("❚❚")
                    .into()
            }
        });
        fragment([
            Node::from(
                div()
                    .class("progress")
                    .attr("aria-label", t("Avancement du replay", "Replay progress"))
                    .child(span().attr_dyn("style", move || {
                        format!("width:{:.1}%", advance.get() * 100.0)
                    })),
            ),
            div()
                .class("speed-grid")
                .children(SPEEDS.iter().map(|&v| {
                    button()
                        .class("seg")
                        .class_if("seg-active", move || speed.get() == v)
                        .on_click(move |_| live.send(ClientMsg::Speed { speed: v }))
                        .text(format!("×{v}"))
                }))
                .into(),
            div()
                .class("controls")
                .child(
                    button()
                        .class("btn btn-ghost")
                        .on_click(move |_| live.send(ClientMsg::Seek { seconds: -120 }))
                        .attr("aria-label", t("Reculer de 2 minutes", "Back 2 minutes"))
                        .text("−2 min"),
                )
                .child(play_pause)
                .child(
                    button()
                        .class("btn btn-ghost")
                        .on_click(move |_| live.send(ClientMsg::Seek { seconds: 120 }))
                        .attr("aria-label", t("Avancer de 2 minutes", "Forward 2 minutes"))
                        .text("+2 min"),
                )
                .into(),
        ])
    });

    let hero = section()
        .class("card hero")
        .child(
            div()
                .class("card-head")
                .child(
                    span()
                        .class("badge")
                        .class_if("badge-live", move || !is_replay.get())
                        .text_dyn(move || {
                            if is_replay.get() {
                                format!("Replay ×{}", speed.get())
                            } else {
                                t("● En direct", "● Live").into()
                            }
                        }),
                )
                .child(span().class("muted").text_dyn(move || clock.get())),
        )
        .child(h2().class("hero-title").text_dyn(move || heading.get()))
        .child(
            p().class("lap-counter")
                .text_dyn(move || lap.get())
                .text_dyn(move || state_note.get().to_string()),
        )
        .child(track_status)
        .child(ticker)
        .child(replay_controls)
        .child(
            button()
                .class("btn btn-ghost")
                .on_click(move |_| live.send(ClientMsg::Stop))
                .text_dyn(move || {
                    if is_replay.get() {
                        t("Quitter le replay", "Leave the replay")
                    } else {
                        t("Quitter le direct", "Leave live")
                    }
                    .to_string()
                }),
        );

    let tab = |v: View, name: &'static str| {
        button()
            .class("seg")
            .class_if("seg-active", move || view.get() == v)
            .on_click(move |_| view.set(v))
            .text(name)
    };
    let tabs = div()
        .class("segmented segmented-4")
        .attr("role", "tablist")
        .child(tab(View::Order, t("Ordre", "Order")))
        .child(tab(View::Map, t("Carte", "Map")))
        .child(tab(View::Strategy, t("Stratégie", "Strategy")))
        .child(tab(View::Timeline, t("Chrono", "Timeline")));

    // Reconstruit au changement d'onglet seulement ; chaque vue suit la course elle-même.
    let content = dynamic(move || match view.get() {
        View::Order => fragment([
            Node::from(
                ol().class("rows rows-card live-board")
                    .child(board_rows(live)),
            ),
            board_legend(),
        ]),
        View::Map => live_map(live),
        View::Strategy => strategy(live),
        View::Timeline => fragment([live_extras(live), timeline(live)]),
    });

    fragment([banner_view, hero.into(), tabs.into(), content])
}

/// Alerte : au plus une à la fois, une toutes les 8 s, jamais au premier affichage.
fn alerts(live: LiveHandle, banner: State<Option<(u32, String)>>) {
    let clock = snap_memo(live, |s| s.clock.clone());
    let seen = Rc::new(RefCell::new(HashSet::<String>::new()));
    let first = Rc::new(Cell::new(true));
    let banner_id = Rc::new(Cell::new(0u32));
    let last_banner = Rc::new(Cell::new(0.0f64));
    // Minuteur qui masque la bannière : arrêté en quittant le Race Center.
    let timer = Rc::new(RefCell::new(None::<Timeout>));
    {
        let timer = timer.clone();
        on_cleanup(move || *timer.borrow_mut() = None);
    }
    effect(move || {
        // Une fois par photo (nouvelle heure de session).
        clock.with(|_| ());
        let fresh: Vec<RaceEvent> = untrack(|| {
            read(live, |snap| {
                let mut seen = seen.borrow_mut();
                snap.events
                    .iter()
                    .filter(|e| seen.insert(event_key(e)))
                    .cloned()
                    .collect()
            })
        });
        if first.replace(false) {
            return;
        }
        let now = now_ms();
        if now - last_banner.get() < 8_000.0 {
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
        last_banner.set(now);
        let id = banner_id.get() + 1;
        banner_id.set(id);
        untrack(|| {
            banner.set(Some((
                id,
                format!("{} {}", event_icon(&e.kind), event_text(&e.kind)),
            )))
        });
        let hide = Timeout::new(4_000, move || {
            if banner.is_alive() && banner.with(|b| b.as_ref().is_some_and(|b| b.0 == id)) {
                banner.set(None);
            }
        });
        // Remplace (et annule) le minuteur de l'alerte précédente.
        *timer.borrow_mut() = Some(hide);
    });
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

/// Classe de gomme (`tyre-soft`…) suivant `compound`.
fn tyre_classes(e: Element, compound: State<String>) -> Element {
    [
        "tyre-soft",
        "tyre-medium",
        "tyre-hard",
        "tyre-inter",
        "tyre-wet",
    ]
    .into_iter()
    .fold(e, |e, class| {
        e.class_if(class, move || compound.with(|c| tyre(c).1) == class)
    })
}

/// « ▲2 » / « ▼1 » précédé d'une espace.
fn gain_tag(delta: i32) -> Node {
    fragment([
        Node::from(" "),
        span()
            .class(if delta > 0 { "gain-up" } else { "gain-down" })
            .text(if delta > 0 {
                format!("▲{delta}")
            } else {
                format!("▼{}", -delta)
            })
            .into(),
    ])
}

/// Écarts toujours étiquetés : « devant » = voiture juste devant, « leader » = 1er.
fn gaps_text(c: &Car) -> String {
    if c.position == 1 {
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
    }
}

/// Classement : une ligne par place, mise à jour sur place chaque seconde.
fn board_rows(live: LiveHandle) -> Node {
    let count = snap_memo(live, |s| s.cars.len());
    slots(count, move |i| car_row(live, i))
}

fn car_row(live: LiveHandle, i: usize) -> Node {
    let car = snap_memo(live, move |s| s.cars.get(i).cloned());
    let retired = pick(car, |c| c.retired);
    let fastest = pick(car, |c| c.fastest);
    let in_pit = pick(car, |c| c.in_pit);
    // Places gagnées ou perdues depuis le départ.
    let gained = pick(car, |c| c.gained.filter(|d| *d != 0));
    let gaps = pick(car, gaps_text);
    let has_gaps = memo(move || gaps.with(|g| !g.is_empty()));
    let has_tyre = pick(car, |c| c.compound.is_some());
    let tag = |on: State<bool>, name: &'static str| {
        dynamic(move || {
            if on.get() {
                fragment([Node::from(" "), span().class("tag").text(name).into()])
            } else {
                Node::Empty
            }
        })
    };

    let title = span()
        .class("row-title")
        .child(strong().text_dyn(shown(car, |c| c.code.clone())))
        .text(" ")
        .child(
            span()
                .class("muted")
                .text_dyn(shown(car, |c| c.team.clone())),
        )
        .child(dynamic(move || {
            if !fastest.get() {
                return Node::Empty;
            }
            fragment([
                Node::from(" "),
                span()
                    .class("tag tag-purple")
                    .attr(
                        "title",
                        t("Meilleur tour de la session", "Session fastest lap"),
                    )
                    .text("⏱")
                    .into(),
            ])
        }))
        .child(tag(in_pit, t("Stand", "Pit")))
        .child(tag(retired, t("Abandon", "Out")))
        .child(dynamic(move || {
            gained.get().map(gain_tag).unwrap_or_default()
        }));

    let lap_line = span()
        .class("row-sub lap-line")
        .text_dyn(shown(car, |c| {
            c.last_lap
                .map(|l| tr!("dernier tour {}", "last lap {}", format_lap(l)))
                .unwrap_or_default()
        }))
        .child(
            span()
                .class("sectors")
                .attr(
                    "aria-label",
                    t("Secteurs du dernier tour", "Last lap sectors"),
                )
                .children((0..3).map(|k| {
                    let flag = pick(car, move |c| c.sector_flags[k]);
                    span()
                        .class("sec")
                        .class_if("sec-purple", move || flag.get() == 3)
                        .class_if("sec-green", move || flag.get() == 2)
                        .class_if("sec-yellow", move || flag.get() == 1)
                })),
        );

    li().class("row")
        .class_if("row-out", move || retired.get())
        .attr_dyn("style", shown(car, |c| format!("--team:#{}", c.colour)))
        .child(
            span()
                .class("pos")
                .text_dyn(shown(car, |c| c.position.to_string())),
        )
        .child(
            span()
                .class("row-main")
                .child(title)
                .child(dynamic(move || {
                    if !has_gaps.get() {
                        return Node::Empty;
                    }
                    span().class("row-sub").text_dyn(move || gaps.get()).into()
                }))
                .child(lap_line),
        )
        .child(dynamic(move || {
            if has_tyre.get() {
                tyre_cell(car)
            } else {
                Node::Empty
            }
        }))
        .into()
}

/// Pneu actuel et nombre de tours effectués avec.
fn tyre_cell(car: State<Option<Car>>) -> Node {
    let compound = pick(car, |c| c.compound.clone().unwrap_or_default());
    let age = pick(car, |c| c.tyre_age.unwrap_or(0));
    let pits = pick(car, |c| c.pits);
    span()
        .class("tyre-cell")
        .attr_dyn("title", move || {
            tr!(
                "Pneu {}, {} tours, {} arrêt(s)",
                "{} tyre, {} laps, {} stop(s)",
                compound.with(|c| compound_name(c)),
                age.get(),
                pits.get()
            )
        })
        .child(
            tyre_classes(span().class("tyre"), compound)
                .text_dyn(move || compound.with(|c| tyre(c).0).to_string()),
        )
        .child(small().text_dyn(move || tr!("{} t.", "{} l.", age.get())))
        .into()
}

/// Légende du classement (toujours accessible, repliable).
fn board_legend() -> Node {
    let item = |key: Element, what: &'static str| li().child(key).text(what);
    let key = |k: &'static str| span().class("legend-key").text(k);
    let tyre_item = |c: &'static str| {
        let (letter, class) = tyre(c);
        li().child(span().class("tyre").class(class).text(letter))
            .text(compound_name(c))
    };
    details()
        .class("card legend")
        .attr("open", "")
        .child(summary().child(strong().text(t("Légende", "Legend"))))
        .child(
            ul().class("legend-list")
                .child(item(
                    key(t("devant +0.738", "ahead +0.738")),
                    t(
                        "écart en secondes avec la voiture juste devant",
                        "gap in seconds to the car just ahead",
                    ),
                ))
                .child(item(
                    key(t("leader +5.089", "leader +5.089")),
                    t("écart avec le premier", "gap to the race leader"),
                ))
                .child(item(
                    key(t("+1 TOUR", "+1 LAP")),
                    t("doublé par le leader", "lapped by the leader"),
                ))
                .child(item(
                    key(t("dernier tour", "last lap")),
                    t(
                        "temps de son dernier tour complet",
                        "time of the last completed lap",
                    ),
                ))
                .child(item(
                    span()
                        .class("sectors")
                        .child(span().class("sec sec-purple"))
                        .child(span().class("sec sec-green"))
                        .child(span().class("sec sec-yellow")),
                    t(
                        "secteurs du dernier tour : violet = meilleur de tous, vert = record perso, jaune = plus lent",
                        "last lap sectors: purple = overall best, green = personal best, yellow = slower",
                    ),
                ))
                .child(item(
                    span().class("tag tag-purple").text("⏱"),
                    t(
                        "détient le meilleur tour de la session",
                        "holds the session's fastest lap",
                    ),
                ))
                .child(item(
                    span().class("tag").text(t("Stand", "Pit")),
                    t("dans la voie des stands", "in the pit lane"),
                ))
                .child(item(
                    span().class("legend-bar"),
                    t("bande de couleur = écurie", "colour strip = team"),
                ))
                .child(li().text(t(
                    "Pneu actuel et nombre de tours effectués avec (« 12 t. ») :",
                    "Current tyre and laps done on it (“12 l.”):",
                ))),
        )
        .child(
            ul().class("legend-tyres")
                .children(["SOFT", "MEDIUM", "HARD", "INTERMEDIATE", "WET"].map(tyre_item)),
        )
        .child(p().class("muted").text(t(
            "Alertes (une petite bannière en bas de l'écran, 8 s minimum entre deux) :",
            "Alerts (a small banner at the bottom, at least 8 s apart):",
        )))
        .child(alert_settings())
        .into()
}

/// Cases à cocher des alertes, enregistrées sur l'appareil.
fn alert_settings() -> Node {
    ul().class("alert-settings")
        .children(AlertKind::ALL.iter().map(|&k| {
            let on = use_state(alert_enabled(k));
            li().child(
                label()
                    .child(
                        input()
                            .attr("type", "checkbox")
                            .bool_attr("checked", move || on.get())
                            .on("change", move |_| {
                                let next = !on.get();
                                set_alert(k, next);
                                on.set(next);
                            }),
                    )
                    .text(k.label()),
            )
        }))
        .into()
}

fn timeline(live: LiveHandle) -> Node {
    let events = snap_memo(live, |s| s.events.clone());
    let no_event = memo(move || events.with(Vec::is_empty));
    let messages = snap_memo(live, |s| s.race_control.clone());
    let has_messages = memo(move || messages.with(|m| !m.is_empty()));
    fragment([
        Node::from(
            section()
                .class("card")
                .child(h2().text(t("Chronologie", "Timeline")))
                .child(dynamic(move || {
                    if !no_event.get() {
                        return Node::Empty;
                    }
                    p().class("muted")
                        .text(t("Pas encore d'événement.", "No event yet."))
                        .into()
                }))
                // Du plus récent au plus ancien : un nouvel événement s'ajoute en tête, les
                // autres lignes ne bougent pas.
                .child(ol().class("timeline").children_keyed(
                    move || events.get(),
                    event_key,
                    |e| {
                        li().child(
                            span()
                                .class("tl-icon")
                                .attr("aria-hidden", "true")
                                .text(event_icon(&e.kind)),
                        )
                        .child(
                            span()
                                .class("tl-body")
                                .child(span().class("tl-meta").text(clock_local(&e.date)).text(
                                    if e.lap > 0 {
                                        tr!(" · T{}", " · L{}", e.lap)
                                    } else {
                                        String::new()
                                    },
                                ))
                                .child(span().text(event_text(&e.kind))),
                        )
                        .into()
                    },
                )),
        ),
        dynamic(move || {
            if !has_messages.get() {
                return Node::Empty;
            }
            section()
                .class("card")
                .child(h2().text(t("Direction de course", "Race control")))
                .child(ul().class("sessions").children_keyed(
                    move || messages.get(),
                    |m| (m.date.clone(), m.message.clone()),
                    |m| {
                        li().class("session rc")
                            .child(
                                span()
                                    .class("session-time")
                                    .text(clock_local(&m.date))
                                    .text(
                                        m.lap
                                            .map(|l| tr!(" · T{}", " · L{}", l))
                                            .unwrap_or_default(),
                                    ),
                            )
                            .child(span().class("rc-msg").text(m.message.clone()))
                            .into()
                    },
                ))
                .into()
        }),
    ])
}

// ---------- Carte en direct ----------

const PAD: f64 = 40.0;

/// Voitures à placer sur la carte : en piste, le leader en dernier (dessiné au-dessus).
fn on_track(snap: &Snapshot) -> Vec<&Car> {
    let mut cars: Vec<&Car> = snap
        .cars
        .iter()
        .filter(|c| c.lap_progress.is_some() && !c.retired)
        .collect();
    cars.reverse(); // le leader dessiné en dernier (au-dessus)
    cars
}

/// Point du tracé atteint à la fraction `f` du tour de référence.
fn at_fraction(plan: &TrackMap, f: f32) -> (f64, f64) {
    let pts = &plan.points;
    let target = f * plan.lap_time as f32;
    let i = pts
        .partition_point(|pt| pt.t < target)
        .min(pts.len().saturating_sub(1));
    pts.get(i)
        .map(|pt| (pt.x as f64 + PAD, pt.y as f64 + PAD))
        .unwrap_or((PAD, PAD))
}

/// Position approximative de chaque voiture : avancement dans son tour reporté sur le tracé
/// du meilleur tour de la session.
fn live_map(live: LiveHandle) -> Node {
    let three = use_state(false);
    // Le tracé n'arrive qu'une fois par session : la carte (et la vue 3D) n'est construite
    // qu'à ce moment-là, pas à chaque photo.
    let circuit = memo(move || live.state.with(|s| s.track.clone().map(Same)));
    let body = dynamic(move || {
        let Some(Same(plan)) = circuit.get() else {
            return p()
                .class("muted")
                .text(t(
                    "Carte indisponible pour cette session.",
                    "Map unavailable for this session.",
                ))
                .into();
        };
        let tab = |on: bool, name: &'static str| {
            button()
                .class("seg")
                .class_if("seg-active", move || three.get() == on)
                .attr_dyn("aria-pressed", move || (three.get() == on).to_string())
                .on_click(move |_| three.set(on))
                .text(name)
        };
        let drawing = dynamic(move || {
            if three.get() {
                map_3d(live, plan.clone())
            } else {
                map_2d(live, &plan)
            }
        });
        fragment([
            Node::from(h2().text(t("Carte en direct", "Live map"))),
            div()
                .class("segmented")
                .child(tab(false, t("Plan 2D", "2D map")))
                .child(tab(true, t("Relief 3D", "3D relief")))
                .into(),
            drawing,
        ])
    });
    section()
        .class("card")
        .child(body)
        .child(weather_grid(live))
        .into()
}

fn map_3d(live: LiveHandle, plan: Rc<TrackMap>) -> Node {
    // Relu par la vue 3D à chaque photo ; la vue elle-même n'est pas reconstruite.
    let markers = move || {
        live.state.with(|s| {
            s.snapshot.as_deref().map(|snap| {
                Rc::new(
                    on_track(snap)
                        .iter()
                        .map(|c| Marker {
                            key: c.code.clone(),
                            label: c.position.to_string(),
                            colour: c.colour.clone(),
                            fraction: c.lap_progress.unwrap_or(0.0),
                        })
                        .collect::<Vec<_>>(),
                )
            })
        })
    };
    fragment([
        view_3d(
            Scene::Track {
                map: plan,
                ghost: false,
            },
            markers,
        ),
        p().class("muted")
            .text(t(
                "Voitures aux couleurs de leur écurie (chiffre = position), placées d'après l'avancement de chaque pilote dans son tour. Glisse pour tourner autour du circuit.",
                "Cars in team colours (number = position), placed from each driver's progress through the lap. Drag to orbit the circuit.",
            ))
            .into(),
    ])
}

fn map_2d(live: LiveHandle, plan: &Rc<TrackMap>) -> Node {
    let (w, h) = (plan.width + 2.0 * PAD, plan.height + 2.0 * PAD);
    let points = plan
        .points
        .iter()
        .map(|pt| format!("{:.1},{:.1}", pt.x as f64 + PAD, pt.y as f64 + PAD))
        .collect::<Vec<_>>()
        .join(" ");
    let count = snap_memo(live, |s| on_track(s).len());
    let dots = {
        let plan = plan.clone();
        slots(count, move |j| car_dot(live, plan.clone(), j))
    };
    fragment([
        Node::from(
            svg()
                .class("track")
                .attr("viewBox", format!("0 0 {w:.0} {h:.0}"))
                .attr("role", "img")
                .attr(
                    "aria-label",
                    t(
                        "Position des voitures sur le circuit",
                        "Car positions on the circuit",
                    ),
                )
                .child(
                    polyline()
                        .class("track-base")
                        .attr("pathLength", "1")
                        .attr("points", points.clone()),
                )
                .child(
                    polyline()
                        .class("track-line")
                        .attr("pathLength", "1")
                        .attr("points", points),
                )
                .child(dots),
        ),
        p().class("muted")
            .text(t(
                "Positions estimées d'après l'avancement de chaque pilote dans son tour (couleur = écurie, chiffre = position).",
                "Positions estimated from each driver's progress through the lap (colour = team, number = position).",
            ))
            .into(),
    ])
}

/// Une voiture sur le plan 2D (la `j`-ième dans l'ordre de dessin).
#[derive(Clone, Default, PartialEq)]
struct Dot {
    x: f64,
    y: f64,
    colour: String,
    pit: bool,
    position: u32,
}

fn car_dot(live: LiveHandle, plan: Rc<TrackMap>, j: usize) -> Node {
    let dot = snap_memo(live, move |s| {
        on_track(s).get(j).map(|c| {
            let (x, y) = at_fraction(&plan, c.lap_progress.unwrap_or(0.0));
            Dot {
                x,
                y,
                colour: c.colour.clone(),
                pit: c.in_pit,
                position: c.position,
            }
        })
    });
    let pit = pick(dot, |d| d.pit);
    g().class("car")
        .attr_dyn(
            "transform",
            shown(dot, |d| format!("translate({:.1},{:.1})", d.x, d.y)),
        )
        .child(
            circle()
                .attr("r", "26")
                .attr_dyn("fill", shown(dot, |d| format!("#{}", d.colour)))
                .class_if("car-pit", move || pit.get()),
        )
        .child(
            text_svg()
                .class("car-label")
                .attr("text-anchor", "middle")
                .attr("dy", "9")
                .text_dyn(shown(dot, |d| d.position.to_string())),
        )
        .into()
}

/// Cases météo (comme `stat_grid`).
fn weather_cells(w: &Weather) -> Vec<(&'static str, String)> {
    vec![
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
        (
            t("Vent", "Wind"),
            format!(
                "{:.0} km/h{}",
                w.wind_speed * 3.6,
                w.wind_direction
                    .map(|d| format!(" {}", super::compass(d)))
                    .unwrap_or_default()
            ),
        ),
        (
            t("Écart piste/air", "Track vs air"),
            format!("{:+.0}°", w.track_temperature - w.air_temperature),
        ),
        (
            t("Pression", "Pressure"),
            w.pressure
                .map(|p| format!("{p:.0} hPa"))
                .unwrap_or_else(|| "–".into()),
        ),
    ]
}

/// Météo de la session : même grille que `stat_grid`, mais les valeurs changent sur place à
/// chaque relevé (les cases n'apparaissent qu'une fois).
fn weather_grid(live: LiveHandle) -> Node {
    let cells = snap_memo(live, |s| s.weather.as_ref().map(weather_cells));
    let present = memo(move || cells.with(Option::is_some));
    dynamic(move || {
        if !present.get() {
            return Node::Empty;
        }
        dl().class("stats")
            .children((0..6).map(|k| {
                let cell = memo(move || {
                    cells.with(|c| {
                        c.as_ref()
                            .and_then(|c| c.get(k).cloned())
                            .unwrap_or_default()
                    })
                });
                div()
                    .child(dt().text_dyn(move || cell.with(|c| c.0.to_string())))
                    .child(
                        // Valeurs longues (unités) : police adaptée à la largeur de la case.
                        dd().class_if("dd-long", move || cell.with(|c| c.1.chars().count() > 5))
                            .text_dyn(move || cell.with(|c| c.1.clone())),
                    )
            }))
            .into()
    })
}

// ---------- Stratégie ----------

fn parse_gap(s: &str) -> Option<f64> {
    s.trim_start_matches('+').parse().ok()
}

/// Place actuelle, place à la sortie des stands, voiture juste devant et juste derrière
/// (avec l'écart).
type Projection = Option<(u32, usize, Option<(String, f64)>, Option<(String, f64)>)>;

/// « Et s'il s'arrêtait maintenant ? » : temps perdu dans les stands ajouté à son écart au leader.
fn project(snap: &Snapshot, code: &str) -> Projection {
    let loss = snap.pit_loss.unwrap_or(22.0);
    let gap_of = |c: &Car| {
        if c.position == 1 {
            Some(0.0)
        } else {
            parse_gap(&c.gap)
        }
    };
    let car = snap.cars.iter().find(|c| c.code == code)?;
    let after = gap_of(car)? + loss;
    let others = || {
        snap.cars
            .iter()
            .filter(|c| c.code != car.code && !c.retired)
            .filter_map(|c| Some((c, gap_of(c)?)))
    };
    let mut ahead: Vec<(&Car, f64)> = others().filter(|(_, gap)| *gap < after).collect();
    ahead.sort_by(|a, b| a.1.total_cmp(&b.1));
    let new_pos = ahead.len() + 1;
    let in_front = ahead.last().map(|(c, gap)| (c.code.clone(), after - gap));
    let behind = others()
        .filter(|(_, gap)| *gap >= after)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, gap)| (c.code.clone(), gap - after));
    Some((car.position, new_pos, in_front, behind))
}

fn strategy(live: LiveHandle) -> Node {
    let chosen = use_state(untrack(|| {
        read(live, |s| {
            s.cars
                .get(3)
                .or(s.cars.first())
                .map(|c| c.code.clone())
                .unwrap_or_default()
        })
    }));
    let total = snap_memo(live, |s| s.total_laps.unwrap_or(s.lap.max(1)).max(1) as f32);
    let lap = snap_memo(live, |s| s.lap);
    let loss = snap_memo(live, |s| s.pit_loss.unwrap_or(22.0));
    let count = snap_memo(live, |s| s.cars.len());
    // Menu des pilotes : ne change qu'avec l'ordre de la course.
    let options = snap_memo(live, |s| {
        s.cars
            .iter()
            .map(|c| {
                (
                    c.code.clone(),
                    format!("P{} · {} {}", c.position, c.code, c.team),
                )
            })
            .collect::<Vec<_>>()
    });
    let projection = memo(move || {
        let code = chosen.get();
        read(live, |s| project(s, &code))
    });
    let has_projection = memo(move || projection.with(Option::is_some));

    let driver_select = select()
        .attr("aria-label", t("Choisir un pilote", "Choose a driver"))
        .on("change", move |e| chosen.set(e.value()))
        .children_keyed(
            move || options.get(),
            |(code, _)| code.clone(),
            move |(code, name)| {
                let o = option().attr("value", code.clone());
                let o = if untrack(|| chosen.with(|c| c == code)) {
                    o.attr("selected", "")
                } else {
                    o
                };
                o.text(name.clone()).into()
            },
        )
        // Garde le pilote choisi affiché quand l'ordre (et donc le menu) change.
        .attr_dyn("value", move || {
            options.with(|_| ());
            chosen.get()
        });

    let outcome = dynamic(move || {
        if !has_projection.get() {
            return p()
                .class("muted")
                .text(t(
                    "Écarts indisponibles pour ce pilote (doublé ou pas encore en course).",
                    "Gaps unavailable for this driver (lapped or not racing yet).",
                ))
                .into();
        }
        let has_front =
            memo(move || projection.with(|x| x.as_ref().is_some_and(|x| x.2.is_some())));
        let has_behind =
            memo(move || projection.with(|x| x.as_ref().is_some_and(|x| x.3.is_some())));
        fragment([
            Node::from(p().class("projection").text_dyn(move || {
                projection.with(|x| {
                    x.as_ref()
                        .map(|(from, to, _, _)| {
                            tr!(
                                "P{from} → ressortirait P{to}",
                                "P{from} → would rejoin P{to}"
                            )
                        })
                        .unwrap_or_default()
                })
            })),
            ul().class("sessions")
                .child(dynamic(move || {
                    if !has_front.get() {
                        return Node::Empty;
                    }
                    li().class("session")
                        .child(span().class("session-name").text(t("Derrière", "Behind")))
                        .child(span().class("session-time").text_dyn(move || {
                            projection.with(|x| {
                                x.as_ref()
                                    .and_then(|x| x.2.as_ref())
                                    .map(|(code, gap)| format!("{code} (+{gap:.1} s)"))
                                    .unwrap_or_default()
                            })
                        }))
                        .into()
                }))
                .child(dynamic(move || {
                    if !has_behind.get() {
                        return Node::Empty;
                    }
                    li().class("session")
                        .child(span().class("session-name").text(t("Devant", "Ahead of")))
                        .child(span().class("session-time").text_dyn(move || {
                            projection.with(|x| {
                                x.as_ref()
                                    .and_then(|x| x.3.as_ref())
                                    .map(|(code, gap)| format!("{code} ({gap:.1} s)"))
                                    .unwrap_or_default()
                            })
                        }))
                        .into()
                }))
                .into(),
        ])
    });

    fragment([
        Node::from(
            section()
                .class("card")
                .child(h2().text(t("Stratégie des pneus", "Tyre strategy")))
                .child(
                    ol().class("strategy")
                        .child(slots(count, move |i| strategy_row(live, i, total, lap))),
                )
                .child(p().class("muted").text(t(
                    "Chaque barre = la course d'un pilote, découpée en relais (lettre = gomme). Trait blanc = tour actuel.",
                    "Each bar = one driver's race, split into stints (letter = compound). White line = current lap.",
                ))),
        ),
        section()
            .class("card")
            .child(h2().text(t(
                "Et s'il s'arrêtait maintenant ?",
                "What if they pit now?",
            )))
            .child(
                label()
                    .class("select")
                    .child(span().class("select-label").text(t("Pilote", "Driver")))
                    .child(driver_select),
            )
            .child(outcome)
            .child(p().class("muted").text_dyn(move || {
                tr!(
                    "Estimation : temps perdu aux stands ≈ {:.1} s (médiane de la course), écarts actuels figés. Ne tient pas compte des pneus neufs ni du trafic.",
                    "Estimate: pit stop loss ≈ {:.1} s (race median), current gaps frozen. Ignores fresh tyres and traffic.",
                    loss.get()
                )
            }))
            .into(),
    ])
}

/// Barre de relais d'un pilote (la `i`-ième voiture de l'ordre de course).
fn strategy_row(live: LiveHandle, i: usize, total: State<f32>, lap: State<u32>) -> Node {
    let code = snap_memo(live, move |s| {
        s.cars.get(i).map(|c| c.code.clone()).unwrap_or_default()
    });
    let stints = snap_memo(live, move |s| {
        s.cars.get(i).map(|c| c.stints.clone()).unwrap_or_default()
    });
    let count = memo(move || stints.with(Vec::len));
    li().child(span().class("strat-code").text_dyn(move || code.get()))
        .child(
            span()
                .class("strat-bar")
                .child(slots(count, move |k| stint(stints, k, total)))
                .child(span().class("strat-now").attr_dyn("style", move || {
                    format!(
                        "left:{:.2}%",
                        (lap.get() as f32 / total.get() * 100.0).min(100.0)
                    )
                })),
        )
        .into()
}

fn stint(stints: State<Vec<StintInfo>>, k: usize, total: State<f32>) -> Node {
    let info = memo(move || stints.with(|l| l.get(k).cloned()));
    let compound = pick(info, |s| s.compound.clone());
    let place = memo(move || {
        let total = total.get();
        info.with(|s| {
            s.as_ref()
                .map(|s| {
                    let left = (s.from.saturating_sub(1)) as f32 / total * 100.0;
                    let width =
                        ((s.to.max(s.from) - s.from + 1) as f32 / total * 100.0).min(100.0 - left);
                    format!("left:{left:.2}%;width:{width:.2}%")
                })
                .unwrap_or_default()
        })
    });
    tyre_classes(span().class("stint"), compound)
        .attr_dyn(
            "title",
            shown(info, |s| {
                format!("{} · {}–{}", compound_name(&s.compound), s.from, s.to)
            }),
        )
        .attr_dyn("style", move || place.get())
        .text_dyn(move || compound.with(|c| tyre(c).0).to_string())
        .into()
}

// ---------- Championnat, radios, stands, vitesses ----------

/// Radios d'équipe, passages aux stands et vitesses de pointe (flux de chronométrage F1).
fn live_extras(live: LiveHandle) -> Node {
    fragment([
        live_championship(live),
        team_radios(live),
        pit_lane_times(live),
        top_speeds(live),
    ])
}

fn live_championship(live: LiveHandle) -> Node {
    let count = snap_memo(live, |s| s.championship.len().min(10));
    let present = memo(move || count.get() > 0);
    dynamic(move || {
        if !present.get() {
            return Node::Empty;
        }
        section()
            .class("card")
            .child(h2().text(t("Championnat en direct", "Live championship")))
            .child(
                ol().class("rows")
                    .child(slots(count, move |i| champ_row(live, i))),
            )
            .child(p().class("muted").text(t(
                "Classement pilotes si la course s'arrêtait maintenant.",
                "Drivers' standings if the race ended now.",
            )))
            .into()
    })
}

fn champ_row(live: LiveHandle, i: usize) -> Node {
    let row = snap_memo(live, move |s| s.championship.get(i).cloned());
    let moved = pick(row, |r| r.position_before as i32 - r.position_now as i32);
    let earned = pick(row, |r| r.points_now - r.points_before);
    let has_earned = memo(move || earned.get() > 0.0);
    li().class("row")
        .attr_dyn("style", shown(row, |r| format!("--team:#{}", r.colour)))
        .child(
            span()
                .class("pos")
                .text_dyn(shown(row, |r| r.position_now.to_string())),
        )
        .child(
            span().class("row-main").child(
                span()
                    .class("row-title")
                    .child(strong().text_dyn(shown(row, |r| r.name.clone())))
                    .child(dynamic(move || match moved.get() {
                        0 => Node::Empty,
                        m => gain_tag(m),
                    })),
            ),
        )
        .child(strong().text_dyn(shown(row, |r| format!("{:.0} pts", r.points_now))))
        .child(dynamic(move || {
            if !has_earned.get() {
                return Node::Empty;
            }
            small()
                .class("gain-up")
                .text_dyn(move || format!(" +{:.0}", earned.get()))
                .into()
        }))
        .into()
}

fn team_radios(live: LiveHandle) -> Node {
    let radios = snap_memo(live, |s| s.radios.clone());
    let present = memo(move || radios.with(|r| !r.is_empty()));
    dynamic(move || {
        if !present.get() {
            return Node::Empty;
        }
        section()
            .class("card")
            .child(h2().text(t("Radios d'équipe", "Team radio")))
            // Une radio arrivée s'ajoute en tête ; celle qu'on écoute n'est pas interrompue.
            .child(ol().class("rows").children_keyed(
                move || radios.get(),
                |r| r.url.clone(),
                |r| {
                    li().class("row")
                        .style(format!("--team:#{}", r.colour))
                        .child(
                            span().class("row-main").child(
                                span()
                                    .class("row-title")
                                    .child(strong().text(r.code.clone()))
                                    .text(" ")
                                    .child(span().class("muted").text(local_date(&r.date, true))),
                            ),
                        )
                        .child(
                            audio()
                                .attr("controls", "")
                                .attr("preload", "none")
                                .attr("src", r.url.clone())
                                .class("radio-audio"),
                        )
                        .into()
                },
            ))
            .into()
    })
}

fn pit_lane_times(live: LiveHandle) -> Node {
    let pits = snap_memo(live, |s| s.pit_times.clone());
    let present = memo(move || pits.with(|l| !l.is_empty()));
    dynamic(move || {
        if !present.get() {
            return Node::Empty;
        }
        section()
            .class("card")
            .child(h2().text(t("Passages aux stands", "Pit stops")))
            .child(ol().class("rows").children_keyed(
                move || pits.get(),
                |pit| (pit.date.clone(), pit.code.clone()),
                |pit| {
                    li().class("row")
                        .style(format!("--team:#{}", pit.colour))
                        .child(
                            span().class("row-main").child(
                                span()
                                    .class("row-title")
                                    .child(strong().text(pit.code.clone()))
                                    .text(
                                        pit.lap
                                            .map(|l| tr!(" · tour {l}", " · lap {l}"))
                                            .unwrap_or_default(),
                                    ),
                            ),
                        )
                        .child(
                            strong().text(
                                pit.duration
                                    .map(|d| format!("{d:.1} s"))
                                    .unwrap_or_else(|| "–".into()),
                            ),
                        )
                        .into()
                },
            ))
            .into()
    })
}

fn top_speeds(live: LiveHandle) -> Node {
    // (code, couleur, vitesse) des 10 plus rapides.
    let fast = snap_memo(live, |s| {
        let mut fast: Vec<&Car> = s.cars.iter().filter(|c| c.top_speed.is_some()).collect();
        fast.sort_by_key(|c| std::cmp::Reverse(c.top_speed));
        fast.iter()
            .take(10)
            .map(|c| (c.code.clone(), c.colour.clone(), c.top_speed.unwrap_or(0)))
            .collect::<Vec<_>>()
    });
    let count = memo(move || fast.with(Vec::len));
    let present = memo(move || count.get() > 0);
    dynamic(move || {
        if !present.get() {
            return Node::Empty;
        }
        section()
            .class("card")
            .child(h2().text(t("Vitesses de pointe", "Top speeds")))
            .child(ol().class("rows").child(slots(count, move |i| {
                let row = memo(move || fast.with(|l| l.get(i).cloned()));
                li().class("row")
                    .attr_dyn("style", shown(row, |r| format!("--team:#{}", r.1)))
                    .child(
                        span().class("row-main").child(
                            span()
                                .class("row-title")
                                .child(strong().text_dyn(shown(row, |r| r.0.clone()))),
                        ),
                    )
                    .child(strong().text_dyn(shown(row, |r| format!("{} km/h", r.2))))
                    .into()
            })))
            .into()
    })
}
