//! Site de présentation (`/presentation`) : HTML + CSS rendus par le serveur, sans JavaScript.
//! Bilingue : `?lang=fr|en`, sinon langue du navigateur (`Accept-Language`).

use axum::extract::Query;
use axum::http::{HeaderMap, header};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;

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

/// Captures d'écran réelles de l'app (générées avec Playwright), embarquées dans le binaire.
pub const SHOTS: &[(&str, &[u8])] = &[
    ("home-fr.jpg", include_bytes!("../static/shots/home-fr.jpg")),
    ("home-en.jpg", include_bytes!("../static/shots/home-en.jpg")),
    ("live-fr.jpg", include_bytes!("../static/shots/live-fr.jpg")),
    ("live-en.jpg", include_bytes!("../static/shots/live-en.jpg")),
    (
        "strategy-fr.jpg",
        include_bytes!("../static/shots/strategy-fr.jpg"),
    ),
    (
        "strategy-en.jpg",
        include_bytes!("../static/shots/strategy-en.jpg"),
    ),
    (
        "track-fr.jpg",
        include_bytes!("../static/shots/track-fr.jpg"),
    ),
    (
        "track-en.jpg",
        include_bytes!("../static/shots/track-en.jpg"),
    ),
    (
        "records-fr.jpg",
        include_bytes!("../static/shots/records-fr.jpg"),
    ),
    (
        "records-en.jpg",
        include_bytes!("../static/shots/records-en.jpg"),
    ),
    (
        "compare-fr.jpg",
        include_bytes!("../static/shots/compare-fr.jpg"),
    ),
    (
        "compare-en.jpg",
        include_bytes!("../static/shots/compare-en.jpg"),
    ),
];

pub async fn page(Query(q): Query<LangQuery>, headers: HeaderMap) -> Response {
    let lang = match q.lang.as_deref() {
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
    };
    let back = safe_back(q.back.as_deref());
    (
        [
            (header::CACHE_CONTROL, "public, max-age=600"),
            (header::VARY, "Accept-Language"),
        ],
        Html(render(lang, back.as_deref())),
    )
        .into_response()
}

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

