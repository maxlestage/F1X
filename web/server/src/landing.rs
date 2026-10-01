//! Site de présentation (`/presentation`) et pages légales : HTML + CSS rendus par le serveur,
//! sans JavaScript. Bilingue : `?lang=fr|en`, sinon langue du navigateur (`Accept-Language`).

use axum::extract::Query;
use axum::http::{HeaderMap, header};
use axum::response::{Html, IntoResponse, Response};
use chrono::Datelike;
use serde::Deserialize;

use crate::assets::origin;

#[derive(Deserialize)]
pub struct LangQuery {
    lang: Option<String>,
    /// Page de l'app d'où l'on vient, pour y revenir.
    back: Option<String>,
}

/// N'accepte qu'un chemin interne simple (évite toute redirection vers un autre site).
fn safe_back(back: Option<&str>) -> Option<String> {
    let b = back?;
    let ok = b.starts_with('/')
        && !b.starts_with("//")
        && b.len() < 200
        && !b.starts_with("/presentation")
        && b.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'));
    ok.then(|| b.to_string())
}

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Fr,
    En,
}

impl Lang {
    fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }
    fn t(self, fr: &'static str, en: &'static str) -> &'static str {
        if self == Lang::Fr { fr } else { en }
    }
}

fn pick_lang(q: &LangQuery, headers: &HeaderMap) -> Lang {
    match q.lang.as_deref() {
        Some("en") => Lang::En,
        Some("fr") => Lang::Fr,
        _ => {
            let accept = headers
                .get(header::ACCEPT_LANGUAGE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if accept.trim_start().to_lowercase().starts_with("fr") || accept.is_empty() {
                Lang::Fr
            } else {
                Lang::En
            }
        }
    }
}

/// Échappement HTML des valeurs venant de la configuration.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Captures d'écran réelles de l'app (générées avec Playwright), embarquées dans le binaire.
macro_rules! shots {
    ($($name:literal),* $(,)?) => {
        &[$(
            (concat!($name, "-fr.jpg"), include_bytes!(concat!("../static/shots/", $name, "-fr.jpg")) as &[u8]),
            (concat!($name, "-en.jpg"), include_bytes!(concat!("../static/shots/", $name, "-en.jpg")) as &[u8]),
        )*]
    };
}

pub const SHOTS: &[(&str, &[u8])] = shots!(
    "home",
    "live",
    "strategy",
    "track",
    "records",
    "compare",
    "car3d",
    "circuit3d",
    "onboard",
    "photos",
);

pub async fn shot(axum::extract::Path(name): axum::extract::Path<String>) -> Response {
    match SHOTS.iter().find(|(n, _)| *n == name) {
        Some((_, body)) => (
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::CACHE_CONTROL, "public, max-age=86400"),
            ],
            *body,
        )
            .into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

fn respond(html: String) -> Response {
    (
        [
            (header::CACHE_CONTROL, "public, max-age=600"),
            (header::VARY, "Accept-Language"),
        ],
        Html(html),
    )
        .into_response()
}

pub async fn page(Query(q): Query<LangQuery>, headers: HeaderMap) -> Response {
    let lang = pick_lang(&q, &headers);
    let back = safe_back(q.back.as_deref());
    respond(render(lang, back.as_deref(), &origin(&headers)))
}

pub async fn legal(Query(q): Query<LangQuery>, headers: HeaderMap) -> Response {
    let lang = pick_lang(&q, &headers);
    respond(legal_page(lang, &origin(&headers)))
}

pub async fn privacy(Query(q): Query<LangQuery>, headers: HeaderMap) -> Response {
    let lang = pick_lang(&q, &headers);
    respond(privacy_page(lang, &origin(&headers)))
}

pub async fn credits(Query(q): Query<LangQuery>, headers: HeaderMap) -> Response {
    let lang = pick_lang(&q, &headers);
    respond(credits_page(lang, &origin(&headers)))
}

// ---------- Gabarit commun ----------

struct Meta<'a> {
    /// Chemin de la page (sans `?lang=`), pour les liens absolus et la bascule de langue.
    path: &'a str,
    title: &'a str,
    desc: &'a str,
    /// Lien du bouton du haut (retour à l'app).
    app: &'a str,
    back_q: &'a str,
}

fn shell(lang: Lang, origin: &str, m: Meta, body: &str) -> String {
    let t = |f, e| lang.t(f, e);
    let code = lang.code();
    let (other_code, other_label) = if lang == Lang::Fr {
        ("en", "EN")
    } else {
        ("fr", "FR")
    };
    let (locale, alt_locale) = if lang == Lang::Fr {
        ("fr_FR", "en_GB")
    } else {
        ("en_GB", "fr_FR")
    };
    let Meta {
        path,
        title,
        desc,
        app,
        back_q,
    } = m;
    let footer = footer(lang, app);
    let top_button = t("← Retour à l'app", "← Back to the app");
    let og_alt = t(
        "F1X, l'application Formule 1 : accueil et monoplace en 3D sur téléphone",
        "F1X, the Formula 1 app: home screen and 3D car on a phone",
    );
    format!(
        r##"<!doctype html>
<html lang="{code}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="theme-color" content="#0b0b10">
<meta name="color-scheme" content="dark">
<title>{title}</title>
<meta name="description" content="{desc}">
<link rel="canonical" href="{origin}{path}?lang={code}">
<link rel="alternate" hreflang="fr" href="{origin}{path}?lang=fr">
<link rel="alternate" hreflang="en" href="{origin}{path}?lang=en">
<meta property="og:type" content="website">
<meta property="og:site_name" content="F1X">
<meta property="og:locale" content="{locale}">
<meta property="og:locale:alternate" content="{alt_locale}">
<meta property="og:url" content="{origin}{path}?lang={code}">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{desc}">
<meta property="og:image" content="{origin}/static/img/og-{code}.png">
<meta property="og:image:type" content="image/png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="{og_alt}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{title}">
<meta name="twitter:description" content="{desc}">
<meta name="twitter:image" content="{origin}/static/img/og-{code}.png">
<link rel="manifest" href="/manifest.webmanifest">
<link rel="icon" href="/favicon.ico" sizes="48x48">
<link rel="icon" href="/static/icon.svg" type="image/svg+xml">
<link rel="icon" href="/static/img/favicon-32.png" type="image/png" sizes="32x32">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<style>{CSS}</style>
</head>
<body>
<header class="top">
  <a class="brand" href="/presentation?lang={code}" aria-label="F1X"><span>F1</span><span class="x">X</span></a>
  <nav class="top-links">
    <a class="lang" href="{path}?lang={other_code}{back_q}" hreflang="{other_code}">{other_label}</a>
    <a class="btn btn-small" href="{app}">{top_button}</a>
  </nav>
</header>
<main>
{body}
</main>
{footer}
</body>
</html>"##
    )
}

