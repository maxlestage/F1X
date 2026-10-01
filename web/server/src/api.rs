//! Proxy vers l'API Jolpica F1 (successeur d'Ergast), avec cache mémoire,
//! limitation de débit (4 req/s max côté Jolpica) et pagination automatique.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tokio::sync::{Mutex, RwLock};

const BASE_URL: &str = "https://api.jolpi.ca/ergast/f1";
/// Taille de page maximale acceptée par Jolpica.
pub const PAGE: u32 = 100;
/// Garde-fou : nombre maximal de pages agrégées pour un endpoint `all`.
const MAX_PAGES: u32 = 30;
/// Intervalle minimal entre deux requêtes vers Jolpica (burst limité à 4 req/s).
const MIN_SPACING: Duration = Duration::from_millis(260);
/// Données des saisons passées : elles ne changent plus.
const HISTORICAL_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
/// Données de carrière / globales (changent au plus une fois par Grand Prix) : 3 h.
const GLOBAL_TTL: Duration = Duration::from_secs(3 * 3600);
/// Pause par défaut après une réponse 429 de Jolpica.
const DEFAULT_BACKOFF: Duration = Duration::from_secs(60);
const MAX_ENTRIES: usize = 3000;

#[derive(Clone)]
pub struct F1Api {
    http: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, (Instant, Value)>>>,
    last_call: Arc<Mutex<Instant>>,
    champions: Arc<Mutex<Option<(Instant, Value)>>>,
    /// Coupe-circuit : pas d'appel à Jolpica avant cet instant (après un 429).
    blocked_until: Arc<Mutex<Option<Instant>>>,
    ttl: Duration,
}

impl F1Api {
    pub fn new(ttl: Duration) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(crate::user_agent())
            .timeout(Duration::from_secs(15))
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            cache: Arc::new(RwLock::new(HashMap::new())),
            last_call: Arc::new(Mutex::new(Instant::now() - MIN_SPACING)),
            champions: Arc::new(Mutex::new(None)),
            blocked_until: Arc::new(Mutex::new(None)),
            ttl,
        }
    }

    /// Les chemins commençant par une année passée (`1998/...`) sont figés.
    pub fn is_historical(path: &str) -> bool {
        path.split('/')
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .is_some_and(|year| year < current_year())
    }

    /// Saisons passées : 7 jours. Saison en cours : `ttl` (5 min par défaut).
    /// Le reste (carrières, listes globales) : 1 h.
    fn ttl_for(&self, path: &str) -> Duration {
        let first = path.split('/').next().unwrap_or_default();
        if Self::is_historical(path) {
            HISTORICAL_TTL
        } else if first == "current" || first.parse::<u32>().is_ok() {
            self.ttl
        } else {
            GLOBAL_TTL.max(self.ttl)
        }
    }

    /// Une page de `path` (ex. `2026/15/results.json`). Sert le cache s'il est frais,
    /// et une copie périmée si l'API est indisponible ou nous limite.
    pub async fn page(&self, path: &str, limit: u32, offset: u32) -> Option<Value> {
        let key = format!("{path}?limit={limit}&offset={offset}");
        let cached = self.cache.read().await.get(&key).cloned();
        if let Some((at, value)) = &cached {
            if at.elapsed() < self.ttl_for(path) {
                return Some(value.clone());
            }
        }
        match self.fetch(&key).await {
            Ok(value) => {
                let mut cache = self.cache.write().await;
                if cache.len() >= MAX_ENTRIES {
                    let historical = HISTORICAL_TTL;
                    cache.retain(|_, (at, _)| at.elapsed() < historical / 7);
                    if cache.len() >= MAX_ENTRIES {
                        cache.clear();
                    }
                }
                cache.insert(key, (Instant::now(), value.clone()));
                Some(value)
            }
            Err(err) => {
                tracing::warn!(%key, %err, "F1 API request failed");
                cached.map(|(_, v)| v)
            }
        }
    }

    /// Toutes les pages de `path`, fusionnées en une seule réponse.
    pub async fn all(&self, path: &str) -> Option<Value> {
        let mut merged = self.page(path, PAGE, 0).await?;
        let total = total_of(&merged).min(PAGE * MAX_PAGES);
        let mut offset = PAGE;
        while offset < total {
            let page = self.page(path, PAGE, offset).await?;
            append_tables(&mut merged, &page);
            offset += PAGE;
        }
        if let Some(mr) = merged.get_mut("MRData") {
            mr["limit"] = Value::String(total.to_string());
        }
        Some(merged)
    }

    async fn fetch(&self, path_and_query: &str) -> Result<Value, String> {
        if let Some(until) = *self.blocked_until.lock().await {
            if Instant::now() < until {
                return Err("limite de débit Jolpica atteinte, nouvel essai plus tard".into());
            }
        }
        {
            // Espace les appels pour respecter la limite de Jolpica.
            let mut last = self.last_call.lock().await;
            let wait = MIN_SPACING.saturating_sub(last.elapsed());
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
            *last = Instant::now();
        }
        let resp = self
            .http
            .get(format!("{BASE_URL}/{path_and_query}"))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let wait = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(Duration::from_secs)
                .unwrap_or(DEFAULT_BACKOFF);
            tracing::warn!(?wait, "Jolpica rate limit hit, pausing upstream calls");
            *self.blocked_until.lock().await = Some(Instant::now() + wait);
        }
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }
        let value: Value = resp.json().await.map_err(|e| e.to_string())?;
        if value.get("MRData").is_none() {
            return Err("réponse sans MRData".into());
        }
        Ok(value)
    }
}

