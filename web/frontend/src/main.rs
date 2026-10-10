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
    Season {
        season: String,
    },
    Race {
        season: String,
        round: u32,
    },
    DriverStandings {
        season: String,
    },
    TeamStandings {
        season: String,
    },
    Driver {
        id: String,
    },
    Team {
        id: String,
    },
    Circuit {
        id: String,
    },
    Archives,
    Records,
    Quiz,
    Predict,
    Fantasy,
    News,
    Glossary,
    Compare,
    CompareWith {
        a: String,
        b: String,
    },
    AllSeasons,
    AllDrivers,
    AllTeams,
    AllCircuits,
    Data,
    DataYear {
        year: u32,
    },
    DataMeeting {
        key: u32,
    },
    DataSession {
        key: u32,
    },
    // Anciennes adresses (saison en cours), conservées pour les liens existants.
    LegacyCalendar,
    LegacyRace {
        round: u32,
    },
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

/// Les écrans de l'app. Deux adresses du même écran (deux saisons du calendrier, deux Grands
/// Prix, deux pilotes…) gardent l'écran affiché : seul ce qui dépend des paramètres change,
/// comme un composant qui reçoit de nouvelles propriétés.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Page {
    Home,
    Live,
    Season,
    Race,
    Standings,
    Driver,
    Team,
    Circuit,
    Archives,
    Records,
    Compare,
    Quiz,
    Predict,
    Fantasy,
    News,
    Glossary,
    AllSeasons,
    AllDrivers,
    AllTeams,
    AllCircuits,
    DataYear,
    DataMeeting,
    DataSession,
    Server,
    NotFound,
}

impl Route {
    fn page(&self) -> Page {
        match self {
            Self::Home => Page::Home,
            Self::Live => Page::Live,
            Self::Season { .. } | Self::LegacyCalendar => Page::Season,
            Self::Race { .. } | Self::LegacyRace { .. } => Page::Race,
            Self::DriverStandings { .. }
            | Self::TeamStandings { .. }
            | Self::LegacyDrivers
            | Self::LegacyTeams => Page::Standings,
            Self::Driver { .. } => Page::Driver,
            Self::Team { .. } => Page::Team,
            Self::Circuit { .. } => Page::Circuit,
            Self::Archives => Page::Archives,
            Self::Records => Page::Records,
            Self::Compare | Self::CompareWith { .. } => Page::Compare,
            Self::Quiz => Page::Quiz,
            Self::Predict => Page::Predict,
            Self::Fantasy => Page::Fantasy,
            Self::News => Page::News,
            Self::Glossary => Page::Glossary,
            Self::AllSeasons => Page::AllSeasons,
            Self::AllDrivers => Page::AllDrivers,
            Self::AllTeams => Page::AllTeams,
            Self::AllCircuits => Page::AllCircuits,
            Self::Data | Self::DataYear { .. } => Page::DataYear,
            Self::DataMeeting { .. } => Page::DataMeeting,
            Self::DataSession { .. } => Page::DataSession,
            Self::Server => Page::Server,
            Self::NotFound => Page::NotFound,
        }
    }

    /// Saison de l'adresse (calendrier, Grand Prix, classements).
    fn season_param(&self) -> String {
        match self {
            Self::Season { season }
            | Self::Race { season, .. }
            | Self::DriverStandings { season }
            | Self::TeamStandings { season } => season.clone(),
            _ => CURRENT.to_string(),
        }
    }

    /// Identifiant de l'adresse (pilote, écurie, circuit).
    fn id_param(&self) -> String {
        match self {
            Self::Driver { id } | Self::Team { id } | Self::Circuit { id } => id.clone(),
            _ => String::new(),
        }
    }

    /// Clé numérique de l'adresse (manche, année, réunion, séance).
    fn number_param(&self) -> u32 {
        match self {
            Self::Race { round, .. } | Self::LegacyRace { round } => *round,
            Self::DataYear { year } => *year,
            Self::DataMeeting { key } | Self::DataSession { key } => *key,
            Self::Data => util::current_year(),
            _ => 0,
        }
    }
}

/// L'écran d'une adresse. `route` change quand on va vers une autre adresse du même écran :
/// les paramètres sont passés aux écrans comme des états (des memos de `route`).
fn screen(route: State<(Route, i18n::Lang)>) -> Node {
    use pages::*;
    fn param<T: Clone + PartialEq + 'static>(
        route: State<(Route, i18n::Lang)>,
        f: fn(&Route) -> T,
    ) -> State<T> {
        memo(move || route.with(|(r, _)| f(r)))
    }
    let page = untrack(|| route.with(|(r, _)| r.page()));
    match page {
        Page::Home => home(),
        Page::Live => live_page(),
        Page::Season => season_page(param(route, Route::season_param)),
        Page::Race => race_page(memo(move || {
            route.with(|(r, _)| (r.season_param(), r.number_param()))
        })),
        Page::Standings => standings_page(memo(move || {
            route.with(|(r, _)| {
                let kind = match r {
                    Route::TeamStandings { .. } | Route::LegacyTeams => StandingsKind::Teams,
                    _ => StandingsKind::Drivers,
                };
                (r.season_param(), kind)
            })
        })),
        Page::Driver => driver_page(param(route, Route::id_param)),
        Page::Team => team_page(param(route, Route::id_param)),
        Page::Circuit => circuit_page(param(route, Route::id_param)),
        Page::Archives => archives_page(),
        Page::Records => records_page(),
        Page::Compare => compare_page(memo(move || {
            route.with(|(r, _)| match r {
                Route::CompareWith { a, b } => (Some(a.clone()), Some(b.clone())),
                _ => (None, None),
            })
        })),
        Page::Quiz => quiz_page(),
        Page::Predict => predict_page(),
        Page::Fantasy => fantasy_page(),
        Page::News => news_page(),
        Page::Glossary => glossary_page(),
        Page::AllSeasons => all_seasons_page(),
        Page::AllDrivers => all_drivers_page(),
        Page::AllTeams => all_teams_page(),
        Page::AllCircuits => all_circuits_page(),
        Page::DataYear => data_year_page(param(route, Route::number_param)),
        Page::DataMeeting => data_meeting_page(param(route, Route::number_param)),
        Page::DataSession => data_session_page(param(route, Route::number_param)),
        Page::Server => {
            // Adresse du serveur atteinte par l'historique : on la charge pour de bon.
            if let Some(w) = web_sys::window() {
                let _ = w.location().reload();
            }
            components::loading()
        }
        Page::NotFound => not_found(),
    }
}

/// L'application : l'écran de l'adresse courante. Il est reconstruit quand on change d'écran
/// ou de langue (tout relit alors la nouvelle langue) ; une autre adresse du même écran le
/// garde et ne met à jour que ce qui dépend de ses paramètres.
fn app() -> Node {
    let path = location();
    let lang = i18n::init();
    switch(
        move || (Route::parse(&path.get()), lang.get()),
        |(route, lang)| (route.page(), *lang),
        |route| {
            i18n::apply(untrack(|| route.with(|(_, lang)| *lang)));
            screen(route)
        },
    )
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
