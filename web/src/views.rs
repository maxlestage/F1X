//! Server-rendered, mobile-first pages.
//!
//! Layout rule: everything stacks vertically. No tables, no carousels, nothing wider than
//! the viewport — long content wraps instead of scrolling sideways.

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::api::{
    Constructor, ConstructorStanding, Driver, DriverStanding, QualifyingResult, Race, RaceResult,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    Calendar,
    Drivers,
    Teams,
}

pub fn layout(title: &str, tab: Option<Tab>, body: Markup) -> Markup {
    let full_title = if title.is_empty() {
        "F1X — La Formule 1 dans ta poche".to_string()
    } else {
        format!("{title} · F1X")
    };
    html! {
        (DOCTYPE)
        html lang="fr" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                meta name="theme-color" content="#0b0b10";
                meta name="description" content="Calendrier, résultats et classements de Formule 1, pensés pour le mobile.";
                meta name="apple-mobile-web-app-capable" content="yes";
                meta name="apple-mobile-web-app-status-bar-style" content="black-translucent";
                title { (full_title) }
                link rel="manifest" href="/manifest.webmanifest";
                link rel="icon" href="/static/icon.svg" type="image/svg+xml";
                link rel="apple-touch-icon" href="/static/icon.svg";
                link rel="stylesheet" href=(concat!("/static/app.css?v=", env!("CARGO_PKG_VERSION")));
            }
            body {
                header.topbar {
                    a.brand href="/" aria-label="F1X — accueil" {
                        span.brand-mark { "F1" } span.brand-x { "X" }
                    }
                    @if !title.is_empty() { h1.topbar-title { (title) } }
                }
                main.page { (body) }
                (nav(tab))
                script src=(concat!("/static/app.js?v=", env!("CARGO_PKG_VERSION"))) defer {}
            }
        }
    }
}

fn nav(active: Option<Tab>) -> Markup {
    let items = [
        (Tab::Home, "/", "Accueil", ICON_HOME),
        (Tab::Calendar, "/calendrier", "Calendrier", ICON_CAL),
        (Tab::Drivers, "/pilotes", "Pilotes", ICON_HELMET),
        (Tab::Teams, "/ecuries", "Écuries", ICON_TEAM),
    ];
    html! {
        nav.tabbar aria-label="Navigation principale" {
            @for (tab, href, label, icon) in items {
                a.tab href=(href) aria-current=[(active == Some(tab)).then_some("page")] {
                    (PreEscaped(icon))
                    span { (label) }
                }
            }
        }
    }
}

// ---------- Pages ----------

pub struct HomeData {
    pub next: Option<Race>,
    pub last: Option<Race>,
    pub drivers: Vec<DriverStanding>,
    pub teams: Vec<ConstructorStanding>,
    pub season: String,
}

pub fn home(d: &HomeData) -> Markup {
    let body = html! {
        @if let Some(race) = &d.next {
            section.card.hero {
                p.eyebrow { "Prochain Grand Prix · Manche " (race.round) }
                h2.hero-title { (flag_country(&race.circuit.location.country)) " " (race.race_name) }
                p.muted { (race.circuit.circuit_name) " — " (race.circuit.location.locality) }
                div.countdown data-countdown=(race.start_iso()) {
                    (countdown_cell("j", "jours")) (countdown_cell("h", "heures"))
                    (countdown_cell("m", "min")) (countdown_cell("s", "sec"))
                }
                (sessions_list(race))
                a.btn href={ "/course/" (race.round) } { "Voir le Grand Prix" }
            }
        } @else {
            section.card {
                h2 { "Saison " (d.season) " terminée" }
                p.muted { "Rendez-vous la saison prochaine !" }
            }
        }

        @if let Some(race) = &d.last {
            section.card {
                div.card-head {
                    h2 { "Dernier résultat" }
                    a.link href={ "/course/" (race.round) } { "Détails" }
                }
                p.muted { (flag_country(&race.circuit.location.country)) " " (race.race_name) }
                @if let Some(results) = &race.results {
                    ol.podium {
                        @for r in results.iter().take(3) {
                            li.podium-step style=(team_style(&r.constructor)) {
                                span.podium-pos { (r.position) }
                                span.podium-name { (r.driver.family_name) }
                                span.podium-team { (r.constructor.name) }
                            }
                        }
                    }
                }
            }
        }

        @if !d.drivers.is_empty() {
            section.card {
                div.card-head {
                    h2 { "Pilotes" }
                    a.link href="/pilotes" { "Tout voir" }
                }
                ol.rows { @for s in d.drivers.iter().take(5) { (driver_standing_row(s)) } }
            }
        }

        @if !d.teams.is_empty() {
            section.card {
                div.card-head {
                    h2 { "Écuries" }
                    a.link href="/ecuries" { "Tout voir" }
                }
                ol.rows { @for s in d.teams.iter().take(3) { (team_standing_row(s)) } }
            }
        }
    };
    layout("", Some(Tab::Home), body)
}

