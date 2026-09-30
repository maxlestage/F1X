//! F1X — frontend 100 % Rust avec Yew, compilé en WebAssembly.

mod api;
mod components;
mod i18n;
mod live;
mod models;
mod pages;
mod util;

use yew::prelude::*;
use yew_router::prelude::*;

/// `season` vaut `"current"` pour la saison en cours, sinon l'année (`"1998"`).
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/direct")]
    Live,
    #[at("/saison/:season")]
    Season { season: String },
    #[at("/saison/:season/course/:round")]
    Race { season: String, round: u32 },
    #[at("/saison/:season/pilotes")]
    DriverStandings { season: String },
    #[at("/saison/:season/ecuries")]
    TeamStandings { season: String },
    #[at("/pilote/:id")]
    Driver { id: String },
    #[at("/ecurie/:id")]
    Team { id: String },
    #[at("/circuit/:id")]
    Circuit { id: String },
    #[at("/archives")]
    Archives,
    #[at("/archives/saisons")]
    AllSeasons,
    #[at("/archives/pilotes")]
    AllDrivers,
    #[at("/archives/ecuries")]
    AllTeams,
    #[at("/archives/circuits")]
    AllCircuits,
    // Anciennes adresses (saison en cours), conservées pour les liens existants.
    #[at("/calendrier")]
    LegacyCalendar,
    #[at("/course/:round")]
    LegacyRace { round: u32 },
    #[at("/pilotes")]
    LegacyDrivers,
    #[at("/ecuries")]
    LegacyTeams,
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub const CURRENT: &str = "current";

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
}

fn switch(route: Route) -> Html {
    use pages::*;
    let cur = || AttrValue::from(CURRENT);
    match route {
        Route::Home => html! { <Home /> },
        Route::Live => html! { <LivePage /> },
        Route::Season { season } => html! { <SeasonPage season={season} /> },
        Route::LegacyCalendar => html! { <SeasonPage season={cur()} /> },
        Route::Race { season, round } => html! { <RacePage season={season} {round} /> },
        Route::LegacyRace { round } => html! { <RacePage season={cur()} {round} /> },
        Route::DriverStandings { season } => {
            html! { <StandingsPage season={season} kind={StandingsKind::Drivers} /> }
        }
        Route::LegacyDrivers => {
            html! { <StandingsPage season={cur()} kind={StandingsKind::Drivers} /> }
        }
        Route::TeamStandings { season } => {
            html! { <StandingsPage season={season} kind={StandingsKind::Teams} /> }
        }
        Route::LegacyTeams => {
            html! { <StandingsPage season={cur()} kind={StandingsKind::Teams} /> }
        }
        Route::Driver { id } => html! { <DriverPage id={id} /> },
        Route::Team { id } => html! { <TeamPage id={id} /> },
        Route::Circuit { id } => html! { <CircuitPage id={id} /> },
        Route::Archives => html! { <ArchivesPage /> },
        Route::AllSeasons => html! { <AllSeasonsPage /> },
        Route::AllDrivers => html! { <AllDriversPage /> },
        Route::AllTeams => html! { <AllTeamsPage /> },
        Route::AllCircuits => html! { <AllCircuitsPage /> },
        Route::NotFound => html! { <NotFound /> },
    }
}

/// Bascule de langue partagée avec la barre du haut.
#[derive(Clone, PartialEq)]
pub struct LangToggle(pub Callback<()>);

#[function_component]
fn App() -> Html {
    let lang = use_state(i18n::initial);
    // Appliquée avant le rendu : toutes les pages lisent la langue courante.
    i18n::apply(*lang);
    let toggle = {
        let lang = lang.clone();
        LangToggle(Callback::from(move |_| {
            let next = lang.other();
            i18n::save(next);
            lang.set(next);
        }))
    };
    html! {
        <ContextProvider<LangToggle> context={toggle}>
            <BrowserRouter>
                // La clé remonte les pages au changement de langue.
                <Switch<Route> key={lang.code()} render={switch} />
            </BrowserRouter>
        </ContextProvider<LangToggle>>
    }
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
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("app"))
        .expect("élément #app introuvable");
    root.set_inner_html(""); // retire l'écran de démarrage
    yew::Renderer::<App>::with_root(root).render();
    // Démarrage réussi : désactive le filet de sécurité de index.html.
    if let Some(window) = web_sys::window() {
        if let Ok(started) = js_sys::Reflect::get(&window, &"__f1xStarted".into()) {
            if let Some(f) = wasm_bindgen::JsCast::dyn_ref::<js_sys::Function>(&started) {
                let _ = f.call0(&window);
            }
        }
    }
}
