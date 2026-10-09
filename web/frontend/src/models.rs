//! Modèles de l'API Jolpica F1 (ex-Ergast), servis par le proxy `/api` du serveur.
//! Une seule enveloppe `MrData` couvre tous les endpoints ; les nombres sont des chaînes.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::util::parse_ms;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Envelope {
    #[serde(rename = "MRData")]
    pub data: MrData,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct MrData {
    pub total: String,
    #[serde(rename = "RaceTable")]
    pub race_table: Option<RaceTable>,
    #[serde(rename = "StandingsTable")]
    pub standings_table: Option<StandingsTable>,
    #[serde(rename = "DriverTable")]
    pub driver_table: Option<DriverTable>,
    #[serde(rename = "ConstructorTable")]
    pub constructor_table: Option<ConstructorTable>,
    #[serde(rename = "CircuitTable")]
    pub circuit_table: Option<CircuitTable>,
    #[serde(rename = "SeasonTable")]
    pub season_table: Option<SeasonTable>,
    #[serde(rename = "StatusTable")]
    pub status_table: Option<StatusTable>,
}

impl MrData {
    pub fn total(&self) -> u32 {
        self.total.parse().unwrap_or(0)
    }
    pub fn races(&self) -> &[Race] {
        self.race_table
            .as_ref()
            .map(|t| t.races.as_slice())
            .unwrap_or_default()
    }
    pub fn race(&self) -> Option<&Race> {
        self.races().first()
    }
    pub fn standings(&self) -> Option<&StandingsList> {
        self.standings_table.as_ref().and_then(|t| t.lists.first())
    }
    pub fn drivers(&self) -> &[Driver] {
        self.driver_table
            .as_ref()
            .map(|t| t.drivers.as_slice())
            .unwrap_or_default()
    }
    pub fn constructors(&self) -> &[Constructor] {
        self.constructor_table
            .as_ref()
            .map(|t| t.constructors.as_slice())
            .unwrap_or_default()
    }
    pub fn circuits(&self) -> &[Circuit] {
        self.circuit_table
            .as_ref()
            .map(|t| t.circuits.as_slice())
            .unwrap_or_default()
    }
    pub fn seasons(&self) -> &[Season] {
        self.season_table
            .as_ref()
            .map(|t| t.seasons.as_slice())
            .unwrap_or_default()
    }
    pub fn statuses(&self) -> &[Status] {
        self.status_table
            .as_ref()
            .map(|t| t.status.as_slice())
            .unwrap_or_default()
    }

    /// Tours fusionnés (une réponse paginée peut répéter la course avec des tours partiels).
    pub fn laps(&self) -> BTreeMap<u32, Vec<Timing>> {
        let mut laps: BTreeMap<u32, Vec<Timing>> = BTreeMap::new();
        for race in self.races() {
            for lap in race.laps.iter().flatten() {
                let n = lap.number.parse().unwrap_or(0);
                laps.entry(n)
                    .or_default()
                    .extend(lap.timings.iter().cloned());
            }
        }
        laps
    }

