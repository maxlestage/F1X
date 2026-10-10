//! À PORTER depuis la version Yew (voir /home/user/yew-src/pages/openf1.rs).
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use std::rc::Rc;

use active::prelude::*;

fn todo(what: &str) -> Node {
    crate::components::empty_card(&format!("TODO {what}"))
}

pub fn data_year_page(year: u32) -> Node { todo("data year") }
pub fn data_meeting_page(key: u32) -> Node { todo("data meeting") }
pub fn data_session_page(key: u32) -> Node { todo("data session") }
/// Lien vers la réunion OpenF1 d'une course (`date` : « 2025-07-06 »).
pub fn meeting_link(year: u32, date: &str) -> Node { todo("meeting link") }
pub fn race_data_links(year: u32, date: &str) -> Node { todo("race data links") }
pub fn race_replay(year: u32, date: &str) -> Node { todo("race replay") }
/// `query` : `session_key=…` ou `year=…&date=…`.
pub fn pit_detail(query: &str) -> Node { todo("pit detail") }
