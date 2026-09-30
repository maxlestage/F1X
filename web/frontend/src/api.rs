//! Accès aux données via le proxy `/api` du serveur, et hook Yew de chargement.
//!
//! - `f1(chemin)`  → une page (`/api/f1/{chemin}?limit=…`)
//! - `all(chemin)` → toutes les pages fusionnées par le serveur (`/api/all/{chemin}`)

use std::cell::Cell;
use std::rc::Rc;

use gloo_net::http::Request;
use yew::prelude::*;

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

/// Charge `/api/{path}` ; `None` = ne rien charger (chargement différé).
/// Relance le chargement quand `path` change.
#[hook]
pub fn use_f1(path: Option<String>) -> Fetch {
    let state = use_state(|| {
        if path.is_some() {
            Fetch::Loading
        } else {
            Fetch::Idle
        }
    });
    {
        let state = state.clone();
        use_effect_with(path, move |path| {
            let alive = Rc::new(Cell::new(true));
            match path.clone() {
                None => state.set(Fetch::Idle),
                Some(path) => {
                    state.set(Fetch::Loading);
                    let alive_task = alive.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let result = get(&path).await;
                        // Ignore une réponse arrivée après un changement de page/paramètre.
                        if alive_task.get() {
                            state.set(match result {
                                Ok(v) => Fetch::Done(Rc::new(v)),
                                Err(e) => Fetch::Failed(e),
                            });
                        }
                    });
                }
            }
            move || alive.set(false)
        });
    }
    (*state).clone()
}

/// Charge un JSON quelconque (`None` = ne rien charger). `Some(Err)` en cas d'échec.
#[hook]
pub fn use_json<T: serde::de::DeserializeOwned + 'static>(
    url: Option<String>,
) -> Option<Result<Rc<T>, (u16, String)>> {
    let state = use_state(|| None);
    {
        let state = state.clone();
        use_effect_with(url, move |url| {
            let alive = Rc::new(Cell::new(true));
            if let Some(url) = url.clone() {
                state.set(None);
                let alive = alive.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let result = match Request::get(&url).send().await {
                        Ok(r) if r.ok() => r
                            .json::<T>()
                            .await
                            .map(Rc::new)
                            .map_err(|e| (0, e.to_string())),
                        Ok(r) => Err((r.status(), r.text().await.unwrap_or_default())),
                        Err(e) => Err((0, e.to_string())),
                    };
                    if alive.get() {
                        state.set(Some(result));
                    }
                });
            }
            move || alive.set(false)
        });
    }
    (*state).clone()
}
