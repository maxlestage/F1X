//! Application installable (PWA) et pied de page.

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use yew::prelude::*;

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
#[function_component]
pub fn InstallCard() -> Html {
    let mode = use_state(detect);
    {
        let mode = mode.clone();
        use_effect_with((), move |_| {
            let listener = Closure::<dyn Fn()>::new(move || mode.set(detect()));
            let win = web_sys::window();
            if let Some(w) = &win {
                let _ = w.add_event_listener_with_callback(
                    "f1x-installable",
                    listener.as_ref().unchecked_ref(),
                );
                let _ = w.add_event_listener_with_callback(
                    "appinstalled",
                    listener.as_ref().unchecked_ref(),
                );
            }
            move || {
                if let Some(w) = &win {
                    let _ = w.remove_event_listener_with_callback(
                        "f1x-installable",
                        listener.as_ref().unchecked_ref(),
                    );
                    let _ = w.remove_event_listener_with_callback(
                        "appinstalled",
                        listener.as_ref().unchecked_ref(),
                    );
                }
            }
        });
    }
    let dismiss = {
        let mode = mode.clone();
        Callback::from(move |_: MouseEvent| {
            crate::util::store(DISMISSED, &true);
            mode.set(Install::Hidden);
        })
    };
    let install = {
        let mode = mode.clone();
        Callback::from(move |_: MouseEvent| {
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
        })
    };
    let body = match *mode {
        Install::Hidden => return html! {},
        Install::Prompt => html! {
            <>
                <p class="muted">{ t(
                    "Ajoute F1X à ton écran d'accueil : plein écran, lancement instantané et consultation hors ligne.",
                    "Add F1X to your home screen: full screen, instant launch and offline access.",
                ) }</p>
                <button class="btn" onclick={install}>{ t("Installer l'app", "Install the app") }</button>
            </>
        },
        Install::Ios => html! {
            <p class="muted">{ t(
                "Dans Safari, touche Partager (carré avec une flèche) puis « Sur l'écran d'accueil » : F1X s'ouvrira en plein écran, comme une app.",
                "In Safari, tap Share (square with an arrow), then “Add to Home Screen”: F1X opens full screen, like an app.",
            ) }</p>
        },
    };
    html! {
        <section class="card install-card">
            <div class="card-head">
                <h2>{ t("📲 Installer F1X", "📲 Install F1X") }</h2>
                <button class="install-close" aria-label={t("Masquer", "Hide")} onclick={dismiss}>{ "✕" }</button>
            </div>
            { body }
        </section>
    }
}

/// Pied de page de l'app : liens légaux, présentation, mention non officielle.
#[function_component]
pub fn AppFooter() -> Html {
    let code = lang().code();
    let link = |path: &str, label: &'static str| {
        html! { <a href={format!("{path}?lang={code}")}>{ label }</a> }
    };
    html! {
        <footer class="app-foot">
            <p class="brand app-foot-brand"><span class="brand-mark">{ "F1" }</span><span class="brand-x">{ "X" }</span></p>
            <nav class="app-foot-links" aria-label={t("Informations", "Information")}>
                { link("/presentation", t("Présentation", "About")) }
                { link("/mentions-legales", t("Mentions légales", "Legal notice")) }
                { link("/confidentialite", t("Confidentialité", "Privacy")) }
                { link("/credits", t("Crédits et sources", "Credits & sources")) }
            </nav>
            <p class="app-foot-author">{ t("Conçu et développé par ", "Designed and built by ") }<strong>{ "Maxime Nathan Lestage" }</strong></p>
            <p>{ crate::tr!("© {} F1X · Tous droits réservés", "© {} F1X · All rights reserved", crate::util::current_year()) }</p>
            <p class="app-foot-note">{ t(
                "Site non officiel, sans lien avec la Formula 1, la FIA ou les écuries. F1 et Formula 1 sont des marques de Formula One Licensing B.V.",
                "Unofficial site, not affiliated with Formula 1, the FIA or the teams. F1 and Formula 1 are trademarks of Formula One Licensing B.V.",
            ) }</p>
        </footer>
    }
}
