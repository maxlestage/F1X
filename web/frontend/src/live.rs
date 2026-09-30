//! Connexion WebSocket au serveur (`/ws`) : direct, replay, spectateurs.
//! Se reconnecte automatiquement et reprend le dernier suivi demandé.

use std::cell::Cell;
use std::rc::Rc;

use f1x_protocol::{ClientMsg, ServerMsg, Snapshot, TrackMap};
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt, future};
use gloo_net::websocket::{Message, futures::WebSocket};
use yew::prelude::*;

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

impl Reducible for LiveState {
    type Action = Action;

    fn reduce(self: Rc<Self>, action: Action) -> Rc<Self> {
        let mut s = (*self).clone();
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
        Rc::new(s)
    }
}

pub struct LiveHandle {
    pub state: UseReducerHandle<LiveState>,
    pub send: Callback<ClientMsg>,
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

#[hook]
pub fn use_live() -> LiveHandle {
    let state = use_reducer(LiveState::default);
    let sender = use_mut_ref(|| None::<mpsc::UnboundedSender<ClientMsg>>);
    let last_command = use_mut_ref(|| None::<ClientMsg>);

    {
        let dispatcher = state.dispatcher();
        let sender = sender.clone();
        let last_command = last_command.clone();
        use_effect_with((), move |_| {
            let alive = Rc::new(Cell::new(true));
            let task_alive = alive.clone();
            let task_sender = sender.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut backoff = 1_000;
                while task_alive.get() {
                    if let Some(ws) = ws_url().and_then(|url| WebSocket::open(&url).ok()) {
                        let (mut write, mut read) = ws.split();
                        let (tx, mut rx) = mpsc::unbounded::<ClientMsg>();
                        // Reprend le suivi en cours après une reconnexion.
                        if let Some(cmd) = last_command.borrow().clone() {
                            let _ = tx.unbounded_send(cmd);
                        }
                        *task_sender.borrow_mut() = Some(tx);
                        dispatcher.dispatch(Action::Connected(true));

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
                                    dispatcher.dispatch(Action::Server(msg));
                                }
                            }
                        };
                        future::select(Box::pin(writer), Box::pin(reader)).await;
                        dispatcher.dispatch(Action::Connected(false));
                    }
                    *task_sender.borrow_mut() = None;
                    if !task_alive.get() {
                        break;
                    }
                    gloo_timers::future::TimeoutFuture::new(backoff).await;
                    backoff = (backoff * 2).min(15_000);
                }
            });
            move || {
                alive.set(false);
                // Fermer l'émetteur termine la boucle d'écriture, donc la connexion.
                *sender.borrow_mut() = None;
            }
        });
    }

    let send = {
        let dispatcher = state.dispatcher();
        Callback::from(move |cmd: ClientMsg| {
            match &cmd {
                ClientMsg::Live | ClientMsg::Replay { .. } => {
                    *last_command.borrow_mut() = Some(cmd.clone())
                }
                ClientMsg::Stop => *last_command.borrow_mut() = None,
                _ => {}
            }
            if let Some(tx) = sender.borrow().as_ref() {
                let _ = tx.unbounded_send(cmd.clone());
            }
            dispatcher.dispatch(Action::Sent(cmd));
        })
    };
    LiveHandle { state, send }
}
