//! À PORTER depuis la version Yew (voir /home/user/yew-src/pages/weather.rs).
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use std::rc::Rc;

use active::prelude::*;

fn todo(what: &str) -> Node {
    crate::components::empty_card(&format!("TODO {what}"))
}

pub fn compass(deg: f64) -> &'static str { "N" }
pub fn weekend_weather(race: crate::models::Race, full: bool) -> Node { todo("weather") }
