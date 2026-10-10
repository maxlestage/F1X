//! Composants partagés.
//!
//! Règle de mise en page : tout s'empile verticalement. Pas de tableau, pas de carrousel,
//! rien de plus large que l'écran — le texte long passe à la ligne.

use active::prelude::*;
use gloo_timers::callback::Interval;

use crate::api::{Fetch, f1, use_f1};
use crate::i18n::t;
use crate::models::{ConstructorStanding, DriverStanding, QualifyingResult, Race, RaceResult};
use crate::tr;
use crate::util::{
    flag_nationality, local_date, now_ms, session_label, set_title, team_style, wins_label,
};
use crate::{CURRENT, Route, link};

/// Une partie de la vue recalculée quand les états lus par `f` changent (un seul nœud) :
/// raccourci de `fragment_dyn(move || vec![f()])`.
pub fn dynamic(f: impl Fn() -> Node + 'static) -> Node {
    fragment_dyn(move || vec![f()])
}

/// `class` si `on`, sinon rien : `div().class(when(fav, "row-fav"))`.
pub fn when(on: bool, class: &'static str) -> &'static str {
    if on { class } else { "" }
}

/// Pictogramme au trait (24 × 24) : `d` est le tracé SVG.
pub fn icon(d: &'static str) -> Element {
    svg()
        .attr("viewBox", "0 0 24 24")
        .attr("aria-hidden", "true")
        .child(path().attr("d", d))
}

/// Lien vers une page rendue par le serveur (présentation, pages légales, fichiers) :
/// chargée normalement, pas par le routeur de l'app.
pub fn server_link(href: impl Into<active::Str>) -> Element {
    a().href(href).attr("rel", "external")
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Home,
    Live,
    Calendar,
    Standings,
    Archives,
}

/// Gabarit de toutes les pages : barre du haut, contenu, pied de page et barre d'onglets.
/// `title` vide : titre de l'accueil.
pub fn layout(title: &str, tab: Option<Tab>, content: impl Into<Node>) -> Node {
    let title = title.to_string();
    layout_dyn(move || title.clone(), tab, content)
}

/// Comme [`layout`], avec un titre calculé à partir d'états (ex. le nom du pilote une fois
/// chargé) : la barre du haut et le titre de l'onglet suivent.
pub fn layout_dyn(
    title: impl Fn() -> String + 'static,
    tab: Option<Tab>,
    content: impl Into<Node>,
) -> Node {
    let title = std::rc::Rc::new(title);
    // Bouton « Site » : uniquement sur l'accueil ; le site y ramène (« back » par défaut = /).
    let on_home = untrack(|| location().get()) == "/";
    let site_href = format!("/presentation?lang={}", crate::i18n::lang().code());
    let other = crate::i18n::lang().other();
    // Changement de rubrique : grand logo animé au centre, bref et sans bloquer l'écran.
    let previous = LAST_TAB.with(|c| c.replace(tab));
    let intro = previous.is_some() && tab.is_some() && previous != tab;
    {
        // Nouveau titre (autre page, autres paramètres, données arrivées) : titre de l'onglet
        // mis à jour et retour en haut de la page.
        let title = title.clone();
        effect(move || {
            let title = title();
            untrack(|| {
                set_title(&title);
                if let Some(w) = web_sys::window() {
                    w.scroll_to_with_x_and_y(0.0, 0.0);
                }
            });
        });
    }
    let topbar_title = {
        let title = title.clone();
        dynamic(move || {
            let title = title();
            if title.is_empty() {
                Node::Empty
            } else {
                h1().class("topbar-title").text(title).into()
            }
        })
    };
    let header = header()
        .class("topbar")
        .child(
            link(Route::Home, "brand")
                .child(span().class("brand-mark").text("F1"))
                .child(span().class("brand-x").text("X")),
        )
        .child(tab.map(|tab| section_logo(tab, false)))
        .child(topbar_title)
        .child(on_home.then(|| {
            server_link(site_href)
                .class("site-link")
                .attr(
                    "aria-label",
                    t("Site de présentation de F1X", "F1X presentation site"),
                )
                .text(t("Site", "Site"))
        }))
        .child(
            button()
                .class("lang-switch")
                .attr("aria-label", t("Switch to English", "Passer en français"))
                .on_click(|_| crate::i18n::toggle())
                .text(other.code().to_uppercase()),
        );
    let intro = tab.filter(|_| intro).map(|tab| {
        let (class, _, label) = section_info(tab);
        div()
            .class("sec-intro")
            .class(class)
            .attr("aria-hidden", "true")
            .child(section_logo(tab, true))
            .child(span().class("sec-intro-label").text(label))
    });
    fragment([
        Node::from(header),
        intro.into(),
        main().class("page").child(content).into(),
        crate::pwa::app_footer(),
        tab_bar(tab),
    ])
}

fn tab_bar(active: Option<Tab>) -> Node {
    let items = [
        (Tab::Home, Route::Home, t("Accueil", "Home"), ICON_HOME),
        (Tab::Live, Route::Live, t("Direct", "Live"), ICON_LIVE),
        (
            Tab::Calendar,
            Route::season(CURRENT),
            t("Calendrier", "Calendar"),
            ICON_CAL,
        ),
        (
            Tab::Standings,
            Route::DriverStandings {
                season: CURRENT.into(),
            },
            t("Classements", "Standings"),
            ICON_TROPHY,
        ),
        (
            Tab::Archives,
            Route::Archives,
            t("Explorer", "Explore"),
            ICON_ARCHIVE,
        ),
    ];
    nav()
        .class("tabbar")
        .attr("aria-label", t("Navigation principale", "Main navigation"))
        .children(items.into_iter().map(|(tab, route, label, svg)| {
            link(route, "tab")
                .class(when(active == Some(tab), "tab-active"))
                .child(icon(svg))
                .child(span().text(label))
        }))
        .into()
}

pub fn loading() -> Node {
    div()
        .class("loading")
        .attr("aria-busy", "true")
        .text(t("Chargement…", "Loading…"))
        .into()
}

/// Affiche le chargement / l'erreur, ou délègue le rendu une fois les données là.
///
/// La partie se reconstruit quand `fetch` change, et aussi quand un autre état lu par `render`
/// change : lisez dans `render` uniquement ce qui doit tout reconstruire, et placez le reste
/// dans des parties dynamiques plus petites.
pub fn fetch_view(
    fetch: State<Fetch>,
    render: impl Fn(&crate::models::MrData) -> Node + 'static,
) -> Node {
    dynamic(move || match fetch.get() {
        Fetch::Idle => Node::Empty,
        Fetch::Loading => loading(),
        Fetch::Failed(message) => error_card(&message),
        Fetch::Done(value) => render(&value),
    })
}

pub fn error_card(message: &str) -> Node {
    section()
        .class("card")
        .child(h2().text(t("Drapeau rouge 🚩", "Red flag 🚩")))
        .child(p().class("muted").text(message.to_string()))
        .child(
            button()
                .class("btn")
                .on_click(|_| {
                    if let Some(w) = web_sys::window() {
                        let _ = w.location().reload();
                    }
                })
                .text(t("Réessayer", "Try again")),
        )
        .into()
}

pub fn empty_card(text: &str) -> Node {
    section()
        .class("card")
        .child(p().class("muted").text(text.to_string()))
        .into()
}

/// Grille de statistiques (3 colonnes égales, jamais plus large que l'écran).
pub fn stat_grid(items: Vec<(&'static str, String)>) -> Node {
    dl().class("stats")
        .children(items.into_iter().map(|(label, value)| {
            // Valeurs longues (temps au tour, unités) : police adaptée à la largeur de la case.
            let long = value.chars().count() > 5;
            div()
                .child(dt().text(label))
                .child(dd().class(when(long, "dd-long")).text(value))
        }))
        .into()
}

/// Valeur d'un compteur `total` (requête `limit=1`), « – » pendant le chargement.
pub fn total_of(fetch: &Fetch) -> String {
    fetch
        .done()
        .map(|d| d.total().to_string())
        .unwrap_or_else(|| "–".into())
}

#[derive(Clone, Copy, PartialEq)]
pub enum SeasonTarget {
    Calendar,
    DriverStandings,
    TeamStandings,
}

impl SeasonTarget {
    fn route(self, season: String) -> Route {
        match self {
            Self::Calendar => Route::Season { season },
            Self::DriverStandings => Route::DriverStandings { season },
            Self::TeamStandings => Route::TeamStandings { season },
        }
    }
}

/// Sélecteur de saison natif (liste déroulante verticale). `season` : saison affichée
/// (`"current"` ou une année), suivie quand elle change ; `target` : page ouverte au changement
/// (lue au moment du choix) ; `only` : liste
/// restreinte (ex. saisons d'un pilote), sinon toutes les saisons depuis 1950.
pub fn season_select(
    season: impl Fn() -> String + 'static,
    target: impl Fn() -> SeasonTarget + 'static,
    only: Option<Vec<String>>,
) -> Node {
    let fetch = use_f1(if only.is_none() {
        f1("seasons.json", 100)
    } else {
        None
    });
    let options = dynamic(move || {
        let season = season();
        let mut seasons: Vec<String> = match &only {
            Some(list) => list.clone(),
            None => fetch.with(|f| {
                f.done()
                    .map(|d| d.seasons().iter().map(|s| s.season.clone()).collect())
                    .unwrap_or_default()
            }),
        };
        seasons.reverse();
        let selected = |o: Element, on: bool| if on { o.attr("selected", "") } else { o };
        let current = only.is_none().then(|| {
            selected(option().attr("value", CURRENT), season == CURRENT)
                .text(t("Saison en cours", "Current season"))
        });
        fragment([
            Node::from(current),
            fragment(
                seasons
                    .into_iter()
                    .map(|s| selected(option().attr("value", s.clone()), season == s).text(s)),
            ),
        ])
    });
    label()
        .class("select")
        .child(span().class("select-label").text(t("Saison", "Season")))
        .child(
            select()
                .attr("aria-label", t("Choisir une saison", "Choose a season"))
                .on("change", move |e| {
                    navigate(&target().route(e.value()).href())
                })
                .child(options),
        )
        .into()
}

/// Compte à rebours : grille fixe de 4 colonnes, mise à jour chaque seconde.
pub fn countdown(target_ms: f64) -> Node {
    let now = use_state(now_ms());
    let interval = Interval::new(1000, move || now.set(now_ms()));
    on_cleanup(move || drop(interval));
    let secs = move || ((target_ms - now.get()) / 1000.0).max(0.0) as u64;
    let cell = |value: Box<dyn Fn(u64) -> String>, label: &str| {
        div()
            .class("cd-cell")
            .child(span().class("cd-num").text_dyn(move || value(secs())))
            .child(span().class("cd-label").text(label.to_string()))
    };
    div()
        .class("countdown")
        .child(cell(
            Box::new(|s| (s / 86400).to_string()),
            t("jours", "days"),
        ))
        .child(cell(
            Box::new(|s| format!("{:02}", s % 86400 / 3600)),
            t("heures", "hours"),
        ))
        .child(cell(Box::new(|s| format!("{:02}", s % 3600 / 60)), "min"))
        .child(cell(Box::new(|s| format!("{:02}", s % 60)), "sec"))
        .into()
}

/// Programme du week-end, à l'heure locale.
pub fn sessions_list(race: &Race) -> Node {
    let has_time = race.has_time();
    ul().class("sessions")
        .children(race.sessions().into_iter().map(|(name, iso)| {
            li().class("session")
                .class(when(name == "Course", "session-race"))
                .child(
                    span()
                        .class("session-name")
                        .text(session_label(name).to_string()),
                )
                .child(
                    time()
                        .class("session-time")
                        .attr("datetime", iso.clone())
                        .text(local_date(&iso, has_time)),
                )
        }))
        .into()
}

fn avatar_of(name: String, url: Option<String>, team: Option<&str>) -> Node {
    let colour = team.map(crate::util::team_color).unwrap_or("#8a8a99");
    crate::photo::avatar(&name, url.as_deref(), colour, 40)
}

pub fn driver_standing_row(s: &DriverStanding) -> Node {
    let team = s.constructors.last();
    let fav = crate::util::fav_driver().as_deref() == Some(s.driver.driver_id.as_str());
    let row = li().class("row").class(when(fav, "row-fav"));
    let row = match team {
        Some(t) => row.style(team_style(&t.constructor_id)),
        None => row,
    };
    let mut sub = team.map(|t| t.name.clone()).unwrap_or_default();
    if s.wins != "0" {
        sub.push_str(&tr!(" · {} V", " · {} W", s.wins));
    }
    row.child(span().class("pos").text(s.rank()))
        .child(avatar_of(
            s.driver.full_name(),
            s.driver.url.clone(),
            team.map(|t| t.constructor_id.as_str()),
        ))
        .child(
            link(Route::driver(&s.driver.driver_id), "row-main")
                .child(
                    span()
                        .class("row-title")
                        .text(format!(
                            "{} {} ",
                            flag_nationality(s.driver.nationality.as_deref()),
                            s.driver.given_name
                        ))
                        .child(strong().text(s.driver.family_name.clone())),
                )
                .child(span().class("row-sub").text(sub)),
        )
        .child(
            span()
                .class("pts")
                .text(s.points.clone())
                .child(small().text(" pts")),
        )
        .into()
}

pub fn team_standing_row(s: &ConstructorStanding, leader_points: f64) -> Node {
    let wins = wins_label(&s.wins);
    let pts: f64 = s.points.parse().unwrap_or(0.0);
    let fav = crate::util::fav_team().as_deref() == Some(s.constructor.constructor_id.as_str());
    let pct = if leader_points > 0.0 {
        (pts / leader_points * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    li().class("row")
        .class(when(fav, "row-fav"))
        .style(team_style(&s.constructor.constructor_id))
        .child(span().class("pos").text(s.rank()))
        .child(
            link(Route::team(&s.constructor.constructor_id), "row-main")
                .child(span().class("row-title").text(format!(
                    "{} {}",
                    flag_nationality(s.constructor.nationality.as_deref()),
                    s.constructor.name
                )))
                .child(
                    span()
                        .class("bar")
                        .attr("aria-hidden", "true")
                        .child(span().class("bar-fill").style(format!("width:{pct:.1}%"))),
                )
                .child((!wins.is_empty()).then(|| span().class("row-sub").text(wins))),
        )
        .child(
            span()
                .class("pts")
                .text(s.points.clone())
                .child(small().text(" pts")),
        )
        .into()
}

pub fn result_row(r: &RaceResult) -> Node {
    let scored = r.points.parse::<f64>().unwrap_or(0.0) > 0.0;
    let mut sub = vec![r.constructor.name.clone()];
    let outcome = r.outcome();
    if !outcome.is_empty() {
        sub.push(outcome);
    }
    let gained = r.places_gained();
    let delta = match (gained, r.grid.as_deref()) {
        (Some(g), Some(grid)) => Some(fragment([
            Node::from(" · "),
            span()
                .class("delta")
                .class(when(g > 0, "up"))
                .class(when(g < 0, "down"))
                .attr("title", tr!("Parti P{grid}", "Started P{grid}"))
                .text(match g {
                    g if g > 0 => format!("▲{g}"),
                    g if g < 0 => format!("▼{}", -g),
                    _ => "=".into(),
                })
                .into(),
        ])),
        _ => None,
    };
    li().class("row")
        .style(team_style(&r.constructor.constructor_id))
        .child(span().class("pos").text(r.position_text.clone()))
        .child(
            link(Route::driver(&r.driver.driver_id), "row-main")
                .child(
                    span()
                        .class("row-title")
                        .text(format!("{} ", r.driver.given_name))
                        .child(strong().text(r.driver.family_name.clone()))
                        .child(r.has_fastest_lap().then(|| {
                            fragment([
                                Node::from(" "),
                                span()
                                    .class("tag tag-purple")
                                    .attr("title", t("Meilleur tour", "Fastest lap"))
                                    .text("⏱")
                                    .into(),
                            ])
                        })),
                )
                .child(span().class("row-sub").text(sub.join(" · ")).child(delta)),
        )
        .child(scored.then(|| span().class("pts").text(format!("+{}", r.points))))
        .into()
}

pub fn qualifying_row(q: &QualifyingResult) -> Node {
    quali_row(q, "")
}

/// Ligne de qualifs sprint (segments SQ1, SQ2, SQ3).
pub fn sprint_qualifying_row(q: &QualifyingResult) -> Node {
    quali_row(q, "S")
}

fn quali_row(q: &QualifyingResult, prefix: &str) -> Node {
    li().class("row")
        .style(team_style(&q.constructor.constructor_id))
        .child(span().class("pos").text(q.position.clone()))
        .child(
            link(Route::driver(&q.driver.driver_id), "row-main")
                .child(
                    span()
                        .class("row-title")
                        .text(format!("{} ", q.driver.given_name))
                        .child(strong().text(q.driver.family_name.clone())),
                )
                .child(span().class("row-sub").text(q.constructor.name.clone())),
        )
        .child(q.best().map(|(seg, time)| {
            span()
                .class("pts pts-time")
                .text(time.to_string())
                .child(small().text(format!(" {prefix}{seg}")))
        }))
        .into()
}

/// Lien de navigation en carte (flèche à droite), pour les listes d'archives.
pub fn nav_row(
    route: Route,
    title: impl Into<Node>,
    sub: String,
    trailing: Option<String>,
) -> Node {
    li().class("row row-plain")
        .child(
            link(route, "row-main")
                .child(span().class("row-title").child(title))
                .child((!sub.is_empty()).then(|| span().class("row-sub").text(sub))),
        )
        .child(trailing.map(|t| span().class("pts").text(t)))
        .into()
}

/// Identité visuelle de chaque rubrique : classe CSS (couleur), pictogramme, nom.
fn section_info(tab: Tab) -> (&'static str, &'static str, &'static str) {
    match tab {
        Tab::Home => ("sec-home", ICON_HOME, t("Accueil", "Home")),
        Tab::Live => ("sec-live", ICON_LIVE, t("Direct", "Live")),
        Tab::Calendar => ("sec-cal", ICON_CAL, t("Calendrier", "Calendar")),
        Tab::Standings => ("sec-standings", ICON_TROPHY, t("Classements", "Standings")),
        Tab::Archives => ("sec-explore", ICON_ARCHIVE, t("Explorer", "Explore")),
    }
}

/// Logo de rubrique : écusson incliné à la couleur de la rubrique, animé à l'affichage.
fn section_logo(tab: Tab, big: bool) -> Element {
    let (class, svg, _) = section_info(tab);
    span()
        .class("sec-logo")
        .class(class)
        .class(when(big, "sec-logo-big"))
        .attr("aria-hidden", "true")
        .child(span().class("sec-trail"))
        .child(span().class("sec-badge").child(icon(svg)))
}

thread_local! {
    /// Dernière rubrique affichée (pour n'animer l'intro qu'au changement de rubrique).
    static LAST_TAB: std::cell::Cell<Option<Tab>> = const { std::cell::Cell::new(None) };
}

const ICON_HOME: &str = "M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z";
const ICON_LIVE: &str = "M12 12h.01M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7M5.6 5.6a9 9 0 0 0 0 12.8M18.4 5.6a9 9 0 0 1 0 12.8";
const ICON_CAL: &str =
    "M7 3v3M17 3v3M4 9h16M5 5h14a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z";
const ICON_TROPHY: &str =
    "M8 4h8v5a4 4 0 0 1-8 0zM8 6H5a3 3 0 0 0 3 4M16 6h3a3 3 0 0 1-3 4M12 13v4M8 21h8M9 17h6v4H9z";
const ICON_ARCHIVE: &str = "M4 4h16v4H4zM5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8M10 12h4";

/// Bouton « Exporter en CSV ».
pub fn export_csv(filename: String, rows: Vec<Vec<String>>) -> Node {
    let disabled = rows.len() < 2;
    let b = button().class("btn btn-ghost btn-small");
    let b = if disabled { b.attr("disabled", "") } else { b };
    b.on_click(move |_| crate::util::download_csv(&filename, &rows))
        .text(t("⬇ Exporter (CSV)", "⬇ Export (CSV)"))
        .into()
}

/// Bouton étoile : pilote / écurie favori (un de chaque, enregistré sur l'appareil).
/// `kind` : « driver » ou « team ».
pub fn fav_button(kind: &str, id: &str) -> Node {
    let key = format!("f1x-fav-{kind}");
    let current = use_state(crate::util::load::<String>(&key));
    let id = id.to_string();
    let on = {
        let id = id.clone();
        move || current.with(|c| c.as_deref() == Some(id.as_str()))
    };
    let on_click = {
        let on = on.clone();
        move |_: Event| {
            let next = if on() { None } else { Some(id.clone()) };
            match &next {
                Some(v) => crate::util::store(&key, v),
                None => crate::util::store(&key, &Option::<String>::None),
            }
            current.set(next);
        }
    };
    button()
        .class("fav")
        .class_if("fav-on", on.clone())
        .attr_dyn("aria-pressed", {
            let on = on.clone();
            move || on().to_string()
        })
        .on_click(on_click)
        .text_dyn(move || {
            if on() {
                t("★ Mon favori", "★ My favourite")
            } else {
                t("☆ Ajouter aux favoris", "☆ Add to favourites")
            }
            .to_string()
        })
        .into()
}

/// Résumé Wikipédia (en français quand l'article existe), repliable. `url` : article
/// Wikipédia anglais (fourni par Jolpica).
pub fn wiki_bio(url: &str, title: &str) -> Node {
    let page = url.rsplit('/').next().unwrap_or_default().to_string();
    let lang = if crate::i18n::is_fr() { "fr" } else { "en" };
    let data = crate::api::use_json::<serde_json::Value>(
        (!page.is_empty()).then(|| format!("/api/wiki/{lang}/{page}")),
    );
    let title = title.to_string();
    dynamic(move || {
        let Some(Ok(v)) = data.get() else {
            return Node::Empty;
        };
        let text = v
            .get("extract")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();
        let link = v
            .get("url")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();
        if text.is_empty() {
            return Node::Empty;
        }
        details()
            .class("card wiki-bio")
            .attr("open", "")
            .child(summary().child(h2().text(title.clone())))
            .child(p().text(text))
            .child((!link.is_empty()).then(|| {
                a().class("link")
                    .href(link)
                    .attr("target", "_blank")
                    .attr("rel", "noopener")
                    .text(t("Lire sur Wikipédia ↗", "Read on Wikipedia ↗"))
            }))
            .into()
    })
}
