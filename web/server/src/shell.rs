//! Page d'accueil de l'application monopage, écrite avec active : `<head>` complet (SEO,
//! fiche de partage, icônes, application installable), écran de démarrage et `#app`, que le
//! module WebAssembly (le frontend active) remplit au démarrage.

use active::prelude::*;

/// Apple Maps (MapKit JS) pour la fiche circuit : jeton signé par le serveur, repli
/// OpenStreetMap si refus.
const MAPKIT: &str = include_str!("../static/js/mapkit.js");
/// Animation de démarrage, une fois par session.
const SPLASH: &str = include_str!("../static/js/splash.js");
/// Moteur d'animations (curseur, révélations, chiffres qui comptent, ondes au toucher).
const MOTION: &str = include_str!("../static/js/motion.js");
/// Démarrage du module WebAssembly avec filet de sécurité, invite d'installation, service worker.
const BOOT: &str = include_str!("../static/js/boot.js");

/// Balises du `<head>` hors SEO : thème, application installable, icônes, police.
const HEAD: &str = concat!(
    r##"<meta name="theme-color" content="#0e0d0c"/><meta name="color-scheme" content="dark"/>"##,
    r#"<meta name="application-name" content="F1X"/>"#,
    r#"<meta name="apple-mobile-web-app-title" content="F1X"/>"#,
    r#"<meta name="apple-mobile-web-app-capable" content="yes"/>"#,
    r#"<meta name="mobile-web-app-capable" content="yes"/>"#,
    r#"<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent"/>"#,
    r#"<meta name="format-detection" content="telephone=no"/>"#,
    r#"<link rel="manifest" href="/manifest.webmanifest"/>"#,
    r#"<link rel="icon" href="/favicon.ico" sizes="48x48"/>"#,
    r#"<link rel="icon" href="/static/icon.svg" type="image/svg+xml"/>"#,
    r#"<link rel="icon" href="/static/img/favicon-32.png" type="image/png" sizes="32x32"/>"#,
    r#"<link rel="apple-touch-icon" href="/apple-touch-icon.png"/>"#,
    r#"<link rel="preconnect" href="https://fonts.googleapis.com"/>"#,
    r#"<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin=""/>"#,
);

const FONT: &str =
    "https://fonts.googleapis.com/css2?family=Archivo:wdth,wght@62..125,100..900&display=swap";

const TITLE: &str = "F1X — Toute la Formule 1 dans ta poche";
const SHORT: &str = "Race Center en temps réel, circuits et monoplaces en 3D, 75 ans d'archives. Gratuit, sans compte.";

/// `<script>` en ligne (code de confiance, écrit dans le dépôt).
fn script(code: impl Into<active::Str>) -> Element {
    el("script").html(code)
}

/// Le logo « F1X ».
fn brand() -> Element {
    span()
        .class("brand")
        .child(span().class("brand-mark").text("F1"))
        .child(span().class("brand-x").text("X"))
}

/// La page servie pour toutes les adresses de l'app ; `origin` : adresse publique du site
/// (liens absolus de la fiche de partage).
pub fn render(origin: &str) -> String {
    let (app, css, car) = (
        env!("F1X_APP_HASH"),
        env!("F1X_CSS_HASH"),
        crate::assets::car_hash(),
    );
    let image = format!("{origin}/static/img/og-fr.png");
    let seo = Seo::new()
        .title(TITLE)
        .description("Calendrier, résultats, classements, Race Center en temps réel, circuits et monoplaces en 3D, 75 ans d'archives de Formule 1. Gratuit, sans compte, pensé pour le mobile.")
        .canonical(format!("{origin}/"))
        .site_name("F1X")
        .locale("fr_FR")
        .image(image.clone())
        .meta_property("og:locale:alternate", "en_GB")
        .meta_property("og:image:type", "image/png")
        .meta_property("og:image:width", "1200")
        .meta_property("og:image:height", "630")
        .meta_property(
            "og:image:alt",
            "F1X, l'application Formule 1 : accueil et monoplace en 3D sur téléphone",
        )
        .meta_name("twitter:title", TITLE)
        .meta_name("twitter:description", SHORT)
        .meta_name("twitter:image", image);

    // Ouverture : le signe F1X se trace pendant que le compteur monte jusqu'à 100 %.
    let splash = div()
        .id("splash")
        .attr("aria-hidden", "true")
        .child(
            svg()
                .class("porte-signe")
                .attr("viewBox", "0 0 200 100")
                .child(text_svg().attr("x", "6").attr("y", "80").text("F1"))
                .child(
                    text_svg()
                        .class("trace-x")
                        .attr("x", "122")
                        .attr("y", "80")
                        .text("X"),
                ),
        )
        .child(
            p().class("porte-ligne")
                .text("Formule 1 · saison en direct"),
        )
        .child(
            div()
                .class("porte-compte")
                .child(span().id("porte-n").text("0")),
        );
    let body = fragment([
        Node::from(splash),
        div().class("grain").attr("aria-hidden", "true").into(),
        div()
            .class("curseur")
            .attr("aria-hidden", "true")
            .child(span())
            .into(),
        script(MAPKIT).into(),
        script(SPLASH).into(),
        script(MOTION).into(),
        // Écran de démarrage, remplacé par l'app.
        div()
            .id("app")
            .child(div().class("boot").child(brand()))
            .into(),
        el("noscript")
            .child(p().class("boot").text(
                "F1X a besoin de JavaScript/WebAssembly activé. · F1X needs JavaScript/WebAssembly.",
            ))
            .into(),
        // Seul chargeur JavaScript de l'app : démarre le module WebAssembly (active) avec un
        // filet de sécurité si le démarrage échoue ou n'aboutit pas.
        script(BOOT.replace("{{APP}}", app).replace("{{CAR}}", car)).into(),
    ]);
    Document::new(body)
        .lang("fr")
        .viewport("width=device-width, initial-scale=1, viewport-fit=cover")
        .seo(seo)
        .head(HEAD)
        .stylesheet(FONT)
        .stylesheet(format!("/static/app.css?v={css}"))
        // Le module et son binaire se téléchargent pendant la lecture de la page.
        .head(format!(
            r#"<link rel="modulepreload" href="/pkg/{app}/f1x_frontend.js"/><link rel="preload" href="/pkg/{app}/f1x_frontend_bg.wasm" as="fetch" type="application/wasm" crossorigin=""/>"#
        ))
        .render()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_shell_loads_the_app_and_describes_the_site() {
        let html = super::render("https://x.test");
        assert!(html.starts_with(r#"<!DOCTYPE html><html lang="fr"><head>"#));
        assert!(
            html.contains(r#"content="width=device-width, initial-scale=1, viewport-fit=cover""#)
        );
        assert_eq!(html.matches("<title>").count(), 1);
        assert!(html.contains(r#"<link rel="canonical" href="https://x.test/"/>"#));
        assert!(html.contains(
            r#"<meta property="og:image" content="https://x.test/static/img/og-fr.png"/>"#
        ));
        assert!(html.contains(r#"<div id="app"><div class="boot">"#));
        assert!(html.contains(&format!("/pkg/{}/f1x_frontend.js", env!("F1X_APP_HASH"))));
        assert!(!html.contains("{{"));
        assert!(html.ends_with("</script></body></html>"));
    }
}
