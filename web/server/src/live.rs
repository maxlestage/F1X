//! Direct et replay en WebSocket (`/ws`).
//!
//! - **Hub** : diffuse à tous les connectés le nombre de spectateurs, l'état du direct et,
//!   pendant une session, une photo de la course chaque seconde.
//! - **Replay** : par connexion, rejoue une session passée comme si elle était en direct
//!   (même format de messages), avec vitesse, pause et avance/retour.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use chrono::Utc;
use f1x_protocol::{ClientMsg, Mode, ServerMsg};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::openf1::{Dataset, OpenF1, parse_date};
use crate::race::{Frame, snapshot, time_range};

const TICK: Duration = Duration::from_secs(1);
/// Le direct OpenF1 a ~3 s de retard : on affiche l'état d'il y a 5 s.
const LIVE_DELAY_MS: i64 = 5_000;
const SPEEDS: [u32; 6] = [1, 2, 5, 10, 30, 60];

fn encode(msg: &ServerMsg) -> Arc<str> {
    serde_json::to_string(msg).unwrap_or_default().into()
}

pub struct Hub {
    pub openf1: Arc<OpenF1>,
    /// Messages pour tout le monde (spectateurs, état du direct).
    global: broadcast::Sender<Arc<str>>,
    /// Photos du direct.
    live: broadcast::Sender<Arc<str>>,
    status: RwLock<Arc<str>>,
    latest_live: RwLock<Option<Arc<str>>>,
    /// Tracé du circuit de la session en direct (envoyé à chaque nouveau spectateur).
    latest_track: RwLock<Option<Arc<str>>>,
    viewers: AtomicUsize,
}

impl Hub {
    pub fn new(openf1: Arc<OpenF1>) -> Arc<Self> {
        // Sans accès OpenF1 « direct », le flux de chronométrage public F1 prend le relais.
        let message = "Aucune session en cours pour le moment.";
        let status = encode(&ServerMsg::Status {
            live_available: true,
            live_active: false,
            message: message.into(),
        });
        let hub = Arc::new(Self {
            openf1,
            global: broadcast::channel(64).0,
            live: broadcast::channel(16).0,
            status: RwLock::new(status),
            latest_live: RwLock::new(None),
            latest_track: RwLock::new(None),
            viewers: AtomicUsize::new(0),
        });
        if hub.openf1.has_live_access() {
            tokio::spawn(live_loop(hub.clone()));
        } else {
            tokio::spawn(feed_loop(hub.clone()));
        }
        hub
    }

    fn viewers_msg(&self) -> Arc<str> {
        encode(&ServerMsg::Viewers {
            count: self.viewers.load(Ordering::Relaxed),
        })
    }

    fn join(&self) {
        self.viewers.fetch_add(1, Ordering::Relaxed);
        let _ = self.global.send(self.viewers_msg());
    }

    fn leave(&self) {
        self.viewers.fetch_sub(1, Ordering::Relaxed);
        let _ = self.global.send(self.viewers_msg());
    }

    async fn set_status(&self, live_active: bool, message: String) {
        let msg = encode(&ServerMsg::Status {
            live_available: true,
            live_active,
            message,
        });
        *self.status.write().await = msg.clone();
        let _ = self.global.send(msg);
    }
}

