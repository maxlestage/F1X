//! F1X — frontend 100 % Rust avec Yew, compilé en WebAssembly.

mod api;
mod components;
mod models;
mod pages;
mod util;

use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/calendrier")]
    Calendar,
    #[at("/course/:round")]
    Race { round: u32 },
    #[at("/pilotes")]
    Drivers,
    #[at("/pilote/:id")]
    Driver { id: String },
    #[at("/ecuries")]
    Teams,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(route: Route) -> Html {
    match route {
        Route::Home => html! { <pages::Home /> },
        Route::Calendar => html! { <pages::Calendar /> },
        Route::Race { round } => html! { <pages::RacePage {round} /> },
        Route::Drivers => html! { <pages::Drivers /> },
        Route::Driver { id } => html! { <pages::DriverPage id={id} /> },
        Route::Teams => html! { <pages::Teams /> },
        Route::NotFound => html! { <pages::NotFound /> },
    }
}

#[function_component]
fn App() -> Html {
    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}

fn main() {
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("app"))
        .expect("élément #app introuvable");
    root.set_inner_html(""); // retire l'écran de démarrage
    yew::Renderer::<App>::with_root(root).render();
}
