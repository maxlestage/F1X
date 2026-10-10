//! Application installable (PWA) et pied de page.

use active::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::components::dynamic;
use crate::i18n::{lang, t};

const DISMISSED: &str = "f1x-install-dismissed";

#[derive(Clone, Copy, PartialEq)]
enum Install {
    Hidden,
    /// Chrome / Edge / Android : invite native disponible.
    Prompt,
    /// iPhone / iPad : installation manuelle depuis Safari.
    Ios,
}

fn standalone() -> bool {
    let Some(win) = web_sys::window() else {
        return true;
    };
    let media = win
        .match_media("(display-mode: standalone)")
        .ok()
        .flatten()
        .is_some_and(|m| m.matches());
    let ios = js_sys::Reflect::get(&win.navigator(), &"standalone".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    media || ios
}

/// Invite différée par `index.html` (événement `beforeinstallprompt`).
fn deferred() -> Option<JsValue> {
    let win = web_sys::window()?;
    let v = js_sys::Reflect::get(&win, &"__f1xDeferred".into()).ok()?;
    (!v.is_null() && !v.is_undefined()).then_some(v)
}

fn detect() -> Install {
    if standalone() || crate::util::load::<bool>(DISMISSED).unwrap_or(false) {
        return Install::Hidden;
    }
    if deferred().is_some() {
        return Install::Prompt;
    }
    let ua = web_sys::window()
        .and_then(|w| w.navigator().user_agent().ok())
        .unwrap_or_default();
    if ua.contains("iPhone") || ua.contains("iPad") {
        Install::Ios
    } else {
        Install::Hidden
    }
}

/// Carte « Installer F1X » (accueil) : invite native ou mode d'emploi pour iOS.
pub fn install_card() -> Node {
    let mode = use_state(detect());
    // L'invite native peut arriver après coup (`f1x-installable`), ou l'app être installée.
    let listener = Closure::<dyn Fn()>::new(move || mode.set(detect()));
    if let Some(w) = web_sys::window() {
        for event in ["f1x-installable", "appinstalled"] {
            let _ = w.add_event_listener_with_callback(event, listener.as_ref().unchecked_ref());
        }
        on_cleanup(move || {
            for event in ["f1x-installable", "appinstalled"] {
                let _ =
                    w.remove_event_listener_with_callback(event, listener.as_ref().unchecked_ref());
            }
        });
    }
    let dismiss = move |_: Event| {
        crate::util::store(DISMISSED, &true);
        mode.set(Install::Hidden);
    };
    let install = move |_: Event| {
        if let Some(ev) = deferred() {
            if let Ok(prompt) = js_sys::Reflect::get(&ev, &"prompt".into()) {
                if let Ok(f) = prompt.dyn_into::<js_sys::Function>() {
                    let _ = f.call0(&ev);
                }
            }
            if let Some(w) = web_sys::window() {
                let _ = js_sys::Reflect::set(&w, &"__f1xDeferred".into(), &JsValue::NULL);
            }
            mode.set(Install::Hidden);
        }
    };
    dynamic(move || {
        let body = match mode.get() {
            Install::Hidden => return Node::Empty,
            Install::Prompt => fragment([
                Node::from(p().class("muted").text(t(
                    "Ajoute F1X à ton écran d'accueil : plein écran, lancement instantané et consultation hors ligne.",
                    "Add F1X to your home screen: full screen, instant launch and offline access.",
                ))),
                button()
                    .class("btn")
                    .on_click(install)
                    .text(t("Installer l'app", "Install the app"))
                    .into(),
            ]),
            Install::Ios => p()
                .class("muted")
                .text(t(
                    "Dans Safari, touche Partager (carré avec une flèche) puis « Sur l'écran d'accueil » : F1X s'ouvrira en plein écran, comme une app.",
                    "In Safari, tap Share (square with an arrow), then “Add to Home Screen”: F1X opens full screen, like an app.",
                ))
                .into(),
        };
        section()
            .class("card install-card")
            .child(
                div()
                    .class("card-head")
                    .child(h2().text(t("📲 Installer F1X", "📲 Install F1X")))
                    .child(
                        button()
                            .class("install-close")
                            .attr("aria-label", t("Masquer", "Hide"))
                            .on_click(dismiss)
                            .text("✕"),
                    ),
            )
            .child(body)
            .into()
    })
}

/// Pied de page de l'app : liens légaux, présentation, mention non officielle.
pub fn app_footer() -> Node {
    let code = lang().code();
    let link = |path: &str, label: &'static str| {
        crate::components::server_link(format!("{path}?lang={code}")).text(label)
    };
    footer()
        .class("app-foot")
        .child(
            p().class("brand app-foot-brand")
                .child(span().class("brand-mark").text("F1"))
                .child(span().class("brand-x").text("X")),
        )
        .child(
            nav()
                .class("app-foot-links")
                .attr("aria-label", t("Informations", "Information"))
                .child(link("/presentation", t("Présentation", "About")))
                .child(link("/mentions-legales", t("Mentions légales", "Legal notice")))
                .child(link("/confidentialite", t("Confidentialité", "Privacy")))
                .child(link("/credits", t("Crédits et sources", "Credits & sources"))),
        )
        .child(
            p().class("app-foot-author")
                .text(t("Conçu et développé par ", "Designed and built by "))
                .child(strong().text("Maxime Nathan Lestage")),
        )
        .child(p().text(crate::tr!(
            "© {} F1X · Tous droits réservés",
            "© {} F1X · All rights reserved",
            crate::util::current_year()
        )))
        .child(p().class("app-foot-note").text(t(
            "Site non officiel, sans lien avec la Formula 1, la FIA ou les écuries. F1 et Formula 1 sont des marques de Formula One Licensing B.V.",
            "Unofficial site, not affiliated with Formula 1, the FIA or the teams. F1 and Formula 1 are trademarks of Formula One Licensing B.V.",
        )))
        .into()
}
