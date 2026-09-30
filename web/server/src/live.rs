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
    viewers: AtomicUsize,
}

impl Hub {
    pub fn new(openf1: Arc<OpenF1>) -> Arc<Self> {
        let message = if openf1.has_live_access() {
            "Aucune session en cours pour le moment."
        } else {
            "Le direct nécessite un accès OpenF1 (abonnement). En attendant, rejoue n'importe quelle session depuis 2023."
        };
        let status = encode(&ServerMsg::Status {
            live_available: openf1.has_live_access(),
            live_active: false,
            message: message.into(),
        });
        let hub = Arc::new(Self {
            openf1,
            global: broadcast::channel(64).0,
            live: broadcast::channel(16).0,
            status: RwLock::new(status),
            latest_live: RwLock::new(None),
            viewers: AtomicUsize::new(0),
        });
        if hub.openf1.has_live_access() {
            tokio::spawn(live_loop(hub.clone()));
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
