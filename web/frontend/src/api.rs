//! Accès aux données via le proxy `/api` du serveur, et hook Yew de chargement.

use std::rc::Rc;

use gloo_net::http::Request;
use serde::de::DeserializeOwned;
use yew::prelude::*;

pub enum Fetch<T> {
    Loading,
    Done(Rc<T>),
    Failed(String),
}

impl<T> Clone for Fetch<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Loading => Self::Loading,
            Self::Done(v) => Self::Done(Rc::clone(v)),
            Self::Failed(e) => Self::Failed(e.clone()),
        }
    }
}

impl<T> Fetch<T> {
    pub fn done(&self) -> Option<&T> {
        match self {
            Self::Done(v) => Some(v),
            _ => None,
        }
    }
}

async fn get_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let resp = Request::get(&format!("/api/{path}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!(
            "Les données F1 sont momentanément indisponibles ({}).",
            resp.status()
        ));
    }
    resp.json::<T>()
        .await
        .map_err(|e| format!("Réponse inattendue : {e}"))
}

/// Charge `/api/{path}` et relance le chargement si `path` change.
#[hook]
pub fn use_api<T: DeserializeOwned + 'static>(path: String) -> Fetch<T> {
    let state = use_state(|| Fetch::Loading);
    {
        let state = state.clone();
        use_effect_with(path, move |path| {
            let path = path.clone();
            wasm_bindgen_futures::spawn_local(async move {
                state.set(match get_json::<T>(&path).await {
                    Ok(v) => Fetch::Done(Rc::new(v)),
                    Err(e) => Fetch::Failed(e),
                });
            });
        });
    }
    (*state).clone()
}
