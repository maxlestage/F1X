//! À PORTER depuis la version Yew (voir /home/user/yew-src/pages/fantasy.rs).
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use std::rc::Rc;

use active::prelude::*;

fn todo(what: &str) -> Node {
    crate::components::empty_card(&format!("TODO {what}"))
}

pub fn fantasy_page() -> Node { todo("fantasy") }
