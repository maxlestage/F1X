//! Composants partagés : mise en page, barre d'onglets, compte à rebours, lignes de classement.
//!
//! Règle de mise en page : tout s'empile verticalement. Pas de tableau, pas de carrousel,
//! rien de plus large que l'écran — le texte long passe à la ligne.

use gloo_timers::callback::Interval;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;
use crate::api::Fetch;
use crate::models::{ConstructorStanding, DriverStanding, QualifyingResult, Race, RaceResult};
use crate::util::{flag_nationality, local_date, now_ms, set_title, team_style, wins_label};

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Home,
    Calendar,
    Drivers,
    Teams,
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
        (Tab::Home, Route::Home, "Accueil", ICON_HOME),
        (Tab::Calendar, Route::Calendar, "Calendrier", ICON_CAL),
        (Tab::Drivers, Route::Drivers, "Pilotes", ICON_HELMET),
        (Tab::Teams, Route::Teams, "Écuries", ICON_TEAM),
    ];
    html! {
        <nav class="tabbar" aria-label="Navigation principale">
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

/// Affiche le chargement / l'erreur, ou délègue le rendu une fois les données là.
pub fn fetch_view<T>(fetch: &Fetch<T>, render: impl FnOnce(&T) -> Html) -> Html {
    match fetch {
        Fetch::Loading => html! { <div class="loading" aria-busy="true">{ "Chargement…" }</div> },
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
            <h2>{ "Drapeau rouge 🚩" }</h2>
            <p class="muted">{ props.message.clone() }</p>
            <button class="btn" onclick={reload}>{ "Réessayer" }</button>
        </section>
    }
}

#[derive(Properties, PartialEq)]
pub struct CountdownProps {
    pub target_ms: f64,
}

/// Compte à rebours : grille fixe de 4 colonnes, jamais plus large que l'écran.
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
            { cell((secs / 86400).to_string(), "jours") }
            { cell(format!("{:02}", secs % 86400 / 3600), "heures") }
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
    html! {
        <ul class="sessions">
            { for props.race.sessions().into_iter().map(|(name, iso)| html! {
                <li class={classes!("session", (name == "Course").then_some("session-race"))}>
                    <span class="session-name">{ name }</span>
                    <time class="session-time" datetime={iso.clone()}>{ local_date(&iso, true) }</time>
                </li>
            }) }
        </ul>
    }
}

pub fn driver_standing_row(s: &DriverStanding) -> Html {
    let team = s.constructors.last();
    html! {
        <li class="row" style={team.map(|t| team_style(&t.constructor_id))}>
            <span class="pos">{ s.position.clone().unwrap_or_else(|| s.position_text.clone()) }</span>
            <Link<Route> to={Route::Driver { id: s.driver.driver_id.clone() }} classes="row-main">
                <span class="row-title">
                    { flag_nationality(s.driver.nationality.as_deref()) }{ " " }
                    { &s.driver.given_name }{ " " }<strong>{ &s.driver.family_name }</strong>
                </span>
                <span class="row-sub">
                    { team.map(|t| t.name.clone()).unwrap_or_default() }
                    if s.wins != "0" { { format!(" · {} V", s.wins) } }
                </span>
            </Link<Route>>
            <span class="pts">{ &s.points }<small>{ " pts" }</small></span>
        </li>
    }
}

pub fn team_standing_row(s: &ConstructorStanding) -> Html {
    let wins = wins_label(&s.wins);
    html! {
        <li class="row" style={team_style(&s.constructor.constructor_id)}>
            <span class="pos">{ s.position.clone().unwrap_or_else(|| s.position_text.clone()) }</span>
            <span class="row-main">
                <span class="row-title">{ &s.constructor.name }</span>
                if !wins.is_empty() { <span class="row-sub">{ wins }</span> }
            </span>
            <span class="pts">{ &s.points }<small>{ " pts" }</small></span>
        </li>
    }
}

pub fn result_row(r: &RaceResult) -> Html {
    let scored = r.points.parse::<f64>().unwrap_or(0.0) > 0.0;
    let outcome = r.outcome();
    html! {
        <li class="row" style={team_style(&r.constructor.constructor_id)}>
            <span class="pos">{ &r.position_text }</span>
            <Link<Route> to={Route::Driver { id: r.driver.driver_id.clone() }} classes="row-main">
                <span class="row-title">
                    { &r.driver.given_name }{ " " }<strong>{ &r.driver.family_name }</strong>
                    if r.has_fastest_lap() { { " " }<span class="tag tag-purple" title="Meilleur tour">{ "⏱" }</span> }
                </span>
                <span class="row-sub">
                    { &r.constructor.name }
                    if !outcome.is_empty() { { format!(" · {outcome}") } }
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
            <Link<Route> to={Route::Driver { id: q.driver.driver_id.clone() }} classes="row-main">
                <span class="row-title">{ &q.driver.given_name }{ " " }<strong>{ &q.driver.family_name }</strong></span>
                <span class="row-sub">{ &q.constructor.name }</span>
            </Link<Route>>
            if let Some((seg, time)) = q.best() {
                <span class="pts pts-time">{ time }<small>{ format!(" {seg}") }</small></span>
            }
        </li>
    }
}

const ICON_HOME: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z"/></svg>"#;
const ICON_CAL: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 3v3M17 3v3M4 9h16M5 5h14a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z"/></svg>"#;
const ICON_HELMET: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 15a9 9 0 0 1 18-2v4a1 1 0 0 1-1 1H9l-3 2H4a1 1 0 0 1-1-1zM12 11h8"/></svg>"#;
const ICON_TEAM: &str =
    r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 21V4M5 4h11l-2 4 2 4H5"/></svg>"#;
