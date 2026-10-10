//! F1X — frontend 100 % Rust avec active, compilé en WebAssembly.
//!
//! L'application est montée dans `#app` par [`active::mount_to`] ; le routeur d'active suit
//! l'adresse (liens internes, retour arrière) et [`Route::parse`] choisit la page.

mod api;
mod components;
mod gl3d;
mod i18n;
mod live;
mod models;
mod pages;
mod photo;
mod pwa;
mod util;

use active::prelude::*;

/// `season` vaut `"current"` pour la saison en cours, sinon l'année (`"1998"`).
#[derive(Clone, PartialEq, Debug)]
pub enum Route {
    Home,
    Live,
    Season { season: String },
    Race { season: String, round: u32 },
    DriverStandings { season: String },
    TeamStandings { season: String },
    Driver { id: String },
    Team { id: String },
    Circuit { id: String },
    Archives,
    Records,
    Quiz,
    Predict,
    Fantasy,
    News,
    Glossary,
    Compare,
    CompareWith { a: String, b: String },
    AllSeasons,
    AllDrivers,
    AllTeams,
    AllCircuits,
    Data,
    DataYear { year: u32 },
    DataMeeting { key: u32 },
    DataSession { key: u32 },
    // Anciennes adresses (saison en cours), conservées pour les liens existants.
    LegacyCalendar,
    LegacyRace { round: u32 },
    LegacyDrivers,
    LegacyTeams,
    /// Page rendue par le serveur (présentation, pages légales…) : chargée normalement.
    Server,
    NotFound,
}

pub const CURRENT: &str = "current";

/// Premiers segments des adresses que le serveur rend lui-même.
const SERVER_PAGES: &[&str] = &[
    "presentation",
    "mentions-legales",
    "confidentialite",
    "credits",
    "calendar.ics",
    "api",
    "static",
    "pkg",
];

impl Route {
    pub fn season(season: &str) -> Self {
        Self::Season {
            season: season.to_string(),
        }
    }
    pub fn race(season: &str, round: u32) -> Self {
        Self::Race {
            season: season.to_string(),
            round,
        }
    }
    pub fn driver(id: &str) -> Self {
        Self::Driver { id: id.to_string() }
    }
    pub fn team(id: &str) -> Self {
        Self::Team { id: id.to_string() }
    }
    pub fn circuit(id: &str) -> Self {
        Self::Circuit { id: id.to_string() }
    }

    /// La page d'une adresse (`/saison/2024/course/3`).
    pub fn parse(path: &str) -> Self {
        let parts: Vec<String> = path
            .split('/')
            .filter(|p| !p.is_empty())
            .map(|p| {
                if !p.contains('%') {
                    return p.to_string();
                }
                js_sys::decode_uri_component(p)
                    .ok()
                    .and_then(|s| s.as_string())
                    .unwrap_or_else(|| p.to_string())
            })
            .collect();
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        let num = |s: &str, page: fn(u32) -> Route| s.parse().map_or(Route::NotFound, page);
        let s = |v: &str| v.to_string();
        match parts.as_slice() {
            [] => Self::Home,
            ["direct"] => Self::Live,
            ["saison", season] => Self::Season { season: s(season) },
            ["saison", season, "course", round] => match round.parse() {
                Ok(round) => Self::Race {
                    season: s(season),
                    round,
                },
                Err(_) => Self::NotFound,
            },
            ["saison", season, "pilotes"] => Self::DriverStandings { season: s(season) },
            ["saison", season, "ecuries"] => Self::TeamStandings { season: s(season) },
            ["pilote", id] => Self::Driver { id: s(id) },
            ["ecurie", id] => Self::Team { id: s(id) },
            ["circuit", id] => Self::Circuit { id: s(id) },
            ["archives"] => Self::Archives,
            ["archives", "records"] => Self::Records,
            ["archives", "saisons"] => Self::AllSeasons,
            ["archives", "pilotes"] => Self::AllDrivers,
            ["archives", "ecuries"] => Self::AllTeams,
            ["archives", "circuits"] => Self::AllCircuits,
            ["quiz"] => Self::Quiz,
            ["pronostics"] => Self::Predict,
            ["fantasy"] => Self::Fantasy,
            ["actus"] => Self::News,
            ["lexique"] => Self::Glossary,
            ["comparer"] => Self::Compare,
            ["comparer", a, b] => Self::CompareWith { a: s(a), b: s(b) },
            ["donnees"] => Self::Data,
            ["donnees", year] => num(year, |year| Route::DataYear { year }),
            ["donnees", "reunion", key] => num(key, |key| Route::DataMeeting { key }),
            ["donnees", "session", key] => num(key, |key| Route::DataSession { key }),
            ["calendrier"] => Self::LegacyCalendar,
            ["course", round] => num(round, |round| Route::LegacyRace { round }),
            ["pilotes"] => Self::LegacyDrivers,
            ["ecuries"] => Self::LegacyTeams,
            [first, ..] if SERVER_PAGES.contains(first) => Self::Server,
            _ => Self::NotFound,
        }
    }