fn render(lang: Lang, back: Option<&str>) -> String {
    let fr = lang == Lang::Fr;
    let t = |f: &'static str, e: &'static str| if fr { f } else { e };
    let code = if fr { "fr" } else { "en" };
    let other = if fr { ("en", "EN") } else { ("fr", "FR") };
    let img = |name: &str, alt: &str| {
        format!(
            r#"<figure class="phone"><img src="/presentation/shots/{name}-{code}.jpg" alt="{alt}" width="390" height="844" loading="lazy"></figure>"#
        )
    };

    let feature = |eyebrow: &str,
                   title: &str,
                   text: &str,
                   bullets: &[&str],
                   shots: &str,
                   reverse: bool| {
        let items: String = bullets.iter().map(|b| format!("<li>{b}</li>")).collect();
        format!(
            r#"<section class="feature{rev}"><div class="feature-text"><p class="eyebrow">{eyebrow}</p><h2>{title}</h2><p>{text}</p><ul class="checks">{items}</ul></div><div class="feature-shots">{shots}</div></section>"#,
            rev = if reverse { " reverse" } else { "" }
        )
    };

    let features = [
        feature(
            t("Race Center", "Race Center"),
            t("La course comme si tu y étais.", "The race as if you were there."),
            t(
                "Rejoue n'importe quelle session depuis 2023 comme en direct, de ×1 à ×60, avec pause et retour en arrière. Tout arrive en temps réel par WebSocket.",
                "Replay any session since 2023 as if it were live, from ×1 to ×60, with pause and rewind. Everything streams in real time over WebSocket.",
            ),
            &[
                t("Classement, écarts avec la voiture devant et le leader", "Order, gaps to the car ahead and to the leader"),
                t("Secteurs violet / vert / jaune, pneus et âge des gommes", "Purple / green / yellow sectors, tyres and tyre age"),
                t("Carte avec les voitures, stratégie des relais, chronologie", "Map with the cars, stint strategy, timeline"),
                t("« Et s'il s'arrêtait maintenant ? » : position de sortie des stands", "“What if they pit now?”: projected rejoin position"),
            ],
            &(img("live", t("Race Center en replay", "Race Center replay")) + &img("strategy", t("Stratégie des pneus", "Tyre strategy"))),
            false,
        ),
        feature(
            t("Circuits", "Circuits"),
            t("Chaque circuit, tracé au GPS.", "Every circuit, drawn from GPS."),
            t(
                "Le tracé est reconstitué à partir des positions réelles d'une voiture sur son meilleur tour, coloré selon la vitesse. Touche-le pour lire la télémétrie.",
                "The layout is rebuilt from a real car's positions on its fastest lap, coloured by speed. Touch it to read the telemetry.",
            ),
            &[
                t("Vitesse, rapport, accélérateur et freinage en chaque point", "Speed, gear, throttle and braking at every point"),
                t("Longueur, vitesses max / mini / moyenne, % à fond", "Length, top / min / average speed, % full throttle"),
                t("Carte, prochain GP, record, rois du circuit, palmarès", "Map, next GP, lap record, kings of the circuit, winners"),
            ],
            &img("track", t("Tracé de Bakou coloré par la vitesse", "Baku layout coloured by speed")),
            true,
        ),
        feature(
            t("75 ans d'histoire", "75 years of history"),
            t("Toutes les saisons depuis 1950.", "Every season since 1950."),
            t(
                "Calendriers, résultats, classements, carrières des pilotes, palmarès des écuries : toute l'histoire de la F1 est à portée de pouce.",
                "Calendars, results, standings, driver careers, team records: the whole history of F1 at your fingertips.",
            ),
            &[
                t("Records : titres, victoires, poles, séries, plus jeunes vainqueurs", "Records: titles, wins, poles, streaks, youngest winners"),
                t("881 pilotes et 214 écuries, avec recherche", "881 drivers and 214 teams, searchable"),
                t("Analyse tour par tour des Grands Prix depuis 1996", "Lap-by-lap analysis of Grands Prix since 1996"),
            ],
            &img("records", t("Records de la F1", "F1 records")),
            false,
        ),
        feature(
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
            true,
        ),
    ]
    .join("");

    let cards = [
        ("🌦️", t("Météo du week-end", "Weekend weather"), t("Prévisions par session et impact sur la course.", "Forecast per session and race impact.")),
        ("⏱", t("Compte à rebours", "Countdown"), t("Prochaine séance à ton heure locale.", "Next session in your local time.")),
        ("📰", t("Actualités", "News"), t("Les derniers titres de la presse F1.", "Latest F1 headlines.")),
        ("⭐", t("Favoris", "Favourites"), t("Ton pilote et ton écurie mis en avant.", "Your driver and team highlighted.")),
        ("🌍", t("Français / English", "English / Français"), t("Toute l'app dans les deux langues.", "The whole app in both languages.")),
        ("📱", t("Pensée pour le mobile", "Mobile first"), t("Zéro défilement horizontal, installable.", "No horizontal scrolling, installable.")),
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
    let top_button = if back.is_some() {
        t("← Retour à l'app", "← Back to the app")
    } else {
        t("Ouvrir l'app", "Open the app")
    };
    format!(
        r##"<!doctype html>
<html lang="{code}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="theme-color" content="#0b0b10">
<title>{title}</title>
<meta name="description" content="{desc}">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{desc}">
<meta property="og:type" content="website">
<meta property="og:image" content="/presentation/shots/live-{code}.jpg">
<link rel="alternate" hreflang="fr" href="/presentation?lang=fr">
<link rel="alternate" hreflang="en" href="/presentation?lang=en">
<link rel="icon" href="/static/icon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/static/icon.svg">
<style>{CSS}</style>
</head>
<body>
<header class="top">
  <a class="brand" href="/presentation?lang={code}" aria-label="F1X"><span>F1</span><span class="x">X</span></a>
  <nav class="top-links">
    <a class="lang" href="/presentation?lang={other_code}{back_q}" hreflang="{other_code}">{other_label}</a>
    <a class="btn btn-small" href="{app}">{top_button}</a>
  </nav>
</header>
<main>
  <section class="hero">
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
    <li><strong>78</strong><span>circuits</span></li>
  </ul>

  <div id="fonctionnalites">{features}</div>

  <section class="band">
    <h2>{more}</h2>
    <ul class="minis">{cards}</ul>
  </section>

  <section class="band install">
    <h2>{install_h}</h2>
    <ol class="steps">
      <li><strong>iPhone (Safari)</strong><span>{install_ios}</span></li>
      <li><strong>Android (Chrome)</strong><span>{install_android}</span></li>
    </ol>
  </section>

  <section class="band tech">
    <p class="eyebrow">{tech_eyebrow}</p>
    <h2>{tech_h}</h2>
    <p>{tech_p}</p>
    <ul class="chips"><li>Rust</li><li>Yew 0.23</li><li>WebAssembly</li><li>axum</li><li>WebSocket</li><li>Heroku</li></ul>
  </section>

  <section class="final">
    <h2>{final_h}</h2>
    <a class="btn btn-big" href="{app}">{cta} <span aria-hidden="true">→</span></a>
  </section>
</main>
<footer>
  <p>{sources}</p>
  <p>{disclaimer}</p>
  <p><a href="https://github.com/maxlestage/f1x">GitHub</a> · <a href="{app}">{open}</a></p>
</footer>
</body>
</html>"##,
        CSS = CSS,
        app = app,
        back_q = back_q,
        top_button = top_button,
        title = t(
            "F1X — Toute la Formule 1 dans ta poche",
            "F1X — All of Formula 1 in your pocket"
        ),
        desc = t(
            "Application web gratuite : calendrier, résultats, Race Center en temps réel, tracés GPS des circuits, 75 ans d'archives, records, pronostics et Fantasy. En français et en anglais.",
            "Free web app: calendar, results, real-time Race Center, GPS circuit layouts, 75 years of history, records, predictions and Fantasy. In English and French.",
        ),
        other_code = other.0,
        other_label = other.1,
        open = t("Ouvrir l'app", "Open the app"),
        eyebrow = t(
            "Application web gratuite · 100 % Rust",
            "Free web app · 100% Rust"
        ),
        h1 = t(
            "Toute la Formule 1 dans ta poche.",
            "All of Formula 1 in your pocket."
        ),
        lead = t(
            "Calendrier, résultats, Race Center en temps réel, tracés GPS des circuits et 75 ans d'archives. Rapide, clair, pensé pour le téléphone.",
            "Calendar, results, a real-time Race Center, GPS circuit layouts and 75 years of history. Fast, clear and built for your phone.",
        ),
        cta = t("Voir l'app web", "See the web app"),
        discover = t("Découvrir", "Discover"),
        note = t(
            "Sans compte, sans installation, sans publicité.",
            "No account, no install, no ads."
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
        features = features,
        more = t("Et aussi", "And also"),
        cards = cards,
        install_h = t("Installe-la comme une app", "Install it like an app"),
        install_ios = t(
            "Ouvre F1X, touche Partager puis « Sur l'écran d'accueil ».",
            "Open F1X, tap Share, then “Add to Home Screen”."
        ),
        install_android = t(
            "Ouvre F1X, menu ⋮ puis « Installer l'application ».",
            "Open F1X, ⋮ menu, then “Install app”."
        ),
        tech_eyebrow = t("Sous le capot", "Under the hood"),
        tech_h = t(
            "Écrite en Rust, de bout en bout.",
            "Written in Rust, end to end."
        ),
        tech_p = t(
            "L'interface est en Rust compilé en WebAssembly (Yew), le serveur aussi (axum). Le temps réel passe par WebSocket, les données sont mises en cache pour rester rapides.",
            "The interface is Rust compiled to WebAssembly (Yew), and so is the server (axum). Real time runs over WebSocket, and data is cached to stay fast.",
        ),
        final_h = t(
            "Prêt pour le prochain Grand Prix ?",
            "Ready for the next Grand Prix?"
        ),
        sources = t(
            "Données : Jolpica F1 (ex-Ergast), OpenF1, Open-Meteo, OpenStreetMap et les flux RSS de la presse spécialisée.",
            "Data: Jolpica F1 (formerly Ergast), OpenF1, Open-Meteo, OpenStreetMap and specialist press RSS feeds.",
        ),
        disclaimer = t(
            "F1X est un projet de fan non officiel, sans lien avec la Formula 1, la FIA ou les écuries. F1, Formula 1 et les marques associées appartiennent à leurs propriétaires.",
            "F1X is an unofficial fan project, not affiliated with Formula 1, the FIA or the teams. F1, Formula 1 and related marks belong to their owners.",
        ),
    )
}

const CSS: &str = r#"
:root{--bg:#0b0b10;--surface:#15151e;--surface-2:#1e1e2a;--line:#2a2a38;--text:#f4f4f8;--muted:#a3a3b5;--red:#e10600;color-scheme:dark}
*,*::before,*::after{box-sizing:border-box;min-width:0}
html,body{margin:0;max-width:100%;overflow-x:hidden}
body{background:var(--bg);color:var(--text);font:17px/1.55 system-ui,-apple-system,"SF Pro Text","Segoe UI",Roboto,sans-serif;overflow-wrap:anywhere;-webkit-text-size-adjust:100%}
a{color:inherit}
h1,h2,p{margin:0}
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
.band h2{font-size:clamp(1.5rem,6vw,2.1rem);font-weight:900;margin-bottom:18px;letter-spacing:-.02em}
.minis{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,230px),1fr));gap:10px}
.mini{background:var(--surface);border:1px solid var(--line);border-radius:16px;padding:16px;display:flex;flex-direction:column;gap:4px}
.mini span:last-child{color:var(--muted);font-size:.92rem}
.mini-icon{font-size:1.4rem}
.steps{margin:0;padding:0;list-style:none;display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,260px),1fr));gap:10px}
.steps li{background:var(--surface);border:1px solid var(--line);border-radius:16px;padding:16px;display:flex;flex-direction:column;gap:4px}
.steps span{color:var(--muted)}
.tech p:not(.eyebrow){color:var(--muted);max-width:60ch}
.tech h2{margin-top:8px}
.chips{list-style:none;margin:16px 0 0;padding:0;display:flex;flex-wrap:wrap;gap:8px}
.chips li{background:var(--surface-2);border:1px solid var(--line);border-radius:999px;padding:6px 12px;font-size:.85rem;font-weight:700}
.final{padding:56px 0 64px;border-top:1px solid var(--line);display:flex;flex-direction:column;align-items:center;gap:20px;text-align:center}
.final h2{font-size:clamp(1.7rem,7vw,2.6rem);font-weight:900;letter-spacing:-.02em}
footer{border-top:1px solid var(--line);padding:24px 16px calc(env(safe-area-inset-bottom) + 28px);color:var(--muted);font-size:.82rem;display:flex;flex-direction:column;gap:8px;max-width:1080px;margin:0 auto}
@media (min-width:860px){
  .hero{grid-template-columns:minmax(0,1.1fr) minmax(0,.9fr);padding:72px 0 40px}
  .numbers{grid-template-columns:repeat(4,minmax(0,1fr))}
  .feature{grid-template-columns:minmax(0,1fr) minmax(0,1fr);align-items:center;gap:48px;padding:72px 0}
  .feature.reverse .feature-text{order:2}
}
"#;

#[cfg(test)]
mod tests {
    use super::safe_back;

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
}
