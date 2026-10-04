//! Résultats tout frais : Jolpica publie les résultats quelques heures après la course ;
//! en attendant, le serveur les reconstitue au format Ergast depuis OpenF1 (classement
//! officiel de la séance), pour que l'app et le site soient à jour dès l'arrivée.

use serde_json::{Value, json};

use crate::api::F1Api;
use crate::openf1::OpenF1;

fn races(v: &Value) -> Vec<Value> {
    v.pointer("/MRData/RaceTable/Races")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn set_races(v: &mut Value, list: Vec<Value>) {
    if let Some(table) = v.pointer_mut("/MRData/RaceTable") {
        table["Races"] = Value::Array(list);
    }
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// Course terminée (départ + 3 h) d'après le calendrier.
fn is_over(race: &Value) -> bool {
    let date = s(race, "date");
    let time = race
        .get("time")
        .and_then(Value::as_str)
        .unwrap_or("12:00:00Z");
    chrono::DateTime::parse_from_rfc3339(&format!("{date}T{time}"))
        .map(|d| chrono::Utc::now() > d.with_timezone(&chrono::Utc) + chrono::Duration::hours(3))
        .unwrap_or(false)
}

/// « 6434.808 » → « 1:47:14.808 ».
fn race_time(secs: f64) -> String {
    let ms = (secs * 1000.0).round() as i64;
    let (h, m, s, r) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    format!("{h}:{m:02}:{s:02}.{r:03}")
}

/// Course `round` de `season` au format Ergast, reconstituée depuis OpenF1.
pub async fn race(api: &F1Api, of1: &OpenF1, season: &str, round: &str) -> Option<Value> {
    let schedule = api.page(&format!("{season}.json"), 100, 0).await?;
    let mut race = races(&schedule)
        .into_iter()
        .find(|r| s(r, "round") == round)?;
    if !is_over(&race) {
        return None;
    }
    let year: u32 = s(&race, "season").parse().ok()?;
    let key = of1.race_session(year, s(&race, "date")).await.ok()??;
    let q = format!("session_key={key}");
    let results = of1.relay("session_result", &q).await.ok()?;
    let drivers = of1.relay("drivers", &q).await.ok()?;
    let standings = api
        .page(&format!("{season}/driverStandings.json"), 100, 0)
        .await
        .unwrap_or(Value::Null);
    let rows = standings
        .pointer("/MRData/StandingsTable/StandingsLists/0/DriverStandings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let list = results.as_array()?;
    if list.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for r in list {
        let n = r.get("driver_number").and_then(Value::as_u64)?;
        let d = drivers
            .as_array()?
            .iter()
            .find(|d| d.get("driver_number").and_then(Value::as_u64) == Some(n));
        let code = d.map(|d| s(d, "name_acronym")).unwrap_or_default();
        // Pilote et écurie au format Jolpica (d'après le classement), sinon d'après OpenF1.
        let known = rows.iter().find(|row| {
            let drv = row.get("Driver").unwrap_or(&Value::Null);
            s(drv, "code") == code || s(drv, "permanentNumber") == n.to_string()
        });
        let driver = known
            .and_then(|k| k.get("Driver").cloned())
            .unwrap_or_else(|| {
                json!({
                    "driverId": code.to_lowercase(),
                    "permanentNumber": n.to_string(),
                    "code": code,
                    "givenName": d.map(|d| s(d, "first_name")).unwrap_or_default(),
                    "familyName": d.map(|d| s(d, "last_name")).unwrap_or_default(),
                })
            });
        let constructor = known
            .and_then(|k| k.get("Constructors")?.as_array()?.last().cloned())
            .unwrap_or_else(|| {
                let team = d.map(|d| s(d, "team_name")).unwrap_or_default();
                json!({ "constructorId": team.to_lowercase().replace(' ', "_"), "name": team })
            });
        let flag = |k: &str| r.get(k).and_then(Value::as_bool).unwrap_or(false);
        let pos = r.get("position").and_then(Value::as_u64);
        let (position_text, status) = if flag("dsq") {
            ("D".to_string(), "Disqualified")
        } else if flag("dns") {
            ("W".to_string(), "Did not start")
        } else if flag("dnf") {
            ("R".to_string(), "Retired")
        } else {
            (pos.map(|p| p.to_string()).unwrap_or_default(), "Finished")
        };
        let time = match r.get("gap_to_leader") {
            Some(Value::Number(g)) if g.as_f64() == Some(0.0) => r
                .get("duration")
                .and_then(Value::as_f64)
                .map(|t| json!({ "time": race_time(t) })),
            Some(Value::Number(g)) => g.as_f64().map(|g| json!({ "time": format!("+{g:.3}") })),
            Some(Value::String(t)) => Some(json!({ "time": t })),
            _ => None,
        };
        let mut row = json!({
            "number": n.to_string(),
            "position": pos.unwrap_or(out.len() as u64 + 1).to_string(),
            "positionText": position_text,
            "points": r.get("points").and_then(Value::as_f64).map(|p| {
                if p.fract() == 0.0 { format!("{p:.0}") } else { p.to_string() }
            }).unwrap_or_else(|| "0".into()),
            "Driver": driver,
            "Constructor": constructor,
            "laps": r.get("number_of_laps").map(|l| l.to_string()).unwrap_or_default(),
            "status": status,
        });
        if let Some(t) = time.filter(|_| status == "Finished") {
            row["Time"] = t;
        }
        out.push(row);
    }
    race["Results"] = Value::Array(out);
    Some(race)
}

/// Complète une réponse Jolpica de résultats encore vide ou en retard.
pub async fn patch(api: &F1Api, of1: &OpenF1, path: &str, value: &mut Value) {
    let parts: Vec<&str> = path.trim_end_matches(".json").split('/').collect();
    match parts.as_slice() {
        // Résultats d'une course.
        [season, round, "results"] if round.chars().all(|c| c.is_ascii_digit()) => {
            if races(value).is_empty() {
                if let Some(r) = race(api, of1, season, round).await {
                    set_races(value, vec![r]);
                }
            }
        }
        // Dernière course.
        ["current", "last", "results"] => {
            let Some(schedule) = api.page("current.json", 100, 0).await else {
                return;
            };
            let Some(last) = races(&schedule).into_iter().filter(is_over).next_back() else {
                return;
            };
            let have = races(value).first().map(|r| s(r, "round").to_string());
            if have.as_deref() != Some(s(&last, "round")) {
                if let Some(r) = race(api, of1, "current", s(&last, "round")).await {
                    set_races(value, vec![r]);
                }
            }
        }
        // Vainqueurs de la saison.
        [season, "results", "1"]
            if season.chars().all(|c| c.is_ascii_digit()) || *season == "current" =>
        {
            let Some(schedule) = api.page(&format!("{season}.json"), 100, 0).await else {
                return;
            };
            let mut list = races(value);
            for r in races(&schedule).iter().filter(|r| is_over(r)) {
                let round = s(r, "round");
                if list.iter().any(|x| s(x, "round") == round) {
                    continue;
                }
                if let Some(mut full) = race(api, of1, season, round).await {
                    if let Some(res) = full.get_mut("Results").and_then(Value::as_array_mut) {
                        res.truncate(1);
                    }
                    list.push(full);
                }
            }
            set_races(value, list);
        }
        // Classements : ajoute les points des courses que Jolpica n'a pas encore intégrées.
        [season, kind @ ("driverStandings" | "constructorStandings")] => {
            let drivers = *kind == "driverStandings";
            let Some(list) = value
                .pointer("/MRData/StandingsTable/StandingsLists/0")
                .cloned()
            else {
                return;
            };
            let done: u32 = s(&list, "round").parse().unwrap_or(0);
            let Some(schedule) = api.page(&format!("{season}.json"), 100, 0).await else {
                return;
            };
            let missing: Vec<String> = races(&schedule)
                .iter()
                .filter(|r| is_over(r) && s(r, "round").parse::<u32>().is_ok_and(|n| n > done))
                .map(|r| s(r, "round").to_string())
                .collect();
            if missing.is_empty() {
                return;
            }
            let table = if drivers {
                "DriverStandings"
            } else {
                "ConstructorStandings"
            };
            let mut rows = list
                .get(table)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut last = done;
            for round in &missing {
                let Some(r) = race(api, of1, season, round).await else {
                    break;
                };
                last = round.parse().unwrap_or(last);
                for res in r
                    .get("Results")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                {
                    let pts: f64 = s(&res, "points").parse().unwrap_or(0.0);
                    let win = s(&res, "positionText") == "1";
                    let id = if drivers {
                        s(res.get("Driver").unwrap_or(&Value::Null), "driverId").to_string()
                    } else {
                        s(
                            res.get("Constructor").unwrap_or(&Value::Null),
                            "constructorId",
                        )
                        .to_string()
                    };
                    let row = rows.iter_mut().find(|row| {
                        let o = row
                            .get(if drivers { "Driver" } else { "Constructor" })
                            .unwrap_or(&Value::Null);
                        s(o, if drivers { "driverId" } else { "constructorId" }) == id
                    });
                    if let Some(row) = row {
                        let p: f64 = s(row, "points").parse().unwrap_or(0.0) + pts;
                        let w: u32 = s(row, "wins").parse::<u32>().unwrap_or(0) + u32::from(win);
                        row["points"] = Value::String(if p.fract() == 0.0 {
                            format!("{p:.0}")
                        } else {
                            p.to_string()
                        });
                        row["wins"] = Value::String(w.to_string());
                    }
                }
            }
            if last == done {
                return;
            }
            rows.sort_by(|a, b| {
                let pa: f64 = s(a, "points").parse().unwrap_or(0.0);
                let pb: f64 = s(b, "points").parse().unwrap_or(0.0);
                pb.total_cmp(&pa)
            });
            for (i, row) in rows.iter_mut().enumerate() {
                row["position"] = Value::String((i + 1).to_string());
                row["positionText"] = Value::String((i + 1).to_string());
            }
            if let Some(l) = value.pointer_mut("/MRData/StandingsTable/StandingsLists/0") {
                l[table] = Value::Array(rows);
                l["round"] = Value::String(last.to_string());
            }
        }
        _ => {}
    }
}