    /// L'adresse de la page, pour `a().href(…)` ou [`active::navigate`].
    pub fn href(&self) -> String {
        match self {
            Self::Home => "/".into(),
            Self::Live => "/direct".into(),
            Self::Season { season } => format!("/saison/{season}"),
            Self::Race { season, round } => format!("/saison/{season}/course/{round}"),
            Self::DriverStandings { season } => format!("/saison/{season}/pilotes"),
            Self::TeamStandings { season } => format!("/saison/{season}/ecuries"),
            Self::Driver { id } => format!("/pilote/{id}"),
            Self::Team { id } => format!("/ecurie/{id}"),
            Self::Circuit { id } => format!("/circuit/{id}"),
            Self::Archives => "/archives".into(),
            Self::Records => "/archives/records".into(),
            Self::Quiz => "/quiz".into(),
            Self::Predict => "/pronostics".into(),
            Self::Fantasy => "/fantasy".into(),
            Self::News => "/actus".into(),
            Self::Glossary => "/lexique".into(),
            Self::Compare => "/comparer".into(),
            Self::CompareWith { a, b } => format!("/comparer/{a}/{b}"),
            Self::AllSeasons => "/archives/saisons".into(),
            Self::AllDrivers => "/archives/pilotes".into(),
            Self::AllTeams => "/archives/ecuries".into(),
            Self::AllCircuits => "/archives/circuits".into(),
            Self::Data => "/donnees".into(),
            Self::DataYear { year } => format!("/donnees/{year}"),
            Self::DataMeeting { key } => format!("/donnees/reunion/{key}"),
            Self::DataSession { key } => format!("/donnees/session/{key}"),
            Self::LegacyCalendar => "/calendrier".into(),
            Self::LegacyRace { round } => format!("/course/{round}"),
            Self::LegacyDrivers => "/pilotes".into(),
            Self::LegacyTeams => "/ecuries".into(),
            Self::Server | Self::NotFound => "/404".into(),
        }
    }
}

/// Lien interne : `link(Route::Archives, "btn")` = `<a class="btn" href="/archives">`.
/// Le routeur d'active l'ouvre sans recharger la page.
pub fn link(route: Route, class: &'static str) -> Element {
    let a = a().href(route.href());
    if class.is_empty() { a } else { a.class(class) }
}

fn switch(route: Route) -> Node {
    use pages::*;
    match route {
        Route::Home => home(),
        Route::Live => live_page(),
        Route::Season { season } => season_page(&season),
        Route::LegacyCalendar => season_page(CURRENT),
        Route::Race { season, round } => race_page(&season, round),
        Route::LegacyRace { round } => race_page(CURRENT, round),
        Route::DriverStandings { season } => standings_page(&season, StandingsKind::Drivers),
        Route::LegacyDrivers => standings_page(CURRENT, StandingsKind::Drivers),
        Route::TeamStandings { season } => standings_page(&season, StandingsKind::Teams),
        Route::LegacyTeams => standings_page(CURRENT, StandingsKind::Teams),
        Route::Driver { id } => driver_page(&id),
        Route::Team { id } => team_page(&id),
        Route::Circuit { id } => circuit_page(&id),
        Route::Archives => archives_page(),
        Route::Records => records_page(),
        Route::Compare => compare_page(None, None),
        Route::CompareWith { a, b } => compare_page(Some(a), Some(b)),
        Route::Quiz => quiz_page(),
        Route::Predict => predict_page(),
        Route::Fantasy => fantasy_page(),
        Route::News => news_page(),
        Route::Glossary => glossary_page(),
        Route::AllSeasons => all_seasons_page(),
        Route::AllDrivers => all_drivers_page(),
        Route::AllTeams => all_teams_page(),
        Route::AllCircuits => all_circuits_page(),
        Route::Data => data_year_page(util::current_year()),
        Route::DataYear { year } => data_year_page(year),
        Route::DataMeeting { key } => data_meeting_page(key),
        Route::DataSession { key } => data_session_page(key),
        Route::Server => {
            // Adresse du serveur atteinte par l'historique : on la charge pour de bon.
            if let Some(w) = web_sys::window() {
                let _ = w.location().reload();
            }
            components::loading()
        }
        Route::NotFound => not_found(),
    }
}

/// L'application : la page de l'adresse courante, reconstruite quand l'adresse ou la langue
/// change (toutes les pages relisent alors la nouvelle langue).
fn app() -> Node {
    let path = location();
    let lang = i18n::init();
    fragment_dyn(move || {
        let route = Route::parse(&path.get());
        i18n::apply(lang.get());
        // Les pages lisent leurs états dans des closures : leur construction ne doit pas
        // abonner le routeur (seules l'adresse et la langue le reconstruisent).
        vec![untrack(|| switch(route))]
    })
}

/// En cas de panique Rust : message lisible + bouton « Recharger » au lieu d'un écran figé.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let message = info.to_string();
        web_sys::console::error_1(&message.clone().into());
        if let Some(window) = web_sys::window() {
            if let Ok(fail) = js_sys::Reflect::get(&window, &"__f1xFail".into()) {
                if let Some(fail) = wasm_bindgen::JsCast::dyn_ref::<js_sys::Function>(&fail) {
                    // `force` : affiche le message même si l'app avait démarré.
                    let _ = fail.call2(&window, &message.into(), &true.into());
                }
            }
        }
    }));
}

fn main() {
    install_panic_hook();
    // Remplace l'écran de démarrage de `#app`.
    active::mount_to("#app", app());
    // Démarrage réussi : désactive le filet de sécurité de la page.
    if let Some(window) = web_sys::window() {
        if let Ok(started) = js_sys::Reflect::get(&window, &"__f1xStarted".into()) {
            if let Some(f) = wasm_bindgen::JsCast::dyn_ref::<js_sys::Function>(&started) {
                let _ = f.call0(&window);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Route;

    #[test]
    fn every_route_round_trips() {
        for route in [
            Route::Home,
            Route::race("2024", 3),
            Route::DriverStandings {
                season: "current".into(),
            },
            Route::CompareWith {
                a: "hamilton".into(),
                b: "max_verstappen".into(),
            },
            Route::DataSession { key: 9158 },
            Route::AllCircuits,
        ] {
            assert_eq!(Route::parse(&route.href()), route);
        }
        assert_eq!(Route::parse("/presentation"), Route::Server);
        assert_eq!(Route::parse("/saison/2024/course/x"), Route::NotFound);
    }
}
