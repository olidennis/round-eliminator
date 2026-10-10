use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    routing::post,
};
use round_eliminator_3::execute_json;
use tokio::sync::Semaphore;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    let app = Router::new()
        .route("/api", post(api))
        .route("/api/stream", post(computation::api))
        .fallback_service(ServeDir::new("web/dist"))
        .with_state(Arc::new(Semaphore::new(workers)));
    let address = std::env::var("RE3_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Round Eliminator 3 listening on http://{address}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn api(
    State(workers): State<Arc<Semaphore>>,
    body: String,
) -> Result<([(header::HeaderName, &'static str); 1], String), (StatusCode, String)> {
    // CPU searches run outside the async executor. Bound concurrent searches
    // by available CPUs; retain the permit until the computation actually ends.
    let permit = workers
        .acquire_owned()
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let response = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        execute_json(&body)
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(([(header::CONTENT_TYPE, "application/json")], response))
}
mod computation;
