//! Calcule l'état complet d'une course à un instant donné à partir d'un [`Dataset`].
//!
//! Sans état : on recalcule tout à chaque image (quelques dizaines de milliers
//! d'événements, négligeable), ce qui rend l'avance / le retour rapide triviaux.

use std::collections::HashMap;

use f1x_protocol::{Car, Mode, Snapshot, TrackStatus};
use serde_json::Value;

use crate::openf1::{Dataset, Ms, iso};

/// Début et fin utiles d'une session (on saute l'attente avant le départ).
pub fn time_range(d: &Dataset) -> (Ms, Ms) {
    let session_start = d
        .session
        .as_ref()
        .and_then(|s| crate::openf1::parse_date(&s.date_start));
    let first_lap = d.laps.iter().map(|l| l.start).min();
    let start = first_lap
        .map(|t| t - 120_000)
        .or(session_start)
        .unwrap_or(0);
    let last = d
        .laps
        .iter()
        .filter_map(|l| l.end())
        .max()
        .or_else(|| {
            d.session
                .as_ref()
                .and_then(|s| crate::openf1::parse_date(&s.date_end))
        })
        .unwrap_or(start);
    (start, last.max(start) + 30_000)
}

fn gap_text(v: &Value) -> String {
    match v {
        Value::Number(n) => n
            .as_f64()
            .map(|x| {
                if x == 0.0 {
                    String::new()
                } else {
                    format!("+{x:.3}")
                }
            })
            .unwrap_or_default(),
        Value::String(s) => s.replace("LAPS", "TOURS").replace("LAP", "TOUR"),
        _ => String::new(),
    }
}

/// Dernier élément d'un tableau trié par date dont la date est `<= at`.
fn upto<T>(items: &[T], at: Ms, date: impl Fn(&T) -> Ms) -> &[T] {
    &items[..items.partition_point(|x| date(x) <= at)]
}

pub struct Frame {
    pub mode: Mode,
    pub speed: u32,
    pub paused: bool,
    pub progress: f32,
    pub finished: bool,
}