pub fn calendar(races: &[Race], now: DateTime<Utc>) -> Markup {
    let next_round = next_race(races, now).map(|r| r.round.clone());
    let season = races.first().map(|r| r.season.as_str()).unwrap_or("");
    let body = html! {
        p.section-intro { "Saison " (season) " · " (races.len()) " Grands Prix" }
        ol.race-list {
            @for race in races {
                @let done = race_is_over(race, now);
                @let is_next = next_round.as_deref() == Some(race.round.as_str());
                li {
                    a.race-item.done[done].next[is_next] href={ "/course/" (race.round) } {
                        span.race-round { "R" (race.round) }
                        span.race-main {
                            span.race-name { (flag_country(&race.circuit.location.country)) " " (race.race_name) }
                            span.race-meta {
                                (local_time(&race.start_iso(), "date"))
                                @if race.is_sprint_weekend() { " · " span.tag { "Sprint" } }
                            }
                        }
                        @if is_next { span.badge.badge-live { "Prochain" } }
                        @else if done { span.badge { "Terminé" } }
                    }
                }
            }
        }
    };
    layout("Calendrier", Some(Tab::Calendar), body)
}

pub struct RaceData {
    pub race: Race,
    pub results: Vec<RaceResult>,
    pub sprint: Vec<RaceResult>,
    pub qualifying: Vec<QualifyingResult>,
    pub total_rounds: usize,
}

pub fn race(d: &RaceData, now: DateTime<Utc>) -> Markup {
    let race = &d.race;
    let round = race.round_num();
    let over = race_is_over(race, now);
    let body = html! {
        section.card.hero {
            p.eyebrow { "Manche " (race.round) " / " (d.total_rounds) }
            h2.hero-title { (flag_country(&race.circuit.location.country)) " " (race.race_name) }
            p.muted { (race.circuit.circuit_name) }
            p.muted { (race.circuit.location.locality) ", " (race.circuit.location.country) }
            @if !over {
                div.countdown data-countdown=(race.start_iso()) {
                    (countdown_cell("j", "jours")) (countdown_cell("h", "heures"))
                    (countdown_cell("m", "min")) (countdown_cell("s", "sec"))
                }
            }
        }

        section.card {
            h2 { "Programme" }
            (sessions_list(race))
        }

        @if !d.results.is_empty() {
            section.card {
                h2 { "Course" }
                ol.rows { @for r in &d.results { (result_row(r)) } }
            }
        }

        @if !d.sprint.is_empty() {
            section.card {
                h2 { "Sprint" }
                ol.rows { @for r in &d.sprint { (result_row(r)) } }
            }
        }

        @if !d.qualifying.is_empty() {
            section.card {
                h2 { "Qualifications" }
                ol.rows { @for q in &d.qualifying { (qualifying_row(q)) } }
            }
        }

        @if over && d.results.is_empty() {
            section.card { p.muted { "Les résultats ne sont pas encore disponibles." } }
        }

        nav.pager aria-label="Grands Prix" {
            @if round > 1 {
                a.btn.btn-ghost href={ "/course/" (round - 1) } { "← Précédent" }
            } @else { span {} }
            @if (round as usize) < d.total_rounds {
                a.btn.btn-ghost href={ "/course/" (round + 1) } { "Suivant →" }
            }
        }
    };
    layout(&race.race_name, Some(Tab::Calendar), body)
}

