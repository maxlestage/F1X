//! Photos libres (Wikimedia Commons) via l'API de résumé de Wikipédia, mises en cache côté
//! serveur : chaque photo n'est demandée qu'une fois pour tous les visiteurs, ce qui reste
//! largement sous la limite de débit de Wikipédia (que des rafales depuis les navigateurs
//! déclenchent facilement).
//!
//! Seules les images hébergées sur Commons (licences libres) sont retenues ; les fichiers
//! « non libres » propres à Wikipédia (logos, photos officielles) sont ignorés.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const TTL: Duration = Duration::from_secs(7 * 24 * 3600);
const RETRY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Photo {
    pub src: String,
    /// Page Commons du fichier (auteur, licence).
    pub credit: String,
}

#[derive(Deserialize)]
struct Summary {
    thumbnail: Option<Thumb>,
}

#[derive(Deserialize)]
struct Thumb {
    source: String,
}

/// (date de récupération, photo).
type Cached = (Instant, Option<Photo>);

#[derive(Clone)]
pub struct Photos {
    http: reqwest::Client,
    cache: Arc<Mutex<HashMap<String, Cached>>>,
    /// Une seule requête sortante à la fois.
    gate: Arc<Mutex<()>>,
}

impl Photos {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(crate::user_agent())
                .timeout(Duration::from_secs(8))
                .build()
                .expect("http client"),
            cache: Arc::default(),
            gate: Arc::default(),
        }
    }

    /// `Err` : échec passager (le client réessaiera plus tard).
    pub async fn get(&self, title: &str) -> Result<Option<Photo>, ()> {
        if !valid_title(title) {
            return Ok(None);
        }
        if let Some((at, hit)) = self.cache.lock().await.get(title) {
            if at.elapsed() < TTL {
                return Ok(hit.clone());
            }
        }
        let _gate = self.gate.lock().await;
        // Une autre requête a pu remplir le cache pendant l'attente.
        if let Some((at, hit)) = self.cache.lock().await.get(title) {
            if at.elapsed() < RETRY {
                return Ok(hit.clone());
            }
        }
        let mut url =
            reqwest::Url::parse("https://en.wikipedia.org/api/rest_v1/page/summary/").expect("url");
        url.path_segments_mut()
            .expect("base")
            .pop_if_empty()
            .push(title);
        let mut resp = self.http.get(url.clone()).send().await.map_err(|_| ())?;
        for wait in [1, 3] {
            if resp.status() != reqwest::StatusCode::TOO_MANY_REQUESTS {
                break;
            }
            tokio::time::sleep(Duration::from_secs(wait)).await;
            resp = self.http.get(url.clone()).send().await.map_err(|_| ())?;
        }
        let photo = match resp.status().as_u16() {
            404 => None,
            s if (200..300).contains(&s) => {
                let summary: Summary = resp.json().await.map_err(|_| ())?;
                summary.thumbnail.and_then(|t| {
                    let credit = commons_page(&t.source)?;
                    Some(Photo {
                        src: t.source,
                        credit,
                    })
                })
            }
            _ => return Err(()),
        };
        let mut cache = self.cache.lock().await;
        if cache.len() > 2000 {
            cache.clear();
        }
        cache.insert(title.to_string(), (Instant::now(), photo.clone()));
        Ok(photo)
    }
}

/// Titre d'article Wikipédia (décodé) : un seul segment de chemin.
fn valid_title(title: &str) -> bool {
    !title.is_empty()
        && title.len() <= 200
        && !title.starts_with('.')
        && !title.chars().any(|c| c.is_control() || "/?#\\".contains(c))
}

/// URL Commons d'une miniature → page du fichier (`None` si l'image n'est pas sur Commons).
fn commons_page(src: &str) -> Option<String> {
    // …/wikipedia/commons/thumb/d/d3/Fichier.jpg/330px-Fichier.jpg
    let after = src.split("/wikipedia/commons/").nth(1)?;
    let parts: Vec<&str> = after.split('/').collect();
    let file = if parts.first() == Some(&"thumb") {
        parts.get(3)?
    } else {
        parts.get(2)?
    };
    Some(format!("https://commons.wikimedia.org/wiki/File:{file}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commons_only() {
        assert_eq!(
            commons_page(
                "https://upload.wikimedia.org/wikipedia/commons/thumb/d/d3/A_b.jpg/330px-A_b.jpg"
            )
            .as_deref(),
            Some("https://commons.wikimedia.org/wiki/File:A_b.jpg")
        );
        assert_eq!(
            commons_page("https://upload.wikimedia.org/wikipedia/commons/d/d3/A_b.jpg").as_deref(),
            Some("https://commons.wikimedia.org/wiki/File:A_b.jpg")
        );
        assert!(commons_page("https://upload.wikimedia.org/wikipedia/en/a/ab/Logo.png").is_none());
    }

    #[test]
    fn titles() {
        assert!(valid_title("Sergio_Pérez"));
        assert!(valid_title("Lewis_Hamilton"));
        assert!(!valid_title("../etc"));
        assert!(!valid_title("a/b"));
        assert!(!valid_title(""));
    }
}