    /// Arrêts aux stands fusionnés.
    pub fn pit_stops(&self) -> Vec<PitStop> {
        self.races()
            .iter()
            .flat_map(|r| r.pit_stops.iter().flatten().cloned())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct RaceTable {
    #[serde(rename = "Races", default)]
    pub races: Vec<Race>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct StandingsTable {
    #[serde(rename = "StandingsLists", default)]
    pub lists: Vec<StandingsList>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct DriverTable {
    #[serde(rename = "Drivers", default)]
    pub drivers: Vec<Driver>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct ConstructorTable {
    #[serde(rename = "Constructors", default)]
    pub constructors: Vec<Constructor>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct CircuitTable {
    #[serde(rename = "Circuits", default)]
    pub circuits: Vec<Circuit>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct SeasonTable {
    #[serde(rename = "Seasons", default)]
    pub seasons: Vec<Season>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct StatusTable {
    #[serde(rename = "Status", default)]
    pub status: Vec<Status>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Season {
    pub season: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Status {
    pub status: String,
    pub count: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Race {
    pub season: String,
    pub round: String,
    pub url: Option<String>,
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
    /// Qualifs sprint : ajoutées par le serveur F1X depuis OpenF1 (absentes de Jolpica).
    #[serde(rename = "SprintQualifyingResults")]
    pub sprint_qualifying_results: Option<Vec<QualifyingResult>>,
    #[serde(rename = "Laps")]
    pub laps: Option<Vec<Lap>>,
    #[serde(rename = "PitStops")]
    pub pit_stops: Option<Vec<PitStop>>,
}

impl Race {
    pub fn round_num(&self) -> u32 {
        self.round.parse().unwrap_or(0)
    }

    /// Départ de la course en ISO-8601 UTC (00:00Z si l'heure est inconnue).
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

    pub fn has_time(&self) -> bool {
        self.time.is_some()
    }

    /// Sessions du week-end dans l'ordre chronologique, course comprise.
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

    pub fn winner(&self) -> Option<&RaceResult> {
        self.results.as_ref()?.first()
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
    #[serde(rename = "circuitId")]
    pub circuit_id: String,
    pub url: Option<String>,
    #[serde(rename = "circuitName")]
    pub circuit_name: String,
    #[serde(rename = "Location")]
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Location {
    pub lat: Option<String>,
    pub long: Option<String>,
    pub locality: String,
    pub country: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Driver {
    #[serde(rename = "driverId")]
    pub driver_id: String,
    #[serde(rename = "permanentNumber")]
    pub permanent_number: Option<String>,
    pub code: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "givenName")]
    pub given_name: String,
    #[serde(rename = "familyName")]
    pub family_name: String,
    #[serde(rename = "dateOfBirth")]
    pub date_of_birth: Option<String>,
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
    pub url: Option<String>,
    pub name: String,
    pub nationality: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TimeValue {
    pub time: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AverageSpeed {
    pub units: String,
    pub speed: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FastestLap {
    pub rank: Option<String>,
    pub lap: Option<String>,
    #[serde(rename = "Time")]
    pub time: Option<TimeValue>,
    #[serde(rename = "AverageSpeed")]
    pub average_speed: Option<AverageSpeed>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RaceResult {
    pub number: Option<String>,
    pub position: String,
    #[serde(rename = "positionText")]
    pub position_text: String,
    pub points: String,
    #[serde(rename = "Driver")]
    pub driver: Driver,
    #[serde(rename = "Constructor")]
    pub constructor: Constructor,
    pub grid: Option<String>,
    pub laps: Option<String>,
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

    /// Positions gagnées (positif) ou perdues depuis la grille ; `None` si départ des stands.
    pub fn places_gained(&self) -> Option<i32> {
        let grid: i32 = self.grid.as_deref()?.parse().ok()?;
        let pos: i32 = self.position.parse().ok()?;
        (grid > 0).then_some(grid - pos)
    }

    /// Temps / écart, ou statut d'abandon traduit.
    pub fn outcome(&self) -> String {
        if let Some(t) = &self.time {
            return t.time.clone();
        }
        translate_status(self.status.as_deref().unwrap_or_default())
    }
}

/// Arrivée classée (« Finished », « +2 Laps », « Lapped »), par opposition aux abandons.
pub fn is_classified(status: &str) -> bool {
    status == "Finished"
        || status == "Lapped"
        || (status.starts_with('+') && status.contains("Lap"))
}

/// Traduction de tous les statuts d'arrivée de l'API (136 en 2026).
/// Les statuts synonymes reçoivent le même libellé (ex. « Puncture » / « Tyre puncture »).
pub fn translate_status(status: &str) -> String {
    if !crate::i18n::is_fr() {
        // L'API est en anglais : on garde son texte (sauf « Finished », implicite).
        return if status == "Finished" {
            String::new()
        } else {
            status.to_string()
        };
    }
    let fr = match status {
        "Finished" => "",
        "Lapped" => "Doublé",
        "Engine" => "Moteur",
        "Accident" => "Accident",
        "Fatal accident" => "Accident mortel",
        "Collision" => "Accrochage",
        "Collision damage" | "Damage" => "Dégâts après contact",
        "Gearbox" => "Boîte de vitesses",
        "Spun off" => "Tête-à-queue",
        "Suspension" => "Suspension",
        "Transmission" | "Drivetrain" => "Transmission",
        "Electrical" | "Electronics" => "Électronique",
        "Retired" => "Abandon (non précisé)",
        "Brakes" => "Freins",
        "Brake duct" => "Écope de frein",
        "Withdrew" => "Forfait",
        "Clutch" => "Embrayage",
        "Not classified" => "Non classé",
        "Fuel system" => "Circuit d'essence",
        "Disqualified" => "Disqualifié",
        "Excluded" => "Exclu",
        "Underweight" => "Poids insuffisant",
        "Turbo" => "Turbo",
        "Hydraulics" => "Hydraulique",
        "Pneumatics" => "Pneumatique",
        "Overheating" => "Surchauffe",
        "Cooling system" => "Refroidissement",
        "Ignition" => "Allumage",
        "Spark plugs" => "Bougies",
        "Magneto" => "Magnéto",
        "Distributor" => "Distributeur",
        "Oil leak" => "Fuite d'huile",
        "Oil pressure" => "Pression d'huile",
        "Oil pump" => "Pompe à huile",
        "Oil pipe" | "Oil line" => "Durite d'huile",
        "Throttle" => "Accélérateur",
        "Out of fuel" | "Fuel" => "Panne d'essence",
        "Fuel pump" => "Pompe à essence",
        "Fuel leak" => "Fuite d'essence",
        "Fuel pressure" => "Pression d'essence",
        "Fuel pipe" => "Durite d'essence",
        "Fuel rig" | "Refuelling" => "Ravitaillement",
        "Injection" => "Injection",
        "Halfshaft" | "Driveshaft" | "CV joint" | "Axle" => "Arbre de transmission",
        "Differential" => "Différentiel",
        "Crankshaft" => "Vilebrequin",
        "Wheel" | "Wheel rim" => "Roue",
        "Wheel nut" => "Écrou de roue",
        "Wheel bearing" => "Roulement de roue",
        "Tyre" => "Pneu",
        "Puncture" | "Tyre puncture" => "Crevaison",
        "Handling" => "Tenue de route",
        "Steering" | "Track rod" => "Direction",
        "Radiator" => "Radiateur",
        "Water leak" => "Fuite d'eau",
        "Water pressure" => "Pression d'eau",
        "Water pump" => "Pompe à eau",
        "Water pipe" => "Durite d'eau",
        "Power Unit" => "Unité de puissance",
        "ERS" => "Système hybride (ERS)",
        "Battery" => "Batterie",
        "Alternator" => "Alternateur",
        "Power loss" => "Perte de puissance",
        "Supercharger" => "Compresseur",
        "Exhaust" => "Échappement",
        "Chassis" => "Châssis",
        "Undertray" => "Fond plat",
        "Mechanical" | "Technical" => "Problème mécanique",
        "Vibrations" => "Vibrations",
        "Physical" | "Driver unwell" | "Illness" => "Malaise du pilote",
        "Injury" | "Injured" | "Eye injury" => "Blessure",
        "Heat shield fire" | "Fire" | "Engine fire" => "Incendie",
        "Engine misfire" => "Ratés moteur",
        "Did not start" => "Non partant",
        "Did not qualify" => "Non qualifié",
        "Did not prequalify" => "Non préqualifié",
        "Not restarted" => "Non reparti",
        "Stalled" => "Calé",
        "Launch control" => "Aide au départ",
        "Broken wing" | "Front wing" => "Aileron avant",
        "Rear wing" => "Aileron arrière",
        "Debris" => "Débris",
        "Safety" | "Safety concerns" => "Raisons de sécurité",
        "Safety belt" => "Harnais",
        "Seat" | "Driver Seat" => "Siège",
        s if s.starts_with('+') && s.contains("Lap") => {
            let n = s
                .trim_start_matches('+')
                .split_whitespace()
                .next()
                .unwrap_or("1");
            return format!("+{n} tour{}", if n == "1" { "" } else { "s" });
        }
        s => return s.to_string(),
    };
    fr.to_string()
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
    /// Meilleur segment atteint, ex. ("Q3", "1:29.708").
    pub fn best(&self) -> Option<(&'static str, &str)> {
        [("Q3", &self.q3), ("Q2", &self.q2), ("Q1", &self.q1)]
            .into_iter()
            .find_map(|(label, t)| t.as_deref().filter(|t| !t.is_empty()).map(|t| (label, t)))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Lap {
    pub number: String,
    #[serde(rename = "Timings", default)]
    pub timings: Vec<Timing>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Timing {
    #[serde(rename = "driverId")]
    pub driver_id: String,
    pub position: String,
    pub time: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PitStop {
    #[serde(rename = "driverId")]
    pub driver_id: String,
    pub lap: String,
    pub stop: String,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct StandingsList {
    pub season: Option<String>,
    pub round: Option<String>,
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

impl DriverStanding {
    pub fn rank(&self) -> String {
        self.position
            .clone()
            .unwrap_or_else(|| self.position_text.clone())
    }
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

impl ConstructorStanding {
    pub fn rank(&self) -> String {
        self.position
            .clone()
            .unwrap_or_else(|| self.position_text.clone())
    }
}