pub fn drivers(standings: &[DriverStanding]) -> Markup {
    let body = html! {
        @if standings.is_empty() {
            section.card { p.muted { "Le classement n'est pas encore disponible." } }
        } @else {
            ol.rows.rows-card { @for s in standings { (driver_standing_row(s)) } }
        }
    };
    layout("Classement pilotes", Some(Tab::Drivers), body)
}

pub fn teams(standings: &[ConstructorStanding]) -> Markup {
    let leader: f64 = standings
        .first()
        .and_then(|s| s.points.parse().ok())
        .unwrap_or(0.0);
    let body = html! {
        @if standings.is_empty() {
            section.card { p.muted { "Le classement n'est pas encore disponible." } }
        } @else {
            ol.rows.rows-card {
                @for s in standings {
                    @let pts: f64 = s.points.parse().unwrap_or(0.0);
                    @let pct = if leader > 0.0 { (pts / leader * 100.0).clamp(0.0, 100.0) } else { 0.0 };
                    li.row style=(team_style(&s.constructor)) {
                        span.pos { (s.position.as_deref().unwrap_or(&s.position_text)) }
                        span.row-main {
                            span.row-title { (s.constructor.name) }
                            span.bar { span.bar-fill style={ "width:" (format!("{pct:.1}")) "%" } {} }
                            span.row-sub { (s.wins) " victoire" @if s.wins != "1" && s.wins != "0" { "s" } }
                        }
                        span.pts { (s.points) small { " pts" } }
                    }
                }
            }
        }
    };
    layout("Classement écuries", Some(Tab::Teams), body)
}

pub fn driver(standing: Option<&DriverStanding>, races: &[Race]) -> Markup {
    let d: Option<&Driver> = standing.map(|s| &s.driver).or_else(|| {
        races
            .iter()
            .find_map(|r| r.results.as_ref()?.first().map(|x| &x.driver))
    });
    let team = standing.and_then(|s| s.constructors.last()).or_else(|| {
        races
            .iter()
            .rev()
            .find_map(|r| r.results.as_ref()?.first().map(|x| &x.constructor))
    });
    let name = d.map(|d| d.full_name()).unwrap_or_else(|| "Pilote".into());
    let body = html! {
        section.card.hero style=[team.map(team_style)] {
            p.eyebrow {
                @if let Some(n) = d.and_then(|d| d.permanent_number.as_deref()) { "#" (n) " · " }
                @if let Some(t) = team { (t.name) }
            }
            h2.hero-title { (d.map(|d| flag_nationality(d.nationality.as_deref().unwrap_or(""))).unwrap_or("")) " " (name) }
            @if let Some(s) = standing {
                dl.stats {
                    div { dt { "Position" } dd { (s.position.as_deref().unwrap_or(&s.position_text)) } }
                    div { dt { "Points" } dd { (s.points) } }
                    div { dt { "Victoires" } dd { (s.wins) } }
                }
            }
        }
        @if !races.is_empty() {
            section.card {
                h2 { "Saison" }
                ol.rows {
                    @for race in races.iter().rev() {
                        @if let Some(r) = race.results.as_ref().and_then(|v| v.first()) {
                            li.row {
                                span.pos.pos-sm { (r.position_text) }
                                a.row-main href={ "/course/" (race.round) } {
                                    span.row-title { (flag_country(&race.circuit.location.country)) " " (race.race_name) }
                                    span.row-sub {
                                        "Départ P" (r.grid.as_deref().unwrap_or("-"))
                                        @let outcome = r.outcome();
                                        @if !outcome.is_empty() { " · " (outcome) }
                                    }
                                }
                                span.pts { "+" (r.points) }
                            }
                        }
                    }
                }
            }
        }
    };
    layout(&name, Some(Tab::Drivers), body)
}

pub fn error(message: &str) -> Markup {
    let body = html! {
        section.card {
            h2 { "Drapeau rouge 🚩" }
            p.muted { (message) }
            a.btn href="" { "Réessayer" }
        }
    };
    layout("Oups", None, body)
}

