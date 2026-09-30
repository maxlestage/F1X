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
    pub finished: bool,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub air_temperature: f64,
    pub track_temperature: f64,
    pub humidity: f64,
    pub wind_speed: f64,
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
