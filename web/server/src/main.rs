mod api;
mod landing;
mod live;
mod news;
mod openf1;
mod race;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use tower_http::compression::CompressionLayer;

use api::F1Api;

/// Frontend Yew compilé en WebAssembly par `build.rs`.
const FRONTEND_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/pkg/f1x_frontend.js"));
const FRONTEND_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pkg/f1x_frontend_bg.wasm"));
const INDEX_HTML: &str = include_str!("../static/index.html");

#[derive(Clone)]
struct AppState {
    api: F1Api,
    hub: std::sync::Arc<live::Hub>,
    news: news::News,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,f1x_web=info".into()),
        )
        .init();

    let ttl = std::env::var("CACHE_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    // Accès « direct » OpenF1 (abonnement) : optionnel, le replay fonctionne sans.
    let credentials = match (
        std::env::var("OPENF1_USERNAME"),
        std::env::var("OPENF1_PASSWORD"),
    ) {
        (Ok(user), Ok(pass)) if !user.is_empty() && !pass.is_empty() => Some((user, pass)),
        _ => None,
    };
    let openf1 = std::sync::Arc::new(openf1::OpenF1::new(credentials));
    let state = AppState {
        api: F1Api::new(Duration::from_secs(ttl)),
        hub: live::Hub::new(openf1),
        news: news::News::new(),
    };
    // Cache persistant optionnel (utile en local pour ne pas épuiser le quota Jolpica).
    let cache_file = std::env::var("CACHE_FILE").ok();
    if let Some(file) = &cache_file {
        state.api.load(file).await;
    }
    let api = state.api.clone();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind port");
    tracing::info!("F1X en écoute sur http://{addr}");

    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
    if let Some(file) = &cache_file {
        api.save(file).await;
    }
}

fn app(state: AppState) -> Router {
    let app_hash = env!("F1X_APP_HASH");
    let api = Router::new()
        .route("/f1/{*path}", get(f1_page))
        .route("/all/{*path}", get(f1_all))
        .route("/live/sessions/{year}", get(live_sessions))
        .route("/track/{circuit_id}", get(track))
        .route(
            "/news/{lang}",
            get(
                |State(s): State<AppState>, Path(lang): Path<String>| async move {
                    let list = s
                        .news
                        .articles(if lang == "fr" { "fr" } else { "en" })
                        .await;
                    (
                        [(header::CACHE_CONTROL, "public, max-age=300")],
                        axum::Json(list.as_ref().clone()),
                    )
                },
            ),
        )
        .route(
            "/champions",
            get(|State(s): State<AppState>| async move {
                (
                    [(header::CACHE_CONTROL, "public, max-age=3600")],
                    axum::Json(s.api.champions().await),
                )
            }),
        )
        .fallback(|| async { StatusCode::NOT_FOUND });

    Router::new()
        .nest("/api", api)
        .route("/healthz", get(|| async { "ok" }))
        .route("/ws", get(ws))
        .route(
            &format!("/pkg/{app_hash}/f1x_frontend.js"),
            get(|| asset("text/javascript; charset=utf-8", FRONTEND_JS.as_bytes())),
        )
        .route(
            &format!("/pkg/{app_hash}/f1x_frontend_bg.wasm"),
            get(|| asset("application/wasm", FRONTEND_WASM)),
        )
        .route(
            "/static/app.css",
            get(|| {
                asset(
                    "text/css; charset=utf-8",
                    include_bytes!("../static/app.css"),
                )
            }),
        )
        .route(
            "/static/icon.svg",
            get(|| plain_asset("image/svg+xml", include_bytes!("../static/icon.svg"))),
        )
        .route(
            "/manifest.webmanifest",
            get(|| {
                plain_asset(
                    "application/manifest+json",
                    include_bytes!("../static/manifest.webmanifest"),
                )
            }),
        )
        // Site de présentation (HTML/CSS rendus par le serveur, sans JavaScript).
        .route("/presentation", get(landing::page))
        .route("/presentation/shots/{name}", get(landing::shot))
        // Fichiers inconnus (ex. ancienne empreinte) : vrai 404, jamais la page HTML à la place
        // d'un script (sinon le navigateur resterait bloqué sur l'écran de démarrage).
        .route("/pkg/{*rest}", get(|| async { StatusCode::NOT_FOUND }))
        .route("/static/{*rest}", get(|| async { StatusCode::NOT_FOUND }))
        // Application monopage : toutes les autres URL servent le shell, le routeur Yew prend le relais.
        .fallback(get(index))
        .layer(CompressionLayer::new())
        .with_state(state)
}

