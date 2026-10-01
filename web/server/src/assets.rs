//! Icônes, image de partage et service worker (application installable).

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// Images embarquées dans le binaire, servies sous `/static/img/{nom}`.
const IMAGES: &[(&str, &str, &[u8])] = &[
    (
        "favicon.ico",
        "image/x-icon",
        include_bytes!("../static/img/favicon.ico"),
    ),
    (
        "favicon-32.png",
        "image/png",
        include_bytes!("../static/img/favicon-32.png"),
    ),
    (
        "apple-touch-icon.png",
        "image/png",
        include_bytes!("../static/img/apple-touch-icon.png"),
    ),
    (
        "icon-192.png",
        "image/png",
        include_bytes!("../static/img/icon-192.png"),
    ),
    (
        "icon-512.png",
        "image/png",
        include_bytes!("../static/img/icon-512.png"),
    ),
    (
        "maskable-512.png",
        "image/png",
        include_bytes!("../static/img/maskable-512.png"),
    ),
    (
        "og-fr.png",
        "image/png",
        include_bytes!("../static/img/og-fr.png"),
    ),
    (
        "og-en.png",
        "image/png",
        include_bytes!("../static/img/og-en.png"),
    ),
];

fn image(name: &str) -> Response {
    match IMAGES.iter().find(|(n, _, _)| *n == name) {
        Some((_, content_type, body)) => (
            [
                (header::CONTENT_TYPE, *content_type),
                (header::CACHE_CONTROL, "public, max-age=604800"),
            ],
            *body,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Modèle 3D de la monoplace (généré par tools/carmodel/build.py), partagé avec l'app iOS.
const CAR: &[u8] = include_bytes!("../static/car.bin");

/// Empreinte du modèle, pour une mise en cache permanente côté navigateur.
pub fn car_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in CAR {
            h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
        }
        format!("{h:016x}")
    })
}

pub async fn car() -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        CAR,
    )
        .into_response()
}

pub async fn img(Path(name): Path<String>) -> Response {
    image(&name)
}

pub async fn favicon() -> Response {
    image("favicon.ico")
}

pub async fn apple_touch_icon() -> Response {
    image("apple-touch-icon.png")
}

/// Service worker : précharge la version courante (empreintes du build) de l'app.
pub async fn service_worker() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        include_str!("../static/sw.js")
            .replace("{{APP}}", env!("F1X_APP_HASH"))
            .replace("{{CSS}}", env!("F1X_CSS_HASH")),
    )
        .into_response()
}

pub async fn robots(headers: HeaderMap) -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!(
            "User-agent: *\nAllow: /\nDisallow: /api/\n\nSitemap: {}/sitemap.xml\n",
            origin(&headers)
        ),
    )
        .into_response()
}

pub async fn sitemap(headers: HeaderMap) -> Response {
    let o = origin(&headers);
    let urls: String = [
        "/",
        "/presentation?lang=fr",
        "/presentation?lang=en",
        "/direct",
        "/saison/current",
        "/saison/current/pilotes",
        "/archives",
        "/mentions-legales",
        "/confidentialite",
        "/credits",
    ]
    .iter()
    .map(|p| format!("<url><loc>{o}{}</loc></url>", p.replace('&', "&amp;")))
    .collect();
    (
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{urls}</urlset>"#
        ),
    )
        .into_response()
}

/// Adresse publique du site (`PUBLIC_URL`, sinon déduite de la requête) pour les liens absolus
/// exigés par les fiches de partage.
pub fn origin(headers: &HeaderMap) -> String {
    if let Ok(url) = std::env::var("PUBLIC_URL") {
        let url = url.trim_end_matches('/');
        if url.starts_with("https://") || url.starts_with("http://") {
            return url.to_string();
        }
    }
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .filter(|h| {
            !h.is_empty()
                && h.len() < 100
                && h.chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-:".contains(c))
        })
        .unwrap_or("localhost");
    let local = host.starts_with("localhost") || host.starts_with("127.");
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .filter(|p| *p == "https" || *p == "http")
        .unwrap_or(if local { "http" } else { "https" });
    format!("{proto}://{host}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_from_headers() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "f1x.example.com".parse().unwrap());
        h.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(origin(&h), "https://f1x.example.com");
        h.insert(header::HOST, "evil.com/<script>".parse().unwrap());
        assert_eq!(origin(&h), "https://localhost");
        assert!(IMAGES.iter().all(|(_, _, b)| !b.is_empty()));
    }
}