pub fn snapshot(d: &Dataset, at: Ms, frame: Frame) -> Snapshot {
    let positions = upto(&d.positions, at, |p| p.0);
    let intervals = upto(&d.intervals, at, |p| p.0);
    let rc = upto(&d.race_control, at, |p| p.0);
    let pits = upto(&d.pits, at, |p| p.time);

    let mut pos: HashMap<u32, u32> = HashMap::new();
    for (_, driver, p) in positions {
        pos.insert(*driver, *p);
    }
    let mut gaps: HashMap<u32, (&Value, &Value)> = HashMap::new();
    for (_, driver, gap, interval) in intervals {
        gaps.insert(*driver, (gap, interval));
    }

    // Tours : tour en cours, dernier tour bouclé, meilleur tour.
    let mut current_lap: HashMap<u32, u32> = HashMap::new();
    let mut last_lap: HashMap<u32, (Ms, f64)> = HashMap::new();
    let mut best_lap: HashMap<u32, f64> = HashMap::new();
    for l in d.laps.iter().filter(|l| l.start <= at) {
        let e = current_lap.entry(l.driver).or_insert(0);
        *e = (*e).max(l.lap);
        if let (Some(end), Some(dur)) = (l.end(), l.duration) {
            if end <= at {
                let last = last_lap.entry(l.driver).or_insert((end, dur));
                if end >= last.0 {
                    *last = (end, dur);
                }
                let best = best_lap.entry(l.driver).or_insert(dur);
                *best = best.min(dur);
            }
        }
    }
    let session_best = best_lap.values().copied().fold(f64::INFINITY, f64::min);

    let has_intervals = !d.intervals.is_empty();
    let leader_best = d
        .drivers
        .iter()
        .filter(|drv| pos.get(&drv.number) == Some(&1))
        .find_map(|drv| best_lap.get(&drv.number).copied());

    let mut cars: Vec<Car> = d
        .drivers
        .iter()
        .enumerate()
        .map(|(i, drv)| {
            let lap = current_lap.get(&drv.number).copied().unwrap_or(0);
            let stint = d
                .stints
                .iter()
                .filter(|s| s.driver == drv.number && s.lap_start <= lap.max(1))
                .filter(|s| s.lap_end.is_none_or(|end| lap <= end) || lap == 0)
                .max_by_key(|s| s.lap_start);
            let driver_pits: Vec<_> = pits.iter().filter(|p| p.driver == drv.number).collect();
            let in_pit = driver_pits
                .last()
                .is_some_and(|p| at <= p.time + (p.lane.unwrap_or(20.0) * 1000.0) as Ms);
            let best = best_lap.get(&drv.number).copied();
            let (gap, interval) = match gaps.get(&drv.number) {
                Some((g, i)) if has_intervals => (gap_text(g), gap_text(i)),
                _ => match (best, leader_best) {
                    // Essais / qualifs : écart au meilleur tour du leader.
                    (Some(b), Some(l)) if b > l => (format!("+{:.3}", b - l), String::new()),
                    _ => (String::new(), String::new()),
                },
            };
            Car {
                number: drv.number,
                code: drv.code.clone(),
                name: drv.name.clone(),
                team: drv.team.clone(),
                colour: drv.colour.clone(),
                position: pos.get(&drv.number).copied().unwrap_or(100 + i as u32),
                gap,
                interval,
                last_lap: last_lap.get(&drv.number).map(|l| l.1),
                best_lap: best,
                fastest: best.is_some_and(|b| b == session_best),
                lap,
                compound: stint
                    .map(|s| s.compound.clone())
                    .filter(|c| !c.is_empty() && c != "UNKNOWN"),
                tyre_age: stint.map(|s| s.age_at_start + lap.saturating_sub(s.lap_start)),
                pits: driver_pits.len() as u32,
                in_pit,
            }
        })
        .collect();
    cars.sort_by_key(|c| c.position);
    for (i, c) in cars.iter_mut().enumerate() {
        if c.position >= 100 {
            c.position = i as u32 + 1;
        }
    }

    let mut track_status = TrackStatus::Green;
    for (_, m) in rc {
        let msg = m.message.to_uppercase();
        match (m.category.as_str(), m.flag.as_deref()) {
            ("SafetyCar", _) if msg.contains("VIRTUAL") && msg.contains("DEPLOYED") => {
                track_status = TrackStatus::VirtualSafetyCar
            }
            ("SafetyCar", _) if msg.contains("DEPLOYED") => track_status = TrackStatus::SafetyCar,
            (_, Some("RED")) => track_status = TrackStatus::Red,
            (_, Some("CHEQUERED")) => track_status = TrackStatus::Chequered,
            (_, Some("GREEN")) if track_status != TrackStatus::Chequered => {
                track_status = TrackStatus::Green
            }
            // Fin de voiture de sécurité / drapeau jaune général : « TRACK CLEAR ».
            (_, Some("CLEAR"))
                if msg.contains("TRACK CLEAR") && track_status != TrackStatus::Chequered =>
            {
                track_status = TrackStatus::Green
            }
            (_, Some("DOUBLE YELLOW" | "YELLOW"))
                if track_status == TrackStatus::Green
                    && msg.contains("TRACK")
                    && !msg.contains("SECTOR") =>
            {
                track_status = TrackStatus::Yellow
            }
            _ => {}
        }
    }

    let total_laps = d
        .laps
        .iter()
        .map(|l| l.lap)
        .max()
        .filter(|_| frame.mode == Mode::Replay);
    let leader_lap = cars.first().map(|c| c.lap).unwrap_or(0);

    Snapshot {
        mode: frame.mode,
        session: d
            .session
            .clone()
            .unwrap_or_else(|| f1x_protocol::SessionSummary {
                session_key: 0,
                session_name: String::new(),
                session_type: String::new(),
                location: String::new(),
                country: String::new(),
                circuit: String::new(),
                date_start: String::new(),
                date_end: String::new(),
                year: 0,
            }),
        clock: iso(at),
        progress: frame.progress,
        speed: frame.speed,
        paused: frame.paused,
        lap: leader_lap,
        total_laps,
        track_status,
        cars,
        weather: upto(&d.weather, at, |w| w.0).last().map(|w| w.1.clone()),
        race_control: rc.iter().rev().take(12).map(|m| m.1.clone()).collect(),
        finished: frame.finished,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dataset() -> Dataset {
        let mut d = Dataset::default();
        d.add_drivers(&json!([
            {"driver_number": 1, "name_acronym": "NOR", "full_name": "Lando NORRIS", "team_name": "McLaren", "team_colour": "F47600"},
            {"driver_number": 63, "name_acronym": "RUS", "full_name": "George RUSSELL", "team_name": "Mercedes", "team_colour": "27F4D2"}
        ]));
        d.add_positions(&json!([
            {"date": "2026-09-26T11:00:00+00:00", "driver_number": 63, "position": 1},
            {"date": "2026-09-26T11:00:00+00:00", "driver_number": 1, "position": 2},
            {"date": "2026-09-26T11:05:00+00:00", "driver_number": 1, "position": 1},
            {"date": "2026-09-26T11:05:00+00:00", "driver_number": 63, "position": 2}
        ]));
        d.add_intervals(&json!([
            {"date": "2026-09-26T11:04:00+00:00", "driver_number": 1, "gap_to_leader": 1.5, "interval": 1.5},
            {"date": "2026-09-26T11:06:00+00:00", "driver_number": 63, "gap_to_leader": "+1 LAP", "interval": 0.8}
        ]));
        d.add_laps(&json!([
            {"driver_number": 1, "lap_number": 1, "date_start": "2026-09-26T11:01:00+00:00", "lap_duration": 100.0},
            {"driver_number": 1, "lap_number": 2, "date_start": "2026-09-26T11:02:40+00:00", "lap_duration": 95.5}
        ]));
        d.set_stints(&json!([{"driver_number": 1, "lap_start": 1, "lap_end": 20, "compound": "SOFT", "tyre_age_at_start": 3}]));
        d.add_race_control(&json!([
            {"date": "2026-09-26T11:03:00+00:00", "category": "SafetyCar", "message": "SAFETY CAR DEPLOYED", "flag": null, "lap_number": 2},
            {"date": "2026-09-26T11:20:00+00:00", "category": "Flag", "message": "TRACK CLEAR", "flag": "CLEAR", "lap_number": 5}
        ]));
        d
    }

    fn frame() -> Frame {
        Frame {
            mode: Mode::Replay,
            speed: 1,
            paused: false,
            progress: 0.0,
            finished: false,
        }
    }

    #[test]
    fn computes_state_at_time() {
        let d = dataset();
        let early = snapshot(
            &d,
            crate::openf1::parse_date("2026-09-26T11:01:30+00:00").unwrap(),
            frame(),
        );
        assert_eq!(early.cars[0].code, "RUS");
        assert_eq!(early.track_status, TrackStatus::Green);

        let later = snapshot(
            &d,
            crate::openf1::parse_date("2026-09-26T11:10:00+00:00").unwrap(),
            frame(),
        );
        assert_eq!(later.cars[0].code, "NOR");
        assert_eq!(later.cars[0].lap, 2);
        assert_eq!(later.cars[0].best_lap, Some(95.5));
        assert!(later.cars[0].fastest);
        assert_eq!(later.cars[0].compound.as_deref(), Some("SOFT"));
        assert_eq!(later.cars[0].tyre_age, Some(4));
        assert_eq!(later.cars[1].gap, "+1 TOUR");
        assert_eq!(later.track_status, TrackStatus::SafetyCar);
        assert_eq!(later.total_laps, Some(2));

        let after_sc = snapshot(
            &d,
            crate::openf1::parse_date("2026-09-26T11:21:00+00:00").unwrap(),
            frame(),
        );
        assert_eq!(after_sc.track_status, TrackStatus::Green);
    }

    #[test]
    fn time_range_skips_pre_race() {
        let d = dataset();
        let (start, end) = time_range(&d);
        assert_eq!(
            start,
            crate::openf1::parse_date("2026-09-26T10:59:00+00:00").unwrap()
        );
        assert!(end > start);
    }
}
