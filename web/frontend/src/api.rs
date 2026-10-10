//! Accès aux données via le proxy `/api` du serveur, sous forme d'états active.
//!
//! - `f1(chemin)`  → une page (`/api/f1/{chemin}?limit=…`)
//! - `all(chemin)` → toutes les pages fusionnées par le serveur (`/api/all/{chemin}`)
//!
//! [`use_f1`] et [`use_json`] renvoient un [`State`] rempli quand la réponse arrive : lisez-le
//! dans une closure (`fetch_view`, `children_dyn`, `text_dyn`…) pour que la vue suive.

use std::cell::Cell;
use std::rc::Rc;

use active::{State, effect, untrack, use_state};
use gloo_net::http::Request;

use crate::models::{Envelope, MrData};
use crate::tr;

pub enum Fetch {
    Idle,
    Loading,
    Done(Rc<MrData>),
    Failed(String),
}

impl Clone for Fetch {
    fn clone(&self) -> Self {
        match self {
            Self::Idle => Self::Idle,
            Self::Loading => Self::Loading,
            Self::Done(v) => Self::Done(Rc::clone(v)),
            Self::Failed(e) => Self::Failed(e.clone()),
        }
    }
}

impl Fetch {
    pub fn done(&self) -> Option<&MrData> {
        match self {
            Self::Done(v) => Some(v),
            _ => None,
        }
    }
    /// Les données, partagées (sans copie).
    pub fn data(&self) -> Option<Rc<MrData>> {
        match self {
            Self::Done(v) => Some(Rc::clone(v)),
            _ => None,
        }
    }
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// Une page d'un endpoint, ex. `f1("2026/15/results.json", 100)`.
pub fn f1(path: impl AsRef<str>, limit: u32) -> Option<String> {
    Some(format!("f1/{}?limit={limit}", path.as_ref()))
}

/// Toutes les pages d'un endpoint (pagination faite par le serveur).
pub fn all(path: impl AsRef<str>) -> Option<String> {
    Some(format!("all/{}", path.as_ref()))
}

async fn get(path: &str) -> Result<MrData, String> {
    let resp = Request::get(&format!("/api/{path}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(tr!(
            "Les données F1 sont momentanément indisponibles ({}).",
            "F1 data is temporarily unavailable ({}).",
            resp.status()
        ));
    }
    resp.json::<Envelope>()
        .await
        .map(|e| e.data)
        .map_err(|e| tr!("Réponse inattendue : {e}", "Unexpected response: {e}"))
}

/// Charge `/api/{path}` ; `None` = ne rien charger.
pub fn use_f1(path: Option<String>) -> State<Fetch> {
    use_f1_dyn(move || path.clone())
}

/// Comme [`use_f1`], avec un chemin calculé à partir d'états : le chargement repart quand il
/// change (`None` = ne rien charger, ex. chargement à la demande).
pub fn use_f1_dyn(path: impl Fn() -> Option<String> + 'static) -> State<Fetch> {
    let state = use_state(Fetch::Idle);
    let generation = Rc::new(Cell::new(0u32));
    effect(move || {
        let path = path();
        // Une réponse arrivée après un changement de chemin (ou de page) est ignorée.
        let current = generation.get().wrapping_add(1);
        generation.set(current);
        untrack(|| match path {
            None => state.set(Fetch::Idle),
            Some(path) => {
                state.set(Fetch::Loading);
                let generation = generation.clone();
                let alive = move || state.is_alive() && generation.get() == current;
                wasm_bindgen_futures::spawn_local(async move {
                    // Réessaie automatiquement si le serveur est momentanément limité par l'API F1.
                    let mut result = get(&path).await;
                    for delay in [8_000, 20_000] {
                        if result.is_ok() || !alive() {
                            break;
                        }
                        gloo_timers::future::TimeoutFuture::new(delay).await;
                        result = get(&path).await;
                    }
                    if alive() {
                        state.set(match result {
                            Ok(v) => Fetch::Done(Rc::new(v)),
                            Err(e) => Fetch::Failed(e),
                        });
                    }
                });
            }
        });
    });
    state
}

/// Résultat d'un JSON quelconque : `None` pendant le chargement, `Some(Err((statut, texte)))`
/// en cas d'échec (statut 0 : réseau ou JSON invalide).
pub type Json<T> = Option<Result<Rc<T>, (u16, String)>>;

/// Charge un JSON quelconque (`None` = ne rien charger).
pub fn use_json<T: serde::de::DeserializeOwned + 'static>(url: Option<String>) -> State<Json<T>> {
    use_json_dyn(move || url.clone())
}

/// Comme [`use_json`], avec une adresse calculée à partir d'états (rechargée quand elle change).
pub fn use_json_dyn<T: serde::de::DeserializeOwned + 'static>(
    url: impl Fn() -> Option<String> + 'static,
) -> State<Json<T>> {
    let state = use_state(None);
    let generation = Rc::new(Cell::new(0u32));
    effect(move || {
        let url = url();
        let current = generation.get().wrapping_add(1);
        generation.set(current);
        untrack(|| {
            state.set(None);
            let Some(url) = url else { return };
            let generation = generation.clone();
            let alive = move || state.is_alive() && generation.get() == current;
            wasm_bindgen_futures::spawn_local(async move {
                let fetch = || async {
                    match Request::get(&url).send().await {
                        Ok(r) if r.ok() => r
                            .json::<T>()
                            .await
                            .map(Rc::new)
                            .map_err(|e| (0, e.to_string())),
                        Ok(r) => Err((r.status(), r.text().await.unwrap_or_default())),
                        Err(e) => Err((0, e.to_string())),
                    }
                };
                // Réessaie les erreurs temporaires (serveur limité par une API), pas les 404.
                let mut result = fetch().await;
                for delay in [8_000, 20_000] {
                    match &result {
                        Err((status, _)) if *status != 404 && alive() => {
                            gloo_timers::future::TimeoutFuture::new(delay).await;
                            result = fetch().await;
                        }
                        _ => break,
                    }
                }
                if alive() {
                    state.set(Some(result));
                }
            });
        });
    });
    state
}
