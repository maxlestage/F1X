//! Protocole WebSocket de F1X (`/ws`), partagé par le serveur (axum) et le frontend (Yew).
//! Tous les messages sont du JSON avec un champ `type`.

use serde::{Deserialize, Serialize};

/// Messages envoyés par le navigateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Suivre la session en direct (si le serveur a accès au direct OpenF1).
    Live,
    /// Rejouer une session passée comme si elle était en direct.
    Replay {
        session_key: u32,
        speed: u32,
    },
    /// Changer la vitesse du replay (1 = temps réel).
    Speed {
        speed: u32,
    },
    Pause,
    Resume,
    /// Avancer / reculer dans le replay, en secondes de session.
    Seek {
        seconds: i64,
    },
    /// Arrêter le suivi (retour à l'écran de choix).
    Stop,
}

/// Messages envoyés par le serveur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// État du direct : disponible ou non, et pourquoi.
    Status {
        live_available: bool,
        live_active: bool,
        message: String,
    },
    /// Nombre de personnes connectées en ce moment.
    Viewers {
        count: usize,
    },
    /// Chargement des données d'une session (replay).
    Loading {
        message: String,
    },
    /// Photo complète de la course à l'instant `clock`, envoyée chaque seconde.
    Snapshot(Box<Snapshot>),
    /// Tracé du circuit de la session (envoyé une fois au début du replay).
    Track(Box<TrackMap>),
    /// Fin du suivi (session terminée ou arrêt demandé).
    Stopped,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Live,
    Replay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackStatus {
    Green,
    Yellow,
    SafetyCar,
    VirtualSafetyCar,
    Red,
    Chequered,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_key: u32,
    /// « Race », « Qualifying », « Sprint », « Practice 1 »…
    pub session_name: String,
    pub session_type: String,
    pub location: String,
    pub country: String,
    pub circuit: String,
    pub date_start: String,
    pub date_end: String,
    pub year: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub mode: Mode,
    pub session: SessionSummary,
    /// Heure de la session (ISO-8601 UTC).
    pub clock: String,
    /// Avancement du replay (0 → 1).
    pub progress: f32,
    pub speed: u32,
    pub paused: bool,
    pub lap: u32,
    pub total_laps: Option<u32>,
    pub track_status: TrackStatus,
    pub cars: Vec<Car>,
    pub weather: Option<Weather>,
    /// Derniers messages de la direction de course, du plus récent au plus ancien.
    pub race_control: Vec<RaceControl>,
    /// Chronologie de la course (dépassements, arrêts, drapeaux…), du plus récent au plus ancien.
    pub events: Vec<RaceEvent>,
    /// Temps perdu estimé pour un arrêt aux stands (médiane de la course), en secondes.
    pub pit_loss: Option<f64>,
    pub finished: bool,
    /// Dernières radios d'équipe, de la plus récente à la plus ancienne.
    #[serde(default)]
    pub radios: Vec<LiveRadio>,
    /// Passages aux stands, du plus récent au plus ancien.
    #[serde(default)]
    pub pit_times: Vec<LivePit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceEvent {
    pub date: String,
    pub lap: u32,
    #[serde(flatten)]
    pub kind: EventKind,
}

/// Événements de course (le texte est composé côté client, dans la langue choisie).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    Overtake {
        driver: String,
        passed: String,
        position: u32,
    },
    Pit {
        driver: String,
        duration: Option<f64>,
    },
    FastestLap {
        driver: String,
        time: f64,
    },
    Status {
        status: TrackStatus,
    },
    Penalty {
        message: String,
    },
    Retired {
        driver: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StintInfo {
    pub compound: String,
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Car {
    pub number: u32,
    pub code: String,
    pub name: String,
    pub team: String,
    /// Couleur d'écurie, hexadécimal sans `#`.
    pub colour: String,
    pub position: u32,
    /// Écart au leader (« +12.345 », « +1 TOUR »), vide pour le leader.
    pub gap: String,
    /// Écart avec la voiture de devant.
    pub interval: String,
    pub last_lap: Option<f64>,
    pub best_lap: Option<f64>,
    /// Détient le meilleur tour de la session.
    pub fastest: bool,
    pub lap: u32,
    pub compound: Option<String>,
    pub tyre_age: Option<u32>,
    pub pits: u32,
    pub in_pit: bool,
    /// Temps des 3 secteurs du dernier tour bouclé.
    pub sectors: [Option<f64>; 3],
    /// 0 : inconnu, 1 : normal, 2 : record personnel (vert), 3 : meilleur de la session (violet).
    pub sector_flags: [u8; 3],
    /// Avancement dans le tour en cours (0 → 1), pour placer la voiture sur la carte.
    pub lap_progress: Option<f32>,
    /// Relais de pneus jusqu'ici.
    pub stints: Vec<StintInfo>,
    pub retired: bool,
    /// Vitesse de pointe au piège à radar (km/h), direct uniquement.
    #[serde(default)]
    pub top_speed: Option<u32>,
    /// Meilleurs temps personnels de chaque secteur.
    #[serde(default)]
    pub best_sectors: [Option<f64>; 3],
    /// Places gagnées (positif) ou perdues depuis le départ.
    #[serde(default)]
    pub gained: Option<i32>,
}

/// Message radio d'équipe (fichier audio public).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveRadio {
    pub date: String,
    pub code: String,
    pub colour: String,
    pub url: String,
}

/// Passage par la voie des stands : temps passé dans la voie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LivePit {
    pub date: String,
    pub code: String,
    pub colour: String,
    pub lap: Option<u32>,
    /// Secondes dans la voie des stands.
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub air_temperature: f64,
    pub track_temperature: f64,
    pub humidity: f64,
    /// Vent moyen en m/s (OpenF1).
    pub wind_speed: f64,
    /// Direction d'où vient le vent, en degrés.
    #[serde(default)]
    pub wind_direction: Option<f64>,
    /// Pression atmosphérique (hPa).
    #[serde(default)]
    pub pressure: Option<f64>,
    pub rainfall: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceControl {
    pub date: String,
    pub lap: Option<u32>,
    pub category: String,
    pub flag: Option<String>,
    pub message: String,
}

/// « 92.345 » → « 1:32.345 ».
pub fn format_lap(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor();
    let rest = seconds - minutes * 60.0;
    if minutes > 0.0 {
        format!("{}:{:06.3}", minutes as u32, rest)
    } else {
        format!("{rest:.3}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_laps() {
        assert_eq!(format_lap(92.345), "1:32.345");
        assert_eq!(format_lap(59.9), "59.900");
        assert_eq!(format_lap(605.0), "10:05.000");
    }
}

/// Tracé d'un circuit reconstitué à partir des positions GPS d'un tour réel (OpenF1),
/// avec la télémétrie de la voiture en chaque point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackMap {
    pub circuit_id: String,
    pub year: u32,
    pub session_key: u32,
    pub event: String,
    pub driver: String,
    pub team: String,
    /// Couleur d'écurie, hexadécimal sans `#`.
    pub colour: String,
    pub lap: u32,
    pub lap_time: f64,
    /// Dimensions du repère des points (largeur fixée à 1000).
    pub width: f64,
    pub height: f64,
    pub points: Vec<TrackPoint>,
    pub stats: TrackStats,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackPoint {
    pub x: f32,
    pub y: f32,
    /// Altitude relative (même échelle que x et y, 0 = point le plus bas).
    #[serde(default)]
    pub z: f32,
    /// Secondes écoulées depuis le début du tour.
    pub t: f32,
    pub speed: u16,
    pub gear: u8,
    pub throttle: u8,
    pub brake: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackStats {
    pub top_speed: u16,
    pub min_speed: u16,
    pub avg_speed: f32,
    /// Part du tour à fond (accélérateur ≥ 98 %).
    pub full_throttle_pct: f32,
    /// Part du tour au freinage.
    pub braking_pct: f32,
    /// Longueur estimée (intégration de la vitesse), en km.
    pub length_km: f32,
    pub gear_changes: u32,
}
