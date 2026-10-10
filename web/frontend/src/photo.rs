//! Photos libres (Wikimedia Commons), obtenues via le serveur (`/api/photo/{titre}`).
//!
//! Seules les images hébergées sur Commons (licences libres) sont utilisées ; les fichiers
//! « non libres » propres à Wikipédia (logos, photos officielles) sont ignorés. Chaque photo est
//! créditée par un lien vers sa page Commons (auteur et licence).

use std::cell::RefCell;
use std::collections::HashMap;

use gloo_net::http::Request;
use serde::Deserialize;
use active::prelude::*;

use crate::components::dynamic;
use crate::i18n::t;

#[derive(Clone, PartialEq, serde::Serialize, Deserialize)]
pub struct Photo {
    pub src: String,
    /// Page Commons du fichier (auteur, licence).
    pub credit: String,
}

thread_local! {
    static CACHE: RefCell<HashMap<String, Option<Photo>>> = RefCell::new(HashMap::new());
    /// Requêtes de photos en cours : on les étale pour ne pas saturer le réseau mobile.
    static IN_FLIGHT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

const MAX_IN_FLIGHT: u32 = 4;

/// « https://en.wikipedia.org/wiki/Lewis_Hamilton » → « Lewis_Hamilton ».
fn title(url: &str) -> Option<String> {
    let t = url.split("/wiki/").nth(1)?.split(['#', '?']).next()?;
    (!t.is_empty()).then(|| t.to_string())
}

/// `Err` : échec passager (réseau, limite de débit) → non mis en cache, retenté plus tard.
async fn fetch(url: &str) -> Result<Option<Photo>, ()> {
    let Some(title) = title(url) else {
        return Ok(None);
    };
    // Passe par le serveur : cache partagé, une seule requête Wikipédia par photo.
    let api = format!("/api/photo/{title}");
    let mut attempt = 0;
    let resp = loop {
        while IN_FLIGHT.with(|n| n.get()) >= MAX_IN_FLIGHT {
            gloo_timers::future::TimeoutFuture::new(120).await;
        }
        IN_FLIGHT.with(|n| n.set(n.get() + 1));
        let resp = Request::get(&api).send().await;
        IN_FLIGHT.with(|n| n.set(n.get() - 1));
        let resp = resp.map_err(|_| ())?;
        if matches!(resp.status(), 429 | 503) && attempt < 3 {
            attempt += 1;
            gloo_timers::future::TimeoutFuture::new(1500 * attempt).await;
            continue;
        }
        break resp;
    };
    if resp.status() == 404 {
        return Ok(None);
    }
    if !resp.ok() {
        return Err(());
    }
    resp.json().await.map(Some).map_err(|_| ())
}

const STORE_KEY: &str = "f1x-photos";

fn lookup(url: &str) -> Option<Option<Photo>> {
    if let Some(hit) = CACHE.with(|c| c.borrow().get(url).cloned()) {
        return Some(hit);
    }
    // Cache persistant : évite de redemander les mêmes photos à chaque visite.
    let stored: HashMap<String, Option<Photo>> = crate::util::load(STORE_KEY)?;
    let hit = stored.get(url).cloned()?;
    CACHE.with(|c| c.borrow_mut().insert(url.to_string(), hit.clone()));
    Some(hit)
}

fn remember(url: String, photo: Option<Photo>) {
    let mut stored: HashMap<String, Option<Photo>> =
        crate::util::load(STORE_KEY).unwrap_or_default();
    if stored.len() > 400 {
        stored.clear();
    }
    stored.insert(url.clone(), photo.clone());
    crate::util::store(STORE_KEY, &stored);
    CACHE.with(|c| c.borrow_mut().insert(url, photo));
}

/// Photo libre associée à une page Wikipédia (`None` tant qu'elle charge ou s'il n'y en a pas).
pub fn use_wiki_photo(url: Option<String>) -> State<Option<Photo>> {
    let cached = url.as_deref().and_then(lookup);
    let state = use_state(cached.clone().flatten());
    if let (Some(url), None) = (url, cached) {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(photo) = fetch(&url).await {
                remember(url, photo.clone());
                if state.is_alive() {
                    state.set(photo);
                }
            }
        });
    }
    state
}

fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.as_slice() {
        [] => "?".into(),
        [one] => one.chars().take(2).collect(),
        [first, .., last] => format!(
            "{}{}",
            first.chars().next().unwrap_or(' '),
            last.chars().next().unwrap_or(' ')
        ),
    }
    .to_uppercase()
}

/// La photo, sauf si elle n'a pas pu s'afficher (`failed`).
fn usable(photo: State<Option<Photo>>, failed: State<Option<String>>) -> Option<Photo> {
    photo
        .get()
        .filter(|p| failed.with(|f| f.as_deref() != Some(p.src.as_str())))
}

/// Avatar rond : photo libre si disponible, sinon initiales sur la couleur de l'écurie
/// (`colour`, CSS ; `size` en pixels, 40 d'habitude).
pub fn avatar(name: &str, url: Option<&str>, colour: &str, size: u32) -> Node {
    let photo = use_wiki_photo(url.map(str::to_string));
    let failed = use_state(None::<String>);
    let initials = initials(name);
    span()
        .class("avatar")
        .style(format!("--av:{colour};--size:{size}px"))
        .attr("aria-hidden", "true")
        .child(dynamic(move || match usable(photo, failed) {
            // Image injoignable : retour aux initiales plutôt qu'une icône cassée.
            Some(p) => {
                let src = p.src.clone();
                img()
                    .attr("src", p.src)
                    .attr("alt", "")
                    .attr("loading", "lazy")
                    .attr("referrerpolicy", "no-referrer")
                    .on("error", move |_| failed.set(Some(src.clone())))
                    .into()
            }
            None => span()
                .class("avatar-initials")
                .text(initials.clone())
                .into(),
        }))
        .into()
}

/// Photo créditée (masquée s'il n'existe pas d'image libre). `wide` : image large (fiche
/// pilote) ou plan / vue (circuit).
pub fn wiki_photo(url: &str, alt: &str, wide: bool) -> Node {
    let photo = use_wiki_photo(Some(url.to_string()));
    let failed = use_state(None::<String>);
    let alt = alt.to_string();
    dynamic(move || {
        let Some(p) = usable(photo, failed) else {
            return Node::Empty;
        };
        let src = p.src.clone();
        figure()
            .class("wiki-photo")
            .class(crate::components::when(wide, "wiki-photo-wide"))
            .child(
                img()
                    .attr("src", p.src)
                    .attr("alt", alt.clone())
                    .attr("loading", "lazy")
                    .attr("referrerpolicy", "no-referrer")
                    .on("error", move |_| failed.set(Some(src.clone()))),
            )
            .child(
                figcaption().child(
                    a().href(p.credit)
                        .attr("target", "_blank")
                        .attr("rel", "noopener")
                        .text(t(
                            "Photo : Wikimedia Commons (auteur et licence) ↗",
                            "Photo: Wikimedia Commons (author and licence) ↗",
                        )),
                ),
            )
            .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_titles_and_initials() {
        assert_eq!(
            title("https://en.wikipedia.org/wiki/Lewis_Hamilton").as_deref(),
            Some("Lewis_Hamilton")
        );
        assert_eq!(initials("Lewis Hamilton"), "LH");
    }
}
