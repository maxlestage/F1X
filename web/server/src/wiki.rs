//! Résumés Wikipédia (API REST publique) pour les fiches pilote, écurie et circuit :
//! texte d'introduction dans la langue demandée (via les liens interlangues) et lien vers l'article.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;

const TTL: Duration = Duration::from_secs(6 * 3600);

#[derive(Clone, Serialize)]
pub struct Summary {
    pub title: String,
    pub extract: String,
    pub url: String,
    pub lang: String,
}

pub struct Wiki {
    http: reqwest::Client,
    cache: Mutex<HashMap<String, (Instant, Option<Summary>)>>,
}

impl Wiki {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("F1X/1.0 (https://f1x-29170430865f.herokuapp.com)")
                .timeout(Duration::from_secs(10))
                .build()
                .expect("client"),
            cache: Mutex::new(HashMap::new()),
        }
    }

    async fn json(&self, url: reqwest::Url) -> Option<Value> {
        let resp = self.http.get(url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        resp.json().await.ok()
    }

    /// Titre de l'article dans une autre langue (lien interlangue depuis l'anglais).
    async fn translated(&self, en_title: &str, lang: &str) -> Option<String> {
        let mut url = reqwest::Url::parse("https://en.wikipedia.org/w/api.php").ok()?;
        url.query_pairs_mut()
            .append_pair("action", "query")
            .append_pair("prop", "langlinks")
            .append_pair("lllang", lang)
            .append_pair("redirects", "1")
            .append_pair("format", "json")
            .append_pair("titles", en_title);
        let v = self.json(url).await?;
        v.get("query")?
            .get("pages")?
            .as_object()?
            .values()
            .find_map(|p| {
                p.get("langlinks")?
                    .get(0)?
                    .get("*")?
                    .as_str()
                    .map(str::to_string)
            })
    }

    async fn summary(&self, lang: &str, title: &str) -> Option<Summary> {
        let mut url = reqwest::Url::parse(&format!(
            "https://{lang}.wikipedia.org/api/rest_v1/page/summary/"
        ))
        .ok()?;
        url.path_segments_mut().ok()?.pop_if_empty().push(title);
        let v = self.json(url).await?;
        let extract = v.get("extract")?.as_str()?.trim().to_string();
        if extract.is_empty() {
            return None;
        }
        Some(Summary {
            title: v
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(title)
                .to_string(),
            extract,
            url: v
                .pointer("/content_urls/desktop/page")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            lang: lang.to_string(),
        })
    }

    /// Résumé de l'article anglais `en_title`, en français si demandé et disponible.
    pub async fn get(&self, lang: &str, en_title: &str) -> Option<Summary> {
        if en_title.is_empty()
            || en_title.len() > 200
            || en_title
                .chars()
                .any(|c| c.is_control() || "/?#\\".contains(c))
        {
            return None;
        }
        let lang = if lang == "fr" { "fr" } else { "en" };
        let key = format!("{lang}:{en_title}");
        if let Some((at, hit)) = self.cache.lock().await.get(&key) {
            if at.elapsed() < TTL {
                return hit.clone();
            }
        }
        let mut found = None;
        if lang == "fr" {
            if let Some(fr) = self.translated(en_title, "fr").await {
                found = self.summary("fr", &fr).await;
            }
        }
        if found.is_none() {
            found = self.summary("en", en_title).await;
        }
        let mut cache = self.cache.lock().await;
        if cache.len() > 3000 {
            cache.clear();
        }
        cache.insert(key, (Instant::now(), found.clone()));
        found
    }
}
