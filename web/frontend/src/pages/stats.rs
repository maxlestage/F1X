//! Calculs partagés : carrière d'un pilote, champions, classements « top N ».

use std::collections::HashMap;

use serde::Deserialize;

use crate::models::{Constructor, Driver, Race, RaceResult, is_classified};

/// Champion d'une saison (`/api/champions`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Champion {
    pub season: String,
    #[serde(default)]
    pub in_progress: bool,
    pub driver: Driver,
    pub driver_team: Option<Constructor>,
    pub driver_points: Option<String>,
    pub driver_wins: Option<String>,
    pub constructor: Option<Constructor>,
    pub constructor_points: Option<String>,
}

/// Titres par pilote (saisons terminées seulement).
pub fn driver_titles(champions: &[Champion], id: &str) -> Vec<String> {
    champions
        .iter()
        .filter(|c| !c.in_progress && c.driver.driver_id == id)
        .map(|c| c.season.clone())
        .collect()
}

pub fn team_titles(champions: &[Champion], id: &str) -> Vec<String> {
    champions
        .iter()
        .filter(|c| {
            !c.in_progress
                && c.constructor
                    .as_ref()
                    .is_some_and(|k| k.constructor_id == id)
        })
        .map(|c| c.season.clone())
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Career {
    pub starts: u32,
    pub wins: u32,
    pub podiums: u32,
    pub poles: u32,
    pub fastest: u32,
    pub points: f64,
    pub retirements: u32,
    pub finishes: u32,
    pub finish_sum: u32,
    pub best: Option<u32>,
    pub seasons: Vec<String>,
    pub first_win: Option<String>,
    pub last_win: Option<String>,
}

impl Career {
    pub fn avg_finish(&self) -> Option<f64> {
        (self.finishes > 0).then(|| self.finish_sum as f64 / self.finishes as f64)
    }
    pub fn win_rate(&self) -> f64 {
        if self.starts == 0 {
            0.0
        } else {
            self.wins as f64 / self.starts as f64 * 100.0
        }
    }
}

/// Résultat du pilote dans chaque course (une entrée par course).
pub fn entries(races: &[Race]) -> Vec<(&Race, &RaceResult)> {
    races
        .iter()
        .filter_map(|r| Some((r, r.results.as_ref()?.first()?)))
        .collect()
}

pub fn career(races: &[Race]) -> Career {
    let mut c = Career::default();
    for (race, r) in entries(races) {
        c.starts += 1;
        let pos: Option<u32> = r.position.parse().ok();
        if r.position == "1" {
            c.wins += 1;
            c.first_win.get_or_insert_with(|| race.season.clone());
            c.last_win = Some(race.season.clone());
        }
        if matches!(r.position.as_str(), "1" | "2" | "3") {
            c.podiums += 1;
        }
        if r.grid.as_deref() == Some("1") {
            c.poles += 1;
        }
        if r.has_fastest_lap() {
            c.fastest += 1;
        }
        c.points += r.points.parse::<f64>().unwrap_or(0.0);
        let classified = r.status.as_deref().is_none_or(is_classified) || r.time.is_some();
        if classified {
            if let Some(p) = pos {
                c.finishes += 1;
                c.finish_sum += p;
                c.best = Some(c.best.map_or(p, |b| b.min(p)));
            }
        } else {
            c.retirements += 1;
        }
        if c.seasons.last() != Some(&race.season) {
            c.seasons.push(race.season.clone());
        }
    }
    c
}

/// Compte les occurrences (clé → libellé, nombre), trié décroissant.
pub fn tally(items: impl Iterator<Item = (String, String)>) -> Vec<(String, String, u32)> {
    let mut map: HashMap<String, (String, u32)> = HashMap::new();
    for (id, name) in items {
        map.entry(id).or_insert((name, 0)).1 += 1;
    }
    let mut v: Vec<(String, String, u32)> = map
        .into_iter()
        .map(|(id, (name, n))| (id, name, n))
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));
    v
}

/// Âge en années décimales entre une date de naissance et une date de course (AAAA-MM-JJ).
pub fn age_at(birth: &str, date: &str) -> Option<f64> {
    let b = crate::util::parse_ms(&format!("{birth}T00:00:00Z"));
    let d = crate::util::parse_ms(&format!("{date}T00:00:00Z"));
    (!b.is_nan() && !d.is_nan()).then(|| (d - b) / (365.2425 * 86_400_000.0))
}
