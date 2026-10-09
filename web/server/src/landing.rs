//! Site de présentation (`/presentation`) et pages légales : HTML + CSS rendus par le serveur,
//! lisibles sans JavaScript (un petit script facultatif ne fait qu'animer). Bilingue : `?lang=fr|en`, sinon langue du navigateur (`Accept-Language`).

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
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Archivo:wdth,wght@62..125,100..900&display=swap">
<style>{CSS}</style>
<script>
// Animations d'entrée prévues dès le départ (sinon tout reste visible) ; filet de sécurité si le script de fin ne passe pas.
(function () {{
  var r = document.documentElement;
  if (matchMedia("(prefers-reduced-motion: reduce)").matches || !("IntersectionObserver" in window)) return;
  r.classList.add("anime");
  setTimeout(function () {{ if (!window.__f1xVu) r.classList.remove("anime"); }}, 3000);
}})();
</script>
</head>
<body>
<div class="grain" aria-hidden="true"></div>
<div class="curseur" aria-hidden="true"><span></span></div>
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
<script>
(function () {{
  var root = document.documentElement, reduit = matchMedia("(prefers-reduced-motion: reduce)").matches;
  var souris = matchMedia("(hover: hover) and (pointer: fine)").matches;
  // Barre du haut plus dense au défilement ; barre de progression si le CSS ne sait pas la lier au défilement.
  var progres = !(window.CSS && CSS.supports && CSS.supports("animation-timeline: scroll()")), prevu = false;
  addEventListener("scroll", function () {{
    if (prevu) return;
    prevu = true;
    requestAnimationFrame(function () {{
      prevu = false;
      var y = scrollY, h = root.scrollHeight - innerHeight;
      root.toggleAttribute("data-defile", y > 24);
      if (progres) root.style.setProperty("--defile", h > 0 ? Math.min(1, y / h).toFixed(4) : 0);
    }});
  }}, {{ passive: true }});
  // Blocs qui entrent en scène en arrivant à l'écran (en cascade s'ils arrivent ensemble).
  window.__f1xVu = 1;
  if (root.classList.contains("anime")) {{
    var vus = new IntersectionObserver(function (es) {{
      var k = 0;
      es.forEach(function (e) {{
        if (!e.isIntersecting) return;
        vus.unobserve(e.target);
        e.target.style.setProperty("--d", Math.min(k++, 6) * 90 + "ms");
        e.target.classList.add("vu");
      }});
    }}, {{ rootMargin: "0px 0px -8% 0px" }});
    document.querySelectorAll(".numbers li, .feature, .band, .final, .doc > *, .foot-grid > *, .foot-bottom, .foot-legal").forEach(function (el) {{ vus.observe(el); }});
  }}
  if (souris) {{
    // Curseur anneau qui grossit sur ce qui se clique.
    var c = document.querySelector(".curseur"), x = -100, y = -100, cx = x, cy = y;
    addEventListener("mousemove", function (e) {{
      x = e.clientX; y = e.clientY;
      c.classList.toggle("actif", !!(e.target.closest && e.target.closest("a, button")));
    }}, {{ passive: true }});
    (function boucle() {{
      cx += (x - cx) * (reduit ? 1 : .22); cy += (y - cy) * (reduit ? 1 : .22);
      c.style.transform = "translate(" + cx + "px," + cy + "px)";
      requestAnimationFrame(boucle);
    }})();
    if (!reduit) {{
      // Projecteur sur les cartes et téléphone de l'accueil qui s'incline vers la souris.
      document.addEventListener("pointermove", function (e) {{
        var b = e.target.closest && e.target.closest(".mini, .numbers li, .steps li");
        if (!b) return;
        var r = b.getBoundingClientRect();
        b.style.setProperty("--mx", Math.round(e.clientX - r.left) + "px");
        b.style.setProperty("--my", Math.round(e.clientY - r.top) + "px");
      }}, {{ passive: true }});
      var tel = document.querySelector(".hero-shot");
      if (tel) addEventListener("mousemove", function (e) {{
        var kx = e.clientX / innerWidth - .5, ky = e.clientY / innerHeight - .5;
        tel.style.setProperty("--ry", (kx * 14).toFixed(2) + "deg");
        tel.style.setProperty("--rx", (-ky * 10).toFixed(2) + "deg");
      }}, {{ passive: true }});
    }}
  }}
  // Chiffres clés : comptent depuis 0 quand ils arrivent à l'écran (valeur finale déjà affichée sans script).
  if (!matchMedia("(prefers-reduced-motion: reduce)").matches && "IntersectionObserver" in window) {{
    var io = new IntersectionObserver(function (es) {{
      es.forEach(function (e) {{
        if (!e.isIntersecting) return;
        io.unobserve(e.target);
        var el = e.target, final = el.textContent, n = parseInt(final.replace(/\D/g, ""), 10);
        if (!n) return;
        var t0 = performance.now();
        (function tick(t) {{
          var k = Math.min(1, (t - t0) / 1400), v = Math.round(n * (1 - Math.pow(1 - k, 3)));
          el.textContent = k < 1 ? final.replace(/[\d\s\u202f]+/, v.toLocaleString(document.documentElement.lang) + (/\s$/.test(final) ? " " : "")) : final;
          if (k < 1) requestAnimationFrame(tick);
        }})(t0);
      }});
    }}, {{ threshold: .6 }});
    document.querySelectorAll(".numbers strong").forEach(function (el) {{ io.observe(el); }});
  }}
  // Boutons aimantés : suivent légèrement le pointeur (souris uniquement).
  if (matchMedia("(hover: hover) and (pointer: fine)").matches) {{
    document.querySelectorAll(".btn, .lang").forEach(function (b) {{
      b.addEventListener("pointermove", function (e) {{
        var r = b.getBoundingClientRect();
        b.style.transform = "translate(" + (e.clientX - r.left - r.width / 2) * .25 + "px," + (e.clientY - r.top - r.height / 2) * .35 + "px)";
      }});
      b.addEventListener("pointerleave", function () {{ b.style.transform = ""; }});
    }});
  }}
}})();
</script>
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
    <p>© {year} F1X — {by} Maxime Nathan Lestage. {rights}</p>
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
        by = t("conçu et développé par", "designed and built by"),
        disclaimer = t(
            "F1X est un service indépendant et non officiel, sans lien avec Formula One Group, la FIA ou les écuries. F1, FORMULA ONE, FORMULA 1, GRAND PRIX et les marques associées appartiennent à Formula One Licensing B.V. Les noms d'écuries et de pilotes sont cités à titre informatif.",
            "F1X is an independent, unofficial service, not affiliated with Formula One Group, the FIA or the teams. F1, FORMULA ONE, FORMULA 1, GRAND PRIX and related marks are trademarks of Formula One Licensing B.V. Team and driver names are used for information purposes only.",
        ),
    )
}

