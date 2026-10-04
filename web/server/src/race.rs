//! Calcule l'état complet d'une course à un instant donné à partir d'un [`Dataset`].
//!
//! Sans état : on recalcule tout à chaque image (quelques dizaines de milliers
//! d'événements, négligeable), ce qui rend l'avance / le retour rapide triviaux.

use std::collections::HashMap;

use f1x_protocol::{Car, EventKind, Mode, RaceControl, RaceEvent, Snapshot, TrackStatus};
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
    let mut current_lap: HashMap<u32, (u32, Ms)> = HashMap::new();
    // Fin du tour en cours (None : durée inconnue), pour savoir si la voiture est en piste.
    let mut current_end: HashMap<u32, Option<Ms>> = HashMap::new();
    let mut last_lap: HashMap<u32, (Ms, f64, [Option<f64>; 3])> = HashMap::new();
    let mut best_lap: HashMap<u32, f64> = HashMap::new();
    let mut best_sector: HashMap<u32, [f64; 3]> = HashMap::new();
    let mut overall_sector = [f64::INFINITY; 3];
    let mut last_activity: HashMap<u32, Ms> = HashMap::new();
    for l in d.laps.iter().filter(|l| l.start <= at) {
        let e = current_lap.entry(l.driver).or_insert((0, l.start));
        if l.lap >= e.0 {
            *e = (l.lap, l.start);
            current_end.insert(l.driver, l.end());
        }
        let act = last_activity.entry(l.driver).or_insert(l.start);
        *act = (*act).max(l.start);
        if let (Some(end), Some(dur)) = (l.end(), l.duration) {
            if end <= at {
                *act = (*act).max(end);
                let last = last_lap.entry(l.driver).or_insert((end, dur, l.sectors));
                if end >= last.0 {
                    *last = (end, dur, l.sectors);
                }
                let best = best_lap.entry(l.driver).or_insert(dur);
                *best = best.min(dur);
            }
        }
        // Secteurs : visibles une fois le tour bouclé.
        if l.end().is_some_and(|end| end <= at) {
            let pb = best_sector.entry(l.driver).or_insert([f64::INFINITY; 3]);
            for (i, sec) in l.sectors.iter().enumerate() {
                if let Some(v) = sec {
                    pb[i] = pb[i].min(*v);
                    overall_sector[i] = overall_sector[i].min(*v);
                }
            }
        }
    }
    let median_lap = {
        let mut v: Vec<f64> = d
            .laps
            .iter()
            .filter(|l| l.end().is_some_and(|e| e <= at))
            .filter_map(|l| l.duration)
            .collect();
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(95.0)
    };
    let leader_lap_now = current_lap.values().map(|l| l.0).max().unwrap_or(0);
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
            let (lap, lap_start) = current_lap.get(&drv.number).copied().unwrap_or((0, at));
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
                sectors: last_lap.get(&drv.number).map(|l| l.2).unwrap_or([None; 3]),
                sector_flags: {
                    let secs = last_lap.get(&drv.number).map(|l| l.2).unwrap_or([None; 3]);
                    let pb = best_sector
                        .get(&drv.number)
                        .copied()
                        .unwrap_or([f64::INFINITY; 3]);
                    std::array::from_fn(|i| match secs[i] {
                        None => 0,
                        Some(v) if v <= overall_sector[i] => 3,
                        Some(v) if v <= pb[i] => 2,
                        Some(_) => 1,
                    })
                },
                lap_progress: {
                    let reference = last_lap
                        .get(&drv.number)
                        .map(|l| l.1)
                        .unwrap_or(median_lap)
                        .max(1.0);
                    // Essais / qualifications : la voiture n'est sur la carte que pendant un tour
                    // en cours (sinon elle est au stand), au lieu d'attendre sur la ligne.
                    let on_track = has_intervals
                        || match current_end.get(&drv.number).copied().flatten() {
                            Some(end) => at < end,
                            None => ((at - lap_start) as f64 / 1000.0) < reference * 1.5,
                        };
                    (lap > 0 && on_track).then(|| {
                        (((at - lap_start) as f64 / 1000.0) / reference).clamp(0.0, 0.999) as f32
                    })
                },
                stints: d
                    .stints
                    .iter()
                    .filter(|s| s.driver == drv.number && s.lap_start <= lap.max(1))
                    .map(|s| f1x_protocol::StintInfo {
                        compound: s.compound.clone(),
                        from: s.lap_start,
                        to: s.lap_end.unwrap_or(lap).min(lap.max(s.lap_start)),
                    })
                    .collect(),
                // Abandon : plus aucune activité depuis ~3 tours alors que la course continue.
                retired: has_intervals
                    && lap > 0
                    && leader_lap_now >= lap + 2
                    && last_activity
                        .get(&drv.number)
                        .is_some_and(|t| at - t > (median_lap * 3000.0) as Ms),
                top_speed: None,
                best_sectors: best_sector
                    .get(&drv.number)
                    .map(|b| b.map(|v| v.is_finite().then_some(v)))
                    .unwrap_or([None; 3]),
                gained: None,
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

    let (track_status, _) = track_status_at(rc);
    let events = events(d, at, &cars);
    let pit_loss = {
        let mut v: Vec<f64> = d
            .pits
            .iter()
            .filter_map(|p| p.lane)
            .filter(|l| *l > 10.0 && *l < 60.0)
            .collect();
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied()
    };

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
        events,
        pit_loss,
        finished: frame.finished,
        radios: Vec::new(),
        championship: Vec::new(),
        // Arrêts jusqu'ici : temps passé dans la voie des stands.
        pit_times: pits
            .iter()
            .rev()
            .take(20)
            .map(|p| {
                let drv = d.drivers.iter().find(|x| x.number == p.driver);
                f1x_protocol::LivePit {
                    date: iso(p.time),
                    code: drv.map(|x| x.code.clone()).unwrap_or_default(),
                    colour: drv.map(|x| x.colour.clone()).unwrap_or_default(),
                    lap: d
                        .laps
                        .iter()
                        .filter(|l| l.driver == p.driver && l.start <= p.time)
                        .map(|l| l.lap)
                        .max(),
                    duration: p.lane,
                }
            })
            .collect(),
    }
}

