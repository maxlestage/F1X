//! Client for the Jolpica F1 API (successor of Ergast), with an in-memory cache.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::RwLock;

const BASE_URL: &str = "https://api.jolpi.ca/ergast/f1";

#[derive(Clone)]
pub struct F1Api {
    http: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, (Instant, Value)>>>,
    ttl: Duration,
}

#[derive(Debug)]
pub struct ApiError(pub String);

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

type ApiResult<T> = Result<T, ApiError>;

impl F1Api {
    pub fn new(ttl: Duration) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("F1X/0.1 (+https://github.com/maxlestage/f1x)")
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    /// Fetches `path` (relative to the API root), serving fresh cached copies when possible
    /// and falling back to a stale copy if the upstream API is unavailable.
    async fn get_json(&self, path: &str) -> ApiResult<Value> {
        let cached = self.cache.read().await.get(path).cloned();
        if let Some((at, value)) = &cached {
            if at.elapsed() < self.ttl {
                return Ok(value.clone());
            }
        }

        let url = format!("{BASE_URL}/{path}");
        let fetched = async {
            let resp = self.http.get(&url).send().await?.error_for_status()?;
            resp.json::<Value>().await
        }
        .await;

        match fetched {
            Ok(value) => {
                self.cache
                    .write()
                    .await
                    .insert(path.to_string(), (Instant::now(), value.clone()));
                Ok(value)
            }
            Err(err) => {
                tracing::warn!(%url, %err, "F1 API request failed");
                cached
                    .map(|(_, v)| v)
                    .ok_or_else(|| ApiError("Les données F1 sont momentanément indisponibles.".into()))
            }
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, pointer: &str) -> ApiResult<T> {
        let value = self.get_json(path).await?;
        let node = value.pointer(pointer).cloned().unwrap_or(Value::Null);
        serde_json::from_value(node).map_err(|e| ApiError(format!("Réponse inattendue de l'API : {e}")))
    }

    pub async fn schedule(&self) -> ApiResult<Vec<Race>> {
        self.get("current.json", "/MRData/RaceTable/Races").await
    }

    pub async fn race_results(&self, round: u32) -> ApiResult<Option<Race>> {
        let races: Vec<Race> = self
            .get(&format!("current/{round}/results.json"), "/MRData/RaceTable/Races")
            .await?;
        Ok(races.into_iter().next())
    }

    pub async fn last_results(&self) -> ApiResult<Option<Race>> {
        let races: Vec<Race> = self
            .get("current/last/results.json", "/MRData/RaceTable/Races")
            .await?;
        Ok(races.into_iter().next())
    }

    pub async fn qualifying(&self, round: u32) -> ApiResult<Vec<QualifyingResult>> {
        let races: Vec<Race> = self
            .get(&format!("current/{round}/qualifying.json"), "/MRData/RaceTable/Races")
            .await?;
        Ok(races.into_iter().next().and_then(|r| r.qualifying_results).unwrap_or_default())
    }

    pub async fn sprint(&self, round: u32) -> ApiResult<Vec<RaceResult>> {
        let races: Vec<Race> = self
            .get(&format!("current/{round}/sprint.json"), "/MRData/RaceTable/Races")
            .await?;
        Ok(races.into_iter().next().and_then(|r| r.sprint_results).unwrap_or_default())
    }

    pub async fn driver_standings(&self) -> ApiResult<Vec<DriverStanding>> {
        let lists: Vec<StandingsList> = self
            .get("current/driverStandings.json", "/MRData/StandingsTable/StandingsLists")
            .await?;
        Ok(lists.into_iter().next().and_then(|l| l.driver_standings).unwrap_or_default())
    }

    pub async fn constructor_standings(&self) -> ApiResult<Vec<ConstructorStanding>> {
        let lists: Vec<StandingsList> = self
            .get(
                "current/constructorStandings.json",
                "/MRData/StandingsTable/StandingsLists",
            )
            .await?;
        Ok(lists
            .into_iter()
            .next()
            .and_then(|l| l.constructor_standings)
            .unwrap_or_default())
    }

    pub async fn driver_results(&self, driver_id: &str) -> ApiResult<Vec<Race>> {
        self.get(
            &format!("current/drivers/{driver_id}/results.json?limit=100"),
            "/MRData/RaceTable/Races",
        )
        .await
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Race {
    pub season: String,
    pub round: String,
    #[serde(rename = "raceName")]
    pub race_name: String,
    #[serde(rename = "Circuit")]
    pub circuit: Circuit,
    pub date: String,
    pub time: Option<String>,
    #[serde(rename = "FirstPractice")]
    pub first_practice: Option<Session>,
    #[serde(rename = "SecondPractice")]
    pub second_practice: Option<Session>,
    #[serde(rename = "ThirdPractice")]
    pub third_practice: Option<Session>,
    #[serde(rename = "SprintQualifying", alias = "SprintShootout")]
    pub sprint_qualifying: Option<Session>,
    #[serde(rename = "Sprint")]
    pub sprint: Option<Session>,
    #[serde(rename = "Qualifying")]
    pub qualifying: Option<Session>,
    #[serde(rename = "Results")]
    pub results: Option<Vec<RaceResult>>,
    #[serde(rename = "SprintResults")]
    pub sprint_results: Option<Vec<RaceResult>>,
    #[serde(rename = "QualifyingResults")]
    pub qualifying_results: Option<Vec<QualifyingResult>>,
}

impl Race {
    pub fn round_num(&self) -> u32 {
        self.round.parse().unwrap_or(0)
    }

    /// Race start as an ISO-8601 UTC timestamp (defaults to 00:00Z when the time is unknown).
    pub fn start_iso(&self) -> String {
        iso(&self.date, self.time.as_deref())
    }

    pub fn is_sprint_weekend(&self) -> bool {
        self.sprint.is_some()
    }

    /// Weekend sessions in chronological order, including the race itself.
    pub fn sessions(&self) -> Vec<(&'static str, String)> {
        let mut out: Vec<(&'static str, String)> = [
            ("Essais libres 1", &self.first_practice),
            ("Essais libres 2", &self.second_practice),
            ("Essais libres 3", &self.third_practice),
            ("Qualifs sprint", &self.sprint_qualifying),
            ("Sprint", &self.sprint),
            ("Qualifications", &self.qualifying),
        ]
        .into_iter()
        .filter_map(|(name, s)| s.as_ref().map(|s| (name, s.iso())))
        .collect();
        out.push(("Course", self.start_iso()));
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub date: String,
    pub time: Option<String>,
}

impl Session {
    pub fn iso(&self) -> String {
        iso(&self.date, self.time.as_deref())
    }
}

fn iso(date: &str, time: Option<&str>) -> String {
    format!("{date}T{}", time.unwrap_or("00:00:00Z"))
}

#[derive(Debug, Clone, Deserialize)]
pub struct Circuit {
    #[serde(rename = "circuitName")]
    pub circuit_name: String,
    #[serde(rename = "Location")]
    pub location: Location,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Location {
    pub locality: String,
    pub country: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Driver {
    #[serde(rename = "driverId")]
    pub driver_id: String,
    #[serde(rename = "permanentNumber")]
    pub permanent_number: Option<String>,
    #[serde(rename = "givenName")]
    pub given_name: String,
    #[serde(rename = "familyName")]
    pub family_name: String,
    pub nationality: Option<String>,
}

impl Driver {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.given_name, self.family_name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Constructor {
    #[serde(rename = "constructorId")]
    pub constructor_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeValue {
    pub time: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FastestLap {
    pub rank: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RaceResult {
    pub position: String,
    #[serde(rename = "positionText")]
    pub position_text: String,
    pub points: String,
    #[serde(rename = "Driver")]
    pub driver: Driver,
    #[serde(rename = "Constructor")]
    pub constructor: Constructor,
    pub grid: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "Time")]
    pub time: Option<TimeValue>,
    #[serde(rename = "FastestLap")]
    pub fastest_lap: Option<FastestLap>,
}

impl RaceResult {
    pub fn has_fastest_lap(&self) -> bool {
        self.fastest_lap
            .as_ref()
            .and_then(|f| f.rank.as_deref())
            == Some("1")
    }

    /// Finishing time/gap, or the retirement status.
    pub fn outcome(&self) -> String {
        if let Some(t) = &self.time {
            return t.time.clone();
        }
        let status = self.status.as_deref().unwrap_or_default();
        match status {
            "Finished" => String::new(),
            "Retired" | "Accident" | "Collision" => "Abandon".into(),
            "Did not start" => "Non partant".into(),
            "Disqualified" => "Disqualifié".into(),
            "Lapped" => "Doublé".into(),
            s if s.starts_with('+') && s.contains("Lap") => {
                let n = s.trim_start_matches('+').split_whitespace().next().unwrap_or("1");
                format!("+{n} tour{}", if n == "1" { "" } else { "s" })
            }
            s => s.to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct QualifyingResult {
    pub position: String,
    #[serde(rename = "Driver")]
    pub driver: Driver,
    #[serde(rename = "Constructor")]
    pub constructor: Constructor,
    #[serde(rename = "Q1")]
    pub q1: Option<String>,
    #[serde(rename = "Q2")]
    pub q2: Option<String>,
    #[serde(rename = "Q3")]
    pub q3: Option<String>,
}

impl QualifyingResult {
    /// Best segment reached, e.g. ("Q3", "1:29.708").
    pub fn best(&self) -> Option<(&'static str, &str)> {
        [("Q3", &self.q3), ("Q2", &self.q2), ("Q1", &self.q1)]
            .into_iter()
            .find_map(|(label, t)| t.as_deref().filter(|t| !t.is_empty()).map(|t| (label, t)))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StandingsList {
    #[serde(rename = "DriverStandings")]
    pub driver_standings: Option<Vec<DriverStanding>>,
    #[serde(rename = "ConstructorStandings")]
    pub constructor_standings: Option<Vec<ConstructorStanding>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriverStanding {
    pub position: Option<String>,
    #[serde(rename = "positionText")]
    pub position_text: String,
    pub points: String,
    pub wins: String,
    #[serde(rename = "Driver")]
    pub driver: Driver,
    #[serde(rename = "Constructors", default)]
    pub constructors: Vec<Constructor>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConstructorStanding {
    pub position: Option<String>,
    #[serde(rename = "positionText")]
    pub position_text: String,
    pub points: String,
    pub wins: String,
    #[serde(rename = "Constructor")]
    pub constructor: Constructor,
}
