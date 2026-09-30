//! Les écrans de l'application.

mod archives;
mod circuit;
mod driver;
mod home;
mod live;
mod race;
mod season;
mod team;

pub use archives::*;
pub use circuit::*;
pub use driver::*;
pub use home::*;
pub use live::*;
pub use race::*;
pub use season::*;
pub use team::*;

use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::components::Layout;

/// Libellé d'une saison pour les titres.
pub fn season_label(season: &str) -> String {
    if season == crate::CURRENT {
        "Saison en cours".into()
    } else {
        format!("Saison {season}")
    }
}

#[function_component]
pub fn NotFound() -> Html {
    html! {
        <Layout title="Page introuvable">
            <section class="card">
                <h2>{ "Hors piste" }</h2>
                <p class="muted">{ "Cette page n'existe pas." }</p>
                <Link<Route> to={Route::Home} classes="btn">{ "Retour au stand" }</Link<Route>>
            </section>
        </Layout>
    }
}
