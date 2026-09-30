//! Proxy vers l'API Jolpica F1 (successeur d'Ergast), avec cache mémoire.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::RwLock;

const BASE_URL: &str = "https://api.jolpi.ca/ergast/f1";

#[derive(Clone)]
pub struct F1Api {
    http: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, (Instant, Value)>>>,
    ttl: Duration,
}

impl F1Api {
    pub fn new(ttl: Duration) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("F1X/0.2 (+https://github.com/maxlestage/f1x)")
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    /// Récupère `path` (relatif à la racine de l'API) et renvoie le nœud `pointer`.
    /// Sert le cache s'il est frais, et une copie périmée si l'API est indisponible.
    pub async fn get(&self, path: &str, pointer: &str) -> Option<Value> {
        let cached = self.cache.read().await.get(path).cloned();
        let value = match &cached {
            Some((at, value)) if at.elapsed() < self.ttl => value.clone(),
            _ => match self.fetch(path).await {
                Ok(value) => {
                    self.cache
                        .write()
                        .await
                        .insert(path.to_string(), (Instant::now(), value.clone()));
                    value
                }
                Err(err) => {
                    tracing::warn!(%path, %err, "F1 API request failed");
                    cached?.1
                }
            },
        };
        Some(value.pointer(pointer).cloned().unwrap_or(Value::Null))
    }

    async fn fetch(&self, path: &str) -> reqwest::Result<Value> {
        self.http
            .get(format!("{BASE_URL}/{path}"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}
