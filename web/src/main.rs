mod api;
mod views;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use chrono::Utc;
use tower_http::compression::CompressionLayer;
use tower_http::set_header::SetResponseHeaderLayer;

use api::{ApiError, F1Api};

#[derive(Clone)]
struct AppState {
    api: F1Api,
}

type PageResult = Result<Html<String>, AppError>;

struct AppError(ApiError);

impl From<ApiError> for AppError {
    fn from(e: ApiError) -> Self {
        Self(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let page = views::error(&self.0.to_string()).into_string();
        (StatusCode::BAD_GATEWAY, Html(page)).into_response()
    }
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
    Router::new()
        .route("/", get(home))
        .route("/calendrier", get(calendar))
        .route("/course/{round}", get(race))
        .route("/pilotes", get(drivers))
        .route("/pilote/{id}", get(driver))
        .route("/ecuries", get(teams))
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/static/app.css",
            get(|| static_file("text/css; charset=utf-8", include_str!("../static/app.css"))),
        )
        .route(
            "/static/app.js",
            get(|| {
                static_file(
                    "text/javascript; charset=utf-8",
                    include_str!("../static/app.js"),
                )
            }),
        )
        .route(
            "/static/icon.svg",
            get(|| static_file("image/svg+xml", include_str!("../static/icon.svg"))),
        )
        .route(
            "/manifest.webmanifest",
            get(|| {
                static_file(
                    "application/manifest+json",
                    include_str!("../static/manifest.webmanifest"),
                )
            }),
        )
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                Html(views::not_found().into_string()),
            )
        })
        .layer(CompressionLayer::new())
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=60"),
        ))
        .with_state(state)
}

async fn static_file(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=604800"),
        ],
        body,
    )
        .into_response()
}

async fn home(State(s): State<AppState>) -> PageResult {
    let (schedule, last, drivers, teams) = tokio::join!(
        s.api.schedule(),
        s.api.last_results(),
        s.api.driver_standings(),
        s.api.constructor_standings()
    );
    let schedule = schedule?;
    let data = views::HomeData {
        next: views::next_race(&schedule, Utc::now()).cloned(),
        season: schedule
            .first()
            .map(|r| r.season.clone())
            .unwrap_or_default(),
        last: last.unwrap_or_default(),
        drivers: drivers.unwrap_or_default(),
        teams: teams.unwrap_or_default(),
    };
    Ok(Html(views::home(&data).into_string()))
}

async fn calendar(State(s): State<AppState>) -> PageResult {
    let races = s.api.schedule().await?;
    Ok(Html(views::calendar(&races, Utc::now()).into_string()))
}

async fn race(State(s): State<AppState>, Path(round): Path<u32>) -> Result<Response, AppError> {
    let schedule = s.api.schedule().await?;
    let Some(base) = schedule.iter().find(|r| r.round_num() == round).cloned() else {
        return Ok((
            StatusCode::NOT_FOUND,
            Html(views::not_found().into_string()),
        )
            .into_response());
    };

    let now = Utc::now();
    let started =
        views::race_start(&base).is_some_and(|start| now > start - chrono::Duration::days(3));
    let (results, sprint, qualifying) = if started {
        let (r, sp, q) = tokio::join!(
            s.api.race_results(round),
            s.api.sprint(round),
            s.api.qualifying(round)
        );
        (
            r.ok().flatten().and_then(|r| r.results).unwrap_or_default(),
            sp.unwrap_or_default(),
            q.unwrap_or_default(),
        )
    } else {
        Default::default()
    };

    let data = views::RaceData {
        race: base,
        results,
        sprint,
        qualifying,
        total_rounds: schedule.len(),
    };
    Ok(Html(views::race(&data, now).into_string()).into_response())
}

async fn drivers(State(s): State<AppState>) -> PageResult {
    let standings = s.api.driver_standings().await?;
    Ok(Html(views::drivers(&standings).into_string()))
}

async fn driver(State(s): State<AppState>, Path(id): Path<String>) -> Result<Response, AppError> {
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Ok((
            StatusCode::NOT_FOUND,
            Html(views::not_found().into_string()),
        )
            .into_response());
    }
    let (standings, races) = tokio::join!(s.api.driver_standings(), s.api.driver_results(&id));
    let standings = standings.unwrap_or_default();
    let standing = standings.iter().find(|st| st.driver.driver_id == id);
    let races = races?;
    if standing.is_none() && races.is_empty() {
        return Ok((
            StatusCode::NOT_FOUND,
            Html(views::not_found().into_string()),
        )
            .into_response());
    }
    Ok(Html(views::driver(standing, &races).into_string()).into_response())
}

async fn teams(State(s): State<AppState>) -> PageResult {
    let standings = s.api.constructor_standings().await?;
    Ok(Html(views::teams(&standings).into_string()))
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
