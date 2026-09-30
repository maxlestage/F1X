//! Actualités F1 : agrégation de flux RSS publics (titres, extraits et liens vers les articles).

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::DateTime;
use serde::Serialize;
use tokio::sync::Mutex;

const TTL: Duration = Duration::from_secs(15 * 60);

const FR: &[(&str, &str)] = &[("Motorsport.com", "https://fr.motorsport.com/rss/f1/news/")];
const EN: &[(&str, &str)] = &[
    ("Motorsport.com", "https://www.motorsport.com/rss/f1/news/"),
    ("Autosport", "https://www.autosport.com/rss/f1/news/"),
    ("Formula1.com", "https://www.formula1.com/en/latest/all.xml"),
    ("RaceFans", "https://www.racefans.net/feed/"),
];

#[derive(Debug, Clone, Serialize)]
pub struct Article {
    pub title: String,
    pub link: String,
    pub source: String,
    pub date: String,
    pub excerpt: String,
    pub image: Option<String>,
}

/// (langue, date de récupération, articles).
type Cached = (String, Instant, Arc<Vec<Article>>);

#[derive(Clone, Default)]
pub struct News {
    http: reqwest::Client,
    cache: Arc<Mutex<Vec<Cached>>>,
}

impl News {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (compatible; F1X/0.5; +https://github.com/maxlestage/f1x)")
                .timeout(Duration::from_secs(10))
                .build()
                .expect("http client"),
            cache: Arc::default(),
        }
    }

    pub async fn articles(&self, lang: &str) -> Arc<Vec<Article>> {
        let mut cache = self.cache.lock().await;
        if let Some((_, at, list)) = cache.iter().find(|(l, _, _)| l == lang) {
            if at.elapsed() < TTL {
                return list.clone();
            }
        }
        let feeds = if lang == "fr" { FR } else { EN };
        let fetched = futures_util::future::join_all(
            feeds.iter().map(|(source, url)| self.feed(source, url)),
        )
        .await;
        let mut all: Vec<Article> = fetched.into_iter().flatten().collect();
        all.sort_by(|a, b| b.date.cmp(&a.date));
        all.dedup_by(|a, b| a.link == b.link);
        all.truncate(60);
        let list = Arc::new(all);
        if !list.is_empty() {
            cache.retain(|(l, _, _)| l != lang);
            cache.push((lang.to_string(), Instant::now(), list.clone()));
        }
        list
    }

    async fn feed(&self, source: &str, url: &str) -> Vec<Article> {
        let text = match self.http.get(url).send().await {
            Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
            Ok(r) => {
                tracing::warn!(%url, status = %r.status(), "news feed failed");
                return Vec::new();
            }
            Err(err) => {
                tracing::warn!(%url, %err, "news feed failed");
                return Vec::new();
            }
        };
        parse(&text, source)
    }
}

/// Extraction minimale des `<item>` d'un flux RSS 2.0.
pub fn parse(xml: &str, source: &str) -> Vec<Article> {
    xml.split("<item")
        .skip(1)
        .filter_map(|item| {
            let item = item.split("</item>").next()?;
            let title = clean(&tag(item, "title")?);
            let link = clean(&tag(item, "link")?);
            if title.is_empty() || !link.starts_with("http") {
                return None;
            }
            let date = tag(item, "pubDate")
                .and_then(|d| DateTime::parse_from_rfc2822(d.trim()).ok())
                .map(|d| d.to_utc().to_rfc3339())
                .unwrap_or_default();
            let mut excerpt = strip_html(&clean(&tag(item, "description").unwrap_or_default()));
            if excerpt.chars().count() > 220 {
                excerpt = excerpt
                    .chars()
                    .take(217)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
                    + "…";
            }
            let image = attr(item, "enclosure", "url")
                .or_else(|| attr(item, "media:content", "url"))
                .filter(|u| u.starts_with("https://"));
            Some(Article {
                title,
                link,
                source: source.to_string(),
                date,
                excerpt,
                image,
            })
        })
        .collect()
}

fn tag(s: &str, name: &str) -> Option<String> {
    let start = s.find(&format!("<{name}"))?;
    let open_end = start + s[start..].find('>')? + 1;
    if s[start..open_end].ends_with("/>") {
        return Some(String::new());
    }
    let end = open_end + s[open_end..].find(&format!("</{name}>"))?;
    Some(s[open_end..end].to_string())
}

fn attr(s: &str, name: &str, attr: &str) -> Option<String> {
    let start = s.find(&format!("<{name}"))?;
    let end = start + s[start..].find('>')?;
    let el = &s[start..end];
    let key = format!("{attr}=\"");
    let a = el.find(&key)? + key.len();
    let b = a + el[a..].find('"')?;
    Some(decode(&el[a..b]))
}

fn clean(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix("<![CDATA[")
        .and_then(|x| x.strip_suffix("]]>"))
        .unwrap_or(s);
    decode(s.trim())
}

fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode(&out.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Entités HTML/XML courantes.
fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rss_items() {
        let xml = r#"<rss><channel><item><title><![CDATA[Norris &amp; Piastri]]></title>
            <link>https://example.com/a</link><pubDate>Tue, 29 Sep 2026 10:00:00 +0000</pubDate>
            <description>&lt;p&gt;Hello &#8217;world&#8217;&lt;/p&gt;</description>
            <enclosure url="https://example.com/i.jpg" type="image/jpeg"/></item></channel></rss>"#;
        let items = parse(xml, "Test");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Norris & Piastri");
        assert_eq!(items[0].excerpt, "Hello \u{2019}world\u{2019}");
        assert_eq!(items[0].image.as_deref(), Some("https://example.com/i.jpg"));
        assert!(items[0].date.starts_with("2026-09-29T10:00:00"));
    }
}
