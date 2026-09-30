//! Tracé d'un circuit coloré par la vitesse, reconstitué à partir d'un tour réel (OpenF1).

use std::rc::Rc;

use f1x_protocol::{TrackMap, format_lap};
use wasm_bindgen::JsCast;
use web_sys::Element;
use yew::prelude::*;

use crate::components::stat_grid;
use crate::i18n::t;
use crate::tr;

/// Échelle séquentielle à une seule teinte (rouge) : lent = soutenu, rapide = clair.
/// Sur fond sombre, la vitesse élevée ressort le plus.
fn speed_colour(ratio: f32) -> String {
    let r = ratio.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| (a + (b - a) * r).round() as u8;
    // #b3261e → #ffe4de
    format!(
        "#{:02x}{:02x}{:02x}",
        lerp(179.0, 255.0),
        lerp(38.0, 228.0),
        lerp(30.0, 222.0)
    )
}

#[derive(Properties, PartialEq)]
pub struct TrackProps {
    pub track: Rc<TrackMap>,
}

const PAD: f64 = 40.0;

#[function_component]
pub fn TrackView(props: &TrackProps) -> Html {
    let hover = use_state(|| None::<usize>);
    let tr_ = &props.track;
    let pts = &tr_.points;
    if pts.len() < 2 {
        return html! {};
    }
    let (w, h) = (tr_.width + 2.0 * PAD, tr_.height + 2.0 * PAD);
    let (min_s, max_s) = (tr_.stats.min_speed as f32, tr_.stats.top_speed as f32);
    let ratio = |s: u16| (s as f32 - min_s) / (max_s - min_s).max(1.0);
    let px = |i: usize| (pts[i].x as f64 + PAD, pts[i].y as f64 + PAD);

    // Sens de la course : flèche au début du tour.
    let arrow = {
        let (x0, y0) = px(0);
        let (x1, y1) = px(4.min(pts.len() - 1));
        let angle = (y1 - y0).atan2(x1 - x0).to_degrees();
        format!("translate({x0:.1},{y0:.1}) rotate({angle:.1})")
    };

    let onpointermove = {
        let hover = hover.clone();
        let track = Rc::clone(&props.track);
        Callback::from(move |e: PointerEvent| {
            let Some(el) = e
                .current_target()
                .and_then(|t| t.dyn_into::<Element>().ok())
            else {
                return;
            };
            let rect = el.get_bounding_client_rect();
            if rect.width() <= 0.0 {
                return;
            }
            // Coordonnées dans le repère du SVG (preserveAspectRatio par défaut : xMidYMid meet).
            let scale = (rect.width() / w).min(rect.height() / h);
            let ox = rect.left() + (rect.width() - w * scale) / 2.0;
            let oy = rect.top() + (rect.height() - h * scale) / 2.0;
            let x = (e.client_x() as f64 - ox) / scale - PAD;
            let y = (e.client_y() as f64 - oy) / scale - PAD;
            let nearest = track
                .points
                .iter()
                .enumerate()
                .map(|(i, p)| (i, (p.x as f64 - x).powi(2) + (p.y as f64 - y).powi(2)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
            hover.set(nearest);
        })
    };
    let onpointerleave = {
        let hover = hover.clone();
        Callback::from(move |_: PointerEvent| hover.set(None))
    };

    let readout = match *hover {
        Some(i) => {
            let p = &pts[i];
            let brake = if p.brake {
                t(" · freinage", " · braking")
            } else {
                ""
            };
            tr!(
                "{} km/h · rapport {} · gaz {}%{brake} · {:.1} s",
                "{} km/h · gear {} · throttle {}%{brake} · {:.1} s",
                p.speed,
                p.gear,
                p.throttle,
                p.t
            )
        }
        None => t(
            "Touche ou survole le tracé pour lire la télémétrie.",
            "Touch or hover the track to read the telemetry.",
        )
        .to_string(),
    };
    let s = &tr_.stats;

    html! {
        <>
            <p class="chart-readout" aria-live="polite">{ readout }</p>
            <svg class="track" viewBox={format!("0 0 {w:.0} {h:.0}")} role="img"
                 aria-label={tr!("Tracé du circuit coloré selon la vitesse, de {} à {} km/h", "Circuit layout coloured by speed, from {} to {} km/h", s.min_speed, s.top_speed)}
                 {onpointermove} {onpointerleave}>
                // Fond du tracé, pour que la forme reste lisible partout.
                <polyline class="track-base" points={pts.iter().map(|p| format!("{:.1},{:.1}", p.x as f64 + PAD, p.y as f64 + PAD)).collect::<Vec<_>>().join(" ")} />
                { for (0..pts.len() - 1).map(|i| {
                    let (x1, y1) = px(i);
                    let (x2, y2) = px(i + 1);
                    html! { <line x1={format!("{x1:.1}")} y1={format!("{y1:.1}")} x2={format!("{x2:.1}")} y2={format!("{y2:.1}")}
                                  stroke={speed_colour(ratio(pts[i].speed))} class="track-seg" /> }
                }) }
                <g transform={arrow}>
                    <line class="track-start" x1="0" y1="-26" x2="0" y2="26" />
                    <path class="track-arrow" d="M 20 -12 L 40 0 L 20 12 Z" />
                </g>
                if let Some(i) = *hover {
                    <circle class="track-dot" cx={format!("{:.1}", px(i).0)} cy={format!("{:.1}", px(i).1)} r="16" />
                }
            </svg>
            <div class="track-legend" aria-hidden="true">
                <span>{ format!("{} km/h", s.min_speed) }</span>
                <span class="track-ramp" style={format!("background:linear-gradient(90deg,{},{})", speed_colour(0.0), speed_colour(1.0))}></span>
                <span>{ format!("{} km/h", s.top_speed) }</span>
            </div>
            <p class="muted chart-legend">
                <span class="legend-line" style="background:#fff"></span>{ t("ligne de départ, flèche = sens de la course", "start line, arrow = racing direction") }
            </p>
            { stat_grid(vec![
                (t("Tour", "Lap"), format_lap(tr_.lap_time)),
                (t("Longueur", "Length"), format!("{:.2} km", s.length_km)),
                (t("V. max", "Top speed"), format!("{} km/h", s.top_speed)),
                (t("V. mini", "Min speed"), format!("{} km/h", s.min_speed)),
                (t("V. moyenne", "Avg speed"), format!("{:.0} km/h", s.avg_speed)),
                (t("À fond", "Full throttle"), format!("{:.0}%", s.full_throttle_pct)),
                (t("Freinage", "Braking"), format!("{:.0}%", s.braking_pct)),
                (t("Rapports", "Gear shifts"), s.gear_changes.to_string()),
                (t("Année", "Year"), tr_.year.to_string()),
            ]) }
            <p class="muted">
                { tr!(
                    "Meilleur tour de {} ({}) au {} {} — positions GPS et télémétrie OpenF1. Longueur estimée d'après la vitesse.",
                    "Fastest lap by {} ({}) at the {} {} — OpenF1 GPS positions and telemetry. Length estimated from speed.",
                    tr_.driver,
                    tr_.team,
                    tr_.event,
                    tr_.year
                ) }
            </p>
        </>
    }
}

/// Carte OpenStreetMap intégrée, centrée sur le circuit.
pub fn osm_embed(lat: &str, lon: &str) -> Html {
    let (Ok(la), Ok(lo)) = (lat.parse::<f64>(), lon.parse::<f64>()) else {
        return html! {};
    };
    let (dx, dy) = (0.018, 0.011);
    let src = format!(
        "https://www.openstreetmap.org/export/embed.html?bbox={:.5},{:.5},{:.5},{:.5}&layer=mapnik&marker={la},{lo}",
        lo - dx,
        la - dy,
        lo + dx,
        la + dy
    );
    html! {
        <iframe class="osm" src={src} loading="lazy" referrerpolicy="no-referrer"
                title={t("Carte du circuit (OpenStreetMap)", "Circuit map (OpenStreetMap)")}></iframe>
    }
}

/// Silhouette du circuit (sans télémétrie), pour les cartes compactes.
#[derive(Properties, PartialEq)]
pub struct OutlineProps {
    pub circuit_id: AttrValue,
}

#[function_component]
pub fn TrackOutline(props: &OutlineProps) -> Html {
    let track = crate::api::use_json::<TrackMap>(Some(format!("/api/track/{}", props.circuit_id)));
    let Some(Ok(map)) = &track else {
        return html! {};
    };
    let pts = &map.points;
    if pts.len() < 2 {
        return html! {};
    }
    let (w, h) = (map.width + 2.0 * PAD, map.height + 2.0 * PAD);
    let line = pts
        .iter()
        .map(|p| format!("{:.0},{:.0}", p.x as f64 + PAD, p.y as f64 + PAD))
        .collect::<Vec<_>>()
        .join(" ");
    let (x0, y0) = (pts[0].x as f64 + PAD, pts[0].y as f64 + PAD);
    html! {
        <svg class="outline" viewBox={format!("0 0 {w:.0} {h:.0}")} role="img"
             aria-label={t("Tracé du circuit", "Circuit layout")}>
            <polyline class="outline-base" points={line.clone()} />
            <polyline class="outline-line" points={line} />
            <circle class="outline-start" cx={format!("{x0:.0}")} cy={format!("{y0:.0}")} r="16" />
        </svg>
    }
}

/// Carte « Circuit » d'une page de Grand Prix : tracé GPS complet + lien vers la fiche.
#[derive(Properties, PartialEq)]
pub struct CircuitTrackProps {
    pub circuit_id: AttrValue,
    pub name: AttrValue,
}

#[function_component]
pub fn CircuitTrack(props: &CircuitTrackProps) -> Html {
    let track = crate::api::use_json::<TrackMap>(Some(format!("/api/track/{}", props.circuit_id)));
    let body = match &track {
        None => crate::components::loading(),
        Some(Ok(map)) => html! { <TrackView track={map.clone()} /> },
        // Circuit jamais utilisé depuis 2023 (pas de données GPS) : rien à dessiner.
        Some(Err(_)) => return html! {},
    };
    html! {
        <section class="card">
            <div class="card-head">
                <h2>{ t("Le circuit", "The circuit") }</h2>
                <yew_router::prelude::Link<crate::Route> to={crate::Route::circuit(&props.circuit_id)} classes="link">
                    { t("Fiche complète", "Full details") }
                </yew_router::prelude::Link<crate::Route>>
            </div>
            <p class="muted">{ props.name.clone() }</p>
            { body }
        </section>
    }
}
