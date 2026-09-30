mod api;

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
    let state = AppState {
        api: F1Api::new(Duration::from_secs(ttl)),
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
    let version = env!("CARGO_PKG_VERSION");
    let api = Router::new()
        .route("/f1/{*path}", get(f1_page))
        .route("/all/{*path}", get(f1_all))
        .fallback(|| async { StatusCode::NOT_FOUND });

    Router::new()
        .nest("/api", api)
        .route("/healthz", get(|| async { "ok" }))
        .route(
            &format!("/pkg/{version}/f1x_frontend.js"),
            get(|| asset("text/javascript; charset=utf-8", FRONTEND_JS.as_bytes())),
        )
        .route(
            &format!("/pkg/{version}/f1x_frontend_bg.wasm"),
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
            get(|| asset("image/svg+xml", include_bytes!("../static/icon.svg"))),
        )
        .route(
            "/manifest.webmanifest",
            get(|| {
                asset(
                    "application/manifest+json",
                    include_bytes!("../static/manifest.webmanifest"),
                )
            }),
        )
        // Application monopage : toutes les autres URL servent le shell, le routeur Yew prend le relais.
        .fallback(get(index))
        .layer(CompressionLayer::new())
        .with_state(state)
}

async fn index() -> Html<String> {
    Html(INDEX_HTML.replace("{{VERSION}}", env!("CARGO_PKG_VERSION")))
}

async fn asset(content_type: &'static str, body: &'static [u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=604800"),
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