impl F1Api {
    /// Sauvegarde le cache dans `file` (JSON : chemin → [âge en s, valeur]).
    pub async fn save(&self, file: &str) {
        let cache = self.cache.read().await;
        let dump: serde_json::Map<String, Value> = cache
            .iter()
            .map(|(k, (at, v))| (k.clone(), serde_json::json!([at.elapsed().as_secs(), v])))
            .collect();
        match std::fs::write(file, Value::Object(dump).to_string()) {
            Ok(()) => tracing::info!(entries = cache.len(), %file, "cache saved"),
            Err(err) => tracing::warn!(%err, %file, "cache not saved"),
        }
    }

    /// Recharge un cache sauvegardé par [`F1Api::save`].
    pub async fn load(&self, file: &str) {
        let Ok(text) = std::fs::read_to_string(file) else {
            return;
        };
        let Ok(Value::Object(dump)) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        let mut cache = self.cache.write().await;
        for (key, entry) in dump {
            let (Some(age), Some(value)) = (entry.get(0).and_then(Value::as_u64), entry.get(1))
            else {
                continue;
            };
            if let Some(at) = Instant::now().checked_sub(Duration::from_secs(age)) {
                cache.insert(key, (at, value.clone()));
            }
        }
        tracing::info!(entries = cache.len(), %file, "cache loaded");
    }
}

fn total_of(value: &Value) -> u32 {
    value
        .pointer("/MRData/total")
        .and_then(Value::as_str)
        .and_then(|t| t.parse().ok())
        .unwrap_or(0)
}

/// Concatène les tableaux des `*Table` de `src` dans ceux de `dst`.
fn append_tables(dst: &mut Value, src: &Value) {
    let Some(src_mr) = src.get("MRData").and_then(Value::as_object) else {
        return;
    };
    for (table_key, table) in src_mr.iter().filter(|(k, _)| k.ends_with("Table")) {
        let Some(table) = table.as_object() else {
            continue;
        };
        for (list_key, list) in table {
            let (Some(items), Some(target)) = (
                list.as_array(),
                dst.pointer_mut(&format!("/MRData/{table_key}/{list_key}"))
                    .and_then(Value::as_array_mut),
            ) else {
                continue;
            };
            target.extend(items.iter().cloned());
        }
    }
}

fn current_year() -> u32 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Année civile approximative (précise à quelques heures près autour du 1er janvier).
    1970 + (secs / 31_556_952) as u32
}

/// N'accepte que des chemins d'API plausibles : `a-z0-9_-` séparés par `/`, finissant par `.json`.
pub fn valid_path(path: &str) -> bool {
    path.ends_with(".json")
        && path.len() < 200
        && !path.contains("..")
        && !path.starts_with('/')
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '/' | '.'))
}

// ---------- Champions (agrégés saison par saison) ----------

