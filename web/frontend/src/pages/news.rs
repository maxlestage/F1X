//! Actualités F1 (flux RSS publics agrégés par le serveur).

use serde::Deserialize;
use yew::prelude::*;

use crate::api::use_json;
use crate::components::*;
use crate::i18n::{is_fr, t};
use crate::tr;
use crate::util::{now_ms, parse_ms};

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Article {
    title: String,
    link: String,
    source: String,
    date: String,
    excerpt: String,
    image: Option<String>,
}

fn ago(iso: &str) -> String {
    let mins = ((now_ms() - parse_ms(iso)) / 60_000.0).max(0.0) as u64;
    match mins {
        0..=59 => tr!("il y a {mins} min", "{mins} min ago"),
        60..=1439 => tr!("il y a {} h", "{} h ago", mins / 60),
        _ => tr!("il y a {} j", "{} d ago", mins / 1440),
    }
}

#[function_component]
pub fn NewsPage() -> Html {
    let lang = if is_fr() { "fr" } else { "en" };
    let news = use_json::<Vec<Article>>(Some(format!("/api/news/{lang}")));
    let body = match &news {
        None => loading(),
        Some(Err(_)) => empty_card(t(
            "Actualités momentanément indisponibles.",
            "News temporarily unavailable.",
        )),
        Some(Ok(list)) if list.is_empty() => {
            empty_card(t("Aucune actualité pour le moment.", "No news right now."))
        }
        Some(Ok(list)) => html! {
            <ol class="news">
                { for list.iter().map(|a| html! {
                    <li>
                        <a class="news-item" href={a.link.clone()} target="_blank" rel="noopener">
                            if let Some(img) = &a.image {
                                <img class="news-img" src={img.clone()} alt="" loading="lazy" referrerpolicy="no-referrer" />
                            }
                            <span class="news-body">
                                <span class="news-meta">{ format!("{} · {}", a.source, if a.date.is_empty() { String::new() } else { ago(&a.date) }) }</span>
                                <strong class="news-title">{ &a.title }</strong>
                                if !a.excerpt.is_empty() { <span class="news-excerpt">{ &a.excerpt }</span> }
                            </span>
                        </a>
                    </li>
                }) }
            </ol>
        },
    };
    html! {
        <Layout title={t("Actualités", "News")} tab={Tab::Archives}>
            <p class="section-intro">{ t("Les derniers titres de la presse F1 — touche un article pour le lire sur le site d'origine.", "Latest F1 headlines — tap an article to read it on the original site.") }</p>
            { body }
        </Layout>
    }
}
