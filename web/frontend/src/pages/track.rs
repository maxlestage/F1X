//! Tracé d'un circuit coloré par la vitesse, reconstitué à partir d'un tour réel (OpenF1).

use std::rc::Rc;

use active::prelude::*;
use f1x_protocol::{TrackMap, format_lap};
use wasm_bindgen::JsCast;

use crate::components::{dynamic, loading, stat_grid};
use crate::gl3d::{Scene, view_3d};
use crate::i18n::t;
use crate::{Route, link, tr};

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

const PAD: f64 = 40.0;

/// Position du point `i` dans le repère du SVG (marge comprise).
fn at(track: &TrackMap, i: usize) -> (f64, f64) {
    let pt = &track.points[i];
    (pt.x as f64 + PAD, pt.y as f64 + PAD)
}

/// Télémétrie du point survolé, ou l'invitation à toucher le tracé.
fn readout(track: &TrackMap, hover: Option<usize>) -> String {
    match hover.and_then(|i| track.points.get(i)) {
        Some(pt) => {
            let brake = if pt.brake {
                t(" · freinage", " · braking")
            } else {
                ""
            };
            tr!(
                "{} km/h · rapport {} · gaz {}%{brake} · {:.1} s",
                "{} km/h · gear {} · throttle {}%{brake} · {:.1} s",
                pt.speed,
                pt.gear,
                pt.throttle,
                pt.t
            )
        }
        None => t(
            "Touche ou survole le tracé pour lire la télémétrie.",
            "Touch or hover the track to read the telemetry.",
        )
        .to_string(),
    }
}

