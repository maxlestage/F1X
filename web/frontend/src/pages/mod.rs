//! Les écrans de l'application.

mod archives;
mod circuit;
mod compare;
mod driver;
mod fantasy;
mod glossary;
mod home;
mod live;
mod news;
mod openf1;
mod predict;
mod quiz;
mod race;
mod records;
mod season;
pub mod stats;
mod team;
mod track;
mod weather;

pub use archives::*;
pub use circuit::*;
pub use compare::*;
pub use driver::*;
pub use fantasy::*;
pub use glossary::*;
pub use home::*;
pub use live::*;
pub use news::*;
pub use openf1::*;
pub use predict::*;
pub use quiz::*;
pub use race::*;
pub use records::*;
pub use season::*;
pub use team::*;
pub use track::*;
pub use weather::*;

use active::prelude::*;

use crate::components::{Tab, layout};
use crate::i18n::t;
use crate::{Route, link, tr};

/// Libellé d'une saison pour les titres.
pub fn season_label(season: &str) -> String {
    if season == crate::CURRENT {
        t("Saison en cours", "Current season").into()
    } else {
        tr!("Saison {season}", "Season {season}")
    }
}

pub fn not_found() -> Node {
    layout(
        t("Page introuvable", "Page not found"),
        None::<Tab>,
        section()
            .class("card")
            .child(h2().text(t("Hors piste", "Off track")))
            .child(
                p().class("muted")
                    .text(t("Cette page n'existe pas.", "This page doesn't exist.")),
            )
            .child(link(Route::Home, "btn").text(t("Retour au stand", "Back to the pits"))),
    )
}
