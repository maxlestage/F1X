//! Composants partagés.
//!
//! Règle de mise en page : tout s'empile verticalement. Pas de tableau, pas de carrousel,
//! rien de plus large que l'écran — le texte long passe à la ligne.

use gloo_timers::callback::Interval;
use web_sys::HtmlSelectElement;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::api::{Fetch, f1, use_f1};
use crate::i18n::t;
use crate::models::{ConstructorStanding, DriverStanding, QualifyingResult, Race, RaceResult};
use crate::tr;
use crate::util::{
    flag_nationality, local_date, now_ms, session_label, set_title, team_style, wins_label,
};
use crate::{CURRENT, Route};

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Home,
    Live,
    Calendar,
    Standings,
    Archives,
}

#[derive(Properties, PartialEq)]
pub struct LayoutProps {
    #[prop_or_default]
    pub title: AttrValue,
    #[prop_or_default]
    pub tab: Option<Tab>,
    #[prop_or_default]
    pub children: Html,
}

#[function_component]
pub fn Layout(props: &LayoutProps) -> Html {
    let toggle = use_context::<crate::LangToggle>();
    let other = crate::i18n::lang().other();
    let switch_lang = Callback::from(move |_| {
        if let Some(t) = &toggle {
            t.0.emit(());
        }
    });
    use_effect_with(props.title.clone(), |title| {
        set_title(title);
        if let Some(w) = web_sys::window() {
            w.scroll_to_with_x_and_y(0.0, 0.0);
        }
    });
    html! {
        <>
            <header class="topbar">
                <Link<Route> to={Route::Home} classes="brand">
                    <span class="brand-mark">{ "F1" }</span><span class="brand-x">{ "X" }</span>
                </Link<Route>>
                if !props.title.is_empty() {
                    <h1 class="topbar-title">{ props.title.clone() }</h1>
                }
                <button class="lang-switch" onclick={switch_lang}
                        aria-label={t("Switch to English", "Passer en français")}>
                    { other.code().to_uppercase() }
                </button>
            </header>
            <main class="page">{ props.children.clone() }</main>
            <TabBar active={props.tab} />
        </>
    }
}

#[derive(Properties, PartialEq)]
struct TabBarProps {
    active: Option<Tab>,
}

#[function_component]
fn TabBar(props: &TabBarProps) -> Html {
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
            t("Archives", "Archive"),
            ICON_ARCHIVE,
        ),
    ];
    html! {
        <nav class="tabbar" aria-label={t("Navigation principale", "Main navigation")}>
            { for items.into_iter().map(|(tab, route, label, icon)| {
                let current = if props.active == Some(tab) { "tab tab-active" } else { "tab" };
                html! {
                    <Link<Route> to={route} classes={current}>
                        { Html::from_html_unchecked(AttrValue::from(icon)) }
                        <span>{ label }</span>
                    </Link<Route>>
                }
            }) }
        </nav>
    }
}

pub fn loading() -> Html {
    html! { <div class="loading" aria-busy="true">{ t("Chargement…", "Loading…") }</div> }
}

/// Affiche le chargement / l'erreur, ou délègue le rendu une fois les données là.
pub fn fetch_view(fetch: &Fetch, render: impl FnOnce(&crate::models::MrData) -> Html) -> Html {
    match fetch {
        Fetch::Idle => html! {},
        Fetch::Loading => loading(),
        Fetch::Failed(message) => html! { <ErrorCard message={message.clone()} /> },
        Fetch::Done(value) => render(value),
    }
}

#[derive(Properties, PartialEq)]
pub struct ErrorProps {
    pub message: AttrValue,
}

#[function_component]
pub fn ErrorCard(props: &ErrorProps) -> Html {
    let reload = Callback::from(|_| {
        if let Some(w) = web_sys::window() {
            let _ = w.location().reload();
        }
    });
    html! {
        <section class="card">
            <h2>{ t("Drapeau rouge 🚩", "Red flag 🚩") }</h2>
            <p class="muted">{ props.message.clone() }</p>
            <button class="btn" onclick={reload}>{ t("Réessayer", "Try again") }</button>
        </section>
    }
}

