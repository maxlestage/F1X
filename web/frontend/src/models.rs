//! Modèles de l'API Jolpica F1 (ex-Ergast), servis via le proxy `/api` du serveur.
//! L'API renvoie les nombres sous forme de chaînes.

use serde::Deserialize;

use crate::util::parse_ms;

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

    pub fn start_ms(&self) -> f64 {
        parse_ms(&self.start_iso())
    }

    /// La course est considérée terminée ~2 h après le départ.
    pub fn is_over(&self, now_ms: f64) -> bool {
        let start = self.start_ms();
        !start.is_nan() && now_ms > start + 2.0 * 3600.0 * 1000.0
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Circuit {
    #[serde(rename = "circuitName")]
    pub circuit_name: String,
    #[serde(rename = "Location")]
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Location {
    pub locality: String,
    pub country: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Constructor {
    #[serde(rename = "constructorId")]
    pub constructor_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TimeValue {
    pub time: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FastestLap {
    pub rank: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
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
        self.fastest_lap.as_ref().and_then(|f| f.rank.as_deref()) == Some("1")
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
                let n = s
                    .trim_start_matches('+')
                    .split_whitespace()
                    .next()
                    .unwrap_or("1");
                format!("+{n} tour{}", if n == "1" { "" } else { "s" })
            }
            s => s.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StandingsList {
    #[serde(rename = "DriverStandings")]
    pub driver_standings: Option<Vec<DriverStanding>>,
    #[serde(rename = "ConstructorStandings")]
    pub constructor_standings: Option<Vec<ConstructorStanding>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ConstructorStanding {
    pub position: Option<String>,
    #[serde(rename = "positionText")]
    pub position_text: String,
    pub points: String,
    pub wins: String,
    #[serde(rename = "Constructor")]
    pub constructor: Constructor,
}