async fn index() -> impl IntoResponse {
    // La page elle-même n'est jamais mise en cache : elle pointe toujours vers les bons fichiers.
    (
        [(header::CACHE_CONTROL, "no-cache")],
        Html(
            INDEX_HTML
                .replace("{{APP}}", env!("F1X_APP_HASH"))
                .replace("{{CSS}}", env!("F1X_CSS_HASH")),
        ),
    )
}

/// Fichier à adresse « empreinte » (`/pkg/{hash}/…`, `app.css?v={hash}`) : son contenu ne change jamais.
async fn asset(content_type: &'static str, body: &'static [u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        body,
    )
        .into_response()
}

/// Fichier sans empreinte (icône, manifeste) : cache court.
async fn plain_asset(content_type: &'static str, body: &'static [u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        body,
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct PageQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

/// `/api/f1/{chemin}.json?limit=&offset=` : une page de n'importe quel endpoint Jolpica.
async fn f1_page(
    State(s): State<AppState>,
    Path(path): Path<String>,
    Query(q): Query<PageQuery>,
) -> Response {
    if !api::valid_path(&path) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let limit = q.limit.unwrap_or(30).clamp(1, api::PAGE);
    let value = s.api.page(&path, limit, q.offset.unwrap_or(0)).await;
    json_response(&path, value)
}

/// `/api/all/{chemin}.json` : toutes les pages d'un endpoint, fusionnées.
async fn f1_all(State(s): State<AppState>, Path(path): Path<String>) -> Response {
    if !api::valid_path(&path) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let value = s.api.all(&path).await;
    json_response(&path, value)
}

/// WebSocket du direct / replay.
async fn ws(State(s): State<AppState>, upgrade: axum::extract::ws::WebSocketUpgrade) -> Response {
    let hub = s.hub.clone();
    upgrade.on_upgrade(move |socket| live::client(socket, hub))
}

/// Sessions rejouables d'une année (OpenF1, depuis 2023).
async fn live_sessions(State(s): State<AppState>, Path(year): Path<u32>) -> Response {
    if !(2023..=2100).contains(&year) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match s.hub.openf1.sessions(year).await {
        Ok(list) => (
            [(header::CACHE_CONTROL, "public, max-age=300")],
            axum::Json(list),
        )
            .into_response(),
        Err(err) => (StatusCode::BAD_GATEWAY, err).into_response(),
    }
}

/// Tracé d'un circuit (dernière course disputée depuis 2023, données OpenF1).
async fn track(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let ok = |t: std::sync::Arc<f1x_protocol::TrackMap>| {
        (
            [(header::CACHE_CONTROL, "public, max-age=86400")],
            axum::Json(t.as_ref()),
        )
            .into_response()
    };
    if let Some(t) = s.hub.openf1.cached_track(&id).await {
        return ok(t);
    }
    let Some(races) = s.api.all(&format!("circuits/{id}/races.json")).await else {
        return (StatusCode::BAD_GATEWAY, "races unavailable").into_response();
    };
    // Courses disputées depuis 2023 (données OpenF1), de la plus récente à la plus ancienne.
    let candidates: Vec<(u32, String, String)> = races
        .pointer("/MRData/RaceTable/Races")
        .and_then(|r| r.as_array())
        .map(|list| {
            list.iter()
                .rev()
                .filter_map(|r| {
                    let field =
                        |k: &str| r.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let season = field("season").parse::<u32>().ok()?;
                    let date = field("date");
                    // Depuis 2023 (données OpenF1), y compris le GP à venir (essais déjà courus).
                    (season >= 2023).then(|| (season, date, field("raceName")))
                })
                .collect()
        })
        .unwrap_or_default();
    if candidates.is_empty() {
        return (StatusCode::NOT_FOUND, "no race since 2023").into_response();
    }
    match s.hub.openf1.track(&id, &candidates).await {
        Ok(t) => ok(t),
        // Aucune session exploitable (ex. GP pas encore couru) : 404, rien à réessayer.
        Err(err) if err.starts_with("Pas de données") => {
            (StatusCode::NOT_FOUND, err).into_response()
        }
        Err(err) => (StatusCode::BAD_GATEWAY, err).into_response(),
    }
}

fn json_response(path: &str, value: Option<serde_json::Value>) -> Response {
    let Some(value) = value else {
        return (
            StatusCode::BAD_GATEWAY,
            "Les données F1 sont momentanément indisponibles.",
        )
            .into_response();
    };
    let cache = if F1Api::is_historical(path) {
        "public, max-age=86400"
    } else {
        "public, max-age=60"
    };
    ([(header::CACHE_CONTROL, cache)], axum::Json(value)).into_response()
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}
