mod api;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use tower_http::compression::CompressionLayer;
use tower_http::set_header::SetResponseHeaderLayer;

use api::F1Api;

/// Frontend Yew compilé en WebAssembly par `build.rs`.
const FRONTEND_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/pkg/f1x_frontend.js"));
const FRONTEND_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pkg/f1x_frontend_bg.wasm"));
const INDEX_HTML: &str = include_str!("../static/index.html");

const RACES: &str = "/MRData/RaceTable/Races";
const STANDINGS: &str = "/MRData/StandingsTable/StandingsLists";

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
}

fn app(state: AppState) -> Router {
    let version = env!("CARGO_PKG_VERSION");
    let api = Router::new()
        .route("/schedule", get(|s| proxy(s, "current.json".into(), RACES)))
        .route(
            "/last",
            get(|s| proxy(s, "current/last/results.json".into(), RACES)),
        )
        .route("/race/{round}/{kind}", get(race))
        .route(
            "/drivers",
            get(|s| proxy(s, "current/driverStandings.json".into(), STANDINGS)),
        )
        .route(
            "/teams",
            get(|s| proxy(s, "current/constructorStandings.json".into(), STANDINGS)),
        )
        .route("/driver/{id}", get(driver))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=60"),
        ));

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

async fn proxy(State(s): State<AppState>, path: String, pointer: &'static str) -> Response {
    match s.api.get(&path, pointer).await {
        Some(value) => axum::Json(value).into_response(),
        None => (
            StatusCode::BAD_GATEWAY,
            "Les données F1 sont momentanément indisponibles.",
        )
            .into_response(),
    }
}

async fn race(s: State<AppState>, Path((round, kind)): Path<(u32, String)>) -> Response {
    let file = match kind.as_str() {
        "results" => "results",
        "sprint" => "sprint",
        "qualifying" => "qualifying",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    proxy(s, format!("current/{round}/{file}.json"), RACES).await
}

async fn driver(s: State<AppState>, Path(id): Path<String>) -> Response {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    proxy(
        s,
        format!("current/drivers/{id}/results.json?limit=100"),
        RACES,
    )
    .await
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
