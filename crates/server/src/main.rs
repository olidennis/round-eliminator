use axum::{Router, http::header, routing::post};
use round_eliminator_3::execute_json;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/api", post(api))
        .fallback_service(ServeDir::new("web/dist"));
    let address = std::env::var("RE3_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Round Eliminator 3 listening on http://{address}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn api(body: String) -> ([(header::HeaderName, &'static str); 1], String) {
    (
        [(header::CONTENT_TYPE, "application/json")],
        execute_json(&body),
    )
}
