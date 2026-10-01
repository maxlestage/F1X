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
    pub sectors: [Option<f64>; 3],
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
    /// Tracé du circuit (meilleur tour de la session), pour la carte en direct.
    pub track: Option<Arc<TrackMap>>,
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
                sectors: [
                    f(l, "duration_sector_1"),
                    f(l, "duration_sector_2"),
                    f(l, "duration_sector_3"),
                ],
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
    /// Tracés de circuits (ne changent pas : gardés pour toute la vie du serveur).
    tracks: RwLock<HashMap<String, Arc<TrackMap>>>,
}

impl OpenF1 {
    pub fn new(credentials: Option<(String, String)>) -> Self {
        let per_minute = if credentials.is_some() { 60 } else { 30 };
        Self {
            http: reqwest::Client::builder()
                .user_agent(crate::user_agent())
                .timeout(Duration::from_secs(60))
                .build()
                .expect("failed to build HTTP client"),
            calls: Mutex::new(VecDeque::new()),
            per_minute,
            credentials,
            token: Mutex::new(None),
            sessions: RwLock::new(HashMap::new()),
            datasets: Mutex::new(VecDeque::new()),
            tracks: RwLock::new(HashMap::new()),
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
        // Tracé pour la carte : meilleur tour de la session (non bloquant si indisponible).
        let mut best: Vec<(f64, u32, u32, Ms)> = d
            .laps
            .iter()
            .filter(|l| l.lap > 1)
            .filter_map(|l| Some((l.duration?, l.driver, l.lap, l.start)))
            .collect();
        best.sort_by(|a, b| a.0.total_cmp(&b.0));
        best.dedup_by_key(|l| l.1);
        if let Some(session) = d.session.clone() {
            for lap in best.into_iter().take(2) {
                match self
                    .build_track(&session.circuit, session.year, key, &session.location, lap)
                    .await
                {
                    Ok(track) => {
                        d.track = Some(track);
                        break;
                    }
                    Err(err) => tracing::warn!(key, %err, "replay track unavailable"),
                }
            }
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

// ---------- Tracés de circuits ----------

use f1x_protocol::{TrackMap, TrackPoint, TrackStats};

fn query_date(ms: Ms) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|d| d.format("%Y-%m-%dT%H:%M:%S%.3f").to_string())
        .unwrap_or_default()
}

impl OpenF1 {
    pub async fn cached_track(&self, circuit_id: &str) -> Option<Arc<TrackMap>> {
        self.tracks.read().await.get(circuit_id).cloned()
    }

    /// Tracé du circuit : essaie les courses candidates (les plus récentes d'abord) et, pour
    /// chacune, les meilleurs tours jusqu'à trouver des positions GPS complètes.
    pub async fn track(
        &self,
        circuit_id: &str,
        candidates: &[(u32, String, String)],
    ) -> Result<Arc<TrackMap>, String> {
        if let Some(t) = self.cached_track(circuit_id).await {
            return Ok(t);
        }
        let mut attempts = 0;
        let mut last_err = String::from("Pas de données OpenF1 pour ce circuit");
        for (year, date, event) in candidates.iter().take(3) {
            let Some(day) = parse_date(&format!("{date}T12:00:00Z")) else {
                continue;
            };
            // Sessions terminées du week-end (jusqu'à 3,5 jours avant la course) : course d'abord,
            // puis qualifications, sprint et essais (utile pour un GP pas encore couru).
            let rank = |name: &str| match name {
                "Race" => 0,
                "Qualifying" => 1,
                "Sprint" => 2,
                "Sprint Qualifying" | "Sprint Shootout" => 3,
                _ => 4,
            };
            let mut weekend: Vec<SessionSummary> = self
                .sessions(*year)
                .await?
                .into_iter()
                .filter(|s| {
                    parse_date(&s.date_start).is_some_and(|start| {
                        start - day < 36 * 3_600_000 && day - start < 84 * 3_600_000
                    })
                })
                .collect();
            weekend.sort_by_key(|s| rank(&s.session_name));
            let Some(session) = weekend.into_iter().next() else {
                continue;
            };
            let key = session.session_key;
            let laps = self.get(&format!("laps?session_key={key}")).await?;
            // Meilleurs tours de la course (hors tour 1 et sorties des stands).
            let mut best: Vec<(f64, u32, u32, Ms)> = arr(&laps)
                .iter()
                .filter(|l| u(l, "lap_number").unwrap_or(0) > 1)
                .filter(|l| {
                    !l.get("is_pit_out_lap")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .filter_map(|l| {
                    Some((
                        f(l, "lap_duration")?,
                        u(l, "driver_number")?,
                        u(l, "lap_number")?,
                        t(l, "date_start")?,
                    ))
                })
                .collect();
            best.sort_by(|a, b| a.0.total_cmp(&b.0));
            best.dedup_by_key(|l| l.1); // un tour par pilote
            for lap in best.into_iter().take(3) {
                if attempts >= 6 {
                    return Err(last_err);
                }
                attempts += 1;
                match self.build_track(circuit_id, *year, key, event, lap).await {
                    Ok(track) => {
                        self.tracks
                            .write()
                            .await
                            .insert(circuit_id.to_string(), track.clone());
                        tracing::info!(circuit_id, key, points = track.points.len(), "track built");
                        return Ok(track);
                    }
                    Err(err) => last_err = err,
                }
            }
        }
        Err(last_err)
    }

    async fn build_track(
        &self,
        circuit_id: &str,
        year: u32,
        key: u32,
        event: &str,
        (duration, driver, lap, start): (f64, u32, u32, Ms),
    ) -> Result<Arc<TrackMap>, String> {
        let end = start + (duration * 1000.0) as Ms;
        let window = format!(
            "session_key={key}&driver_number={driver}&date>{}&date<{}",
            query_date(start - 200),
            query_date(end + 200)
        );
        let location = self.get(&format!("location?{window}")).await?;
        let car = self.get(&format!("car_data?{window}")).await?;
        let drivers = self
            .get(&format!("drivers?session_key={key}&driver_number={driver}"))
            .await?;
        let info = arr(&drivers).first();

        let mut raw: Vec<(Ms, f64, f64, f64)> = arr(&location)
            .iter()
            .filter_map(|p| {
                Some((
                    t(p, "date")?,
                    f(p, "x")?,
                    f(p, "y")?,
                    f(p, "z").unwrap_or(0.0),
                ))
            })
            .filter(|p| p.1 != 0.0 || p.2 != 0.0)
            .collect();
        raw.sort_by_key(|p| p.0);
        raw.dedup_by(|a, b| a.1 == b.1 && a.2 == b.2);
        let mut samples: Vec<(Ms, u16, u8, u8, bool)> = arr(&car)
            .iter()
            .filter_map(|c| {
                Some((
                    t(c, "date")?,
                    u(c, "speed")? as u16,
                    u(c, "n_gear").unwrap_or(0) as u8,
                    u(c, "throttle").unwrap_or(0).min(100) as u8,
                    u(c, "brake").unwrap_or(0) > 0,
                ))
            })
            .collect();
        samples.sort_by_key(|c| c.0);
        // Un tour complet donne ~4 positions par seconde : on exige au moins 80 % de couverture.
        let expected = duration * 3.5;
        if (raw.len() as f64) < expected * 0.8 || samples.is_empty() {
            return Err("Données de position insuffisantes".into());
        }

        // Repère : largeur 1000, axe y inversé (SVG), proportions conservées.
        let (min_x, max_x) = raw
            .iter()
            .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
        let (min_y, max_y) = raw
            .iter()
            .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.2), a.1.max(p.2)));
        let scale = 1000.0 / (max_x - min_x).max(1.0);
        let height = ((max_y - min_y) * scale).max(1.0);
        let min_z = raw.iter().map(|p| p.3).fold(f64::MAX, f64::min);

        let mut j = 0;
        let points: Vec<TrackPoint> = raw
            .iter()
            .map(|(time, x, y, z)| {
                while j + 1 < samples.len()
                    && (samples[j + 1].0 - time).abs() <= (samples[j].0 - time).abs()
                {
                    j += 1;
                }
                let c = samples[j];
                TrackPoint {
                    x: ((x - min_x) * scale) as f32,
                    y: ((max_y - y) * scale) as f32,
                    z: ((z - min_z) * scale) as f32,
                    t: ((time - start).max(0) as f32) / 1000.0,
                    speed: c.1,
                    gear: c.2,
                    throttle: c.3,
                    brake: c.4,
                }
            })
            .collect();

        let in_lap: Vec<_> = samples
            .iter()
            .filter(|c| c.0 >= start && c.0 <= end)
            .collect();
        let n = in_lap.len().max(1) as f32;
        let mut length_m = 0.0f64;
        for w in in_lap.windows(2) {
            length_m += w[0].1 as f64 / 3.6 * ((w[1].0 - w[0].0) as f64 / 1000.0);
        }
        let stats = TrackStats {
            top_speed: in_lap.iter().map(|c| c.1).max().unwrap_or(0),
            min_speed: in_lap.iter().map(|c| c.1).min().unwrap_or(0),
            avg_speed: in_lap.iter().map(|c| c.1 as f32).sum::<f32>() / n,
            full_throttle_pct: in_lap.iter().filter(|c| c.3 >= 98).count() as f32 / n * 100.0,
            braking_pct: in_lap.iter().filter(|c| c.4).count() as f32 / n * 100.0,
            length_km: (length_m / 1000.0) as f32,
            gear_changes: in_lap.windows(2).filter(|w| w[0].2 != w[1].2).count() as u32,
        };

        Ok(Arc::new(TrackMap {
            circuit_id: circuit_id.to_string(),
            year,
            session_key: key,
            event: event.to_string(),
            driver: info.map(|d| s(d, "full_name")).unwrap_or_default(),
            team: info.map(|d| s(d, "team_name")).unwrap_or_default(),
            colour: info
                .map(|d| s(d, "team_colour"))
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| "E10600".into()),
            lap,
            lap_time: duration,
            width: 1000.0,
            height,
            points,
            stats,
        }))
    }
}