pub fn empty_card(text: &str) -> Html {
    html! { <section class="card"><p class="muted">{ text.to_string() }</p></section> }
}

/// Grille de statistiques (3 colonnes égales, jamais plus large que l'écran).
pub fn stat_grid(items: Vec<(&'static str, String)>) -> Html {
    html! {
        <dl class="stats">
            { for items.into_iter().map(|(label, value)| html! {
                // Valeurs longues (temps au tour, unités) : police adaptée à la largeur de la case.
                <div><dt>{ label }</dt><dd class={classes!((value.chars().count() > 5).then_some("dd-long"))}>{ value }</dd></div>
            }) }
        </dl>
    }
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

#[derive(Properties, PartialEq)]
pub struct SeasonSelectProps {
    /// Saison affichée (`"current"` ou une année).
    pub season: AttrValue,
    /// Page vers laquelle naviguer quand la saison change.
    pub target: SeasonTarget,
    /// Liste restreinte (ex. saisons d'un pilote) ; sinon toutes les saisons depuis 1950.
    #[prop_or_default]
    pub only: Option<Vec<String>>,
}

/// Sélecteur de saison natif (liste déroulante verticale).
#[function_component]
pub fn SeasonSelect(props: &SeasonSelectProps) -> Html {
    let navigator = use_navigator();
    let seasons_fetch = use_f1(if props.only.is_none() {
        f1("seasons.json", 100)
    } else {
        None
    });
    let mut seasons: Vec<String> = match &props.only {
        Some(list) => list.clone(),
        None => seasons_fetch
            .done()
            .map(|d| d.seasons().iter().map(|s| s.season.clone()).collect())
            .unwrap_or_default(),
    };
    seasons.reverse();
    let target = props.target;
    let onchange = Callback::from(move |e: Event| {
        let season = e.target_unchecked_into::<HtmlSelectElement>().value();
        if let Some(nav) = &navigator {
            nav.push(&target.route(season));
        }
    });
    html! {
        <label class="select">
            <span class="select-label">{ t("Saison", "Season") }</span>
            <select {onchange} aria-label={t("Choisir une saison", "Choose a season")}>
                if props.only.is_none() {
                    <option value={CURRENT} selected={props.season == CURRENT}>{ t("Saison en cours", "Current season") }</option>
                }
                { for seasons.iter().map(|s| html! {
                    <option value={s.clone()} selected={props.season == s.as_str()}>{ s }</option>
                }) }
            </select>
        </label>
    }
}

#[derive(Properties, PartialEq)]
pub struct CountdownProps {
    pub target_ms: f64,
}

/// Compte à rebours : grille fixe de 4 colonnes.
#[function_component]
pub fn Countdown(props: &CountdownProps) -> Html {
    let now = use_state(now_ms);
    {
        let now = now.clone();
        use_effect_with((), move |_| {
            let interval = Interval::new(1000, move || now.set(now_ms()));
            move || drop(interval)
        });
    }
    let secs = ((props.target_ms - *now) / 1000.0).max(0.0) as u64;
    let cell = |value: String, label: &str| {
        html! {
            <div class="cd-cell">
                <span class="cd-num">{ value }</span>
                <span class="cd-label">{ label.to_string() }</span>
            </div>
        }
    };
    html! {
        <div class="countdown">
            { cell((secs / 86400).to_string(), t("jours", "days")) }
            { cell(format!("{:02}", secs % 86400 / 3600), t("heures", "hours")) }
            { cell(format!("{:02}", secs % 3600 / 60), "min") }
            { cell(format!("{:02}", secs % 60), "sec") }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct RaceProps {
    pub race: Race,
}

#[function_component]
pub fn SessionsList(props: &RaceProps) -> Html {
    let has_time = props.race.has_time();
    html! {
        <ul class="sessions">
            { for props.race.sessions().into_iter().map(|(name, iso)| html! {
                <li class={classes!("session", (name == "Course").then_some("session-race"))}>
                    <span class="session-name">{ session_label(name) }</span>
                    <time class="session-time" datetime={iso.clone()}>{ local_date(&iso, has_time) }</time>
                </li>
            }) }
        </ul>
    }
}

pub fn driver_standing_row(s: &DriverStanding) -> Html {
    let team = s.constructors.last();
    let fav = crate::util::fav_driver().as_deref() == Some(s.driver.driver_id.as_str());
    html! {
        <li class={classes!("row", fav.then_some("row-fav"))} style={team.map(|t| team_style(&t.constructor_id))}>
            <span class="pos">{ s.rank() }</span>
            <Link<Route> to={Route::driver(&s.driver.driver_id)} classes="row-main">
                <span class="row-title">
                    { flag_nationality(s.driver.nationality.as_deref()) }{ " " }
                    { &s.driver.given_name }{ " " }<strong>{ &s.driver.family_name }</strong>
                </span>
                <span class="row-sub">
                    { team.map(|t| t.name.clone()).unwrap_or_default() }
                    if s.wins != "0" { { tr!(" · {} V", " · {} W", s.wins) } }
                </span>
            </Link<Route>>
            <span class="pts">{ &s.points }<small>{ " pts" }</small></span>
        </li>
    }
}

pub fn team_standing_row(s: &ConstructorStanding, leader_points: f64) -> Html {
    let wins = wins_label(&s.wins);
    let pts: f64 = s.points.parse().unwrap_or(0.0);
    let fav = crate::util::fav_team().as_deref() == Some(s.constructor.constructor_id.as_str());
    let pct = if leader_points > 0.0 {
        (pts / leader_points * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    html! {
        <li class={classes!("row", fav.then_some("row-fav"))} style={team_style(&s.constructor.constructor_id)}>
            <span class="pos">{ s.rank() }</span>
            <Link<Route> to={Route::team(&s.constructor.constructor_id)} classes="row-main">
                <span class="row-title">{ flag_nationality(s.constructor.nationality.as_deref()) }{ " " }{ &s.constructor.name }</span>
                <span class="bar" aria-hidden="true"><span class="bar-fill" style={format!("width:{pct:.1}%")}></span></span>
                if !wins.is_empty() { <span class="row-sub">{ wins }</span> }
            </Link<Route>>
            <span class="pts">{ &s.points }<small>{ " pts" }</small></span>
        </li>
    }
}

pub fn result_row(r: &RaceResult) -> Html {
    let scored = r.points.parse::<f64>().unwrap_or(0.0) > 0.0;
    let mut sub = vec![r.constructor.name.clone()];
    let outcome = r.outcome();
    if !outcome.is_empty() {
        sub.push(outcome);
    }
    let gained = r.places_gained();
    html! {
        <li class="row" style={team_style(&r.constructor.constructor_id)}>
            <span class="pos">{ &r.position_text }</span>
            <Link<Route> to={Route::driver(&r.driver.driver_id)} classes="row-main">
                <span class="row-title">
                    { &r.driver.given_name }{ " " }<strong>{ &r.driver.family_name }</strong>
                    if r.has_fastest_lap() { { " " }<span class="tag tag-purple" title={t("Meilleur tour", "Fastest lap")}>{ "⏱" }</span> }
                </span>
                <span class="row-sub">
                    { sub.join(" · ") }
                    if let (Some(g), Some(grid)) = (gained, r.grid.as_deref()) {
                        { " · " }
                        <span class={classes!("delta", (g > 0).then_some("up"), (g < 0).then_some("down"))}
                              title={tr!("Parti P{grid}", "Started P{grid}")}>
                            { match g { g if g > 0 => format!("▲{g}"), g if g < 0 => format!("▼{}", -g), _ => "=".into() } }
                        </span>
                    }
                </span>
            </Link<Route>>
            if scored { <span class="pts">{ format!("+{}", r.points) }</span> }
        </li>
    }
}

pub fn qualifying_row(q: &QualifyingResult) -> Html {
    html! {
        <li class="row" style={team_style(&q.constructor.constructor_id)}>
            <span class="pos">{ &q.position }</span>
            <Link<Route> to={Route::driver(&q.driver.driver_id)} classes="row-main">
                <span class="row-title">{ &q.driver.given_name }{ " " }<strong>{ &q.driver.family_name }</strong></span>
                <span class="row-sub">{ &q.constructor.name }</span>
            </Link<Route>>
            if let Some((seg, time)) = q.best() {
                <span class="pts pts-time">{ time }<small>{ format!(" {seg}") }</small></span>
            }
        </li>
    }
}

/// Lien de navigation en carte (flèche à droite), pour les listes d'archives.
pub fn nav_row(route: Route, title: Html, sub: String, trailing: Option<String>) -> Html {
    html! {
        <li class="row row-plain">
            <Link<Route> to={route} classes="row-main">
                <span class="row-title">{ title }</span>
                if !sub.is_empty() { <span class="row-sub">{ sub }</span> }
            </Link<Route>>
            if let Some(t) = trailing { <span class="pts">{ t }</span> }
        </li>
    }
}

const ICON_HOME: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z"/></svg>"#;
const ICON_LIVE: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 12h.01M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7M5.6 5.6a9 9 0 0 0 0 12.8M18.4 5.6a9 9 0 0 1 0 12.8"/></svg>"#;
const ICON_CAL: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 3v3M17 3v3M4 9h16M5 5h14a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z"/></svg>"#;
const ICON_TROPHY: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 4h8v5a4 4 0 0 1-8 0zM8 6H5a3 3 0 0 0 3 4M16 6h3a3 3 0 0 1-3 4M12 13v4M8 21h8M9 17h6v4H9z"/></svg>"#;
const ICON_ARCHIVE: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h16v4H4zM5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8M10 12h4"/></svg>"#;

#[derive(Properties, PartialEq)]
pub struct ExportProps {
    pub filename: AttrValue,
    pub rows: Vec<Vec<String>>,
}

/// Bouton « Exporter en CSV ».
#[function_component]
pub fn ExportCsv(props: &ExportProps) -> Html {
    let rows = props.rows.clone();
    let name = props.filename.clone();
    let onclick = Callback::from(move |_| crate::util::download_csv(&name, &rows));
    html! {
        <button class="btn btn-ghost btn-small" {onclick} disabled={props.rows.len() < 2}>
            { t("⬇ Exporter (CSV)", "⬇ Export (CSV)") }
        </button>
    }
}

#[derive(Properties, PartialEq)]
pub struct FavProps {
    /// « driver » ou « team ».
    pub kind: AttrValue,
    pub id: AttrValue,
}

/// Bouton étoile : pilote / écurie favori (un de chaque, enregistré sur l'appareil).
#[function_component]
pub fn FavButton(props: &FavProps) -> Html {
    let key = format!("f1x-fav-{}", props.kind);
    let current = use_state(|| crate::util::load::<String>(&key));
    let on = current.as_deref() == Some(props.id.as_str());
    let onclick = {
        let current = current.clone();
        let id = props.id.to_string();
        Callback::from(move |_| {
            let next = if on { None } else { Some(id.clone()) };
            match &next {
                Some(v) => crate::util::store(&key, v),
                None => crate::util::store(&key, &Option::<String>::None),
            }
            current.set(next);
        })
    };
    html! {
        <button class={classes!("fav", on.then_some("fav-on"))} {onclick} aria-pressed={on.to_string()}>
            { if on { t("★ Mon favori", "★ My favourite") } else { t("☆ Ajouter aux favoris", "☆ Add to favourites") } }
        </button>
    }
}