/// État de la piste après les messages `rc`, et ses changements successifs (pour la chronologie).
fn track_status_at(rc: &[(Ms, RaceControl)]) -> (TrackStatus, Vec<(Ms, Option<u32>, TrackStatus)>) {
    let mut status = TrackStatus::Green;
    let mut changes = Vec::new();
    for (date, m) in rc {
        let before = status;
        let msg = m.message.to_uppercase();
        match (m.category.as_str(), m.flag.as_deref()) {
            ("SafetyCar", _) if msg.contains("VIRTUAL") && msg.contains("DEPLOYED") => {
                status = TrackStatus::VirtualSafetyCar
            }
            ("SafetyCar", _) if msg.contains("DEPLOYED") => status = TrackStatus::SafetyCar,
            (_, Some("RED")) => status = TrackStatus::Red,
            (_, Some("CHEQUERED")) => status = TrackStatus::Chequered,
            (_, Some("GREEN")) if status != TrackStatus::Chequered => status = TrackStatus::Green,
            // Fin de voiture de sécurité / drapeau jaune général : « TRACK CLEAR ».
            (_, Some("CLEAR"))
                if msg.contains("TRACK CLEAR") && status != TrackStatus::Chequered =>
            {
                status = TrackStatus::Green
            }
            (_, Some("DOUBLE YELLOW" | "YELLOW"))
                if status == TrackStatus::Green
                    && msg.contains("TRACK")
                    && !msg.contains("SECTOR") =>
            {
                status = TrackStatus::Yellow
            }
            _ => {}
        }
        if status != before {
            changes.push((*date, m.lap, status));
        }
    }
    (status, changes)
}

