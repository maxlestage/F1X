//! Direct sans abonnement : flux de chronométrage officiel F1 (SignalR Core, accès anonyme).
//!
//! Le flux public donne le classement, les écarts, les tours, les secteurs, les pneus, la
//! direction de course, la météo et l'état de la piste (pas les positions GPS, réservées aux
//! abonnés) : la place de chaque voiture sur la carte est estimée d'après le temps écoulé
//! depuis le début de son tour et les secteurs déjà bouclés. Qualifications comprises.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use f1x_protocol::{
    Car, LivePit, LiveRadio, Mode, RaceControl, SessionSummary, Snapshot, StintInfo, TrackStatus,
    Weather,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Map, Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

type Ms = i64;

const HOST: &str = "livetiming.formula1.com";
const TOPICS: [&str; 14] = [
    "Heartbeat",
    "TimingData",
    "TimingAppData",
    "TimingStats",
    "DriverList",
    "TrackStatus",
    "LapCount",
    "SessionInfo",
    "RaceControlMessages",
    "WeatherData",
    "ExtrapolatedClock",
    "TeamRadio",
    "PitLaneTimeCollection",
    "LapSeries",
];

/// Message du flux : sujet, données (état complet ou modification), horodatage.
pub type FeedMsg = (String, Value, Ms);

fn now_ms() -> Ms {
    Utc::now().timestamp_millis()
}

fn parse_ms(s: &str) -> Option<Ms> {
    // « 2026-10-03T08:52:04.0093535Z » (7 décimales) ou sans fuseau.
    let s = s.trim();
    let fixed = if s.ends_with('Z') || s.contains('+') {
        s.to_string()
    } else {
        format!("{s}Z")
    };
    DateTime::parse_from_rfc3339(&fixed)
        .ok()
        .map(|d| d.timestamp_millis())
}

fn iso(ms: Ms) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_default()
}

/// « 1:36.075 » ou « 36.075 » → secondes.
fn lap_secs(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let secs: f64 = match s.split_once(':') {
        Some((m, rest)) => m.parse::<f64>().ok()? * 60.0 + rest.parse::<f64>().ok()?,
        None => s.parse().ok()?,
    };
    Some((secs * 1000.0).round() / 1000.0)
}

