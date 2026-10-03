//! Thine Metrics server — OTLP-ready metrics platform with a modern explorer UI.

mod api;
mod demo;
mod platform_api;
mod state;

use anyhow::Result;
use axum::Router;
use state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let host = std::env::var("THINE_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("THINE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4318);
    let seed_demo = std::env::var("THINE_SEED_DEMO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(true);

    let state = AppState::new();
    if seed_demo {
        demo::seed(&state.store);
        state.platform.seed_demo();
        info!("seeded demo metrics + platform modules");
    }

    // Background demo generator keeps the UI alive for local demos.
    if seed_demo {
        let store = state.store.clone();
        tokio::spawn(async move {
            demo::run_live_generator(store).await;
        });
    }

    let ui_dir = resolve_ui_dir();
    let index = ui_dir.join("index.html");

    let app = Router::new()
        .merge(api::routes())
        .merge(platform_api::routes())
        .fallback_service(
            ServeDir::new(&ui_dir).not_found_service(ServeFile::new(index)),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    info!(%addr, ui = %ui_dir.display(), "thine-metrics listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn resolve_ui_dir() -> PathBuf {
    if let Ok(p) = std::env::var("THINE_UI_DIR") {
        return PathBuf::from(p);
    }
    let candidates = [
        PathBuf::from("ui/dist"),
        PathBuf::from("../../ui/dist"),
        PathBuf::from("ui"),
    ];
    for c in candidates {
        if c.join("index.html").exists() {
            return c;
        }
    }
    PathBuf::from("ui/dist")
}
