//! À PORTER depuis la version Yew (voir /home/user/yew-src/pages/compare.rs).
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use std::rc::Rc;

use active::prelude::*;

fn todo(what: &str) -> Node {
    crate::components::empty_card(&format!("TODO {what}"))
}

pub fn compare_page(a: Option<String>, b: Option<String>) -> Node { todo("compare") }
