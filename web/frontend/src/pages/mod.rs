//! Les écrans de l'application.

mod archives;
mod circuit;
mod driver;
mod home;
mod live;
mod race;
mod season;
mod team;
mod track;

pub use archives::*;
pub use circuit::*;
pub use driver::*;
pub use home::*;
pub use live::*;
pub use race::*;
pub use season::*;
pub use team::*;
pub use track::*;

use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::components::Layout;
use crate::i18n::t;
use crate::tr;

/// Libellé d'une saison pour les titres.
pub fn season_label(season: &str) -> String {
    if season == crate::CURRENT {
        t("Saison en cours", "Current season").into()
    } else {
        tr!("Saison {season}", "Season {season}")
    }
}

#[function_component]
pub fn NotFound() -> Html {
    html! {
        <Layout title={t("Page introuvable", "Page not found")}>
            <section class="card">
                <h2>{ t("Hors piste", "Off track") }</h2>
                <p class="muted">{ t("Cette page n'existe pas.", "This page doesn't exist.") }</p>
                <Link<Route> to={Route::Home} classes="btn">{ t("Retour au stand", "Back to the pits") }</Link<Route>>
            </section>
        </Layout>
    }
}