pub fn not_found() -> Markup {
    let body = html! {
        section.card {
            h2 { "Hors piste" }
            p.muted { "Cette page n'existe pas." }
            a.btn href="/" { "Retour au stand" }
        }
    };
    layout("Page introuvable", None, body)
}

// ---------- Components ----------

fn countdown_cell(unit: &str, label: &str) -> Markup {
    html! {
        div.cd-cell { span.cd-num data-unit=(unit) { "–" } span.cd-label { (label) } }
    }
}

fn sessions_list(race: &Race) -> Markup {
    html! {
        ul.sessions {
            @for (name, at) in race.sessions() {
                li.session.session-race[name == "Course"] {
                    span.session-name { (name) }
                    span.session-time { (local_time(&at, "datetime")) }
                }
            }
        }
    }
}

fn driver_standing_row(s: &DriverStanding) -> Markup {
    let team = s.constructors.last();
    html! {
        li.row style=[team.map(team_style)] {
            span.pos { (s.position.as_deref().unwrap_or(&s.position_text)) }
            a.row-main href={ "/pilote/" (s.driver.driver_id) } {
                span.row-title {
                    (flag_nationality(s.driver.nationality.as_deref().unwrap_or(""))) " "
                    (s.driver.given_name) " " strong { (s.driver.family_name) }
                }
                span.row-sub {
                    @if let Some(t) = team { (t.name) }
                    @if s.wins != "0" { " · " (s.wins) " V" }
                }
            }
            span.pts { (s.points) small { " pts" } }
        }
    }
}

fn team_standing_row(s: &ConstructorStanding) -> Markup {
    html! {
        li.row style=(team_style(&s.constructor)) {
            span.pos { (s.position.as_deref().unwrap_or(&s.position_text)) }
            span.row-main {
                span.row-title { (s.constructor.name) }
                @if s.wins != "0" { span.row-sub { (s.wins) " victoire" @if s.wins != "1" { "s" } } }
            }
            span.pts { (s.points) small { " pts" } }
        }
    }
}

fn result_row(r: &RaceResult) -> Markup {
    let points = r.points.parse::<f64>().unwrap_or(0.0);
    html! {
        li.row style=(team_style(&r.constructor)) {
            span.pos { (r.position_text) }
            a.row-main href={ "/pilote/" (r.driver.driver_id) } {
                span.row-title {
                    (r.driver.given_name) " " strong { (r.driver.family_name) }
                    @if r.has_fastest_lap() { " " span.tag.tag-purple title="Meilleur tour" { "⏱" } }
                }
                span.row-sub {
                    (r.constructor.name)
                    @let outcome = r.outcome();
                    @if !outcome.is_empty() { " · " (outcome) }
                }
            }
            @if points > 0.0 { span.pts { "+" (r.points) } }
        }
    }
}

fn qualifying_row(q: &QualifyingResult) -> Markup {
    html! {
        li.row style=(team_style(&q.constructor)) {
            span.pos { (q.position) }
            a.row-main href={ "/pilote/" (q.driver.driver_id) } {
                span.row-title { (q.driver.given_name) " " strong { (q.driver.family_name) } }
                span.row-sub { (q.constructor.name) }
            }
            @if let Some((seg, t)) = q.best() {
                span.pts.pts-time { (t) small { " " (seg) } }
            }
        }
    }
}

/// A `<time>` element rendered in UTC on the server and converted to the visitor's
/// local timezone by `app.js`.
fn local_time(iso: &str, style: &str) -> Markup {
    let fallback = DateTime::parse_from_rfc3339(iso)
        .map(|dt| format_fr(dt.with_timezone(&Utc), style == "datetime"))
        .unwrap_or_else(|_| iso.to_string());
    html! { time datetime=(iso) data-local=(style) { (fallback) } }
}

fn format_fr(dt: DateTime<Utc>, with_time: bool) -> String {
    const DAYS: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
    const MONTHS: [&str; 12] = [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
        "déc.",
    ];
    let day = DAYS[dt.weekday().num_days_from_monday() as usize];
    let month = MONTHS[dt.month0() as usize];
    if with_time {
        format!(
            "{day} {} {month} · {:02}:{:02} UTC",
            dt.day(),
            dt.hour(),
            dt.minute()
        )
    } else {
        format!("{day} {} {month}", dt.day())
    }
}

