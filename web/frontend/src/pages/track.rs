//! À PORTER depuis la version Yew (voir /home/user/yew-src/pages/track.rs).
#![allow(unused_variables, dead_code)]

#[allow(unused_imports)]
use std::rc::Rc;

use active::prelude::*;

fn todo(what: &str) -> Node {
    crate::components::empty_card(&format!("TODO {what}"))
}

use f1x_protocol::TrackMap;
pub fn track_view(track: Rc<TrackMap>) -> Node { todo("track view") }
pub fn track_panel(track: Rc<TrackMap>, start_3d: bool) -> Node { todo("track panel") }
pub fn osm_embed(lat: &str, lon: &str) -> Node { todo("osm") }
pub fn apple_map(lat: &str, lon: &str, name: &str) -> Node { todo("apple map") }
pub fn track_outline(circuit_id: &str) -> Node { todo("track outline") }
pub fn track_url(circuit_id: &str, attempt: u32) -> String { String::new() }
pub fn track_unavailable(attempt: State<u32>) -> Node { todo("track unavailable") }
pub fn circuit_track(circuit_id: &str, name: &str) -> Node { todo("circuit track") }
