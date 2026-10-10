//! À PORTER depuis la version Yew (voir /home/user/yew-src/pages/archives.rs).
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use std::rc::Rc;

use active::prelude::*;

fn todo(what: &str) -> Node {
    crate::components::empty_card(&format!("TODO {what}"))
}

pub fn archives_page() -> Node { todo("archives") }
pub fn all_seasons_page() -> Node { todo("seasons") }
pub fn all_drivers_page() -> Node { todo("drivers") }
pub fn all_teams_page() -> Node { todo("teams") }
pub fn all_circuits_page() -> Node { todo("circuits") }
