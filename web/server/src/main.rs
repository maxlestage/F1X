mod api;
mod assets;
mod landing;
mod live;
mod livetiming;
mod mapkit;
mod news;
mod openf1;
mod photos;
mod race;
mod track3d;

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
    photos: photos::Photos,
    /// Décors 3D déjà calculés, par circuit.
    scenery: std::sync::Arc<
        tokio::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Vec<u8>>>>,
    >,
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
        photos: photos::Photos::new(),
        scenery: Default::default(),
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
        .route("/track3d/{circuit_id}", get(track3d))
        .route("/of1/telemetry", get(of1_telemetry))
        .route("/of1/pitdetail", get(of1_pit_detail))
        .route("/of1/driverrace", get(of1_driver_race))
        .route("/mapkit-token", get(mapkit_token))
        .route("/of1/{endpoint}", get(of1_relay))
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
            "/photo/{title}",
            get(
                |State(s): State<AppState>, Path(title): Path<String>| async move {
                    match s.photos.get(&title).await {
                        Ok(Some(photo)) => (
                            [(header::CACHE_CONTROL, "public, max-age=604800")],
                            axum::Json(photo),
                        )
                            .into_response(),
                        Ok(None) => (
                            StatusCode::NOT_FOUND,
                            [(header::CACHE_CONTROL, "public, max-age=86400")],
                        )
                            .into_response(),
                        Err(()) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
                    }
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
        .route("/static/img/{name}", get(assets::img))
        .route("/static/car.bin", get(assets::car))
        .route("/favicon.ico", get(assets::favicon))
        .route("/apple-touch-icon.png", get(assets::apple_touch_icon))
        .route(
            "/apple-touch-icon-precomposed.png",
            get(assets::apple_touch_icon),
        )
        .route("/sw.js", get(assets::service_worker))
        .route("/robots.txt", get(assets::robots))
        .route("/sitemap.xml", get(assets::sitemap))
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
        // Pages légales (rendues par le serveur).
        .route("/mentions-legales", get(landing::legal))
        .route("/confidentialite", get(landing::privacy))
        .route("/credits", get(landing::credits))
        // Fichiers inconnus (ex. ancienne empreinte) : vrai 404, jamais la page HTML à la place
        // d'un script (sinon le navigateur resterait bloqué sur l'écran de démarrage).
        .route("/pkg/{*rest}", get(|| async { StatusCode::NOT_FOUND }))
        .route("/static/{*rest}", get(|| async { StatusCode::NOT_FOUND }))
        // Application monopage : toutes les autres URL servent le shell, le routeur Yew prend le relais.
        .fallback(get(index))
        .layer(CompressionLayer::new())
        .with_state(state)
}

async fn index(headers: axum::http::HeaderMap) -> impl IntoResponse {
    // La page elle-même n'est jamais mise en cache : elle pointe toujours vers les bons fichiers.
    (
        [(header::CACHE_CONTROL, "no-cache")],
        Html(
            INDEX_HTML
                .replace("{{APP}}", env!("F1X_APP_HASH"))
                .replace("{{CSS}}", env!("F1X_CSS_HASH"))
                .replace("{{ORIGIN}}", &assets::origin(&headers))
                .replace("{{CAR}}", assets::car_hash()),
        ),
    )
}

/// Identité des requêtes sortantes (API de données, Wikipédia, flux RSS).
pub fn user_agent() -> String {
    match std::env::var("PUBLIC_URL") {
        Ok(url) if url.starts_with("http") => {
            format!("F1X/{} (+{url})", env!("CARGO_PKG_VERSION"))
        }
        _ => format!(
            "F1X/{} (application web Formule 1)",
            env!("CARGO_PKG_VERSION")
        ),
    }
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
    match load_track(&s, &id).await {
        Ok(t) => (
            [(header::CACHE_CONTROL, "public, max-age=86400")],
            axum::Json(t.as_ref()),
        )
            .into_response(),
        Err(resp) => *resp,
    }
}

/// Décor 3D du circuit (relief, piste, vibreurs, tribunes…), au format binaire partagé.
async fn track3d(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    let ok = |bytes: std::sync::Arc<Vec<u8>>| {
        (
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::CACHE_CONTROL, "public, max-age=3600"),
            ],
            bytes.as_ref().clone(),
        )
            .into_response()
    };
    if let Some(hit) = s.scenery.lock().await.get(&id).cloned() {
        return ok(hit);
    }
    let map = match load_track(&s, &id).await {
        Ok(t) => t,
        Err(resp) => return *resp,
    };
    let bytes = tokio::task::spawn_blocking(move || track3d::build(&map))
        .await
        .unwrap_or_default();
    let bytes = std::sync::Arc::new(bytes);
    s.scenery.lock().await.insert(id, bytes.clone());
    ok(bytes)
}