fn st<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// Fusionne une modification du flux dans l'état : les tableaux sont modifiés par index
/// (`{"3": {...}}`), les objets récursivement.
fn merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(t), Value::Object(p)) => {
            for (k, v) in p {
                match t.get_mut(k) {
                    Some(existing) => merge(existing, v),
                    None => {
                        t.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (Value::Array(t), Value::Object(p)) => {
            for (k, v) in p {
                let Ok(i) = k.parse::<usize>() else { continue };
                while t.len() <= i {
                    t.push(Value::Object(Map::new()));
                }
                merge(&mut t[i], v);
            }
        }
        (t, p) => *t = p.clone(),
    }
}

/// Connexion au flux (via un proxy HTTP CONNECT si `HTTPS_PROXY` est défini) : envoie l'état
/// initial puis chaque modification sur `tx`. Rend la main à la déconnexion.
pub async fn stream(tx: &mpsc::Sender<FeedMsg>) -> Result<(), String> {
    let client = reqwest::Client::new();
    let res = client
        .post(format!(
            "https://{HOST}/signalrcore/negotiate?negotiateVersion=1"
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let cookie: String = res
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|c| c.to_str().ok()?.split(';').next().map(str::to_string))
        .collect::<Vec<_>>()
        .join("; ");
    let neg: Value = res.json().await.map_err(|e| e.to_string())?;
    let token = st(&neg, "connectionToken");
    if token.is_empty() {
        return Err("négociation sans jeton".into());
    }
    let url = format!("wss://{HOST}/signalrcore?id={}", urlencoding_simple(token));
    let mut req = url.into_client_request().map_err(|e| e.to_string())?;
    if !cookie.is_empty() {
        req.headers_mut()
            .insert("Cookie", cookie.parse().map_err(|_| "cookie invalide")?);
    }
    let tcp = connect_tcp().await?;
    let (ws, _) = tokio_tungstenite::client_async_tls_with_config(req, tcp, None, None)
        .await
        .map_err(|e| e.to_string())?;
    let (mut write, mut read) = ws.split();
    let send = |v: Value| Message::Text(format!("{v}\u{1e}").into());
    write
        .send(send(json!({"protocol": "json", "version": 1})))
        .await
        .map_err(|e| e.to_string())?;
    write
        .send(send(json!({
            "type": 1, "target": "Subscribe", "invocationId": "0", "arguments": [TOPICS],
        })))
        .await
        .map_err(|e| e.to_string())?;
    let mut ping = tokio::time::interval(Duration::from_secs(15));
    loop {
        tokio::select! {
            _ = ping.tick() => {
                if write.send(send(json!({"type": 6}))).await.is_err() {
                    return Err("ping impossible".into());
                }
            }
            msg = tokio::time::timeout(Duration::from_secs(90), read.next()) => {
                let Ok(Some(Ok(msg))) = msg else {
                    return Err("flux interrompu".into());
                };
                let Message::Text(text) = msg else {
                    if matches!(msg, Message::Close(_)) {
                        return Err("flux fermé".into());
                    }
                    continue;
                };
                for part in text.split('\u{1e}').filter(|p| !p.trim().is_empty()) {
                    let Ok(v) = serde_json::from_str::<Value>(part) else { continue };
                    match v.get("type").and_then(Value::as_u64) {
                        // État complet de chaque sujet, en réponse à l'abonnement.
                        Some(3) => {
                            if let Some(r) = v.get("result").and_then(Value::as_object) {
                                for (topic, data) in r {
                                    if tx.send((topic.clone(), data.clone(), now_ms())).await.is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        Some(1) if st(&v, "target") == "feed" => {
                            let args = v.get("arguments").and_then(Value::as_array);
                            if let Some([topic, data, ts, ..]) = args.map(Vec::as_slice) {
                                let at = ts.as_str().and_then(parse_ms).unwrap_or_else(now_ms);
                                let topic = topic.as_str().unwrap_or_default().to_string();
                                if tx.send((topic, data.clone(), at)).await.is_err() {
                                    return Ok(());
                                }
                            }
                        }
                        Some(7) => return Err("fermeture demandée par le serveur".into()),
                        _ => {}
                    }
                }
            }
        }
    }
}

fn urlencoding_simple(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Connexion TCP au flux, directe ou à travers le proxy HTTP indiqué par `HTTPS_PROXY`.
async fn connect_tcp() -> Result<TcpStream, String> {
    let proxy = std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .ok()
        .filter(|p| !p.is_empty());
    let Some(proxy) = proxy else {
        return TcpStream::connect((HOST, 443))
            .await
            .map_err(|e| e.to_string());
    };
    let addr = proxy
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let mut tcp = TcpStream::connect(&addr).await.map_err(|e| e.to_string())?;
    tcp.write_all(format!("CONNECT {HOST}:443 HTTP/1.1\r\nHost: {HOST}:443\r\n\r\n").as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") && buf.len() < 8192 {
        tcp.read_exact(&mut byte).await.map_err(|e| e.to_string())?;
        buf.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&buf);
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
        return Err(format!(
            "proxy : {}",
            head.lines().next().unwrap_or_default()
        ));
    }
    Ok(tcp)
}

/// État courant du flux et suivi des tours de chaque voiture (pour la carte).
#[derive(Default)]
pub struct Feed {
    state: Map<String, Value>,
    /// Début du tour en cours de chaque voiture (passage sur la ligne ou sortie des stands).
    lap_start: HashMap<String, Ms>,
    /// Dernier signe de vie du flux.
    pub heartbeat: Ms,
    /// Passages aux stands vus pendant la séance : (date, voiture, tour, durée dans la voie).
    pits: Vec<(Ms, String, Option<u32>, Option<f64>)>,
}

impl Feed {
    pub fn apply(&mut self, topic: &str, data: &Value, at: Ms) {
        if topic == "Heartbeat" {
            self.heartbeat = data
                .get("Utc")
                .and_then(Value::as_str)
                .and_then(parse_ms)
                .unwrap_or(at)
                .max(self.heartbeat);
        } else {
            self.heartbeat = self.heartbeat.max(at.min(now_ms()));
        }
        // Repère les débuts de tour : tour bouclé ou sortie des stands.
        // Temps passés dans la voie des stands (le flux ne garde que le dernier par voiture).
        if topic == "PitLaneTimeCollection" {
            if let Some(times) = data.get("PitTimes").and_then(Value::as_object) {
                for (n, p) in times {
                    if n == "_deleted" {
                        continue;
                    }
                    let lap = st(p, "Lap").parse().ok();
                    let duration = st(p, "Duration").parse().ok();
                    if duration.is_some() && !self.pits.iter().any(|x| &x.1 == n && x.2 == lap) {
                        self.pits.push((at, n.clone(), lap, duration));
                    }
                }
            }
        }
        if topic == "TimingData" {
            if let Some(lines) = data.get("Lines").and_then(Value::as_object) {
                for (n, l) in lines {
                    let new_lap = l.get("NumberOfLaps").is_some();
                    let pit_out = l.get("PitOut").and_then(Value::as_bool) == Some(true);
                    if new_lap || pit_out {
                        self.lap_start.insert(n.clone(), at);
                    }
                }
            }
        }
        match self.state.get_mut(topic) {
            Some(existing) => merge(existing, data),
            None => {
                self.state.insert(topic.to_string(), data.clone());
            }
        }
    }

    fn topic(&self, k: &str) -> &Value {
        self.state.get(k).unwrap_or(&Value::Null)
    }

    /// Session en cours d'après le flux (aucune si le flux est muet depuis 2 minutes).
    pub fn active(&self) -> bool {
        let status = st(self.topic("SessionInfo"), "SessionStatus");
        now_ms() - self.heartbeat < 120_000
            && matches!(
                status,
                "Started" | "Aborted" | "Finished" | "Finalised" | "Ends"
            )
    }

    pub fn session(&self) -> SessionSummary {
        let si = self.topic("SessionInfo");
        let meeting = si.get("Meeting").unwrap_or(&Value::Null);
        // Dates locales du circuit → UTC (décalage « 08:00:00 »).
        let offset_ms = {
            let o = st(si, "GmtOffset");
            let neg = o.starts_with('-');
            let mut it = o
                .trim_start_matches('-')
                .split(':')
                .map(|x| x.parse::<i64>().unwrap_or(0));
            let (h, m) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
            (h * 3_600_000 + m * 60_000) * if neg { -1 } else { 1 }
        };
        let utc = |k: &str| {
            parse_ms(st(si, k))
                .map(|t| iso(t - offset_ms))
                .unwrap_or_default()
        };
        SessionSummary {
            session_key: si.get("Key").and_then(Value::as_u64).unwrap_or(0) as u32,
            session_name: st(si, "Name").to_string(),
            session_type: st(si, "Type").to_string(),
            location: st(meeting, "Location").to_string(),
            country: meeting
                .get("Country")
                .map(|c| st(c, "Name").to_string())
                .unwrap_or_default(),
            circuit: meeting
                .get("Circuit")
                .map(|c| st(c, "ShortName").to_string())
                .unwrap_or_default(),
            date_start: utc("StartDate"),
            date_end: utc("EndDate"),
            year: st(si, "StartDate")
                .get(..4)
                .and_then(|y| y.parse().ok())
                .unwrap_or(0),
        }
    }

    /// Image de la séance à l'instant `now`, au format du Race Center.
    pub fn snapshot(&self, now: Ms) -> Snapshot {
        let session = self.session();
        let racing = st(self.topic("SessionInfo"), "Type") == "Race";
        let timing = self.topic("TimingData");
        let part = timing
            .get("SessionPart")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1) as usize;
        let drivers = self.topic("DriverList");
        let app = self.topic("TimingAppData").get("Lines");
        let lines = timing
            .get("Lines")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let best_of = |l: &Value| {
            l.get("BestLapTime")
                .map(|b| st(b, "Value"))
                .and_then(lap_secs)
        };
        let session_best = lines
            .values()
            .filter_map(best_of)
            .fold(f64::INFINITY, f64::min);
        let median = {
            let mut v: Vec<f64> = lines
                .values()
                .filter_map(|l| {
                    l.get("LastLapTime")
                        .map(|x| st(x, "Value"))
                        .and_then(lap_secs)
                })
                .filter(|t| *t > 40.0)
                .collect();
            v.sort_by(f64::total_cmp);
            v.get(v.len() / 2).copied().unwrap_or(95.0)
        };

        let mut cars: Vec<Car> = lines
            .iter()
            .map(|(n, l)| {
                let d = drivers.get(n).unwrap_or(&Value::Null);
                let stats = self
                    .topic("TimingStats")
                    .get("Lines")
                    .and_then(|x| x.get(n));
                let number = n.parse().unwrap_or(0);
                let flag = |k: &str| l.get(k).and_then(Value::as_bool).unwrap_or(false);
                let (in_pit, retired) = (flag("InPit"), flag("Retired") || flag("Stopped"));
                let last = l
                    .get("LastLapTime")
                    .map(|x| st(x, "Value"))
                    .and_then(lap_secs);
                let best = best_of(l);
                let lap = l.get("NumberOfLaps").and_then(Value::as_u64).unwrap_or(0) as u32;
                // Écarts : course → au leader / à la voiture devant ; qualifs → au meilleur
                // temps de la partie en cours (Q1, Q2, Q3).
                let (gap, interval) = if racing {
                    (
                        st(l, "GapToLeader").to_string(),
                        l.get("IntervalToPositionAhead")
                            .map(|x| st(x, "Value").to_string())
                            .unwrap_or_default(),
                    )
                } else {
                    let stats = l
                        .get("Stats")
                        .and_then(Value::as_array)
                        .and_then(|s| s.get(part - 1));
                    stats
                        .map(|s| {
                            (
                                st(s, "TimeDiffToFastest").to_string(),
                                st(s, "TimeDifftoPositionAhead").to_string(),
                            )
                        })
                        .unwrap_or_default()
                };
                let sectors_v = l.get("Sectors").and_then(Value::as_array);
                let sector = |i: usize| sectors_v.and_then(|s| s.get(i));
                let sectors: [Option<f64>; 3] = std::array::from_fn(|i| {
                    sector(i).and_then(|s| {
                        let v = st(s, "Value");
                        lap_secs(if v.is_empty() {
                            st(s, "PreviousValue")
                        } else {
                            v
                        })
                    })
                });
                let sector_flags: [u8; 3] = std::array::from_fn(|i| match sector(i) {
                    Some(s) if s.get("OverallFastest").and_then(Value::as_bool) == Some(true) => 3,
                    Some(s) if s.get("PersonalFastest").and_then(Value::as_bool) == Some(true) => 2,
                    Some(_) if sectors[i].is_some() => 1,
                    _ => 0,
                });
                // Place sur la carte : secteurs bouclés dans le tour en cours, affinés par le
                // temps écoulé depuis le début du tour.
                let done = (0..3)
                    .take_while(|&i| sector(i).is_some_and(|s| !st(s, "Value").is_empty()))
                    .count();
                let reference = last
                    .filter(|t| *t < median * 1.3)
                    .unwrap_or(median)
                    .max(30.0);
                let lap_progress = (!in_pit && !retired && (racing || l.get("Sectors").is_some()))
                    .then(|| {
                        let lo = (done as f64 / 3.0).min(0.98);
                        let hi = ((done + 1) as f64 / 3.0 - 0.01).min(0.999);
                        let by_time = self
                            .lap_start
                            .get(n)
                            .map(|s| (now - s) as f64 / 1000.0 / reference)
                            .unwrap_or(lo + 1.0 / 6.0);
                        by_time.clamp(lo, hi) as f32
                    })
                    // Qualifs : voiture au stand ou pas encore sortie.
                    .filter(|_| racing || self.lap_start.contains_key(n) || done > 0);
                let stints_v = app
                    .and_then(|a| a.get(n))
                    .and_then(|a| a.get("Stints"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut from = 1;
                let stints: Vec<StintInfo> = stints_v
                    .iter()
                    .map(|s| {
                        let total = s.get("TotalLaps").and_then(Value::as_u64).unwrap_or(0) as u32;
                        let start = s.get("StartLaps").and_then(Value::as_u64).unwrap_or(0) as u32;
                        let laps = total.saturating_sub(start).max(1);
                        let info = StintInfo {
                            compound: st(s, "Compound").to_string(),
                            from,
                            to: from + laps - 1,
                        };
                        from += laps;
                        info
                    })
                    .collect();
                let current = stints_v.last();
                Car {
                    number,
                    code: st(d, "Tla").to_string(),
                    name: {
                        let (f, la) = (st(d, "FirstName"), st(d, "LastName"));
                        if f.is_empty() {
                            st(d, "FullName").to_string()
                        } else {
                            format!("{f} {}", la.to_uppercase())
                        }
                    },
                    team: st(d, "TeamName").to_string(),
                    colour: st(d, "TeamColour").to_string(),
                    position: st(l, "Position").parse().unwrap_or(99),
                    gap,
                    interval,
                    last_lap: last,
                    best_lap: best,
                    fastest: best.is_some_and(|b| b == session_best),
                    lap,
                    compound: current
                        .map(|s| st(s, "Compound").to_string())
                        .filter(|c| !c.is_empty() && c != "UNKNOWN"),
                    tyre_age: current
                        .and_then(|s| s.get("TotalLaps"))
                        .and_then(Value::as_u64)
                        .map(|t| t as u32),
                    pits: l
                        .get("NumberOfPitStops")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32,
                    in_pit,
                    sectors,
                    sector_flags,
                    lap_progress,
                    stints,
                    retired,
                    top_speed: stats
                        .and_then(|x| x.get("BestSpeeds"))
                        .and_then(|b| b.get("ST"))
                        .and_then(|v| st(v, "Value").parse().ok()),
                    best_sectors: std::array::from_fn(|i| {
                        stats
                            .and_then(|x| x.get("BestSectors"))
                            .and_then(Value::as_array)
                            .and_then(|a| a.get(i))
                            .and_then(|v| lap_secs(st(v, "Value")))
                    }),
                    // Places gagnées depuis la grille (position au tour 0 de l'historique).
                    gained: racing
                        .then(|| {
                            let series = self.topic("LapSeries").get(n)?.get("LapPosition")?;
                            let first = match series {
                                Value::Array(a) => a.first().cloned(),
                                Value::Object(o) => o.get("0").cloned(),
                                _ => None,
                            }?;
                            let start: i32 = first.as_str()?.parse().ok()?;
                            let now: i32 = st(l, "Position").parse().ok()?;
                            Some(start - now)
                        })
                        .flatten(),
                }
            })
            .collect();
        cars.sort_by_key(|c| c.position);

        let track_status = match st(self.topic("TrackStatus"), "Status") {
            "2" => TrackStatus::Yellow,
            "4" => TrackStatus::SafetyCar,
            "5" => TrackStatus::Red,
            "6" | "7" => TrackStatus::VirtualSafetyCar,
            _ => TrackStatus::Green,
        };
        let messages = self
            .topic("RaceControlMessages")
            .get("Messages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let chequered = messages.iter().any(|m| st(m, "Flag") == "CHEQUERED")
            && matches!(
                st(self.topic("SessionInfo"), "SessionStatus"),
                "Finished" | "Finalised" | "Ends"
            );
        let race_control: Vec<RaceControl> = messages
            .iter()
            .rev()
            .take(12)
            .map(|m| RaceControl {
                date: parse_ms(st(m, "Utc")).map(iso).unwrap_or_default(),
                lap: m.get("Lap").and_then(Value::as_u64).map(|x| x as u32),
                category: st(m, "Category").to_string(),
                flag: Some(st(m, "Flag").to_string()).filter(|f| !f.is_empty()),
                message: st(m, "Message").to_string(),
            })
            .collect();
        let w = self.topic("WeatherData");
        let num = |k: &str| st(w, k).parse::<f64>().ok();
        let weather = num("AirTemp").map(|air| Weather {
            air_temperature: air,
            track_temperature: num("TrackTemp").unwrap_or(0.0),
            humidity: num("Humidity").unwrap_or(0.0),
            wind_speed: num("WindSpeed").unwrap_or(0.0),
            wind_direction: num("WindDirection"),
            pressure: num("Pressure"),
            rainfall: num("Rainfall").unwrap_or(0.0) > 0.0,
        });
        let lc = self.topic("LapCount");
        let leader_lap = cars.first().map(|c| c.lap).unwrap_or(0);
        Snapshot {
            mode: Mode::Live,
            session,
            clock: iso(now),
            progress: 0.0,
            speed: 1,
            paused: false,
            lap: lc
                .get("CurrentLap")
                .and_then(Value::as_u64)
                .map(|x| x as u32)
                .unwrap_or(leader_lap),
            total_laps: lc
                .get("TotalLaps")
                .and_then(Value::as_u64)
                .map(|x| x as u32),
            track_status: if chequered {
                TrackStatus::Chequered
            } else {
                track_status
            },
            cars,
            weather,
            race_control,
            events: Vec::new(),
            pit_loss: None,
            finished: chequered,
            radios: {
                let base = st(self.topic("SessionInfo"), "Path");
                let caps = match self.topic("TeamRadio").get("Captures") {
                    Some(Value::Array(a)) => a.clone(),
                    Some(Value::Object(o)) => o.values().cloned().collect(),
                    _ => Vec::new(),
                };
                let mut list: Vec<LiveRadio> = caps
                    .iter()
                    .filter(|c| !st(c, "Path").is_empty())
                    .map(|c| {
                        let d = drivers.get(st(c, "RacingNumber")).unwrap_or(&Value::Null);
                        LiveRadio {
                            date: parse_ms(st(c, "Utc")).map(iso).unwrap_or_default(),
                            code: st(d, "Tla").to_string(),
                            colour: st(d, "TeamColour").to_string(),
                            url: format!("https://{HOST}/static/{base}{}", st(c, "Path")),
                        }
                    })
                    .collect();
                list.sort_by(|a, b| b.date.cmp(&a.date));
                list.truncate(15);
                list
            },
            pit_times: self
                .pits
                .iter()
                .rev()
                .take(20)
                .map(|(at, n, lap, dur)| {
                    let d = drivers.get(n).unwrap_or(&Value::Null);
                    LivePit {
                        date: iso(*at),
                        code: st(d, "Tla").to_string(),
                        colour: st(d, "TeamColour").to_string(),
                        lap: *lap,
                        duration: *dur,
                    }
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_indexed_patches() {
        let mut s = json!({"Lines": {"1": {"Sectors": [{"Value": "1"}, {"Value": "2"}]}}});
        merge(
            &mut s,
            &json!({"Lines": {"1": {"Sectors": {"1": {"Value": "9"}}, "InPit": true}}}),
        );
        assert_eq!(s["Lines"]["1"]["Sectors"][1]["Value"], "9");
        assert_eq!(s["Lines"]["1"]["InPit"], true);
        assert_eq!(lap_secs("1:36.075"), Some(96.075));
        assert!(parse_ms("2026-10-03T08:00:00").is_some());
    }
}

#[cfg(test)]
mod fixture {
    #[test]
    fn snapshot_from_saved_state() {
        let Ok(path) = std::env::var("F1X_FEED_FIXTURE") else {
            return;
        };
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut feed = super::Feed::default();
        for (k, d) in v.as_object().unwrap() {
            feed.apply(k, d, super::now_ms());
        }
        let snap = feed.snapshot(super::now_ms());
        for c in snap.cars.iter().take(6) {
            println!(
                "{} {} gap={} top={:?} best={:?} gained={:?}",
                c.position, c.code, c.gap, c.top_speed, c.best_sectors, c.gained
            );
        }
        for r in snap.radios.iter().take(3) {
            println!("radio {} {} {}", r.date, r.code, r.url);
        }
        println!("pits {}", snap.pit_times.len());
    }
}
