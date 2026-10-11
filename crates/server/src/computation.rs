//! Streaming progress ties a request's lifetime to its CPU computation.

use std::{
    convert::Infallible,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    extract::State,
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use round_eliminator_3::{
    api::{ApiError, ComputationEvent, Response, execute_text},
    computation::{Cancellation, Computation, Phase, Progress},
};
use tokio::sync::{Semaphore, mpsc};
use tokio_stream::{Stream, wrappers::ReceiverStream};

pub(crate) async fn api(
    State(workers): State<Arc<Semaphore>>,
    body: String,
) -> impl axum::response::IntoResponse {
    Sse::new(start(workers, body)).keep_alive(KeepAlive::default())
}

fn event(event: ComputationEvent) -> Result<Event, Infallible> {
    Ok(Event::default()
        .json_data(event)
        .expect("protocol events must serialize"))
}

fn start(workers: Arc<Semaphore>, body: String) -> ComputationStream {
    // Bound buffered updates. A slow client cannot accumulate unbounded memory.
    let (sender, receiver) = mpsc::channel(16);
    let cancellation = Cancellation::default();
    let cancel_work = cancellation.clone();
    tokio::spawn(async move {
        if sender
            .send(event(ComputationEvent::Progress(Progress {
                phase: Phase::Queued,
                iterations: 0,
            })))
            .await
            .is_err()
        {
            return;
        }
        let permit = tokio::select! {
            permit = workers.acquire_owned() => match permit { Ok(permit) => permit, Err(_) => return },
            _ = sender.closed() => return,
        };
        let errors = sender.clone();
        let task = tokio::task::spawn_blocking(move || {
            // Retain the CPU permit until work actually exits after cancellation.
            let _permit = permit;
            let mut report = |progress| {
                if sender
                    .blocking_send(event(ComputationEvent::Progress(progress)))
                    .is_err()
                {
                    cancel_work.cancel();
                }
            };
            let response = execute_text(
                &body,
                &mut Computation::new(cancel_work.clone(), &mut report),
            );
            let _ = sender.blocking_send(event(ComputationEvent::Response(Box::new(response))));
        });
        if let Err(error) = task.await {
            let _ = errors
                .send(event(ComputationEvent::Response(Box::new(
                    Response::Error(ApiError {
                        message: format!("Computation failed: {error}"),
                        location: None,
                    }),
                ))))
                .await;
        }
    });
    ComputationStream {
        receiver: ReceiverStream::new(receiver),
        cancellation,
    }
}

pub(crate) struct ComputationStream {
    receiver: ReceiverStream<Result<Event, Infallible>>,
    cancellation: Cancellation,
}

impl Stream for ComputationStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(context)
    }
}

impl Drop for ComputationStream {
    fn drop(&mut self) {
        // Axum drops the response stream on disconnect. Aborting fetch therefore
        // stops server work too, without a separate cancellation request or ID.
        self.cancellation.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::StreamExt;

    fn analysis_request() -> String {
        let text = serde_json::from_value(serde_json::json!({
            "kind":"independent", "input":{"active":"abcdefghij^20", "passive":"a^2"},
            "output":{"active":"A^20", "passive":"A^2"}
        }))
        .unwrap();
        let problem = round_eliminator_3::parser::parse_problem(text).unwrap();
        serde_json::json!({"type":"check_zero_round", "data":{"problem":problem}}).to_string()
    }

    #[tokio::test]
    async fn http_disconnect_cancels_cpu_work() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let workers = Arc::new(Semaphore::new(1));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .route("/api/stream", axum::routing::post(api))
            .with_state(workers.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let body = analysis_request();
        let mut connection = tokio::net::TcpStream::connect(address).await.unwrap();
        connection.write_all(format!("POST /api/stream HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        let mut lines = BufReader::new(connection).lines();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if lines
                    .next_line()
                    .await
                    .unwrap()
                    .unwrap()
                    .contains("enumerating")
                {
                    break;
                }
            }
        })
        .await
        .unwrap();
        drop(lines); // Close a real HTTP client, not just the Rust stream wrapper.
        let _permit = tokio::time::timeout(std::time::Duration::from_secs(2), workers.acquire())
            .await
            .unwrap()
            .unwrap();
        server.abort();
    }

    #[tokio::test]
    async fn dropping_a_running_stream_releases_its_cpu_permit() {
        let workers = Arc::new(Semaphore::new(1));
        let body = analysis_request();
        let mut stream = start(workers.clone(), body);
        // Queued and enumeration: the CPU worker now holds its permit.
        for _ in 0..2 {
            assert!(stream.next().await.is_some());
        }
        let cancellation = stream.cancellation.clone();
        drop(stream);
        assert!(cancellation.is_cancelled());
        let _permit = tokio::time::timeout(std::time::Duration::from_secs(2), workers.acquire())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn dropping_a_queued_stream_does_not_start_cpu_work() {
        let workers = Arc::new(Semaphore::new(0));
        let mut stream = start(workers, "invalid json".to_owned());
        assert!(stream.next().await.is_some());
        let cancellation = stream.cancellation.clone();
        drop(stream);
        assert!(cancellation.is_cancelled());
    }
}