/// Surveille OpenF1 et diffuse la session en cours (nécessite un accès « direct »).
async fn live_loop(hub: Arc<Hub>) {
    loop {
        let session = match hub.openf1.get("sessions?session_key=latest").await {
            Ok(v) => v
                .as_array()
                .and_then(|a| a.first())
                .and_then(crate::openf1::session_from),
            Err(err) => {
                tracing::warn!(%err, "live: cannot fetch latest session");
                None
            }
        };
        let now = Utc::now().timestamp_millis();
        let active = session.as_ref().is_some_and(|s| {
            let start = parse_date(&s.date_start).unwrap_or(i64::MAX);
            let end = parse_date(&s.date_end).unwrap_or(0);
            now >= start - 15 * 60_000 && now <= end + 30 * 60_000
        });
        let Some(session) = session.filter(|_| active) else {
            hub.set_status(false, "Aucune session en cours pour le moment.".into())
                .await;
            *hub.latest_live.write().await = None;
            tokio::time::sleep(Duration::from_secs(60)).await;
            continue;
        };

        tracing::info!(key = session.session_key, "live: session active");
        hub.set_status(
            true,
            format!(
                "En direct : {} — {}",
                session.location, session.session_name
            ),
        )
        .await;
        let key = session.session_key;
        let mut data = Dataset {
            session: Some(session.clone()),
            ..Default::default()
        };
        let q = format!("session_key={key}");
        let mut last_poll = 0u32;
        let end = parse_date(&session.date_end).unwrap_or(i64::MAX);
        while Utc::now().timestamp_millis() <= end + 30 * 60_000 {
            // Rafraîchit les données (incrémental sur la date) toutes les ~4 s.
            let since = data
                .latest()
                .map(|t| format!("&date>{}", crate::openf1::iso(t - 1_000)))
                .unwrap_or_default();
            let o = &hub.openf1;
            if data.drivers.is_empty() || last_poll % 30 == 0 {
                if let Ok(v) = o.get(&format!("drivers?{q}")).await {
                    data.add_drivers(&v)
                }
                if let Ok(v) = o.get(&format!("stints?{q}")).await {
                    data.set_stints(&v)
                }
                if let Ok(v) = o.get(&format!("laps?{q}")).await {
                    data.add_laps(&v)
                }
                if let Ok(v) = o.get(&format!("pit?{q}")).await {
                    data.add_pits(&v)
                }
            }
            if let Ok(v) = o.get(&format!("position?{q}{since}")).await {
                data.add_positions(&v)
            }
            if let Ok(v) = o.get(&format!("intervals?{q}{since}")).await {
                data.add_intervals(&v)
            }
            if let Ok(v) = o.get(&format!("race_control?{q}{since}")).await {
                data.add_race_control(&v)
            }
            if last_poll % 5 == 0 {
                if let Ok(v) = o.get(&format!("weather?{q}{since}")).await {
                    data.add_weather(&v)
                }
                if let Ok(v) = o
                    .get(&format!(
                        "laps?{q}&lap_number>{}",
                        data.laps
                            .iter()
                            .map(|l| l.lap)
                            .max()
                            .unwrap_or(1)
                            .saturating_sub(1)
                    ))
                    .await
                {
                    data.add_laps(&v)
                }
            }
            last_poll += 1;

            // Diffuse une image par seconde jusqu'au prochain rafraîchissement.
            for _ in 0..4 {
                let at = Utc::now().timestamp_millis() - LIVE_DELAY_MS;
                let frame = Frame {
                    mode: Mode::Live,
                    speed: 1,
                    paused: false,
                    progress: 0.0,
                    finished: false,
                };
                let msg = encode(&ServerMsg::Snapshot(Box::new(snapshot(&data, at, frame))));
                *hub.latest_live.write().await = Some(msg.clone());
                let _ = hub.live.send(msg);
                tokio::time::sleep(TICK).await;
            }
        }
    }
}

/// Direct par le flux de chronométrage public F1 : une image par seconde pendant les séances
/// (essais, qualifications, sprint, course).
async fn feed_loop(hub: Arc<Hub>) {
    let (tx, mut rx) = mpsc::channel::<crate::livetiming::FeedMsg>(1024);
    tokio::spawn(async move {
        loop {
            match crate::livetiming::stream(&tx).await {
                Ok(()) => return,
                Err(err) => tracing::warn!(%err, "flux F1 : reconnexion dans 20 s"),
            }
            tokio::time::sleep(Duration::from_secs(20)).await;
        }
    });
    let mut feed = crate::livetiming::Feed::default();
    let mut tick = tokio::time::interval(TICK);
    let mut shown: Option<(u32, bool)> = None;
    let mut standings: Vec<(String, String, f64)> = Vec::new();
    let mut standings_for = None;
    let mut track_for = None;
    loop {
        tokio::select! {
            msg = rx.recv() => {
                let Some((topic, data, at)) = msg else { return };
                feed.apply(&topic, &data, at);
            }
            _ = tick.tick() => {
                let active = feed.active();
                let session = feed.session();
                if shown != Some((session.session_key, active)) {
                    shown = Some((session.session_key, active));
                    if active {
                        tracing::info!(key = session.session_key, "flux F1 : séance en direct");
                        hub.set_status(true, format!("En direct : {} — {}", session.location, session.session_name)).await;
                    } else {
                        hub.set_status(false, "Aucune session en cours pour le moment.".into()).await;
                        *hub.latest_live.write().await = None;
                    }
                }
                if !active {
                    continue;
                }
                // Tracé du circuit (même que le décor 3D), une fois par séance.
                if track_for != Some(session.session_key) {
                    track_for = Some(session.session_key);
                    let id = crate::openf1::ergast_circuit(&session.circuit)
                        .or_else(|| crate::openf1::ergast_circuit(&session.location));
                    let track = match id {
                        Some(id) => match hub.openf1.cached_track(id).await {
                            Some(t) => Some(t),
                            None => match crate::bundled_track(id)
                                .and_then(|j| serde_json::from_str::<f1x_protocol::TrackMap>(j).ok())
                            {
                                Some(t) => Some(hub.openf1.remember_track(id, t).await),
                                None => None,
                            },
                        },
                        None => None,
                    };
                    let msg = track.map(|t| encode(&ServerMsg::Track(Box::new((*t).clone()))));
                    if let Some(m) = &msg {
                        let _ = hub.live.send(m.clone());
                    }
                    *hub.latest_track.write().await = msg;
                }
                // Classement du championnat avant la séance (Jolpica), une fois par séance.
                if standings_for != Some(session.session_key) {
                    standings_for = Some(session.session_key);
                    standings = fetch_standings().await;
                }
                let at = Utc::now().timestamp_millis();
                let mut snap = feed.snapshot(at);
                snap.championship = live_championship(&snap, &standings);
                let msg = encode(&ServerMsg::Snapshot(Box::new(snap)));
                *hub.latest_live.write().await = Some(msg.clone());
                let _ = hub.live.send(msg);
            }
        }
    }
}

