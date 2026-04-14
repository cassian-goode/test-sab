use std::{env, error::Error, path::PathBuf};

use axum::{
    body::Body,
    http::{HeaderValue, Request},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let spa_dist_dir =
        env::var("SPA_DIST_DIR").unwrap_or_else(|_| "../my-app/build".to_string());
    let spa_dist_dir = PathBuf::from(spa_dist_dir);

    let fallback_file = spa_dist_dir.join("200.html");

    if !fallback_file.exists() {
        eprintln!(
            "Could not find {}.\nRun `npm run build` in ../my-app first.",
            fallback_file.display()
        );
        std::process::exit(1);
    }

    let static_files =
        ServeDir::new(&spa_dist_dir).not_found_service(ServeFile::new(&fallback_file));

    let app = Router::new()
        .route("/api/health", get(health))
        .fallback_service(static_files)
        .layer(middleware::from_fn(add_cross_origin_isolation_headers));

    let addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = TcpListener::bind(&addr).await?;

    println!("Serving SPA from: {}", spa_dist_dir.display());
    println!("Open: http://{addr}");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse { ok: true })
}

async fn add_cross_origin_isolation_headers(
    request: Request<Body>,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        "Cross-Origin-Opener-Policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "Cross-Origin-Embedder-Policy",
        HeaderValue::from_static("require-corp"),
    );
    headers.insert(
        "Cross-Origin-Resource-Policy",
        HeaderValue::from_static("same-origin"),
    );

    response
}