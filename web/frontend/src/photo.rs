//! Photos libres (Wikimedia Commons), obtenues via le serveur (`/api/photo/{titre}`).
//!
//! Seules les images hébergées sur Commons (licences libres) sont utilisées ; les fichiers
//! « non libres » propres à Wikipédia (logos, photos officielles) sont ignorés. Chaque photo est
//! créditée par un lien vers sa page Commons (auteur et licence).

use std::cell::RefCell;
use std::collections::HashMap;

use gloo_net::http::Request;
use serde::Deserialize;
use yew::prelude::*;

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
#[hook]
pub fn use_wiki_photo(url: Option<String>) -> Option<Photo> {
    let cached = url.as_deref().and_then(lookup);
    let state = use_state(|| cached.clone().flatten());
    {
        let state = state.clone();
        use_effect_with(url, move |url| {
            if let Some(url) = url.clone() {
                match lookup(&url) {
                    Some(hit) => state.set(hit),
                    None => wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(photo) = fetch(&url).await {
                            remember(url, photo.clone());
                            state.set(photo);
                        }
                    }),
                }
            }
        });
    }
    (*state).clone()
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

#[derive(Properties, PartialEq)]
pub struct AvatarProps {
    pub name: AttrValue,
    #[prop_or_default]
    pub url: Option<AttrValue>,
    /// Couleur d'écurie (CSS).
    #[prop_or_else(|| AttrValue::from("#8a8a99"))]
    pub colour: AttrValue,
    #[prop_or(40)]
    pub size: u32,
}

/// Avatar rond : photo libre si disponible, sinon initiales sur la couleur de l'écurie.
#[function_component]
pub fn Avatar(props: &AvatarProps) -> Html {
    let photo = use_wiki_photo(props.url.as_ref().map(|u| u.to_string()));
    let failed = use_state(|| None::<String>);
    // Image injoignable : retour aux initiales plutôt qu'une icône cassée.
    let photo = photo.filter(|p| failed.as_deref() != Some(p.src.as_str()));
    let style = format!("--av:{};--size:{}px", props.colour, props.size);
    html! {
        <span class="avatar" style={style} aria-hidden="true">
            if let Some(p) = photo {
                <img src={p.src.clone()} alt="" loading="lazy" referrerpolicy="no-referrer"
                    onerror={let failed = failed.clone(); move |_| failed.set(Some(p.src.clone()))} />
            } else {
                <span class="avatar-initials">{ initials(&props.name) }</span>
            }
        </span>
    }
}

#[derive(Properties, PartialEq)]
pub struct PhotoProps {
    pub url: AttrValue,
    pub alt: AttrValue,
    /// Image large (fiche pilote) ou plan / vue (circuit).
    #[prop_or_default]
    pub wide: bool,
}

/// Photo créditée (masquée s'il n'existe pas d'image libre).
#[function_component]
pub fn WikiPhoto(props: &PhotoProps) -> Html {
    let photo = use_wiki_photo(Some(props.url.to_string()));
    let failed = use_state(|| None::<String>);
    let Some(p) = photo.filter(|p| failed.as_deref() != Some(p.src.as_str())) else {
        return html! {};
    };
    let onerror = {
        let (failed, src) = (failed.clone(), p.src.clone());
        move |_| failed.set(Some(src.clone()))
    };
    html! {
        <figure class={classes!("wiki-photo", props.wide.then_some("wiki-photo-wide"))}>
            <img src={p.src.clone()} alt={props.alt.clone()} loading="lazy" referrerpolicy="no-referrer" {onerror} />
            <figcaption>
                <a href={p.credit} target="_blank" rel="noopener">{ t("Photo : Wikimedia Commons (auteur et licence) ↗", "Photo: Wikimedia Commons (author and licence) ↗") }</a>
            </figcaption>
        </figure>
    }
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
