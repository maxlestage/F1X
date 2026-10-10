//! À PORTER depuis la version Yew (voir /home/user/yew-src/pages/glossary.rs).
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use std::rc::Rc;

use active::prelude::*;

fn todo(what: &str) -> Node {
    crate::components::empty_card(&format!("TODO {what}"))
}

pub fn glossary_page() -> Node { todo("glossary") }
