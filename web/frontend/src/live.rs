//! Connexion WebSocket au serveur (`/ws`) : direct, replay, spectateurs.
//! Se reconnecte automatiquement et reprend le dernier suivi demandé.

use std::cell::Cell;
use std::rc::Rc;

use active::{State, on_cleanup, use_state};
use f1x_protocol::{ClientMsg, ServerMsg, Snapshot, TrackMap};
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt, future};
use gloo_net::websocket::{Message, futures::WebSocket};

#[derive(Clone, Default, PartialEq)]
pub struct LiveState {
    pub connected: bool,
    pub viewers: usize,
    pub live_available: bool,
    pub live_active: bool,
    pub status: String,
    pub loading: Option<String>,
    pub error: Option<String>,
    pub snapshot: Option<Rc<Snapshot>>,
    pub track: Option<Rc<TrackMap>>,
}

pub enum Action {
    Connected(bool),
    Server(ServerMsg),
    /// Commande envoyée : on efface l'erreur et on attend la réponse.
    Sent(ClientMsg),
}

impl LiveState {
    fn apply(&mut self, action: Action) {
        let s = self;
        match action {
            Action::Connected(c) => s.connected = c,
            Action::Sent(ClientMsg::Stop) => {
                s.snapshot = None;
                s.loading = None;
                s.error = None;
            }
            Action::Sent(ClientMsg::Replay { .. } | ClientMsg::Live) => {
                s.snapshot = None;
                s.track = None;
                s.error = None;
            }
            Action::Sent(_) => {}
            Action::Server(msg) => match msg {
                ServerMsg::Status {
                    live_available,
                    live_active,
                    message,
                } => {
                    s.live_available = live_available;
                    s.live_active = live_active;
                    s.status = message;
                }
                ServerMsg::Viewers { count } => s.viewers = count,
                ServerMsg::Loading { message } => s.loading = Some(message),
                ServerMsg::Snapshot(snap) => {
                    s.loading = None;
                    s.snapshot = Some(Rc::new(*snap));
                }
                ServerMsg::Track(track) => s.track = Some(Rc::new(*track)),
                ServerMsg::Stopped => {
                    s.snapshot = None;
                    s.loading = None;
                }
                ServerMsg::Error { message } => {
                    s.loading = None;
                    s.error = Some(message);
                }
            },
        }
    }
}

/// La connexion : son émetteur et la dernière commande de suivi (reprise après reconnexion).
#[derive(Default)]
struct Link {
    sender: Option<mpsc::UnboundedSender<ClientMsg>>,
    last_command: Option<ClientMsg>,
}

/// Le direct : l'état reçu du serveur (`state`, à lire dans des closures) et l'envoi de
/// commandes (`send`). `Copy` : se déplace dans autant de closures que nécessaire.
#[derive(Clone, Copy)]
pub struct LiveHandle {
    pub state: State<LiveState>,
    link: State<Link>,
}

impl LiveHandle {
    pub fn send(self, cmd: ClientMsg) {
        if !self.link.is_alive() {
            return;
        }
        self.link.update(|link| {
            match &cmd {
                ClientMsg::Live | ClientMsg::Replay { .. } => link.last_command = Some(cmd.clone()),
                ClientMsg::Stop => link.last_command = None,
                _ => {}
            }
            if let Some(tx) = link.sender.as_ref() {
                let _ = tx.unbounded_send(cmd.clone());
            }
        });
        self.state.update(|s| s.apply(Action::Sent(cmd)));
    }
}

fn ws_url() -> Option<String> {
    let location = web_sys::window()?.location();
    let scheme = if location.protocol().ok()? == "https:" {
        "wss"
    } else {
        "ws"
    };
    Some(format!("{scheme}://{}/ws", location.host().ok()?))
}

/// Ouvre la connexion WebSocket pour le composant en cours ; elle se ferme quand il est
/// libéré (changement de page).
pub fn use_live() -> LiveHandle {
    let state = use_state(LiveState::default());
    let link = use_state(Link::default());
    let alive = Rc::new(Cell::new(true));
    let dispatch = move |action: Action| {
        if state.is_alive() {
            state.update(|s| s.apply(action));
        }
    };
    {
        let alive = alive.clone();
        on_cleanup(move || {
            alive.set(false);
            // Fermer l'émetteur termine la boucle d'écriture, donc la connexion.
            link.update(|l| l.sender = None);
        });
    }
    wasm_bindgen_futures::spawn_local(async move {
        let mut backoff = 1_000;
        while alive.get() {
            if let Some(ws) = ws_url().and_then(|url| WebSocket::open(&url).ok()) {
                let (mut write, mut read) = ws.split();
                let (tx, mut rx) = mpsc::unbounded::<ClientMsg>();
                if !alive.get() {
                    break;
                }
                link.update(|l| {
                    // Reprend le suivi en cours après une reconnexion.
                    if let Some(cmd) = l.last_command.clone() {
                        let _ = tx.unbounded_send(cmd);
                    }
                    l.sender = Some(tx);
                });
                dispatch(Action::Connected(true));

                let writer = async {
                    while let Some(cmd) = rx.next().await {
                        let text = serde_json::to_string(&cmd).unwrap_or_default();
                        if write.send(Message::Text(text)).await.is_err() {
                            break;
                        }
                    }
                };
                let reader = async {
                    while let Some(Ok(Message::Text(text))) = read.next().await {
                        backoff = 1_000;
                        if let Ok(msg) = serde_json::from_str::<ServerMsg>(&text) {
                            dispatch(Action::Server(msg));
                        }
                    }
                };
                future::select(Box::pin(writer), Box::pin(reader)).await;
                dispatch(Action::Connected(false));
            }
            if !alive.get() {
                break;
            }
            link.update(|l| l.sender = None);
            gloo_timers::future::TimeoutFuture::new(backoff).await;
            backoff = (backoff * 2).min(15_000);
        }
    });
    LiveHandle { state, link }
}