/// Classement pilotes actuel (Jolpica) : (code, nom, points).
async fn fetch_standings() -> Vec<(String, String, f64)> {
    let url = "https://api.jolpi.ca/ergast/f1/current/driverStandings.json?limit=100";
    let Ok(resp) = reqwest::get(url).await else {
        return Vec::new();
    };
    let Ok(v) = resp.json::<serde_json::Value>().await else {
        return Vec::new();
    };
    v.pointer("/MRData/StandingsTable/StandingsLists/0/DriverStandings")
        .and_then(|x| x.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    let d = r.get("Driver")?;
                    let code = d.get("code")?.as_str()?.to_string();
                    let name = format!(
                        "{} {}",
                        d.get("givenName")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default(),
                        d.get("familyName")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                    );
                    let pts = r.get("points")?.as_str()?.parse().ok()?;
                    Some((code, name, pts))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Championnat « si la course s'arrêtait maintenant » : points d'avant + barème des positions actuelles.
fn live_championship(
    snap: &f1x_protocol::Snapshot,
    standings: &[(String, String, f64)],
) -> Vec<f1x_protocol::ChampRow> {
    let scale: &[f64] = match snap.session.session_name.as_str() {
        "Race" => &[25.0, 18.0, 15.0, 12.0, 10.0, 8.0, 6.0, 4.0, 2.0, 1.0],
        "Sprint" => &[8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0],
        _ => return Vec::new(),
    };
    if standings.is_empty() {
        return Vec::new();
    }
    let mut rows: Vec<f1x_protocol::ChampRow> = standings
        .iter()
        .enumerate()
        .map(|(i, (code, name, pts))| {
            let car = snap.cars.iter().find(|c| &c.code == code);
            let earned = car
                .filter(|c| !c.retired)
                .and_then(|c| scale.get(c.position.saturating_sub(1) as usize).copied())
                .unwrap_or(0.0);
            f1x_protocol::ChampRow {
                code: code.clone(),
                name: name.clone(),
                colour: car.map(|c| c.colour.clone()).unwrap_or_default(),
                points_before: *pts,
                points_now: pts + earned,
                position_before: i as u32 + 1,
                position_now: 0,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.points_now
            .total_cmp(&a.points_now)
            .then(a.position_before.cmp(&b.position_before))
    });
    for (i, r) in rows.iter_mut().enumerate() {
        r.position_now = i as u32 + 1;
    }
    rows
}

enum Control {
    Speed(u32),
    Pause(bool),
    Seek(i64),
}

/// Rejoue une session : une image par seconde, `speed` secondes de course par image.
async fn replay(
    hub: Arc<Hub>,
    key: u32,
    mut speed: u32,
    out: mpsc::Sender<Arc<str>>,
    mut control: mpsc::UnboundedReceiver<Control>,
) {
    let _ = out
        .send(encode(&ServerMsg::Loading {
            message: "Chargement des données de la session…".into(),
        }))
        .await;
    let data = match hub.openf1.dataset(key).await {
        Ok(d) => d,
        Err(message) => {
            let _ = out.send(encode(&ServerMsg::Error { message })).await;
            return;
        }
    };
    if let Some(track) = &data.track {
        let _ = out
            .send(encode(&ServerMsg::Track(Box::new((**track).clone()))))
            .await;
    }
    let (start, end) = time_range(&data);
    let mut clock = start;
    let mut paused = false;
    let mut ticker = tokio::time::interval(TICK);
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            cmd = control.recv() => {
                match cmd {
                    Some(Control::Speed(s)) => speed = s,
                    Some(Control::Pause(p)) => paused = p,
                    Some(Control::Seek(secs)) => clock = (clock + secs * 1000).clamp(start, end),
                    None => return,
                }
            }
        }
        let finished = clock >= end;
        let frame = Frame {
            mode: Mode::Replay,
            speed,
            paused,
            progress: ((clock - start) as f32 / (end - start).max(1) as f32).clamp(0.0, 1.0),
            finished,
        };
        let msg = encode(&ServerMsg::Snapshot(Box::new(snapshot(
            &data, clock, frame,
        ))));
        if out.send(msg).await.is_err() {
            return;
        }
        if !paused && !finished {
            clock = (clock + speed as i64 * TICK.as_millis() as i64).min(end);
        }
    }
}

/// Une connexion WebSocket.
pub async fn client(socket: WebSocket, hub: Arc<Hub>) {
    let (mut sink, mut stream) = socket.split();
    let (out, mut out_rx) = mpsc::channel::<Arc<str>>(32);
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if sink.send(Message::Text(msg.as_ref().into())).await.is_err() {
                break;
            }
        }
    });

    let mut global = hub.global.subscribe();
    hub.join();
    let _ = out.send(hub.status.read().await.clone()).await;

    let mut task: Option<JoinHandle<()>> = None;
    let mut control: Option<mpsc::UnboundedSender<Control>> = None;
    // Heroku coupe les WebSockets inactives après 55 s : petit signe de vie régulier.
    let period = Duration::from_secs(25);
    let mut keepalive = tokio::time::interval_at(tokio::time::Instant::now() + period, period);

    loop {
        tokio::select! {
            incoming = stream.next() => {
                let Some(Ok(message)) = incoming else { break };
                let Message::Text(text) = message else {
                    if matches!(message, Message::Close(_)) { break }
                    continue;
                };
                let Ok(cmd) = serde_json::from_str::<ClientMsg>(&text) else {
                    let _ = out.send(encode(&ServerMsg::Error { message: "Message invalide".into() })).await;
                    continue;
                };
                match cmd {
                    ClientMsg::Live => {
                        if let Some(t) = task.take() { t.abort(); }
                        control = None;
                        if let Some(track) = hub.latest_track.read().await.clone() {
                            let _ = out.send(track).await;
                        }
                        if let Some(latest) = hub.latest_live.read().await.clone() {
                            let _ = out.send(latest).await;
                        }
                        let mut rx = hub.live.subscribe();
                        let out = out.clone();
                        task = Some(tokio::spawn(async move {
                            loop {
                                match rx.recv().await {
                                    Ok(msg) => if out.send(msg).await.is_err() { break },
                                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                                    Err(_) => break,
                                }
                            }
                        }));
                    }
                    ClientMsg::Replay { session_key, speed } => {
                        if let Some(t) = task.take() { t.abort(); }
                        let (tx, rx) = mpsc::unbounded_channel();
                        control = Some(tx);
                        let speed = if SPEEDS.contains(&speed) { speed } else { 10 };
                        task = Some(tokio::spawn(replay(hub.clone(), session_key, speed, out.clone(), rx)));
                    }
                    ClientMsg::Speed { speed } if SPEEDS.contains(&speed) => {
                        if let Some(c) = &control { let _ = c.send(Control::Speed(speed)); }
                    }
                    ClientMsg::Speed { .. } => {}
                    ClientMsg::Pause | ClientMsg::Resume => {
                        if let Some(c) = &control { let _ = c.send(Control::Pause(matches!(cmd, ClientMsg::Pause))); }
                    }
                    ClientMsg::Seek { seconds } => {
                        if let Some(c) = &control { let _ = c.send(Control::Seek(seconds.clamp(-7200, 7200))); }
                    }
                    ClientMsg::Stop => {
                        if let Some(t) = task.take() { t.abort(); }
                        control = None;
                        let _ = out.send(encode(&ServerMsg::Stopped)).await;
                    }
                }
            }
            msg = global.recv() => {
                match msg {
                    Ok(msg) => { let _ = out.send(msg).await; }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
            _ = keepalive.tick() => {
                if task.is_none() { let _ = out.send(hub.viewers_msg()).await; }
            }
        }
    }

    if let Some(t) = task {
        t.abort();
    }
    hub.leave();
    writer.abort();
}