impl F1Api {
    /// Champions pilotes et constructeurs de chaque saison depuis 1950.
    /// Les saisons terminées viennent de `static/champions.json` (généré une fois depuis l'API :
    /// le passé ne change pas) ; seules les saisons suivantes sont demandées à Jolpica.
    pub async fn champions(&self) -> Value {
        let mut guard = self.champions.lock().await;
        if let Some((at, value)) = guard.as_ref() {
            if at.elapsed() < Duration::from_secs(3600) {
                return value.clone();
            }
        }
        let mut out: Vec<Value> =
            serde_json::from_str(include_str!("../static/champions.json")).unwrap_or_default();
        let last_static = out
            .last()
            .and_then(|c| c.get("season")?.as_str()?.parse::<u32>().ok())
            .unwrap_or(1949);
        for year in last_static + 1..=current_year() {
            let first = |v: Option<Value>, list: &str| -> Option<Value> {
                v?.pointer(&format!("/MRData/StandingsTable/StandingsLists/0/{list}/0"))
                    .cloned()
            };
            let driver = first(
                self.page(&format!("{year}/driverStandings.json"), 1, 0)
                    .await,
                "DriverStandings",
            );
            let Some(driver) = driver else { continue };
            let constructor = first(
                self.page(&format!("{year}/constructorStandings.json"), 1, 0)
                    .await,
                "ConstructorStandings",
            );
            // Saison en cours tant que le calendrier n'est pas terminé.
            let schedule = self.page(&format!("{year}.json"), 100, 0).await;
            let total = schedule
                .as_ref()
                .and_then(|v| v.pointer("/MRData/total")?.as_str()?.parse::<u32>().ok())
                .unwrap_or(0);
            let round = driver_round(
                &self
                    .page(&format!("{year}/driverStandings.json"), 1, 0)
                    .await,
            );
            out.push(serde_json::json!({
                "season": year.to_string(),
                "in_progress": round < total,
                "driver": driver.get("Driver"),
                "driver_team": driver.pointer("/Constructors/0"),
                "driver_points": driver.get("points"),
                "driver_wins": driver.get("wins"),
                "constructor": constructor.as_ref().and_then(|c| c.get("Constructor")),
                "constructor_points": constructor.as_ref().and_then(|c| c.get("points")),
            }));
        }
        let value = Value::Array(out);
        *guard = Some((Instant::now(), value.clone()));
        value
    }
}

/// Manche après laquelle un classement a été établi.
fn driver_round(v: &Option<Value>) -> u32 {
    v.as_ref()
        .and_then(|v| {
            v.pointer("/MRData/StandingsTable/StandingsLists/0/round")?
                .as_str()?
                .parse()
                .ok()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_paths() {
        assert!(valid_path("2026/15/results.json"));
        assert!(valid_path("drivers/max_verstappen/results/1.json"));
        assert!(!valid_path("../secret.json"));
        assert!(!valid_path("/etc/passwd.json"));
        assert!(!valid_path("2026/results"));
        assert!(!valid_path("a?b=c.json"));
    }

    #[test]
    fn picks_ttl() {
        let api = F1Api::new(Duration::from_secs(300));
        assert_eq!(api.ttl_for("1998/1/results.json"), HISTORICAL_TTL);
        assert_eq!(
            api.ttl_for("current/driverStandings.json"),
            Duration::from_secs(300)
        );
        assert_eq!(api.ttl_for("drivers/hamilton/results.json"), GLOBAL_TTL);
    }

    #[test]
    fn detects_historical() {
        assert!(F1Api::is_historical("1998/driverStandings.json"));
        assert!(!F1Api::is_historical("current/driverStandings.json"));
        assert!(!F1Api::is_historical("drivers/hamilton.json"));
    }

    #[test]
    fn merges_pages() {
        let mut a = json!({"MRData": {"total": "3", "DriverTable": {"Drivers": [1, 2]}}});
        let b = json!({"MRData": {"total": "3", "DriverTable": {"Drivers": [3]}}});
        append_tables(&mut a, &b);
        assert_eq!(
            a.pointer("/MRData/DriverTable/Drivers").unwrap(),
            &json!([1, 2, 3])
        );
    }
}
