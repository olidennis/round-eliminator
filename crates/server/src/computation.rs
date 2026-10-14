//! Latest-update streaming ties a request's lifetime to its CPU computation.

use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use axum::{
    extract::State,
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use round_eliminator_3::{
    api::{ApiError, ComputationEvent, Response, ResponsePacket, execute_text_with_events},
    computation::{Cancellation, Phase, Progress},
};
use tokio::{
    sync::{Semaphore, oneshot, watch},
    time::{Interval, MissedTickBehavior},
};
use tokio_stream::Stream;

const UPDATE_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) async fn api(
    State(workers): State<Arc<Semaphore>>,
    body: String,
) -> impl axum::response::IntoResponse {
    Sse::new(start(workers, body)).keep_alive(KeepAlive::default())
}

fn event(event: &ComputationEvent) -> Result<Event, Infallible> {
    Ok(Event::default()
        .json_data(event)
        .expect("protocol events must serialize"))
}

fn start(workers: Arc<Semaphore>, body: String) -> ComputationStream {
    start_work(workers, move |cancellation, report| {
        execute_text_with_events(&body, cancellation, report)
    })
}

fn start_work<R: Into<ResponsePacket> + Send + 'static>(
    workers: Arc<Semaphore>,
    work: impl FnOnce(Cancellation, &mut dyn FnMut(ComputationEvent)) -> R + Send + 'static,
) -> ComputationStream {
    // Each watch channel retains one snapshot. CPU work never waits for the
    // browser, and overwritten progress never incurs JSON or network work.
    let (progress_tx, progress) = watch::channel(None);
    let (cache_tx, cache) = watch::channel(None);
    let (partial_tx, partial) = watch::channel(None);
    let (mut response_tx, response) = oneshot::channel();
    let cancellation = Cancellation::default();
    let cancel_work = cancellation.clone();
    tokio::spawn(async move {
        let permit = tokio::select! {
            permit = workers.acquire_owned() => match permit { Ok(permit) => permit, Err(_) => return },
            _ = response_tx.closed() => return,
        };
        let task = tokio::task::spawn_blocking(move || {
            // Retain the CPU permit until work actually exits after cancellation.
            let _permit = permit;
            let mut report = |update| match update {
                ComputationEvent::Progress(progress) => {
                    progress_tx.send_replace(Some(progress));
                }
                update @ ComputationEvent::CacheUpdate(_) => {
                    cache_tx.send_replace(Some(Arc::new(update)));
                }
                // Partial results are cumulative proof snapshots, not deltas.
                // A later snapshot supersedes the earlier one, retaining its proof.
                other => {
                    partial_tx.send_replace(Some(Arc::new(other)));
                }
            };
            work(cancel_work, &mut report).into()
        });
        let result = task.await.unwrap_or_else(|error| {
            Response::Error(ApiError {
                message: format!("Computation failed: {error}"),
                location: None,
            })
            .into()
        });
        // Final delivery is independent of the progress slots and never blocks.
        let _ = response_tx.send(result);
    });
    let mut tick = tokio::time::interval(UPDATE_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ComputationStream {
        progress,
        partial,
        cache,
        response,
        tick,
        cancellation,
        queued: true,
        finished: false,
        final_response: None,
        pending_progress: None,
        pending_partial: None,
        last_progress: None,
        last_partial: None,
        pending_cache: None,
        last_cache: None,
    }
}

pub(crate) struct ComputationStream {
    progress: watch::Receiver<Option<Progress>>,
    partial: watch::Receiver<Option<Arc<ComputationEvent>>>,
    cache: watch::Receiver<Option<Arc<ComputationEvent>>>,
    response: oneshot::Receiver<ResponsePacket>,
    tick: Interval,
    cancellation: Cancellation,
    queued: bool,
    finished: bool,
    final_response: Option<ResponsePacket>,
    pending_cache: Option<Arc<ComputationEvent>>,
    last_cache: Option<Arc<ComputationEvent>>,
    pending_progress: Option<Progress>,
    pending_partial: Option<Arc<ComputationEvent>>,
    last_progress: Option<Progress>,
    last_partial: Option<Arc<ComputationEvent>>,
}

impl Stream for ComputationStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }
        if self.queued {
            self.queued = false;
            return Poll::Ready(Some(event(&ComputationEvent::Progress(Progress {
                phase: Phase::Queued,
                iterations: 0,
                work: None,
            }))));
        }
        if self.final_response.is_none() {
            match Pin::new(&mut self.response).poll(context) {
                Poll::Ready(result) => {
                    self.final_response = Some(result.unwrap_or_else(|_| {
                        Response::Error(ApiError {
                            message: "Computation worker stopped without a result.".to_owned(),
                            location: None,
                        })
                        .into()
                    }));
                }
                Poll::Pending => {}
            }
        }
        // Polling a timer rather than every watch change caps both traffic and
        // async wakeups. A final response immediately flushes the latest updates.
        if self.final_response.is_some() || self.tick.poll_tick(context).is_ready() {
            let progress = *self.progress.borrow_and_update();
            if progress != self.last_progress {
                self.pending_progress = progress;
            }
            let cache = self.cache.borrow_and_update().clone();
            if cache.as_ref().is_some_and(|update| {
                self.last_cache
                    .as_ref()
                    .is_none_or(|previous| !Arc::ptr_eq(previous, update))
            }) {
                self.pending_cache = cache;
            }
            let partial = self.partial.borrow_and_update().clone();
            if partial.as_ref().is_some_and(|update| {
                self.last_partial
                    .as_ref()
                    .is_none_or(|previous| !Arc::ptr_eq(previous, update))
            }) {
                self.pending_partial = partial;
            }
        }
        if let Some(progress) = self.pending_progress.take() {
            self.last_progress = Some(progress);
            return Poll::Ready(Some(event(&ComputationEvent::Progress(progress))));
        }
        if let Some(cache) = self.pending_cache.take() {
            self.last_cache = Some(cache.clone());
            return Poll::Ready(Some(event(&cache)));
        }
        if let Some(partial) = self.pending_partial.take() {
            self.last_partial = Some(partial.clone());
            return Poll::Ready(Some(event(&partial)));
        }
        if let Some(response) = self.final_response.take() {
            self.finished = true;
            return Poll::Ready(Some(event(&ComputationEvent::Response(Box::new(response)))));
        }
        // A ready tick resets the interval's sleep. Poll again to register the
        // next wakeup when no update was available on this tick.
        if self.tick.poll_tick(context).is_ready() {
            context.waker().wake_by_ref();
        }
        Poll::Pending
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
    use axum::response::IntoResponse;
    use tokio_stream::StreamExt;

    async fn read_events(stream: ComputationStream) -> Vec<serde_json::Value> {
        let body = Sse::new(stream).into_response().into_body();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|json| serde_json::from_str(json).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn unread_stream_does_not_block_work_or_lose_latest_proof_and_result() {
        use round_eliminator_3::algorithms::solvability::coloring::ColoringBounds;
        let workers = Arc::new(Semaphore::new(1));
        let (done_tx, done_rx) = oneshot::channel();
        let stream = start_work(workers.clone(), move |_, report| {
            for iterations in 0..100_000 {
                report(ComputationEvent::Progress(Progress {
                    phase: Phase::Solving,
                    iterations,
                    work: None,
                }));
            }
            for lower in [3, 4] {
                report(ComputationEvent::PartialResult(Box::new(
                    Response::ColoringBounds(ColoringBounds {
                        minimum: 1,
                        testing: None,
                        lower: Some(lower),
                        upper: None,
                        witness: None,
                    }),
                )));
            }
            let mut cache = round_eliminator_3::api::AnalysisCache::default();
            cache.zero_round = Some(round_eliminator_3::algorithms::solvability::zero_round::ZeroRoundResult::Unsolvable);
            report(ComputationEvent::CacheUpdate(Box::new(cache.clone())));
            cache.maximization = Some(
                round_eliminator_3::algorithms::maximize::MaximizationResult::OutputOnly(
                    round_eliminator_3::model::Constraint::new(Vec::new()),
                ),
            );
            report(ComputationEvent::CacheUpdate(Box::new(cache)));
            let _ = done_tx.send(());
            Response::Cancelled
        });
        // Leave the stream entirely unread until the burst and work complete.
        tokio::time::timeout(Duration::from_secs(2), done_rx)
            .await
            .unwrap()
            .unwrap();
        let permit = tokio::time::timeout(Duration::from_secs(2), workers.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        let updates = read_events(stream).await;
        assert_eq!(updates.len(), 5); // Queued, progress, cache, proof, final result.
        assert_eq!(updates[1]["data"]["iterations"], 99_999);
        assert_eq!(updates[2]["type"], "cache_update");
        assert_eq!(updates[2]["data"]["zero_round"]["status"], "unsolvable");
        assert_eq!(updates[2]["data"]["maximization"]["kind"], "output_only");
        assert_eq!(updates[3]["data"]["data"]["lower"], 4);
        assert_eq!(updates[4]["type"], "response");
        assert_eq!(updates[4]["data"]["response"]["type"], "cancelled");
    }

    #[tokio::test(start_paused = true)]
    async fn frequent_updates_wait_for_the_next_tick_but_the_final_flush_does_not() {
        let (ready_tx, ready_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let (updated_tx, updated_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let mut stream = start_work(Arc::new(Semaphore::new(1)), move |_, report| {
            report(ComputationEvent::Progress(Progress {
                phase: Phase::Solving,
                iterations: 0,
                work: None,
            }));
            let _ = ready_tx.send(());
            let _ = release_rx.blocking_recv();
            for iterations in 1..100_000 {
                report(ComputationEvent::Progress(Progress {
                    phase: Phase::Verifying,
                    iterations,
                    work: None,
                }));
            }
            let _ = updated_tx.send(());
            let _ = finish_rx.blocking_recv();
            report(ComputationEvent::Progress(Progress {
                phase: Phase::Reducing,
                iterations: 100_000,
                work: None,
            }));
            Response::Cancelled
        });
        ready_rx.await.unwrap();
        assert!(stream.next().await.is_some()); // Queued.
        assert!(stream.next().await.is_some());
        assert_eq!(stream.last_progress.unwrap().phase, Phase::Solving);
        release_tx.send(()).unwrap();
        updated_rx.await.unwrap(); // The burst is already available to the reader.
        // Poll with a real waker without allowing paused-time auto-advance.
        std::future::poll_fn(|context| {
            assert!(Pin::new(&mut stream).poll_next(context).is_pending());
            Poll::Ready(())
        })
        .await;
        tokio::time::advance(Duration::from_millis(99)).await;
        std::future::poll_fn(|context| {
            assert!(Pin::new(&mut stream).poll_next(context).is_pending());
            Poll::Ready(())
        })
        .await;
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(stream.next().await.is_some());
        assert_eq!(stream.last_progress.unwrap().iterations, 99_999);
        finish_tx.send(()).unwrap();
        let updates = read_events(stream).await;
        assert_eq!(updates.len(), 2); // Immediate last progress and final response.
        assert_eq!(updates[0]["data"]["iterations"], 100_000);
        assert_eq!(updates[1]["data"]["response"]["type"], "cancelled");
    }

    fn analysis_request() -> String {
        let text = serde_json::from_value(serde_json::json!({
            "kind":"independent", "input":{"active":"abcdefghij^20", "passive":"a^2"},
            "output":{"active":"A^20", "passive":"A^2"}
        }))
        .unwrap();
        let problem = round_eliminator_3::parser::parse_problem(text).unwrap();
        serde_json::json!({"type":"compute", "data":{"state":{"problem":problem}, "operation":{"type":"check_zero_round", "data":{}}}}).to_string()
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