/// Relais des 18 points d'accès OpenF1 (`/api/of1/laps?session_key=…`), mis en cache.
/// Les flux très volumineux (télémétrie, positions GPS) exigent un pilote et une fenêtre.
async fn of1_relay(
    State(s): State<AppState>,
    Path(endpoint): Path<String>,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Response {
    let query = query.unwrap_or_default();
    if !openf1::ENDPOINTS.contains(&endpoint.as_str())
        || query.len() > 300
        || !query
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_=&<>:.+-%".contains(c))
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let heavy = matches!(endpoint.as_str(), "car_data" | "location");
    if heavy && !(query.contains("driver_number=") && query.contains("date")) {
        return (
            StatusCode::BAD_REQUEST,
            "driver_number et une fenêtre date sont requis",
        )
            .into_response();
    }
    if !query.contains("session_key=")
        && !query.contains("meeting_key=")
        && endpoint != "meetings"
        && endpoint != "sessions"
    {
        return (StatusCode::BAD_REQUEST, "session_key ou meeting_key requis").into_response();
    }
    match s.hub.openf1.relay(&endpoint, &query).await {
        Ok(v) => (
            [(header::CACHE_CONTROL, "public, max-age=300")],
            axum::Json(v.as_ref().clone()),
        )
            .into_response(),
        Err(err) => (StatusCode::BAD_GATEWAY, err).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct TelemetryQuery {
    session_key: u32,
    drivers: String,
    /// Tour choisi (sinon le meilleur tour de chaque pilote).
    lap: Option<u32>,
}

/// Jeton MapKit JS (Apple Maps sur le site) ; 404 si aucune clé n'est configurée.
async fn mapkit_token(headers: axum::http::HeaderMap) -> Response {
    match mapkit::token(&assets::origin(&headers)) {
        Some(tok) => ([(header::CACHE_CONTROL, "no-store")], tok).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(serde::Deserialize)]
struct PitDetailQuery {
    session_key: Option<u32>,
    year: Option<u32>,
    /// Date de la course « 2025-04-06 ».
    date: Option<String>,
}

/// Détail de chaque arrêt aux stands (séance donnée, ou course retrouvée par année + date).
async fn of1_pit_detail(State(s): State<AppState>, Query(q): Query<PitDetailQuery>) -> Response {
    let key = match (q.session_key, q.year, q.date.as_deref()) {
        (Some(k), _, _) => Some(k),
        (None, Some(y), Some(d))
            if d.len() == 10 && d.chars().all(|c| c.is_ascii_digit() || c == '-') =>
        {
            match s.hub.openf1.race_session(y, d).await {
                Ok(k) => k,
                Err(err) => return (StatusCode::BAD_GATEWAY, err).into_response(),
            }
        }
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    let Some(key) = key else {
        return (
            StatusCode::NOT_FOUND,
            "Pas de séance OpenF1 pour ce Grand Prix",
        )
            .into_response();
    };
    match s.hub.openf1.pit_detail(key).await {
        Ok(v) => (
            [(header::CACHE_CONTROL, "public, max-age=600")],
            axum::Json(v),
        )
            .into_response(),
        Err(err) => (StatusCode::BAD_GATEWAY, err).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct DriverRaceQuery {
    session_key: u32,
    driver: u32,
}

/// Fiche course d'un pilote (tour par tour, résumé, messages, radios).
async fn of1_driver_race(State(s): State<AppState>, Query(q): Query<DriverRaceQuery>) -> Response {
    match s.hub.openf1.driver_race(q.session_key, q.driver).await {
        Ok(v) => (
            [(header::CACHE_CONTROL, "public, max-age=600")],
            axum::Json(v),
        )
            .into_response(),
        Err(err) => (StatusCode::BAD_GATEWAY, err).into_response(),
    }
}

/// Télémétrie comparée (meilleur tour ou tour choisi de 1 à 3 pilotes, alignée sur la distance).
async fn of1_telemetry(State(s): State<AppState>, Query(q): Query<TelemetryQuery>) -> Response {
    let drivers: Vec<u32> = q
        .drivers
        .split(',')
        .filter_map(|d| d.parse().ok())
        .collect();
    if drivers.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match s.hub.openf1.telemetry(q.session_key, &drivers, q.lap).await {
        Ok(v) => (
            [(header::CACHE_CONTROL, "public, max-age=3600")],
            axum::Json(v),
        )
            .into_response(),
        Err(err) => (StatusCode::BAD_GATEWAY, err).into_response(),
    }
}

include!(concat!(env!("OUT_DIR"), "/bundled_tracks.rs"));

/// Tracé d'un circuit : dernière course disputée depuis 2023 (données OpenF1).
async fn load_track(
    s: &AppState,
    id: &str,
) -> Result<std::sync::Arc<f1x_protocol::TrackMap>, Box<Response>> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(Box::new(StatusCode::NOT_FOUND.into_response()));
    }
    if let Some(t) = s.hub.openf1.cached_track(id).await {
        return Ok(t);
    }
    // Tracé embarqué dans le binaire : aucune dépendance à OpenF1.
    if let Some(t) = bundled_track(id).and_then(|json| serde_json::from_str(json).ok()) {
        return Ok(s.hub.openf1.remember_track(id, t).await);
    }
    let Some(races) = s.api.all(&format!("circuits/{id}/races.json")).await else {
        return Err(Box::new(
            (StatusCode::BAD_GATEWAY, "races unavailable").into_response(),
        ));
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
        return Err(Box::new(
            (StatusCode::NOT_FOUND, "no race since 2023").into_response(),
        ));
    }
    match s.hub.openf1.track(id, &candidates).await {
        Ok(t) => Ok(t),
        // Aucune session exploitable (ex. GP pas encore couru) : 404, rien à réessayer.
        Err(err) if err.starts_with("Pas de données") => {
            Err(Box::new((StatusCode::NOT_FOUND, err).into_response()))
        }
        Err(err) => Err(Box::new((StatusCode::BAD_GATEWAY, err).into_response())),
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