/// Chronologie jusqu'à `at` : dépassements, arrêts, meilleurs tours, drapeaux, pénalités, abandons.
/// Du plus récent au plus ancien, 40 au plus.
fn events(d: &Dataset, at: Ms, cars: &[Car]) -> Vec<RaceEvent> {
    let code = |n: u32| {
        d.drivers
            .iter()
            .find(|x| x.number == n)
            .map(|x| x.code.clone())
            .unwrap_or_else(|| n.to_string())
    };
    let lap_of = |n: u32, t: Ms| {
        d.laps
            .iter()
            .filter(|l| l.driver == n && l.start <= t)
            .map(|l| l.lap)
            .max()
            .unwrap_or(0)
    };
    let leader_lap_at = |t: Ms| {
        d.laps
            .iter()
            .filter(|l| l.start <= t)
            .map(|l| l.lap)
            .max()
            .unwrap_or(0)
    };
    let mut out: Vec<(Ms, RaceEvent)> = Vec::new();
    let pits = upto(&d.pits, at, |p| p.time);
    let near_pit = |n: u32, t: Ms| {
        pits.iter()
            .any(|p| p.driver == n && (t - p.time).abs() < 45_000)
    };

    // Dépassements : une voiture gagne une place et celle qui la cède recule au même instant.
    // Seulement une fois la course lancée (tour 2+) et hors passages aux stands.
    let mut pos: HashMap<u32, u32> = HashMap::new();
    let positions = upto(&d.positions, at, |p| p.0);
    let mut i = 0;
    while i < positions.len() {
        let t = positions[i].0;
        let mut j = i;
        let mut batch = Vec::new();
        while j < positions.len() && positions[j].0 == t {
            batch.push((positions[j].1, positions[j].2));
            j += 1;
        }
        if leader_lap_at(t) >= 2 {
            for (driver, new) in &batch {
                let Some(old) = pos.get(driver).copied() else {
                    continue;
                };
                if *new < old && *new <= 10 && !near_pit(*driver, t) {
                    if let Some((passed, _)) = batch
                        .iter()
                        .find(|(o, p)| *p == new + 1 && pos.get(o) == Some(new))
                    {
                        if !near_pit(*passed, t) {
                            out.push((
                                t,
                                RaceEvent {
                                    date: iso(t),
                                    lap: lap_of(*driver, t),
                                    kind: EventKind::Overtake {
                                        driver: code(*driver),
                                        passed: code(*passed),
                                        position: *new,
                                    },
                                },
                            ));
                        }
                    }
                }
            }
        }
        for (driver, new) in batch {
            pos.insert(driver, new);
        }
        i = j;
    }

    for p in pits {
        out.push((
            p.time,
            RaceEvent {
                date: iso(p.time),
                lap: lap_of(p.driver, p.time),
                kind: EventKind::Pit {
                    driver: code(p.driver),
                    duration: p.lane,
                },
            },
        ));
    }

    // Meilleurs tours successifs de la session.
    let mut done: Vec<(Ms, u32, u32, f64)> = d
        .laps
        .iter()
        .filter_map(|l| Some((l.end()?, l.driver, l.lap, l.duration?)))
        .filter(|l| l.0 <= at && l.2 > 1)
        .collect();
    done.sort_by_key(|l| l.0);
    let mut best = f64::INFINITY;
    for (t, driver, lap, dur) in done {
        if dur < best {
            best = dur;
            out.push((
                t,
                RaceEvent {
                    date: iso(t),
                    lap,
                    kind: EventKind::FastestLap {
                        driver: code(driver),
                        time: dur,
                    },
                },
            ));
        }
    }

    let rc = upto(&d.race_control, at, |p| p.0);
    for (t, lap, status) in track_status_at(rc).1 {
        out.push((
            t,
            RaceEvent {
                date: iso(t),
                lap: lap.unwrap_or(0),
                kind: EventKind::Status { status },
            },
        ));
    }
    for (t, m) in rc {
        let msg = m.message.to_uppercase();
        if msg.contains("PENALTY")
            || msg.contains("INVESTIGATION")
            || msg.contains("NOTED") && msg.contains("INCIDENT")
        {
            out.push((
                *t,
                RaceEvent {
                    date: iso(*t),
                    lap: m.lap.unwrap_or(0),
                    kind: EventKind::Penalty {
                        message: m.message.clone(),
                    },
                },
            ));
        }
    }
    for c in cars.iter().filter(|c| c.retired) {
        let t = d
            .laps
            .iter()
            .filter(|l| l.driver == c.number)
            .filter_map(|l| l.end().or(Some(l.start)))
            .max()
            .unwrap_or(at);
        out.push((
            t,
            RaceEvent {
                date: iso(t),
                lap: c.lap,
                kind: EventKind::Retired {
                    driver: c.code.clone(),
                },
            },
        ));
    }

    out.sort_by_key(|e| std::cmp::Reverse(e.0));
    out.into_iter().take(40).map(|e| e.1).collect()
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