/// Pied de page commun (site de présentation et pages légales).
fn footer(lang: Lang, app: &str) -> String {
    let t = |f, e| lang.t(f, e);
    let code = lang.code();
    let year = chrono::Utc::now().year();
    let list = |items: &[(&str, &str)]| -> String {
        items
            .iter()
            .map(|(href, label)| format!(r#"<li><a href="{href}">{label}</a></li>"#))
            .collect()
    };
    let app_links = list(&[
        ("/", t("Accueil", "Home")),
        ("/direct", t("Direct et replays", "Live & replays")),
        ("/saison/current", t("Calendrier", "Calendar")),
        ("/saison/current/pilotes", t("Classements", "Standings")),
        ("/archives", t("Archives depuis 1950", "History since 1950")),
    ]);
    let discover = [
        (
            format!("/presentation?lang={code}#fonctionnalites"),
            t("Fonctionnalités", "Features"),
        ),
        (
            format!("/presentation?lang={code}#installer"),
            t("Installer l'app", "Install the app"),
        ),
        ("/lexique".to_string(), t("Lexique de la F1", "F1 glossary")),
        ("/archives/records".to_string(), "Records"),
        ("/actus".to_string(), t("Actualités", "News")),
    ]
    .iter()
    .map(|(href, label)| format!(r#"<li><a href="{href}">{label}</a></li>"#))
    .collect::<String>();
    let legal_links = list(&[
        (
            &format!("/mentions-legales?lang={code}"),
            t("Mentions légales", "Legal notice"),
        ),
        (
            &format!("/confidentialite?lang={code}"),
            t("Confidentialité", "Privacy"),
        ),
        (
            &format!("/credits?lang={code}"),
            t("Crédits et sources", "Credits & sources"),
        ),
    ]);
    format!(
        r#"<footer class="foot">
  <div class="foot-grid">
    <div class="foot-brand">
      <a class="brand" href="/presentation?lang={code}" aria-label="F1X"><span>F1</span><span class="x">X</span></a>
      <p>{tagline}</p>
      <a class="btn btn-small" href="{app}">{open} <span aria-hidden="true">→</span></a>
    </div>
    <nav aria-label="{h_app}"><h3>{h_app}</h3><ul>{app_links}</ul></nav>
    <nav aria-label="{h_discover}"><h3>{h_discover}</h3><ul>{discover}</ul></nav>
    <nav aria-label="{h_legal}"><h3>{h_legal}</h3><ul>{legal_links}</ul></nav>
  </div>
  <div class="foot-bottom">
    <p>© {year} F1X. {rights}</p>
    <p class="foot-lang"><a href="?lang=fr" hreflang="fr" lang="fr">Français</a><span aria-hidden="true">·</span><a href="?lang=en" hreflang="en" lang="en">English</a></p>
  </div>
  <p class="foot-legal">{disclaimer}</p>
</footer>"#,
        tagline = t(
            "Toute la Formule 1 dans ta poche : direct, circuits en 3D, 75 ans d'archives.",
            "All of Formula 1 in your pocket: live timing, 3D circuits, 75 years of history.",
        ),
        open = t("Ouvrir l'app", "Open the app"),
        h_app = t("Application", "App"),
        h_discover = t("Découvrir", "Discover"),
        h_legal = t("Informations légales", "Legal"),
        rights = t("Tous droits réservés.", "All rights reserved."),
        disclaimer = t(
            "F1X est un service indépendant et non officiel, sans lien avec Formula One Group, la FIA ou les écuries. F1, FORMULA ONE, FORMULA 1, GRAND PRIX et les marques associées appartiennent à Formula One Licensing B.V. Les noms d'écuries et de pilotes sont cités à titre informatif.",
            "F1X is an independent, unofficial service, not affiliated with Formula One Group, the FIA or the teams. F1, FORMULA ONE, FORMULA 1, GRAND PRIX and related marks are trademarks of Formula One Licensing B.V. Team and driver names are used for information purposes only.",
        ),
    )
}

// ---------- Présentation ----------

fn render(lang: Lang, back: Option<&str>, origin: &str) -> String {
    let t = |f: &'static str, e: &'static str| lang.t(f, e);
    let code = lang.code();
    let img = |name: &str, alt: &str| {
        format!(
            r#"<figure class="phone"><img src="/presentation/shots/{name}-{code}.jpg" alt="{alt}" width="390" height="844" loading="lazy"></figure>"#
        )
    };

    let feature = |id: &str,
                   eyebrow: &str,
                   title: &str,
                   text: &str,
                   bullets: &[&str],
                   shots: &str,
                   reverse: bool| {
        let items: String = bullets.iter().map(|b| format!("<li>{b}</li>")).collect();
        format!(
            r#"<section class="feature{rev}" id="{id}"><div class="feature-text"><p class="eyebrow">{eyebrow}</p><h2>{title}</h2><p>{text}</p><ul class="checks">{items}</ul></div><div class="feature-shots">{shots}</div></section>"#,
            rev = if reverse { " reverse" } else { "" }
        )
    };

    let features = [
        feature(
            "race-center",
            "Race Center",
            t("La course comme si tu y étais.", "The race as if you were there."),
            t(
                "Rejoue n'importe quelle session depuis 2023 comme en direct, de ×1 à ×60, avec pause et retour en arrière. Tout arrive en temps réel par WebSocket.",
                "Replay any session since 2023 as if it were live, from ×1 to ×60, with pause and rewind. Everything streams in real time over WebSocket.",
            ),
            &[
                t("Classement, écarts avec la voiture devant et le leader", "Order, gaps to the car ahead and to the leader"),
                t("Secteurs violet / vert / jaune, pneus et âge des gommes", "Purple / green / yellow sectors, tyres and tyre age"),
                t("Carte en 2D ou en relief 3D : les voitures roulent sur le circuit", "2D or 3D relief map: watch the cars lap the circuit"),
                t("« Et s'il s'arrêtait maintenant ? » : position de sortie des stands", "“What if they pit now?”: projected rejoin position"),
            ],
            &(img("live", t("Race Center en replay", "Race Center replay")) + &img("strategy", t("Stratégie des pneus", "Tyre strategy"))),
            false,
        ),
        feature(
            "monoplaces-3d",
            t("Nouveau · 3D", "New · 3D"),
            t("Les monoplaces en 3D.", "The cars in 3D."),
            t(
                "Chaque écurie a sa monoplace en 3D, à ses couleurs. Fais-la tourner du doigt, zoome, admire les ailerons, le halo et les gommes.",
                "Every team has its car in 3D, in its colours. Spin it with your finger, zoom in, check out the wings, the halo and the tyres.",
            ),
            &[
                t("Rendu en temps réel (WebGL 2), fluide sur téléphone", "Real-time rendering (WebGL 2), smooth on phones"),
                t("Sur chaque page écurie et pilote", "On every team and driver page"),
                t("Modèle stylisé original, généré par le code", "Original stylised model, generated by code"),
            ],
            &img("car3d", t("Monoplace en 3D aux couleurs de l'écurie", "3D car in team colours")),
            true,
        ),
        feature(
            "circuits",
            "Circuits",
            t("Chaque circuit, au GPS et en relief.", "Every circuit, from GPS, in 3D."),
            t(
                "Le tracé est reconstitué à partir des positions réelles d'une voiture sur son meilleur tour, avec l'altitude. Survole-le en 3D ou monte à bord : la voiture rejoue le tour à vitesse réelle.",
                "The layout is rebuilt from a real car's positions on its fastest lap, elevation included. Fly over it in 3D or ride onboard: the car replays the lap at real speed.",
            ),
            &[
                t("Relief réel : Raidillon, Monaco, Suzuka comme tu ne les as jamais vus", "Real elevation: Raidillon, Monaco, Suzuka like never before"),
                t("Caméra embarquée sur le meilleur tour", "Onboard camera on the fastest lap"),
                t("Plan 2D : vitesse, rapport, gaz et freinage en chaque point", "2D map: speed, gear, throttle and braking at every point"),
                t("Longueur, vitesses max / mini / moyenne, palmarès, rois du circuit", "Length, top / min / average speed, winners, kings of the circuit"),
            ],
            &(img("circuit3d", t("Circuit de Spa en relief 3D", "Spa circuit in 3D relief")) + &img("onboard", t("Caméra embarquée à Spa", "Onboard camera at Spa"))),
            false,
        ),
        feature(
            "histoire",
            t("75 ans d'histoire", "75 years of history"),
            t("Toutes les saisons depuis 1950.", "Every season since 1950."),
            t(
                "Calendriers, résultats, classements, carrières des pilotes avec leur photo, palmarès des écuries : toute l'histoire de la F1 est à portée de pouce.",
                "Calendars, results, standings, driver careers with photos, team records: the whole history of F1 at your fingertips.",
            ),
            &[
                t("Photos et avatars des pilotes (images libres, créditées)", "Driver photos and avatars (free, credited images)"),
                t("Records : titres, victoires, poles, séries, plus jeunes vainqueurs", "Records: titles, wins, poles, streaks, youngest winners"),
                t("880+ pilotes et 210+ écuries, avec recherche", "880+ drivers and 210+ teams, searchable"),
                t("Analyse tour par tour des Grands Prix depuis 1996", "Lap-by-lap analysis of Grands Prix since 1996"),
            ],
            &(img("photos", t("Fiche pilote avec photo", "Driver page with photo")) + &img("records", t("Records de la F1", "F1 records"))),
            true,
        ),
        feature(
            "fans",
            t("Pour les fans", "For fans"),
            t("Compare, pronostique, joue.", "Compare, predict, play."),
            t(
                "Mets deux pilotes face à face, pronostique chaque Grand Prix, monte ton équipe Fantasy et teste tes connaissances.",
                "Put two drivers head to head, predict every Grand Prix, build your Fantasy team and test your knowledge.",
            ),
            &[
                t("Comparateur : statistiques et face-à-face en course", "Compare: stats and head-to-head results"),
                t("Pronostics notés automatiquement après la course", "Predictions scored automatically after the race"),
                t("Fantasy F1 (100 M€) et quiz « Devine le pilote »", "Fantasy F1 (€100M) and “Guess the driver” quiz"),
            ],
            &img("compare", t("Comparateur Hamilton / Verstappen", "Hamilton / Verstappen comparison")),
            false,
        ),
    ]
    .join("");

    let cards = [
        ("📲", t("Installable", "Installable"), t("Sur l'écran d'accueil, en plein écran, comme une app.", "On your home screen, full screen, like an app.")),
        ("📶", t("Hors ligne", "Offline"), t("Les dernières données consultées restent disponibles.", "The latest data you viewed stays available.")),
        ("🌦️", t("Météo du week-end", "Weekend weather"), t("Prévisions par session et impact sur la course.", "Forecast per session and race impact.")),
        ("⏱", t("Compte à rebours", "Countdown"), t("Prochaine séance à ton heure locale.", "Next session in your local time.")),
        ("📰", t("Actualités", "News"), t("Les derniers titres de la presse F1.", "Latest F1 headlines.")),
        ("⭐", t("Favoris", "Favourites"), t("Ton pilote et ton écurie mis en avant.", "Your driver and team highlighted.")),
        ("🌍", t("Français / English", "English / Français"), t("Toute l'app dans les deux langues.", "The whole app in both languages.")),
        ("📱", t("Pensée pour le mobile", "Mobile first"), t("Zéro défilement horizontal, lisible d'une main.", "No horizontal scrolling, one-handed reading.")),
        ("⬇", t("Export CSV", "CSV export"), t("Classements et résultats à télécharger.", "Download standings and results.")),
        ("📚", t("Lexique", "Glossary"), t("Drapeaux, pneus, stratégie, règlement 2026.", "Flags, tyres, strategy, 2026 rules.")),
    ]
    .iter()
    .map(|(i, h, p)| format!(r#"<li class="mini"><span class="mini-icon" aria-hidden="true">{i}</span><strong>{h}</strong><span>{p}</span></li>"#))
    .collect::<String>();

    let app = back.unwrap_or("/");
    let back_q = back
        .map(|b| format!("&amp;back={}", b.replace('/', "%2F")))
        .unwrap_or_default();
    let cta = t("Voir l'app web", "See the web app");
    let body = format!(
        r##"  <section class="hero">
    <div class="hero-text">
      <p class="eyebrow">{eyebrow}</p>
      <h1>{h1}</h1>
      <p class="lead">{lead}</p>
      <div class="ctas">
        <a class="btn btn-big" href="{app}">{cta} <span aria-hidden="true">→</span></a>
        <a class="btn btn-ghost" href="#fonctionnalites">{discover}</a>
      </div>
      <p class="note">{note}</p>
    </div>
    <div class="hero-shot">{hero_img}</div>
  </section>

  <ul class="numbers" aria-label="{numbers_label}">
    <li><strong>77</strong><span>{seasons}</span></li>
    <li><strong>1 170+</strong><span>Grands Prix</span></li>
    <li><strong>880+</strong><span>{drivers}</span></li>
    <li><strong>3D</strong><span>{three_d}</span></li>
  </ul>

  <div id="fonctionnalites">{features}</div>

  <section class="band">
    <h2>{more}</h2>
    <ul class="minis">{cards}</ul>
  </section>

  <section class="band install" id="installer">
    <p class="eyebrow">{install_eyebrow}</p>
    <h2>{install_h}</h2>
    <p class="band-lead">{install_lead}</p>
    <ol class="steps">
      <li><strong>iPhone · iPad (Safari)</strong><span>{install_ios}</span></li>
      <li><strong>Android (Chrome)</strong><span>{install_android}</span></li>
      <li><strong>{desktop}</strong><span>{install_desktop}</span></li>
    </ol>
  </section>

  <section class="band tech">
    <p class="eyebrow">{tech_eyebrow}</p>
    <h2>{tech_h}</h2>
    <p>{tech_p}</p>
    <ul class="chips"><li>Rust</li><li>WebAssembly</li><li>Yew</li><li>WebGL 2</li><li>WebSocket</li><li>PWA</li></ul>
  </section>

  <section class="final">
    <h2>{final_h}</h2>
    <a class="btn btn-big" href="{app}">{cta} <span aria-hidden="true">→</span></a>
  </section>"##,
        eyebrow = t("Application web gratuite", "Free web app"),
        h1 = t(
            "Toute la Formule 1 dans ta poche.",
            "All of Formula 1 in your pocket."
        ),
        lead = t(
            "Race Center en temps réel, monoplaces et circuits en 3D, photos des pilotes et 75 ans d'archives. Rapide, clair, pensé pour le téléphone.",
            "A real-time Race Center, 3D cars and circuits, driver photos and 75 years of history. Fast, clear and built for your phone.",
        ),
        discover = t("Découvrir", "Discover"),
        note = t(
            "Sans compte, sans publicité, installable en un geste.",
            "No account, no ads, installs in one tap."
        ),
        hero_img = img(
            "home",
            t(
                "Accueil de F1X : prochain Grand Prix et compte à rebours",
                "F1X home: next Grand Prix and countdown"
            )
        ),
        numbers_label = t("F1X en chiffres", "F1X in numbers"),
        seasons = t("saisons", "seasons"),
        drivers = t("pilotes", "drivers"),
        three_d = t("monoplaces et circuits", "cars and circuits"),
        more = t("Et aussi", "And also"),
        install_eyebrow = t("Application installable", "Installable app"),
        install_h = t(
            "Installe-la comme une vraie app.",
            "Install it like a real app."
        ),
        install_lead = t(
            "Icône sur l'écran d'accueil, ouverture en plein écran, lancement instantané et consultation hors ligne. Rien à télécharger sur un store.",
            "Home screen icon, full screen, instant launch and offline access. Nothing to download from a store.",
        ),
        install_ios = t(
            "Ouvre F1X, touche Partager puis « Sur l'écran d'accueil ».",
            "Open F1X, tap Share, then “Add to Home Screen”."
        ),
        install_android = t(
            "Ouvre F1X : touche « Installer l'app » sur l'accueil, ou menu ⋮ puis « Installer l'application ».",
            "Open F1X: tap “Install the app” on the home screen, or ⋮ menu then “Install app”."
        ),
        desktop = t("Ordinateur (Chrome, Edge)", "Desktop (Chrome, Edge)"),
        install_desktop = t(
            "Clique sur l'icône d'installation dans la barre d'adresse.",
            "Click the install icon in the address bar."
        ),
        tech_eyebrow = t("Sous le capot", "Under the hood"),
        tech_h = t(
            "Écrite en Rust, de bout en bout.",
            "Written in Rust, end to end."
        ),
        tech_p = t(
            "L'interface et le rendu 3D sont en Rust compilé en WebAssembly, le serveur aussi. Le temps réel passe par WebSocket, les données sont mises en cache pour rester rapides, même en 4G.",
            "The interface and the 3D rendering are Rust compiled to WebAssembly, and so is the server. Real time runs over WebSocket, and data is cached to stay fast, even on 4G.",
        ),
        final_h = t(
            "Prêt pour le prochain Grand Prix ?",
            "Ready for the next Grand Prix?"
        ),
    );
    shell(
        lang,
        origin,
        Meta {
            path: "/presentation",
            title: t(
                "F1X — Toute la Formule 1 dans ta poche",
                "F1X — All of Formula 1 in your pocket",
            ),
            desc: t(
                "Application web gratuite : Race Center en temps réel, monoplaces et circuits en 3D, photos des pilotes, 75 ans d'archives, pronostics et Fantasy. Installable, en français et en anglais.",
                "Free web app: real-time Race Center, 3D cars and circuits, driver photos, 75 years of history, predictions and Fantasy. Installable, in English and French.",
            ),
            app,
            back_q: &back_q,
        },
        &body,
    )
}

// ---------- Pages légales ----------

fn doc_page(
    lang: Lang,
    origin: &str,
    path: &str,
    title: &str,
    desc: &str,
    content: &str,
) -> String {
    let updated = lang.t(
        "Dernière mise à jour : 1er octobre 2026",
        "Last updated: 1 October 2026",
    );
    let body = format!(
        r#"  <article class="doc">
    <h1>{title}</h1>
    <p class="doc-date">{updated}</p>
{content}
  </article>"#
    );
    shell(
        lang,
        origin,
        Meta {
            path,
            title: &format!("{title} · F1X"),
            desc,
            app: "/",
            back_q: "",
        },
        &body,
    )
}

/// Éditeur du site, renseigné par l'exploitant (variables d'environnement Heroku).
struct Publisher {
    name: String,
    address: Option<String>,
    contact: Option<String>,
    registration: Option<String>,
}

fn publisher() -> Publisher {
    let var = |k: &str| {
        std::env::var(k)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .map(|v| esc(&v))
    };
    Publisher {
        name: var("LEGAL_PUBLISHER").unwrap_or_else(|| "F1X".into()),
        address: var("LEGAL_ADDRESS"),
        contact: var("LEGAL_CONTACT"),
        registration: var("LEGAL_REGISTRATION"),
    }
}

fn legal_page(lang: Lang, origin: &str) -> String {
    let p = publisher();
    let fr = lang == Lang::Fr;
    let mut editor = format!("<p><strong>{}</strong>", p.name);
    if let Some(a) = &p.address {
        editor.push_str(&format!("<br>{a}"));
    }
    if let Some(r) = &p.registration {
        editor.push_str(&format!("<br>{r}"));
    }
    if let Some(c) = &p.contact {
        let sep = if fr { " :" } else { ":" };
        editor.push_str(&format!(r#"<br>Contact{sep} <a href="mailto:{c}">{c}</a>"#));
    }
    editor.push_str("</p>");
    let content = if fr {
        format!(
            r#"    <h2>Éditeur du site</h2>
    {editor}
    <p>Directeur de la publication : {name}.</p>
    <h2>Hébergement</h2>
    <p>Salesforce, Inc. (Heroku) — 415 Mission Street, Suite 300, San Francisco, CA 94105, États-Unis — <a href="https://www.heroku.com">heroku.com</a>.</p>
    <h2>Propriété intellectuelle</h2>
    <p>L'application F1X, son code, son design, ses textes, ses modèles 3D et son logo sont protégés par le droit d'auteur. Toute reproduction, représentation ou réutilisation, totale ou partielle, sans autorisation écrite préalable de l'éditeur est interdite.</p>
    <p>Les photos proviennent de Wikimedia Commons et restent la propriété de leurs auteurs, sous les licences libres indiquées sur chaque photo. Les données sportives, cartes et contenus tiers sont détaillés dans les <a href="/credits?lang=fr">crédits et sources</a>.</p>
    <h2>Marques</h2>
    <p>F1X est un service indépendant et non officiel, sans lien avec Formula One Group, la Fédération Internationale de l'Automobile (FIA) ou les écuries. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX et les marques associées appartiennent à Formula One Licensing B.V. Les noms d'écuries, de pilotes et de circuits sont cités à titre purement informatif.</p>
    <h2>Responsabilité</h2>
    <p>Les informations (résultats, classements, chronos, météo, actualités) proviennent de sources publiques et sont fournies à titre indicatif, sans garantie d'exactitude ni de disponibilité. Les données « en direct » peuvent présenter un décalage. L'éditeur ne saurait être tenu responsable de l'usage qui en est fait ni du contenu des sites externes vers lesquels pointent les liens.</p>
    <h2>Données personnelles</h2>
    <p>Voir la <a href="/confidentialite?lang=fr">politique de confidentialité</a>.</p>
    <h2>Droit applicable</h2>
    <p>Le présent site est soumis au droit français.</p>"#,
            name = p.name
        )
    } else {
        format!(
            r#"    <h2>Publisher</h2>
    {editor}
    <p>Publication director: {name}.</p>
    <h2>Hosting</h2>
    <p>Salesforce, Inc. (Heroku) — 415 Mission Street, Suite 300, San Francisco, CA 94105, USA — <a href="https://www.heroku.com">heroku.com</a>.</p>
    <h2>Intellectual property</h2>
    <p>The F1X application, its code, design, texts, 3D models and logo are protected by copyright. Any reproduction, display or reuse, in whole or in part, without the publisher's prior written consent is prohibited.</p>
    <p>Photos come from Wikimedia Commons and remain the property of their authors, under the free licences shown on each photo. Sports data, maps and third-party content are detailed in the <a href="/credits?lang=en">credits &amp; sources</a>.</p>
    <h2>Trademarks</h2>
    <p>F1X is an independent, unofficial service, not affiliated with Formula One Group, the Fédération Internationale de l'Automobile (FIA) or the teams. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trademarks of Formula One Licensing B.V. Team, driver and circuit names are used for information purposes only.</p>
    <h2>Liability</h2>
    <p>Information (results, standings, timing, weather, news) comes from public sources and is provided for information only, with no guarantee of accuracy or availability. “Live” data may be delayed. The publisher cannot be held liable for its use or for the content of external sites linked from F1X.</p>
    <h2>Personal data</h2>
    <p>See the <a href="/confidentialite?lang=en">privacy policy</a>.</p>
    <h2>Governing law</h2>
    <p>This site is governed by French law.</p>"#,
            name = p.name
        )
    };
    doc_page(
        lang,
        origin,
        "/mentions-legales",
        lang.t("Mentions légales", "Legal notice"),
        lang.t(
            "Mentions légales de F1X : éditeur, hébergement, propriété intellectuelle et marques.",
            "F1X legal notice: publisher, hosting, intellectual property and trademarks.",
        ),
        &content,
    )
}

fn privacy_page(lang: Lang, origin: &str) -> String {
    let p = publisher();
    let contact = p
        .contact
        .as_ref()
        .map(|c| format!(r#" (<a href="mailto:{c}">{c}</a>)"#))
        .unwrap_or_default();
    let content = if lang == Lang::Fr {
        format!(
            r#"    <p class="doc-lead">En bref : F1X ne demande aucun compte, n'utilise ni cookie publicitaire, ni outil de mesure d'audience, ni traceur. Tes préférences restent sur ton appareil.</p>
    <h2>Ce qui reste sur ton appareil</h2>
    <p>Pour fonctionner, l'app enregistre localement dans ton navigateur (stockage local et cache hors ligne) : la langue, tes favoris, tes pronostics, ton équipe Fantasy, ton meilleur score au quiz, tes réglages d'alertes, ainsi qu'une copie des pages et données consultées pour l'usage hors ligne. Ces informations ne sont jamais envoyées à nos serveurs. Tu peux les effacer à tout moment depuis les réglages de ton navigateur (données de site).</p>
    <h2>Journaux techniques</h2>
    <p>Comme tout site, l'hébergeur (Heroku) traite l'adresse IP et la page demandée pour acheminer les requêtes et assurer la sécurité du service. Ces journaux techniques sont conservés pour une durée limitée et ne servent à aucun profilage.</p>
    <h2>Services tiers chargés par ton navigateur</h2>
    <ul>
      <li><strong>Wikimedia Commons</strong> (photos des pilotes et circuits),</li>
      <li><strong>OpenStreetMap</strong> (carte des circuits),</li>
      <li><strong>Open-Meteo</strong> (prévisions météo).</li>
    </ul>
    <p>Ces services reçoivent ton adresse IP lors du chargement, comme pour tout contenu web, selon leurs propres politiques de confidentialité. Les liens d'actualités ouvrent les sites des éditeurs de presse.</p>
    <h2>Tes droits</h2>
    <p>F1X ne constitue pas de fichier de données personnelles. Pour toute question ou demande relative au RGPD, tu peux contacter l'éditeur{contact}. Tu peux aussi adresser une réclamation à la CNIL (<a href="https://www.cnil.fr">cnil.fr</a>).</p>"#
        )
    } else {
        format!(
            r#"    <p class="doc-lead">In short: F1X requires no account and uses no advertising cookies, no analytics and no trackers. Your preferences stay on your device.</p>
    <h2>What stays on your device</h2>
    <p>To work, the app stores locally in your browser (local storage and offline cache): your language, favourites, predictions, Fantasy team, best quiz score, alert settings, and a copy of the pages and data you viewed for offline use. This information is never sent to our servers. You can delete it at any time from your browser settings (site data).</p>
    <h2>Technical logs</h2>
    <p>Like any website, the host (Heroku) processes your IP address and the requested page to route requests and keep the service secure. These technical logs are kept for a limited time and are never used for profiling.</p>
    <h2>Third-party services loaded by your browser</h2>
    <ul>
      <li><strong>Wikimedia Commons</strong> (driver and circuit photos),</li>
      <li><strong>OpenStreetMap</strong> (circuit maps),</li>
      <li><strong>Open-Meteo</strong> (weather forecasts).</li>
    </ul>
    <p>These services receive your IP address when content loads, as with any web content, under their own privacy policies. News links open the publishers' websites.</p>
    <h2>Your rights</h2>
    <p>F1X does not keep any personal data file. For any GDPR question or request, you can contact the publisher{contact}. You may also lodge a complaint with your data protection authority (in France, the CNIL: <a href="https://www.cnil.fr">cnil.fr</a>).</p>"#
        )
    };
    doc_page(
        lang,
        origin,
        "/confidentialite",
        lang.t("Politique de confidentialité", "Privacy policy"),
        lang.t(
            "Confidentialité sur F1X : sans compte, sans cookie publicitaire, sans traceur.",
            "Privacy on F1X: no account, no advertising cookies, no trackers.",
        ),
        &content,
    )
}

fn credits_page(lang: Lang, origin: &str) -> String {
    let content = if lang == Lang::Fr {
        r#"    <h2>Données sportives</h2>
    <ul>
      <li><strong>Jolpica F1</strong> (successeur de l'API Ergast) — calendriers, résultats, classements et archives depuis 1950.</li>
      <li><strong><a href="https://openf1.org">OpenF1</a></strong> — positions GPS, télémétrie, chronos, pneus et replays des sessions depuis 2023. Données non officielles.</li>
      <li><strong><a href="https://open-meteo.com">Open-Meteo</a></strong> — prévisions météo (licence CC BY 4.0).</li>
    </ul>
    <h2>Cartes et photos</h2>
    <ul>
      <li><strong>OpenStreetMap</strong> — cartes des circuits, © <a href="https://www.openstreetmap.org/copyright">contributeurs OpenStreetMap</a> (ODbL).</li>
      <li><strong><a href="https://commons.wikimedia.org">Wikimedia Commons</a></strong> — photos sous licences libres ; l'auteur et la licence de chaque photo sont accessibles depuis le lien placé sous l'image.</li>
    </ul>
    <h2>Actualités</h2>
    <p>Titres, extraits et liens issus des flux RSS publics de Motorsport.com, Autosport, Formula1.com et RaceFans. Les articles appartiennent à leurs éditeurs ; F1X renvoie vers leurs sites.</p>
    <h2>3D</h2>
    <p>Les monoplaces en 3D sont des modèles stylisés originaux, générés par le code de F1X ; elles ne reproduisent aucune voiture réelle. Les circuits en relief sont reconstitués à partir des positions GPS OpenF1.</p>
    <h2>Technologies</h2>
    <p>Rust, WebAssembly, Yew, axum, WebGL 2.</p>"#
    } else {
        r#"    <h2>Sports data</h2>
    <ul>
      <li><strong>Jolpica F1</strong> (successor to the Ergast API) — calendars, results, standings and history since 1950.</li>
      <li><strong><a href="https://openf1.org">OpenF1</a></strong> — GPS positions, telemetry, timing, tyres and session replays since 2023. Unofficial data.</li>
      <li><strong><a href="https://open-meteo.com">Open-Meteo</a></strong> — weather forecasts (CC BY 4.0 licence).</li>
    </ul>
    <h2>Maps and photos</h2>
    <ul>
      <li><strong>OpenStreetMap</strong> — circuit maps, © <a href="https://www.openstreetmap.org/copyright">OpenStreetMap contributors</a> (ODbL).</li>
      <li><strong><a href="https://commons.wikimedia.org">Wikimedia Commons</a></strong> — freely licensed photos; each photo's author and licence are linked under the image.</li>
    </ul>
    <h2>News</h2>
    <p>Headlines, excerpts and links from the public RSS feeds of Motorsport.com, Autosport, Formula1.com and RaceFans. Articles belong to their publishers; F1X links to their websites.</p>
    <h2>3D</h2>
    <p>The 3D cars are original stylised models generated by F1X's code; they do not replicate any real car. The 3D circuits are rebuilt from OpenF1 GPS positions.</p>
    <h2>Technologies</h2>
    <p>Rust, WebAssembly, Yew, axum, WebGL 2.</p>"#
    };
    doc_page(
        lang,
        origin,
        "/credits",
        lang.t("Crédits et sources", "Credits & sources"),
        lang.t(
            "Sources des données, cartes, photos et actualités utilisées par F1X.",
            "Sources of the data, maps, photos and news used by F1X.",
        ),
        content,
    )
}

const CSS: &str = r#"
:root{--bg:#0b0b10;--surface:#15151e;--surface-2:#1e1e2a;--line:#2a2a38;--text:#f4f4f8;--muted:#a3a3b5;--red:#e10600;color-scheme:dark}
*,*::before,*::after{box-sizing:border-box;min-width:0}
html,body{margin:0;max-width:100%;overflow-x:hidden}
html{scroll-behavior:smooth;scroll-padding-top:72px}
body{background:var(--bg);color:var(--text);font:17px/1.55 system-ui,-apple-system,"SF Pro Text","Segoe UI",Roboto,sans-serif;overflow-wrap:anywhere;-webkit-text-size-adjust:100%}
a{color:inherit}
h1,h2,h3,p{margin:0}
.top{position:sticky;top:0;z-index:10;display:flex;align-items:center;justify-content:space-between;gap:12px;padding:calc(env(safe-area-inset-top) + 10px) 16px 10px;background:rgba(11,11,16,.88);-webkit-backdrop-filter:blur(14px);backdrop-filter:blur(14px);border-bottom:1px solid var(--line)}
.brand{font-weight:900;font-style:italic;font-size:1.4rem;text-decoration:none;letter-spacing:-.02em}
.brand .x{color:var(--red)}
.top-links{display:flex;align-items:center;gap:10px}
.lang{font-size:.8rem;font-weight:800;text-decoration:none;border:1px solid var(--line);border-radius:999px;padding:6px 11px;background:var(--surface-2)}
main{width:100%;max-width:1080px;margin:0 auto;padding:0 16px}
.btn{display:inline-flex;align-items:center;justify-content:center;gap:8px;background:var(--red);color:#fff;font-weight:800;text-decoration:none;border-radius:14px;padding:14px 20px}
.btn-small{padding:8px 13px;font-size:.85rem;border-radius:10px}
.btn-big{font-size:1.1rem;padding:16px 24px;box-shadow:0 10px 30px rgba(225,6,0,.35)}
.btn-ghost{background:var(--surface-2);border:1px solid var(--line);color:var(--text)}
.eyebrow{color:var(--muted);font-size:.78rem;font-weight:800;text-transform:uppercase;letter-spacing:.1em}
.hero{display:grid;grid-template-columns:minmax(0,1fr);gap:28px;padding:40px 0 24px;align-items:center}
.hero h1{font-size:clamp(2.1rem,9vw,3.6rem);line-height:1.05;font-weight:900;letter-spacing:-.03em;margin:10px 0 14px}
.lead{color:var(--muted);font-size:1.1rem}
.ctas{display:flex;flex-wrap:wrap;gap:10px;margin:22px 0 10px}
.ctas .btn{flex:1 1 200px}
.note{color:var(--muted);font-size:.88rem}
.hero-shot{display:flex;justify-content:center}
.phone{margin:0;width:100%;max-width:290px;border-radius:34px;padding:8px;background:linear-gradient(160deg,#2c2c3a,#111118);box-shadow:0 30px 60px rgba(0,0,0,.55),0 0 0 1px #34344a}
.phone img{display:block;width:100%;height:auto;border-radius:27px}
.numbers{list-style:none;margin:8px 0 12px;padding:0;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}
.numbers li{background:var(--surface);border:1px solid var(--line);border-radius:16px;padding:16px;display:flex;flex-direction:column;align-items:flex-start}
.numbers strong{font-size:1.7rem;font-weight:900;color:var(--text)}
.numbers span{color:var(--muted);font-size:.85rem}
.feature{display:grid;grid-template-columns:minmax(0,1fr);gap:22px;padding:44px 0;border-top:1px solid var(--line)}
.feature h2{font-size:clamp(1.6rem,6.5vw,2.4rem);line-height:1.1;font-weight:900;letter-spacing:-.02em;margin:8px 0 12px}
.feature p{color:var(--muted)}
.checks{list-style:none;margin:16px 0 0;padding:0;display:flex;flex-direction:column;gap:10px}
.checks li{padding-left:28px;position:relative}
.checks li::before{content:"";position:absolute;left:0;top:.5em;width:14px;height:8px;border-left:3px solid var(--red);border-bottom:3px solid var(--red);transform:rotate(-45deg)}
.feature-shots{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,220px),1fr));gap:16px;justify-items:center}
.band{padding:44px 0;border-top:1px solid var(--line)}
.band h2{font-size:clamp(1.5rem,6vw,2.1rem);font-weight:900;margin:6px 0 18px;letter-spacing:-.02em}
.band-lead{color:var(--muted);margin:-6px 0 18px;max-width:62ch}
.minis{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,230px),1fr));gap:10px}
.mini{background:var(--surface);border:1px solid var(--line);border-radius:16px;padding:16px;display:flex;flex-direction:column;gap:4px}
.mini span:last-child{color:var(--muted);font-size:.92rem}
.mini-icon{font-size:1.4rem}
.steps{margin:0;padding:0;list-style:none;display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,260px),1fr));gap:10px}
.steps li{background:var(--surface);border:1px solid var(--line);border-radius:16px;padding:16px;display:flex;flex-direction:column;gap:4px}
.steps span{color:var(--muted)}
.tech p:not(.eyebrow){color:var(--muted);max-width:60ch}
.chips{list-style:none;margin:16px 0 0;padding:0;display:flex;flex-wrap:wrap;gap:8px}
.chips li{background:var(--surface-2);border:1px solid var(--line);border-radius:999px;padding:6px 12px;font-size:.85rem;font-weight:700}
.final{padding:56px 0 64px;border-top:1px solid var(--line);display:flex;flex-direction:column;align-items:center;gap:20px;text-align:center}
.final h2{font-size:clamp(1.7rem,7vw,2.6rem);font-weight:900;letter-spacing:-.02em}
.doc{max-width:72ch;margin:0 auto;padding:36px 0 56px}
.doc h1{font-size:clamp(1.9rem,8vw,2.8rem);font-weight:900;letter-spacing:-.02em;line-height:1.1}
.doc-date{color:var(--muted);font-size:.85rem;margin:8px 0 24px}
.doc-lead{font-size:1.08rem;background:var(--surface);border:1px solid var(--line);border-left:4px solid var(--red);border-radius:12px;padding:14px 16px;margin-bottom:8px}
.doc h2{font-size:1.2rem;font-weight:800;margin:28px 0 8px}
.doc p,.doc li{color:#cfcfdc}
.doc p+p{margin-top:10px}
.doc ul{margin:8px 0;padding-left:20px;display:flex;flex-direction:column;gap:6px}
.doc a{color:var(--text);text-decoration-color:var(--red);text-underline-offset:3px}
.foot{border-top:1px solid var(--line);background:#09090d;padding:40px 16px calc(env(safe-area-inset-bottom) + 28px);color:var(--muted);font-size:.9rem}
.foot>*{max-width:1080px;margin-left:auto;margin-right:auto}
.foot-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:28px 20px}
.foot-brand{grid-column:1/-1}
.foot-brand{display:flex;flex-direction:column;align-items:flex-start;gap:12px}
.foot-brand p{max-width:34ch}
.foot h3{color:var(--text);font-size:.78rem;font-weight:800;text-transform:uppercase;letter-spacing:.1em;margin-bottom:10px}
.foot ul{list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:2px}
.foot nav a{display:inline-block;padding:5px 0;text-decoration:none;color:var(--muted)}
.foot nav a:hover,.foot-lang a:hover{color:var(--text)}
.foot-bottom{display:flex;flex-wrap:wrap;justify-content:space-between;gap:10px;border-top:1px solid var(--line);margin-top:32px;padding-top:18px;font-size:.85rem}
.foot-lang{display:flex;gap:8px}
.foot-lang a{text-decoration:none}
.foot-legal{margin-top:14px;font-size:.76rem;line-height:1.5;color:#7d7d90}
@media (min-width:860px){
  .hero{grid-template-columns:minmax(0,1.1fr) minmax(0,.9fr);padding:72px 0 40px}
  .numbers{grid-template-columns:repeat(4,minmax(0,1fr))}
  .feature{grid-template-columns:minmax(0,1fr) minmax(0,1fr);align-items:center;gap:48px;padding:72px 0}
  .feature.reverse .feature-text{order:2}
  .foot-grid{grid-template-columns:minmax(0,1.4fr) repeat(3,minmax(0,1fr))}
  .foot-brand{grid-column:auto}
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_internal_paths() {
        assert_eq!(
            safe_back(Some("/saison/current/course/1")).as_deref(),
            Some("/saison/current/course/1")
        );
        assert_eq!(safe_back(Some("//evil.com")), None);
        assert_eq!(safe_back(Some("https://evil.com")), None);
        assert_eq!(safe_back(Some("/a?b=c")), None);
        assert_eq!(safe_back(Some("/presentation")), None);
        assert_eq!(safe_back(None), None);
    }

    #[test]
    fn pages_render_without_github() {
        for lang in [Lang::Fr, Lang::En] {
            for html in [
                render(lang, None, "https://x.test"),
                legal_page(lang, "https://x.test"),
                privacy_page(lang, "https://x.test"),
                credits_page(lang, "https://x.test"),
            ] {
                assert!(!html.to_lowercase().contains("github"));
                assert!(html.contains("https://x.test/static/img/og-"));
                assert!(html.contains(r#"class="foot""#));
            }
        }
        assert_eq!(esc("<a&\">"), "&lt;a&amp;&quot;&gt;");
    }
}
