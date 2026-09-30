//! Client OpenF1 (https://openf1.org) : données détaillées des sessions depuis 2023.
//!
//! - Historique : gratuit, sans compte (30 req/min), disponible ~30 min après la session.
//! - Direct : réservé aux abonnés OpenF1 → `OPENF1_USERNAME` / `OPENF1_PASSWORD`.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use f1x_protocol::{RaceControl, SessionSummary, Weather};
use serde_json::Value;
use tokio::sync::{Mutex, OnceCell, RwLock};

const BASE: &str = "https://api.openf1.org/v1";
const MIN_SPACING: Duration = Duration::from_millis(350);
const MAX_DATASETS: usize = 6;

pub type Ms = i64;

pub fn parse_date(s: &str) -> Option<Ms> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .or_else(|| DateTime::parse_from_rfc3339(&format!("{s}+00:00")).ok())
        .map(|d| d.with_timezone(&Utc).timestamp_millis())
}

pub fn iso(ms: Ms) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
pub struct DriverInfo {
    pub number: u32,
    pub code: String,
    pub name: String,
    pub team: String,
    pub colour: String,
}

#[derive(Debug, Clone)]
pub struct LapRec {
    pub driver: u32,
    pub lap: u32,
    pub start: Ms,
    pub duration: Option<f64>,
}

impl LapRec {
    pub fn end(&self) -> Option<Ms> {
        self.duration.map(|d| self.start + (d * 1000.0) as Ms)
    }
}

#[derive(Debug, Clone)]
pub struct Stint {
    pub driver: u32,
    pub lap_start: u32,
    pub lap_end: Option<u32>,
    pub compound: String,
    pub age_at_start: u32,
}

#[derive(Debug, Clone)]
pub struct Pit {
    pub time: Ms,
    pub driver: u32,
    pub lane: Option<f64>,
}

/// Toutes les données horodatées d'une session, triées par date.
#[derive(Debug, Clone, Default)]
pub struct Dataset {
    pub session: Option<SessionSummary>,
    pub drivers: Vec<DriverInfo>,
    pub positions: Vec<(Ms, u32, u32)>,
    /// (date, pilote, écart au leader, intervalle) — valeurs brutes (nombre ou texte).
    pub intervals: Vec<(Ms, u32, Value, Value)>,
    pub laps: Vec<LapRec>,
    pub stints: Vec<Stint>,
    pub pits: Vec<Pit>,
    pub race_control: Vec<(Ms, RaceControl)>,
    pub weather: Vec<(Ms, Weather)>,
}

fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or_default()
}
fn u(v: &Value, k: &str) -> Option<u32> {
    v.get(k).and_then(Value::as_u64).map(|n| n as u32)
}
fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}
fn s(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
fn t(v: &Value, k: &str) -> Option<Ms> {
    v.get(k).and_then(Value::as_str).and_then(parse_date)
}

pub fn session_from(v: &Value) -> Option<SessionSummary> {
    Some(SessionSummary {
        session_key: u(v, "session_key")?,
        session_name: s(v, "session_name"),
        session_type: s(v, "session_type"),
        location: s(v, "location"),
        country: s(v, "country_name"),
        circuit: s(v, "circuit_short_name"),
        date_start: s(v, "date_start"),
        date_end: s(v, "date_end"),
        year: u(v, "year").unwrap_or(0),
    })
}

impl Dataset {
    pub fn add_drivers(&mut self, v: &Value) {
        for d in arr(v) {
            let Some(number) = u(d, "driver_number") else {
                continue;
            };
            if self.drivers.iter().any(|x| x.number == number) {
                continue;
            }
            let colour = s(d, "team_colour");
            self.drivers.push(DriverInfo {
                number,
                code: s(d, "name_acronym"),
                name: s(d, "full_name"),
                team: s(d, "team_name"),
                colour: if colour.is_empty() {
                    "8A8A99".into()
                } else {
                    colour
                },
            });
        }
    }

    pub fn add_positions(&mut self, v: &Value) {
        self.positions.extend(
            arr(v)
                .iter()
                .filter_map(|p| Some((t(p, "date")?, u(p, "driver_number")?, u(p, "position")?))),
        );
        self.positions.sort_by_key(|p| p.0);
    }

    pub fn add_intervals(&mut self, v: &Value) {
        self.intervals.extend(arr(v).iter().filter_map(|p| {
            Some((
                t(p, "date")?,
                u(p, "driver_number")?,
                p.get("gap_to_leader").cloned().unwrap_or(Value::Null),
                p.get("interval").cloned().unwrap_or(Value::Null),
            ))
        }));
        self.intervals.sort_by_key(|p| p.0);
    }

    pub fn add_laps(&mut self, v: &Value) {
        for l in arr(v) {
            let (Some(driver), Some(lap), Some(start)) = (
                u(l, "driver_number"),
                u(l, "lap_number"),
                t(l, "date_start"),
            ) else {
                continue;
            };
            let rec = LapRec {
                driver,
                lap,
                start,
                duration: f(l, "lap_duration"),
            };
            // En direct, un tour déjà connu peut revenir complété : on le remplace.
            match self
                .laps
                .iter_mut()
                .find(|x| x.driver == driver && x.lap == lap)
            {
                Some(existing) => *existing = rec,
                None => self.laps.push(rec),
            }
        }
        self.laps.sort_by_key(|l| l.start);
    }

    pub fn set_stints(&mut self, v: &Value) {
        self.stints = arr(v)
            .iter()
            .filter_map(|st| {
                Some(Stint {
                    driver: u(st, "driver_number")?,
                    lap_start: u(st, "lap_start")?,
                    lap_end: u(st, "lap_end"),
                    compound: s(st, "compound"),
                    age_at_start: u(st, "tyre_age_at_start").unwrap_or(0),
                })
            })
            .collect();
    }

    pub fn add_pits(&mut self, v: &Value) {
        self.pits.extend(arr(v).iter().filter_map(|p| {
            Some(Pit {
                time: t(p, "date")?,
                driver: u(p, "driver_number")?,
                lane: f(p, "lane_duration").or(f(p, "pit_duration")),
            })
        }));
        self.pits.sort_by_key(|p| p.time);
        self.pits
            .dedup_by(|a, b| a.time == b.time && a.driver == b.driver);
    }

    pub fn add_race_control(&mut self, v: &Value) {
        self.race_control.extend(arr(v).iter().filter_map(|m| {
            let date = t(m, "date")?;
            let flag = m.get("flag").and_then(Value::as_str).map(str::to_string);
            Some((
                date,
                RaceControl {
                    date: iso(date),
                    lap: u(m, "lap_number"),
                    category: s(m, "category"),
                    flag,
                    message: s(m, "message"),
                },
            ))
        }));
        self.race_control.sort_by_key(|m| m.0);
        self.race_control
            .dedup_by(|a, b| a.0 == b.0 && a.1.message == b.1.message);
    }

    pub fn add_weather(&mut self, v: &Value) {
        self.weather.extend(arr(v).iter().filter_map(|w| {
            Some((
                t(w, "date")?,
                Weather {
                    air_temperature: f(w, "air_temperature")?,
                    track_temperature: f(w, "track_temperature")?,
                    humidity: f(w, "humidity").unwrap_or(0.0),
                    wind_speed: f(w, "wind_speed").unwrap_or(0.0),
                    rainfall: f(w, "rainfall").unwrap_or(0.0) > 0.0,
                },
            ))
        }));
        self.weather.sort_by_key(|w| w.0);
    }

    /// Dernier horodatage connu (pour les requêtes incrémentales du direct).
    pub fn latest(&self) -> Option<Ms> {
        [
            self.positions.last().map(|p| p.0),
            self.intervals.last().map(|p| p.0),
            self.race_control.last().map(|p| p.0),
            self.weather.last().map(|p| p.0),
        ]
        .into_iter()
        .flatten()
        .max()
    }
}

/// Données d'une session, chargées une seule fois même en cas de demandes simultanées.
type DatasetSlot = Arc<OnceCell<Arc<Dataset>>>;

pub struct OpenF1 {
    http: reqwest::Client,
    calls: Mutex<VecDeque<Instant>>,
    per_minute: usize,
    credentials: Option<(String, String)>,
    token: Mutex<Option<(String, Instant)>>,
    sessions: RwLock<HashMap<u32, (Instant, Vec<SessionSummary>)>>,
    datasets: Mutex<VecDeque<(u32, DatasetSlot)>>,
}

impl OpenF1 {
    pub fn new(credentials: Option<(String, String)>) -> Self {
        let per_minute = if credentials.is_some() { 60 } else { 30 };
        Self {
            http: reqwest::Client::builder()
                .user_agent("F1X/0.4 (+https://github.com/maxlestage/f1x)")
                .timeout(Duration::from_secs(60))
                .build()
                .expect("failed to build HTTP client"),
            calls: Mutex::new(VecDeque::new()),
            per_minute,
            credentials,
            token: Mutex::new(None),
            sessions: RwLock::new(HashMap::new()),
            datasets: Mutex::new(VecDeque::new()),
        }
    }

    pub fn has_live_access(&self) -> bool {
        self.credentials.is_some()
    }

    /// Respecte les limites OpenF1 : ~3 req/s et `per_minute` req/min.
    async fn throttle(&self) {
        loop {
            let mut calls = self.calls.lock().await;
            while calls
                .front()
                .is_some_and(|t| t.elapsed() > Duration::from_secs(60))
            {
                calls.pop_front();
            }
            let spacing = calls
                .back()
                .map(|t| MIN_SPACING.saturating_sub(t.elapsed()))
                .unwrap_or_default();
            let window = if calls.len() >= self.per_minute {
                calls
                    .front()
                    .map(|t| Duration::from_secs(60).saturating_sub(t.elapsed()))
                    .unwrap_or_default()
            } else {
                Duration::ZERO
            };
            let wait = spacing.max(window);
            if wait.is_zero() {
                calls.push_back(Instant::now());
                return;
            }
            drop(calls);
            tokio::time::sleep(wait).await;
        }
    }

    async fn bearer(&self) -> Option<String> {
        let (user, pass) = self.credentials.as_ref()?;
        let mut token = self.token.lock().await;
        if let Some((value, expiry)) = token.as_ref() {
            if Instant::now() < *expiry {
                return Some(value.clone());
            }
        }
        let resp = self
            .http
            .post("https://api.openf1.org/token")
            .form(&[("username", user.as_str()), ("password", pass.as_str())])
            .send()
            .await
            .ok()?;
        let body: Value = resp.json().await.ok()?;
        let value = body.get("access_token")?.as_str()?.to_string();
        let ttl = body
            .get("expires_in")
            .and_then(|e| e.as_str().and_then(|s| s.parse().ok()).or(e.as_u64()))
            .unwrap_or(3600);
        *token = Some((
            value.clone(),
            Instant::now() + Duration::from_secs(ttl.saturating_sub(120)),
        ));
        Some(value)
    }

    pub async fn get(&self, path: &str) -> Result<Value, String> {
        for attempt in 0..2 {
            self.throttle().await;
            let mut req = self.http.get(format!("{BASE}/{path}"));
            if let Some(token) = self.bearer().await {
                req = req.bearer_auth(token);
            }
            let resp = req.send().await.map_err(|e| e.to_string())?;
            let status = resp.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt == 0 {
                tracing::warn!(%path, "OpenF1 rate limit, retrying");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok(Value::Array(vec![]));
            }
            if !status.is_success() {
                return Err(format!("OpenF1 : HTTP {status}"));
            }
            return resp.json().await.map_err(|e| e.to_string());
        }
        Err("OpenF1 : limite de requêtes atteinte".into())
    }

    /// Sessions rejouables d'une année (terminées depuis plus d'une heure, non annulées).
    pub async fn sessions(&self, year: u32) -> Result<Vec<SessionSummary>, String> {
        if let Some((at, list)) = self.sessions.read().await.get(&year) {
            if at.elapsed() < Duration::from_secs(1800) {
                return Ok(list.clone());
            }
        }
        let v = self.get(&format!("sessions?year={year}")).await?;
        let cutoff = Utc::now().timestamp_millis() - 3_600_000;
        let list: Vec<SessionSummary> = arr(&v)
            .iter()
            .filter(|x| {
                !x.get("is_cancelled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .filter_map(session_from)
            .filter(|x| parse_date(&x.date_end).is_some_and(|end| end < cutoff))
            .collect();
        self.sessions
            .write()
            .await
            .insert(year, (Instant::now(), list.clone()));
        Ok(list)
    }

    /// Toutes les données d'une session (mises en cache, chargées une seule fois même
    /// si plusieurs personnes lancent le même replay en même temps).
    pub async fn dataset(&self, key: u32) -> Result<Arc<Dataset>, String> {
        let cell = {
            let mut cache = self.datasets.lock().await;
            match cache.iter().find(|(k, _)| *k == key) {
                Some((_, cell)) => cell.clone(),
                None => {
                    let cell = Arc::new(OnceCell::new());
                    cache.push_back((key, cell.clone()));
                    if cache.len() > MAX_DATASETS {
                        cache.pop_front();
                    }
                    cell
                }
            }
        };
        let result = cell.get_or_try_init(|| self.load(key)).await.cloned();
        if result.is_err() {
            // Ne pas garder un échec en cache.
            self.datasets.lock().await.retain(|(k, _)| *k != key);
        }
        result
    }

    pub async fn load(&self, key: u32) -> Result<Arc<Dataset>, String> {
        let q = format!("session_key={key}");
        let session = self.get(&format!("sessions?{q}")).await?;
        let session = arr(&session)
            .first()
            .and_then(session_from)
            .ok_or("Session introuvable")?;
        let mut d = Dataset {
            session: Some(session),
            ..Default::default()
        };
        d.add_drivers(&self.get(&format!("drivers?{q}")).await?);
        d.add_positions(&self.get(&format!("position?{q}")).await?);
        d.add_intervals(&self.get(&format!("intervals?{q}")).await?);
        d.add_laps(&self.get(&format!("laps?{q}")).await?);
        d.set_stints(&self.get(&format!("stints?{q}")).await?);
        d.add_pits(&self.get(&format!("pit?{q}")).await?);
        d.add_race_control(&self.get(&format!("race_control?{q}")).await?);
        d.add_weather(&self.get(&format!("weather?{q}")).await?);
        if d.drivers.is_empty() {
            return Err("Pas encore de données pour cette session.".into());
        }
        tracing::info!(
            key,
            intervals = d.intervals.len(),
            laps = d.laps.len(),
            "OpenF1 session loaded"
        );
        Ok(Arc::new(d))
    }
}