// ---------- Helpers ----------

pub fn race_start(race: &Race) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&race.start_iso())
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// A race is considered over ~2h after lights out.
pub fn race_is_over(race: &Race, now: DateTime<Utc>) -> bool {
    race_start(race).is_some_and(|s| now > s + Duration::hours(2))
}

pub fn next_race(races: &[Race], now: DateTime<Utc>) -> Option<&Race> {
    races.iter().find(|r| !race_is_over(r, now))
}

fn team_style(c: &Constructor) -> String {
    format!("--team:{}", team_color(&c.constructor_id))
}

pub fn team_color(constructor_id: &str) -> &'static str {
    match constructor_id {
        "mercedes" => "#27F4D2",
        "ferrari" => "#E8002D",
        "red_bull" => "#3671C6",
        "mclaren" => "#FF8000",
        "aston_martin" => "#229971",
        "alpine" => "#FF87BC",
        "williams" => "#64C4FF",
        "rb" => "#6692FF",
        "haas" => "#B6BABD",
        "sauber" => "#52E252",
        "audi" => "#F50537",
        "cadillac" => "#C9A96E",
        _ => "#8A8A99",
    }
}

fn flag_country(country: &str) -> &'static str {
    match country {
        "Australia" => "🇦🇺",
        "Austria" => "🇦🇹",
        "Azerbaijan" => "🇦🇿",
        "Bahrain" => "🇧🇭",
        "Belgium" => "🇧🇪",
        "Brazil" => "🇧🇷",
        "Canada" => "🇨🇦",
        "China" => "🇨🇳",
        "France" => "🇫🇷",
        "Germany" => "🇩🇪",
        "Hungary" => "🇭🇺",
        "Italy" => "🇮🇹",
        "Japan" => "🇯🇵",
        "Malaysia" => "🇲🇾",
        "Mexico" => "🇲🇽",
        "Monaco" => "🇲🇨",
        "Netherlands" => "🇳🇱",
        "Portugal" => "🇵🇹",
        "Qatar" => "🇶🇦",
        "Saudi Arabia" => "🇸🇦",
        "Singapore" => "🇸🇬",
        "Spain" => "🇪🇸",
        "UAE" | "United Arab Emirates" => "🇦🇪",
        "UK" | "United Kingdom" => "🇬🇧",
        "USA" | "United States" => "🇺🇸",
        _ => "🏁",
    }
}

fn flag_nationality(nationality: &str) -> &'static str {
    match nationality {
        "American" => "🇺🇸",
        "Argentine" | "Argentinian" => "🇦🇷",
        "Australian" => "🇦🇺",
        "Austrian" => "🇦🇹",
        "Belgian" => "🇧🇪",
        "Brazilian" => "🇧🇷",
        "British" => "🇬🇧",
        "Canadian" => "🇨🇦",
        "Chinese" => "🇨🇳",
        "Danish" => "🇩🇰",
        "Dutch" => "🇳🇱",
        "Finnish" => "🇫🇮",
        "French" => "🇫🇷",
        "German" => "🇩🇪",
        "Italian" => "🇮🇹",
        "Japanese" => "🇯🇵",
        "Mexican" => "🇲🇽",
        "Monegasque" => "🇲🇨",
        "New Zealander" => "🇳🇿",
        "Polish" => "🇵🇱",
        "Spanish" => "🇪🇸",
        "Swiss" => "🇨🇭",
        "Thai" => "🇹🇭",
        _ => "🏳️",
    }
}

const ICON_HOME: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z"/></svg>"#;
const ICON_CAL: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 3v3M17 3v3M4 9h16M5 5h14a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z"/></svg>"#;
const ICON_HELMET: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 15a9 9 0 0 1 18-2v4a1 1 0 0 1-1 1H9l-3 2H4a1 1 0 0 1-1-1zM12 11h8"/></svg>"#;
const ICON_TEAM: &str =
    r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 21V4M5 4h11l-2 4 2 4H5"/></svg>"#;
