//! Actualités F1 (flux RSS publics agrégés par le serveur).

use active::prelude::*;
use serde::Deserialize;

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

fn news_item(item: &Article) -> Element {
    li().child(
        a().class("news-item")
            .href(item.link.clone())
            .attr("target", "_blank")
            .attr("rel", "noopener")
            .child(item.image.as_ref().map(|img_src| {
                img()
                    .class("news-img")
                    .attr("src", img_src.clone())
                    .attr("alt", "")
                    .attr("loading", "lazy")
                    .attr("referrerpolicy", "no-referrer")
            }))
            .child(
                span()
                    .class("news-body")
                    .child(span().class("news-meta").text(format!(
                        "{} · {}",
                        item.source,
                        if item.date.is_empty() {
                            String::new()
                        } else {
                            ago(&item.date)
                        }
                    )))
                    .child(strong().class("news-title").text(item.title.clone()))
                    .child(
                        (!item.excerpt.is_empty())
                            .then(|| span().class("news-excerpt").text(item.excerpt.clone())),
                    ),
            ),
    )
}

pub fn news_page() -> Node {
    let lang = if is_fr() { "fr" } else { "en" };
    let news = use_json::<Vec<Article>>(Some(format!("/api/news/{lang}")));
    let body = dynamic(move || match news.get() {
        None => loading(),
        Some(Err(_)) => empty_card(t(
            "Actualités momentanément indisponibles.",
            "News temporarily unavailable.",
        )),
        Some(Ok(list)) if list.is_empty() => {
            empty_card(t("Aucune actualité pour le moment.", "No news right now."))
        }
        Some(Ok(list)) => ol()
            .class("news")
            .children(list.iter().map(news_item))
            .into(),
    });
    layout(
        t("Actualités", "News"),
        Some(Tab::Archives),
        fragment([
            Node::from(p().class("section-intro").text(t(
                "Les derniers titres de la presse F1 — touche un article pour le lire sur le site d'origine.",
                "Latest F1 headlines — tap an article to read it on the original site.",
            ))),
            body,
        ]),
    )
}