/// Tracé en plan coloré selon la vitesse, avec la télémétrie du point touché ou survolé.
///
/// Le SVG est construit une fois : le pointeur ne change que le texte de la télémétrie et la
/// position du repère.
pub fn track_view(track: Rc<TrackMap>) -> Node {
    let pts = &track.points;
    if pts.len() < 2 {
        return Node::Empty;
    }
    let hover = use_state(None::<usize>);
    // Le repère n'apparaît / disparaît qu'à l'entrée et à la sortie du tracé.
    let hovering = memo(move || hover.with(Option::is_some));
    let (w, h) = (track.width + 2.0 * PAD, track.height + 2.0 * PAD);
    let stats = &track.stats;
    let (min_s, max_s) = (stats.min_speed as f32, stats.top_speed as f32);
    let ratio = |s: u16| (s as f32 - min_s) / (max_s - min_s).max(1.0);
    let px = |i: usize| at(&track, i);

    // Sens de la course : flèche au début du tour.
    let arrow = {
        let (x0, y0) = px(0);
        let (x1, y1) = px(4.min(pts.len() - 1));
        let angle = (y1 - y0).atan2(x1 - x0).to_degrees();
        format!("translate({x0:.1},{y0:.1}) rotate({angle:.1})")
    };

    let on_move = {
        let track = Rc::clone(&track);
        move |e: Event| {
            let Some(pe) = e.raw().dyn_ref::<web_sys::PointerEvent>() else {
                return;
            };
            let Some(el) = e.current_target() else {
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
            let x = (pe.client_x() as f64 - ox) / scale - PAD;
            let y = (pe.client_y() as f64 - oy) / scale - PAD;
            let nearest = track
                .points
                .iter()
                .enumerate()
                .map(|(k, pt)| (k, (pt.x as f64 - x).powi(2) + (pt.y as f64 - y).powi(2)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(k, _)| k);
            if hover.get() != nearest {
                hover.set(nearest);
            }
        }
    };
    let on_leave = move |_: Event| {
        if hover.get().is_some() {
            hover.set(None);
        }
    };

    // Repère du point survolé : créé à l'entrée sur le tracé, déplacé ensuite par ses attributs.
    let dot = {
        let track = Rc::clone(&track);
        dynamic(move || {
            if !hovering.get() {
                return Node::Empty;
            }
            let coord = |pick: fn((f64, f64)) -> f64| {
                let track = Rc::clone(&track);
                move || {
                    hover
                        .get()
                        .filter(|&i| i < track.points.len())
                        .map(|i| format!("{:.1}", pick(at(&track, i))))
                        .unwrap_or_default()
                }
            };
            circle()
                .class("track-dot")
                .attr_dyn("cx", coord(|(x, _)| x))
                .attr_dyn("cy", coord(|(_, y)| y))
                .attr("r", "16")
                .into()
        })
    };

    let base = pts
        .iter()
        .map(|pt| format!("{:.1},{:.1}", pt.x as f64 + PAD, pt.y as f64 + PAD))
        .collect::<Vec<_>>()
        .join(" ");
    // `--k` : avancement dans le tour, pour que la couleur suive le sens de la course à l'affichage.
    let segments = (0..pts.len() - 1).map(|k| {
        let (x1, y1) = px(k);
        let (x2, y2) = px(k + 1);
        line()
            .attr("x1", format!("{x1:.1}"))
            .attr("y1", format!("{y1:.1}"))
            .attr("x2", format!("{x2:.1}"))
            .attr("y2", format!("{y2:.1}"))
            .attr("stroke", speed_colour(ratio(pts[k].speed)))
            .class("track-seg")
            .style(format!("--k:{:.3}", k as f64 / pts.len() as f64))
    });

    let shape = svg()
        .class("track")
        .attr("viewBox", format!("0 0 {w:.0} {h:.0}"))
        .attr("role", "img")
        .attr(
            "aria-label",
            tr!(
                "Tracé du circuit coloré selon la vitesse, de {} à {} km/h",
                "Circuit layout coloured by speed, from {} to {} km/h",
                stats.min_speed,
                stats.top_speed
            ),
        )
        .on("pointermove", on_move)
        .on("pointerleave", on_leave)
        // Fond du tracé, pour que la forme reste lisible partout.
        .child(
            polyline()
                .class("track-base")
                .attr("pathLength", "1")
                .attr("points", base),
        )
        .children(segments)
        .child(
            g().attr("transform", arrow)
                .child(
                    line()
                        .class("track-start")
                        .attr("x1", "0")
                        .attr("y1", "-26")
                        .attr("x2", "0")
                        .attr("y2", "26"),
                )
                .child(
                    path()
                        .class("track-arrow")
                        .attr("d", "M 20 -12 L 40 0 L 20 12 Z"),
                ),
        )
        .child(dot);

    let telemetry = {
        let track = Rc::clone(&track);
        p().class("chart-readout")
            .attr("aria-live", "polite")
            .text_dyn(move || readout(&track, hover.get()))
    };

    fragment([
        Node::from(telemetry),
        shape.into(),
        div()
            .class("track-legend")
            .attr("aria-hidden", "true")
            .child(span().text(format!("{} km/h", stats.min_speed)))
            .child(span().class("track-ramp").style(format!(
                "background:linear-gradient(90deg,{},{})",
                speed_colour(0.0),
                speed_colour(1.0)
            )))
            .child(span().text(format!("{} km/h", stats.top_speed)))
            .into(),
        p().class("muted chart-legend")
            .child(span().class("legend-line").style("background:#fff"))
            .text(t(
                "ligne de départ, flèche = sens de la course",
                "start line, arrow = racing direction",
            ))
            .into(),
        stat_grid(vec![
            (t("Tour", "Lap"), format_lap(track.lap_time)),
            (t("Longueur", "Length"), format!("{:.2} km", stats.length_km)),
            (t("V. max", "Top speed"), format!("{} km/h", stats.top_speed)),
            (t("V. mini", "Min speed"), format!("{} km/h", stats.min_speed)),
            (t("V. moyenne", "Avg speed"), format!("{:.0} km/h", stats.avg_speed)),
            (t("À fond", "Full throttle"), format!("{:.0}%", stats.full_throttle_pct)),
            (t("Freinage", "Braking"), format!("{:.0}%", stats.braking_pct)),
            (t("Rapports", "Gear shifts"), stats.gear_changes.to_string()),
            (t("Année", "Year"), track.year.to_string()),
        ]),
        p().class("muted")
            .text(tr!(
                "Meilleur tour de {} ({}) au {} {} — positions GPS et télémétrie OpenF1. Longueur estimée d'après la vitesse.",
                "Fastest lap by {} ({}) at the {} {} — OpenF1 GPS positions and telemetry. Length estimated from speed.",
                track.driver,
                track.team,
                track.event,
                track.year
            ))
            .into(),
    ])
}

/// Tracé en plan (télémétrie) ou en relief 3D, au choix. Seule la vue choisie se reconstruit
/// quand on change d'onglet.
pub fn track_panel(track: Rc<TrackMap>, start_3d: bool) -> Node {
    let three = use_state(start_3d);
    let tab = move |on: bool, label: &'static str| {
        button()
            .class("seg")
            .class_if("seg-active", move || three.get() == on)
            .attr_dyn("aria-pressed", move || (three.get() == on).to_string())
            .on_click(move |_| {
                if three.get() != on {
                    three.set(on);
                }
            })
            .text(label)
    };
    let relief = track.points.iter().map(|pt| pt.z).fold(0.0f32, f32::max) > 1.0;
    let view = dynamic(move || {
        if !three.get() {
            return track_view(Rc::clone(&track));
        }
        fragment([
            view_3d(
                Scene::Track {
                    map: Rc::clone(&track),
                    ghost: true,
                },
                || None,
            ),
            p().class("muted")
                .text(if relief {
                    t(
                        "Tracé GPS réel avec son relief (dénivelé exagéré ×2,5). Tout le plateau tourne sur le meilleur tour : choisis la caméra (poursuite, embarquée, hélico, TV) et le pilote suivi.",
                        "Real GPS layout with its elevation (exaggerated ×2.5). The whole field laps on the fastest lap: pick the camera (chase, onboard, helicopter, TV) and the driver to follow.",
                    )
                } else {
                    t(
                        "Tracé GPS réel coloré selon la vitesse. La voiture rejoue le meilleur tour à vitesse réelle.",
                        "Real GPS layout coloured by speed. The car replays the fastest lap at real speed.",
                    )
                })
                .into(),
        ])
    });
    fragment([
        Node::from(
            div()
                .class("segmented")
                .child(tab(false, t("Plan 2D", "2D map")))
                .child(tab(true, t("Relief 3D", "3D relief"))),
        ),
        view,
    ])
}

/// Carte OpenStreetMap intégrée, centrée sur le circuit.
pub fn osm_embed(lat: &str, lon: &str) -> Node {
    let (Ok(la), Ok(lo)) = (lat.parse::<f64>(), lon.parse::<f64>()) else {
        return Node::Empty;
    };
    let (dx, dy) = (0.018, 0.011);
    let src = format!(
        "https://www.openstreetmap.org/export/embed.html?bbox={:.5},{:.5},{:.5},{:.5}&layer=mapnik&marker={la},{lo}",
        lo - dx,
        la - dy,
        lo + dx,
        la + dy
    );
    iframe()
        .class("osm")
        .attr("src", src)
        .attr("loading", "lazy")
        .attr("referrerpolicy", "no-referrer")
        .attr(
            "title",
            t(
                "Carte du circuit (OpenStreetMap)",
                "Circuit map (OpenStreetMap)",
            ),
        )
        .into()
}

/// Lance la carte MapKit dans `el` via `window.f1xAppleMap` (défini par la page) ; `None` si
/// la fonction manque ou si les coordonnées sont invalides.
fn start_apple_map(
    el: &web_sys::Element,
    lat: &str,
    lon: &str,
    name: &str,
) -> Option<js_sys::Promise> {
    let win = web_sys::window()?;
    let f = js_sys::Reflect::get(&win, &"f1xAppleMap".into())
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    let (la, lo) = (lat.parse::<f64>().ok()?, lon.parse::<f64>().ok()?);
    let args = js_sys::Array::of4(el, &la.into(), &lo.into(), &name.into());
    f.apply(&wasm_bindgen::JsValue::NULL, &args)
        .ok()?
        .dyn_into::<js_sys::Promise>()
        .ok()
}

/// Carte Apple Maps (MapKit JS, vue satellite) ; repli sur OpenStreetMap si aucune clé
/// MapKit n'est configurée sur le serveur ou si le chargement échoue.
pub fn apple_map(lat: &str, lon: &str, name: &str) -> Node {
    let failed = use_state(false);
    let (lat, lon, name) = (lat.to_string(), lon.to_string(), name.to_string());
    dynamic(move || {
        if failed.get() {
            return osm_embed(&lat, &lon);
        }
        let start = {
            let (lat, lon, name) = (lat.clone(), lon.clone(), name.clone());
            move |el: &web_sys::Element| {
                let promise = start_apple_map(el, &lat, &lon, &name);
                // Le repli est décidé dans une tâche asynchrone, donc jamais pendant le montage
                // de la partie qui lit `failed`.
                wasm_bindgen_futures::spawn_local(async move {
                    let shown = match promise {
                        Some(promise) => {
                            wasm_bindgen_futures::JsFuture::from(promise).await.is_ok()
                        }
                        None => false,
                    };
                    if !shown && failed.is_alive() {
                        failed.set(true);
                    }
                });
            }
        };
        div()
            .class("apple-map")
            .attr("role", "img")
            .attr("aria-label", format!("{name} — Apple Maps"))
            .on_mount(start)
            .into()
    })
}

/// Silhouette du circuit (sans télémétrie), pour les cartes compactes.
pub fn track_outline(circuit_id: &str) -> Node {
    let json = crate::api::use_json::<TrackMap>(Some(format!("/api/track/{circuit_id}")));
    dynamic(move || {
        let Some(Ok(outline)) = json.get() else {
            return Node::Empty;
        };
        let pts = &outline.points;
        if pts.len() < 2 {
            return Node::Empty;
        }
        let (w, h) = (outline.width + 2.0 * PAD, outline.height + 2.0 * PAD);
        let points = pts
            .iter()
            .map(|pt| format!("{:.0},{:.0}", pt.x as f64 + PAD, pt.y as f64 + PAD))
            .collect::<Vec<_>>()
            .join(" ");
        let (x0, y0) = (pts[0].x as f64 + PAD, pts[0].y as f64 + PAD);
        // Trajet du point rouge qui fait le tour une fois le tracé dessiné.
        let lap = format!("M{}Z", points.replace(' ', "L"));
        svg()
            .class("outline")
            .attr("viewBox", format!("0 0 {w:.0} {h:.0}"))
            .attr("role", "img")
            .attr("aria-label", t("Tracé du circuit", "Circuit layout"))
            .child(
                polyline()
                    .class("outline-base")
                    .attr("points", points.clone()),
            )
            .child(
                polyline()
                    .class("outline-line")
                    .attr("pathLength", "1")
                    .attr("points", points),
            )
            .child(
                circle()
                    .class("outline-start")
                    .attr("cx", format!("{x0:.0}"))
                    .attr("cy", format!("{y0:.0}"))
                    .attr("r", "16"),
            )
            .child(
                circle()
                    .class("outline-runner")
                    .attr("r", "15")
                    .attr("opacity", "0")
                    .child(
                        el("set")
                            .attr("attributeName", "opacity")
                            .attr("to", "1")
                            .attr("begin", "2s"),
                    )
                    .child(
                        el("animateMotion")
                            .attr("dur", "7s")
                            .attr("begin", "2s")
                            .attr("repeatCount", "indefinite")
                            .attr("path", lap),
                    ),
            )
            .into()
    })
}

/// URL du tracé ; `attempt` > 0 force une nouvelle requête (bouton « Réessayer »).
pub fn track_url(circuit_id: &str, attempt: u32) -> String {
    if attempt == 0 {
        format!("/api/track/{circuit_id}")
    } else {
        format!("/api/track/{circuit_id}?essai={attempt}")
    }
}

/// Tracé momentanément indisponible (OpenF1 saturé ou séance en direct) : message + réessai.
pub fn track_unavailable(attempt: State<u32>) -> Node {
    div()
        .class("track-retry")
        .child(p().class("muted").text(t(
            "Tracé momentanément indisponible (source OpenF1 saturée ou séance en direct).",
            "Layout temporarily unavailable (OpenF1 busy or a live session is running).",
        )))
        .child(
            button()
                .class("btn btn-ghost")
                .on_click(move |_| attempt.update(|n| *n += 1))
                .text(t("Réessayer", "Retry")),
        )
        .into()
}

/// Carte « Circuit » d'une page de Grand Prix : tracé GPS complet + lien vers la fiche.
pub fn circuit_track(circuit_id: &str, name: &str) -> Node {
    let attempt = use_state(0u32);
    let track = {
        let id = circuit_id.to_string();
        crate::api::use_json_dyn::<TrackMap>(move || Some(track_url(&id, attempt.get())))
    };
    // Circuit jamais utilisé depuis 2023 (pas de données GPS) : rien à dessiner, pas de carte.
    let missing = memo(move || track.with(|j| matches!(j, Some(Err((404, _))))));
    let (id, name) = (circuit_id.to_string(), name.to_string());
    dynamic(move || {
        if missing.get() {
            return Node::Empty;
        }
        // Seul le contenu suit le chargement : l'en-tête de la carte reste en place.
        let body = dynamic(move || match track.get() {
            None => loading(),
            Some(Ok(layout)) => track_panel(layout, false),
            Some(Err((404, _))) => Node::Empty,
            Some(Err(_)) => track_unavailable(attempt),
        });
        section()
            .class("card")
            .child(
                div()
                    .class("card-head")
                    .child(h2().text(t("Le circuit", "The circuit")))
                    .child(
                        link(Route::circuit(&id), "link").text(t("Fiche complète", "Full details")),
                    ),
            )
            .child(p().class("muted").text(name.clone()))
            .child(body)
            .into()
    })
}
