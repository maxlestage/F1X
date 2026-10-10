//! À PORTER depuis la version Yew (voir /home/user/yew-src/pages/race.rs).
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use std::rc::Rc;

use active::prelude::*;

fn todo(what: &str) -> Node {
    crate::components::empty_card(&format!("TODO {what}"))
}

pub fn race_page(season: &str, round: u32) -> Node { todo("race") }