// ---------- Présentation ----------

/// Découpe un texte en mots animables (`<span class="{class}" style="--i:n">`).
/// La ponctuation isolée (« ? », « ! ») reste collée au mot d'avant (espace insécable).
fn words(text: &str, class: &str) -> String {
    let mut list: Vec<String> = Vec::new();
    for w in text.split(' ') {
        match list.last_mut() {
            Some(prev) if !w.is_empty() && !w.chars().any(char::is_alphanumeric) => {
                prev.push('\u{a0}');
                prev.push_str(w);
            }
            _ => list.push(w.to_string()),
        }
    }
    list.iter()
        .enumerate()
        .map(|(i, w)| format!(r#"<span class="{class}" style="--i:{i}"><span>{w}</span></span> "#))
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Bandeau qui défile : les rubriques de l'app séparées par un point signal.
fn ribbon(lang: Lang) -> String {
    ribbon_of(&[
        lang.t("Race Center", "Race Center"),
        lang.t("Circuits en 3D", "3D circuits"),
        lang.t("Télémétrie", "Telemetry"),
        lang.t("Radios", "Team radio"),
        lang.t("Stratégie", "Strategy"),
        lang.t("Météo", "Weather"),
        lang.t("75 ans d'archives", "75 years of history"),
    ])
}

/// Second bandeau (sens inverse) : ce qu'on fait avec l'app.
fn ribbon_fans(lang: Lang) -> String {
    ribbon_of(&[
        lang.t("Direct", "Live"),
        lang.t("Replays ×60", "Replays ×60"),
        lang.t("Pronostics", "Predictions"),
        "Fantasy",
        "Quiz",
        "Records",
        lang.t("Comparateur", "Head to head"),
        lang.t("Monoplaces 3D", "3D cars"),
    ])
}

fn ribbon_of(items: &[&str]) -> String {
    items
        .iter()
        .map(|w| format!("<span>{w}</span><i></i>"))
        .collect()
}

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
        let items: String = bullets
            .iter()
            .enumerate()
            .map(|(i, b)| format!(r#"<li style="--i:{i}">{b}</li>"#))
            .collect();
        let title = words(title, "mot");
        format!(
            r#"<section class="feature{rev}" id="{id}"><div class="feature-text"><p class="eyebrow">{eyebrow}</p><h2 class="mots">{title}</h2><p>{text}</p><ul class="checks">{items}</ul></div><div class="feature-shots">{shots}</div></section>"#,
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
    .enumerate()
    .map(|(n, (i, h, p))| format!(r#"<li class="mini" style="--i:{n}"><span class="mini-icon" aria-hidden="true">{i}</span><strong>{h}</strong><span>{p}</span></li>"#))
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
      <h1 class="mots">{h1}</h1>
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

  <div class="bande" aria-hidden="true"><div class="bande-piste">{ribbon}{ribbon}</div></div>

  <section class="manifeste"><p>{manifesto}</p></section>

  <div id="fonctionnalites">{features}</div>

  <section class="band">
    <h2 class="mots">{more}</h2>
    <ul class="minis">{cards}</ul>
  </section>

  <section class="band install" id="installer">
    <p class="eyebrow">{install_eyebrow}</p>
    <h2 class="mots">{install_h}</h2>
    <p class="band-lead">{install_lead}</p>
    <ol class="steps">
      <li><strong>iPhone · iPad (Safari)</strong><span>{install_ios}</span></li>
      <li><strong>Android (Chrome)</strong><span>{install_android}</span></li>
      <li><strong>{desktop}</strong><span>{install_desktop}</span></li>
    </ol>
  </section>

  <section class="band tech">
    <p class="eyebrow">{tech_eyebrow}</p>
    <h2 class="mots">{tech_h}</h2>
    <p>{tech_p}</p>
    <ul class="chips"><li>Rust</li><li>WebAssembly</li><li>Yew</li><li>WebGL 2</li><li>WebSocket</li><li>PWA</li></ul>
  </section>

  <div class="bande inverse" aria-hidden="true"><div class="bande-piste">{ribbon2}{ribbon2}</div></div>

  <section class="final">
    <h2 class="mots">{final_h}</h2>
    <a class="btn btn-big" href="{app}">{cta} <span aria-hidden="true">→</span></a>
  </section>"##,
        eyebrow = t("Application web gratuite", "Free web app"),
        h1 = words(
            t(
                "Toute la Formule 1 dans ta poche.",
                "All of Formula 1 in your pocket."
            ),
            "mot"
        ),
        manifesto = words(
            t(
                "Chaque tour, chaque arrêt, chaque radio. La course en direct, les circuits en 3D, 75 ans d'histoire. Tout tient dans ta poche.",
                "Every lap, every stop, every radio call. The race live, the circuits in 3D, 75 years of history. All of it in your pocket."
            ),
            "lueur"
        ),
        ribbon = ribbon(lang),
        ribbon2 = ribbon_fans(lang),
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
        more = words(t("Et aussi", "And also"), "mot"),
        install_eyebrow = t("Application installable", "Installable app"),
        install_h = words(
            t(
                "Installe-la comme une vraie app.",
                "Install it like a real app."
            ),
            "mot"
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
        tech_h = words(
            t(
                "Écrite en Rust, de bout en bout.",
                "Written in Rust, end to end."
            ),
            "mot"
        ),
        tech_p = t(
            "L'interface et le rendu 3D sont en Rust compilé en WebAssembly, le serveur aussi. Le temps réel passe par WebSocket, les données sont mises en cache pour rester rapides, même en 4G.",
            "The interface and the 3D rendering are Rust compiled to WebAssembly, and so is the server. Real time runs over WebSocket, and data is cached to stay fast, even on 4G.",
        ),
        final_h = words(
            t(
                "Prêt pour le prochain Grand Prix ?",
                "Ready for the next Grand Prix?"
            ),
            "mot"
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
    let title_words = words(title, "mot");
    let body = format!(
        r#"  <article class="doc">
    <h1 class="mots">{title_words}</h1>
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
        name: var("LEGAL_PUBLISHER").unwrap_or_else(|| "Maxime Nathan Lestage".into()),
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
      <li><strong>Son du démarrage de l'app</strong> — extrait de « <a href="https://commons.wikimedia.org/wiki/File:Ferrari_F60_(2009).ogg">Ferrari F60 (2009)</a> » par <a href="https://commons.wikimedia.org/wiki/User:Edvvc">Edvvc</a>, Wikimedia Commons, licence <a href="https://creativecommons.org/licenses/by-sa/3.0/deed.fr">CC BY-SA 3.0</a> (raccourci et mis en fondu).</li>
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
      <li><strong>App start-up sound</strong> — excerpt from “<a href="https://commons.wikimedia.org/wiki/File:Ferrari_F60_(2009).ogg">Ferrari F60 (2009)</a>” by <a href="https://commons.wikimedia.org/wiki/User:Edvvc">Edvvc</a>, Wikimedia Commons, <a href="https://creativecommons.org/licenses/by-sa/3.0/">CC BY-SA 3.0</a> licence (trimmed and faded).</li>
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
:root{--bg:#0e0d0c;--surface:#161513;--surface-2:#201e1b;--line:rgba(242,237,227,.12);--text:#f2ede3;--muted:#8d877d;--red:#e10600;--font:"Archivo",system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;--ease:cubic-bezier(.2,.7,.1,1);color-scheme:dark}
*,*::before,*::after{box-sizing:border-box;min-width:0}
html,body{margin:0;max-width:100%;overflow-x:hidden;overflow-x:clip}
html{scroll-behavior:smooth;scroll-padding-top:72px}
body{background:var(--bg);color:var(--text);font:17px/1.55 var(--font);font-variation-settings:"wdth" 100;-webkit-font-smoothing:antialiased;overflow-wrap:anywhere;-webkit-text-size-adjust:100%}
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

/* Habillage atelier : nuit et crème, signal orange, typographie Archivo à largeur variable, grain de film. */
::selection{background:var(--red);color:var(--bg)}
.grain{position:fixed;inset:-50%;z-index:90;pointer-events:none;opacity:.065;background-image:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='220' height='220'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");animation:grain .9s steps(6,end) infinite}
@keyframes grain{0%{transform:translate(0)}20%{transform:translate(-4%,3%)}40%{transform:translate(3%,-5%)}60%{transform:translate(-2%,6%)}80%{transform:translate(5%,2%)}to{transform:translate(0)}}
.top{background:linear-gradient(rgba(8,7,6,.92),rgba(8,7,6,.6) 70%,rgba(8,7,6,0));border-bottom:0}
.brand{font-variation-settings:"wdth" 112;letter-spacing:-.03em}
.lang{background:transparent;border-color:var(--line);font-weight:600;letter-spacing:.08em}
.btn{border-radius:99px;color:var(--bg);font-weight:700;padding:14px 24px;transition:transform .35s var(--ease),background .3s,color .3s}
.btn:hover{transform:translateY(-2px)}
.btn-small{border-radius:99px;padding:8px 16px}
.btn-big{box-shadow:none}
.btn-ghost{background:transparent;color:var(--text);border:1px solid var(--line)}
.btn-ghost:hover{background:var(--text);color:var(--bg)}
.eyebrow{display:inline-flex;align-items:center;gap:10px;font-weight:500;font-size:.75rem;letter-spacing:.16em}
.eyebrow::before{content:"";width:7px;height:7px;border-radius:50%;background:var(--red);box-shadow:0 0 14px var(--red);flex:none}
h1,h2{font-variation-settings:"wdth" 88;text-wrap:balance}
.hero h1{font-size:clamp(2.6rem,11vw,6.2rem);line-height:.9;font-weight:800;letter-spacing:-.03em}
.feature h2,.band h2,.final h2{font-weight:700;letter-spacing:-.02em;line-height:1}
.feature h2{font-size:clamp(2rem,7.5vw,3.6rem)}
.final h2{font-size:clamp(2.2rem,9vw,4.4rem)}
.lead{font-size:clamp(1.1rem,2.3vw,1.6rem);line-height:1.3;color:var(--muted);max-width:30ch}
.numbers li,.mini,.steps li{background:transparent;border:1px solid var(--line);border-radius:18px}
.numbers strong{font-size:clamp(2.4rem,8vw,3.6rem);line-height:.9;font-weight:300;font-variation-settings:"wdth" 62}
.numbers span{letter-spacing:.14em;text-transform:uppercase;font-size:.72rem;margin-top:8px}
.checks li::before{border-color:var(--red)}
.chips li{background:transparent;border-color:var(--line);font-weight:500;letter-spacing:.02em}
.phone{background:linear-gradient(160deg,#2a2622,#100f0d);box-shadow:0 30px 60px rgba(0,0,0,.6),0 0 0 1px rgba(242,237,227,.14)}
.foot{background:#0a0908}
.foot h3{font-weight:500;letter-spacing:.16em}
.foot-legal{color:#6e6961}
.doc p,.doc li{color:#d8d2c6}
@media (prefers-reduced-motion:reduce){.grain{animation:none}.btn{transition:none}.btn:hover{transform:none}}

/* Animations : mots qui montent, bandeau qui défile, manifeste qui s'allume au défilement. */
.mots .mot{display:inline-block;overflow:hidden;vertical-align:top;padding-bottom:.06em}
.mots .mot>span{display:inline-block;animation:mot .95s var(--ease) both;animation-delay:calc(var(--i)*80ms + .1s)}
@keyframes mot{from{transform:translateY(105%) rotate(4deg)}}
.hero .lead,.hero .ctas,.hero .note{animation:monte-in .9s var(--ease) both .55s}
.hero .eyebrow{animation:monte-in .7s var(--ease) both}
@keyframes monte-in{from{opacity:0;transform:translateY(18px)}}
.eyebrow::before{animation:pouls 2.4s ease-in-out infinite}
@keyframes pouls{50%{box-shadow:0 0 4px var(--red);transform:scale(.7)}}
.phone{animation:flotte 7s ease-in-out infinite}
@keyframes flotte{50%{transform:translateY(-10px) rotate(-.6deg)}}
.bande{margin:20px calc(50% - 50vw) 0;border-block:1px solid var(--line);background:var(--surface);padding:18px 0;overflow:hidden;transform:rotate(-2deg) scale(1.04)}
.bande-piste{display:flex;align-items:center;gap:34px;width:max-content;animation:bande 38s linear infinite}
.bande-piste span{font-variation-settings:"wdth" 125;text-transform:uppercase;font-weight:800;font-size:clamp(26px,4.2vw,60px);letter-spacing:-.01em;white-space:nowrap}
.bande-piste i{width:12px;height:12px;border-radius:50%;background:var(--red);box-shadow:0 0 16px var(--red);flex:none}
@keyframes bande{to{transform:translateX(-50%)}}
.manifeste{padding:clamp(70px,16vh,180px) 0}
.manifeste p{max-width:1000px;margin:0 auto;font-size:clamp(28px,4.8vw,68px);line-height:1.08;font-weight:600;letter-spacing:-.015em;font-variation-settings:"wdth" 92;text-wrap:balance}
.lueur{display:inline}
@supports (animation-timeline: view()){
  .lueur>span{animation:lueur linear both;animation-timeline:view();animation-range:entry 30% cover 45%}
}
@keyframes lueur{from{opacity:.12}to{opacity:1}}
@keyframes apparait{from{opacity:0;transform:translateY(48px)}to{opacity:1;transform:none}}
@supports (animation-timeline: scroll()){
  body::after{content:"";position:fixed;top:0;left:0;right:0;height:2px;z-index:80;background:var(--red);transform-origin:0 50%;animation:avance linear both;animation-timeline:scroll(root)}
}
@keyframes avance{from{transform:scaleX(0)}to{transform:scaleX(1)}}
@media (prefers-reduced-motion:reduce){
  .mots .mot>span,.hero .lead,.hero .ctas,.hero .note,.hero .eyebrow,.eyebrow::before,.phone,.bande-piste,.lueur>span,.feature,.numbers li,.mini,.steps li,.final{animation:none}
  body::after{display:none}
}
/* =====================================================================
   Encore plus de mouvement : les blocs entrent en scène au défilement
   (classe .vu posée par le script), les téléphones pivotent, les cartes
   s'allument sous la souris, un second bandeau défile à l'envers.
   ===================================================================== */
.anime :is(.numbers li,.feature,.band,.final,.doc>*,.foot-grid>*,.foot-bottom,.foot-legal):not(.vu){opacity:0}
.anime :is(.numbers li,.feature,.band,.final,.doc>*,.foot-grid>*,.foot-bottom,.foot-legal):not(.vu)::before,
.anime :is(.numbers li,.feature,.band,.final,.doc>*,.foot-grid>*,.foot-bottom,.foot-legal):not(.vu)::after,
.anime :is(.numbers li,.feature,.band,.final,.doc>*,.foot-grid>*,.foot-bottom,.foot-legal):not(.vu) *,
.anime :is(.numbers li,.feature,.band,.final,.doc>*,.foot-grid>*,.foot-bottom,.foot-legal):not(.vu) *::before,
.anime :is(.numbers li,.feature,.band,.final,.doc>*,.foot-grid>*,.foot-bottom,.foot-legal):not(.vu) *::after{animation-play-state:paused!important}
.anime :is(.numbers li,.band,.final,.doc>*,.foot-grid>*,.foot-bottom,.foot-legal).vu{animation:entre .9s var(--ease) var(--d,0ms) backwards}
@keyframes entre{from{opacity:0;transform:translateY(46px)}}
@keyframes surgit{from{opacity:0;transform:translateY(14px) scale(.85)}}

/* Barre du haut : trait rouge qui file au chargement, logo plus petit après défilement. */
.top{transition:box-shadow .4s var(--ease)}
.top::after{content:"";position:absolute;left:0;right:0;bottom:0;height:2px;pointer-events:none;background:linear-gradient(90deg,transparent,var(--red),transparent) no-repeat;background-size:45% 100%;animation:file-haut 1.3s var(--ease) .2s both}
@keyframes file-haut{from{background-position:-90% 0}75%{opacity:1}to{background-position:190% 0;opacity:0}}
[data-defile] .top{box-shadow:0 14px 30px -22px rgba(0,0,0,.95)}
.top .brand{display:inline-block;transform-origin:0 50%;transition:transform .45s var(--ease)}
[data-defile] .top .brand{transform:scale(.86)}
.brand .x{display:inline-block;animation:x-allume 3.2s ease-in-out infinite}
@keyframes x-allume{0%,70%,100%{text-shadow:none}80%{text-shadow:0 0 14px rgba(225,6,0,.9);transform:skewX(-6deg)}}
.lang{transition:background .3s var(--ease),color .3s var(--ease)}
.lang:hover{background:var(--text);color:var(--bg)}
@supports not (animation-timeline: scroll()){
  body::after{content:"";position:fixed;top:0;left:0;right:0;height:2px;z-index:80;pointer-events:none;background:var(--red);transform-origin:0 50%;transform:scaleX(var(--defile,0))}
}

/* Curseur anneau (ordinateur). */
.curseur{position:fixed;top:0;left:0;z-index:95;pointer-events:none;width:36px;height:36px;margin:-18px 0 0 -18px;border-radius:50%;border:1px solid rgba(242,237,227,.45);mix-blend-mode:difference;transform:translate(-100px,-100px);transition:width .35s var(--ease),height .35s var(--ease),margin .35s var(--ease),background .35s,border-color .35s}
.curseur span{position:absolute;left:50%;top:50%;width:4px;height:4px;margin:-2px;border-radius:50%;background:var(--text)}
.curseur.actif{width:72px;height:72px;margin:-36px 0 0 -36px;background:var(--text);border-color:transparent}
@media (hover:none),(pointer:coarse),(max-width:760px){.curseur{display:none}}

/* Boutons pleins : un reflet passe ; l'appel final respire. */
.btn:not(.btn-ghost){position:relative;overflow:hidden;isolation:isolate}
.btn:not(.btn-ghost)::after{content:"";position:absolute;z-index:-1;top:0;bottom:0;left:-50%;width:40%;pointer-events:none;background:linear-gradient(100deg,transparent,rgba(255,255,255,.5),transparent);transform:skewX(-18deg);animation:reflet 5s var(--ease) 1.8s infinite}
@keyframes reflet{0%{transform:translateX(0) skewX(-18deg)}28%,100%{transform:translateX(480%) skewX(-18deg)}}
.final .btn-big{animation:appel 2.6s ease-in-out infinite}
@keyframes appel{50%{box-shadow:0 0 0 10px rgba(225,6,0,.12),0 0 46px rgba(225,6,0,.4)}}
.btn:active{scale:.96}

/* Accueil : halo rouge derrière le téléphone, qui arrive en pivotant puis suit la souris. */
.hero-shot{position:relative;isolation:isolate;transform:perspective(1000px) rotateX(var(--rx,0deg)) rotateY(var(--ry,0deg));transition:transform .6s var(--ease)}
.hero-shot::before{content:"";position:absolute;z-index:-1;inset:14% 12%;border-radius:50%;background:radial-gradient(circle,rgba(225,6,0,.5),transparent 66%);filter:blur(40px);animation:halo 6s ease-in-out infinite alternate}
@keyframes halo{from{transform:scale(.9);opacity:.6}to{transform:scale(1.1) translate(3%,-3%);opacity:1}}
.hero .phone{animation:phone-in 1.3s var(--ease) .35s backwards,flotte 7s ease-in-out 1.7s infinite}
@keyframes phone-in{from{opacity:0;transform:translateY(90px) rotate(7deg) scale(.9)}}
/* Reflet qui glisse sur l'écran des téléphones. */
.phone{position:relative}
.phone::after{content:"";position:absolute;inset:8px;border-radius:27px;pointer-events:none;background:linear-gradient(115deg,transparent 38%,rgba(255,255,255,.14) 50%,transparent 62%) no-repeat 130% 0/260% 100%;animation:reflet-ecran 7s ease-in-out 2.4s infinite}
@keyframes reflet-ecran{from{background-position:130% 0}35%,to{background-position:-130% 0}}

/* Chiffres : un trait rouge se trace sous chacun. */
.numbers li{position:relative;overflow:hidden}
.numbers li::after{content:"";position:absolute;left:16px;right:16px;bottom:0;height:2px;background:var(--red);transform-origin:0 50%;animation:trait 1.1s var(--ease) calc(var(--d,0ms) + .3s) both}
@keyframes trait{from{transform:scaleX(0)}}

/* Fonctionnalités : texte en cascade, coches qui se posent, téléphones qui pivotent depuis le côté. */
.feature-text>.eyebrow{animation:monte-in .7s var(--ease) backwards}
.feature-text>p:not(.eyebrow){animation:monte-in .8s var(--ease) .35s backwards}
.checks li{animation:monte-in .6s var(--ease) calc(var(--i,0)*90ms + .5s) backwards}
.checks li::before{animation:coche .5s cubic-bezier(.3,1.6,.5,1) calc(var(--i,0)*90ms + .7s) backwards}
@keyframes coche{from{transform:rotate(-45deg) scale(0)}}
.feature .phone{animation:phone-cote 1.1s var(--ease) .2s backwards,flotte 7s ease-in-out 1.3s infinite}
.feature-shots .phone:nth-child(2){animation-delay:.45s,1.55s}
.feature.reverse .phone{animation-name:phone-cote-inv,flotte}
@keyframes phone-cote{from{opacity:0;transform:perspective(1000px) rotateY(-32deg) translateY(70px)}}
@keyframes phone-cote-inv{from{opacity:0;transform:perspective(1000px) rotateY(32deg) translateY(70px)}}

/* Petites cartes, étapes, technologies : cascade, survol vivant. */
.mini{animation:surgit .6s var(--ease) calc(var(--i,0)*60ms + .25s) backwards;transition:translate .4s var(--ease),border-color .3s}
.mini-icon{display:inline-block;transition:transform .45s cubic-bezier(.3,1.6,.5,1)}
.steps{counter-reset:etape}
.steps li{counter-increment:etape;animation:monte-in .7s var(--ease) backwards;transition:translate .4s var(--ease),border-color .3s}
.steps li::before{content:counter(etape,decimal-leading-zero);font-weight:300;font-size:2.4rem;line-height:1;font-variation-settings:"wdth" 62;color:var(--red);margin-bottom:8px;animation:surgit .6s var(--ease) backwards}
.steps li:nth-child(2),.steps li:nth-child(2)::before{animation-delay:.15s}
.steps li:nth-child(3),.steps li:nth-child(3)::before{animation-delay:.3s}
.chips li{animation:surgit .5s var(--ease) backwards;transition:background .3s,color .3s}
.chips li:nth-child(2){animation-delay:.07s}.chips li:nth-child(3){animation-delay:.14s}.chips li:nth-child(4){animation-delay:.21s}
.chips li:nth-child(5){animation-delay:.28s}.chips li:nth-child(6){animation-delay:.35s}
@media (hover:hover) and (pointer:fine){
  .mini:hover,.steps li:hover,.numbers li:hover{translate:0 -4px;border-color:rgba(242,237,227,.3);background-image:radial-gradient(320px circle at var(--mx,50%) var(--my,0%),rgba(242,237,227,.07),transparent 62%)}
  .mini:hover .mini-icon{transform:scale(1.25) rotate(-10deg)}
  .chips li:hover{background:var(--text);color:var(--bg)}
}
.numbers li{transition:translate .4s var(--ease),border-color .3s}

/* Second bandeau : lettres en contour, défile à l'envers ; tous s'arrêtent au survol. */
.bande.inverse{margin-top:0;margin-bottom:12px;transform:rotate(1.6deg) scale(1.04);background:transparent}
.bande.inverse .bande-piste{animation-direction:reverse;animation-duration:44s}
.bande.inverse .bande-piste span{color:transparent;-webkit-text-stroke:1px var(--text)}
.bande:hover .bande-piste{animation-play-state:paused}

/* Appel final : halo rouge qui respire. */
.final{position:relative;isolation:isolate;overflow:hidden}
.final::before{content:"";position:absolute;z-index:-1;left:50%;top:50%;width:min(720px,90%);aspect-ratio:1;translate:-50% -50%;border-radius:50%;background:radial-gradient(circle,rgba(225,6,0,.22),transparent 62%);animation:respire-final 5s ease-in-out infinite}
@keyframes respire-final{50%{transform:scale(1.15);opacity:.7}}

/* Pied de page : liens soulignés au survol. */
.foot nav a{position:relative}
.foot nav a::after{content:"";position:absolute;left:0;right:0;bottom:3px;height:1px;background:currentColor;transform:scaleX(0);transform-origin:right;transition:transform .5s var(--ease)}
.foot nav a:hover::after{transform:scaleX(1);transform-origin:left}

@media (prefers-reduced-motion:reduce){
  *,*::before,*::after{animation-duration:.01ms!important;animation-iteration-count:1!important;animation-delay:0s!important;transition-duration:.01ms!important}
  html{scroll-behavior:auto}
  .hero-shot{transform:none}
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
