// crates/server/src/routes/console.rs
use std::convert::Infallible;

use axum::Json;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{self, Stream, StreamExt};
use tokio::sync::broadcast::error::RecvError;

use crate::console_store::{build_console_payload, ConsolePayload};
use crate::state::AppState;

/// SSE endpoint — streams ConsolePayload events to all connected browsers.
pub async fn console_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let started_at = state.started_at;
    let mut rx = state.console.broadcast.subscribe();

    // Send initial snapshot immediately on connect (no waiting for next 5s tick)
    let initial = build_console_payload(&state, started_at).await;
    let initial_json = serde_json::to_string(&initial).unwrap_or_default();
    let initial_stream = stream::once(async move {
        Ok::<Event, Infallible>(Event::default().data(initial_json))
    });

    let broadcast_stream = stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(payload) => return Some((Ok(Event::default().data(payload)), rx)),
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(initial_stream.chain(broadcast_stream))
        .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15)).text("ping"))
}

/// One-shot JSON snapshot — same payload as SSE events, useful for curl/debug.
pub async fn console_metrics_json(
    State(state): State<AppState>,
) -> Json<ConsolePayload> {
    let started_at = state.started_at;
    Json(build_console_payload(&state, started_at).await)
}

use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../ui/build/"]
struct ConsoleAssets;

/// Serves the embedded Svelte SPA.
pub async fn console_asset(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> axum::response::Response {
    serve_asset(&path)
}

pub async fn console_asset_root() -> axum::response::Response {
    serve_asset("index.html")
}

fn serve_asset(path: &str) -> axum::response::Response {
    let path = path.trim_start_matches('/');
    let asset = ConsoleAssets::get(path)
        .or_else(|| ConsoleAssets::get("index.html"));

    match asset {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let body = Body::from(content.data.into_owned());
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())],
                body,
            ).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
