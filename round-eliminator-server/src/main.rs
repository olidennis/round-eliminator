#[cfg(not(target_os = "linux"))]
use mimalloc::MiMalloc;
#[cfg(not(target_os = "linux"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(target_os = "linux")]
use tikv_jemallocator::Jemalloc;
#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;



use futures_util::StreamExt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use warp::{
    ws::{Message, WebSocket, Ws},
    Filter,
};


#[test]
fn pgo_quick_test() {               
    assert!(std::hint::black_box(round_eliminator_lib::test_all_short()) > 0);
}

#[tokio::main]
async fn main() {
    #[cfg(not(target_os = "linux"))]
    unsafe{ libmimalloc_sys::mi_option_set(26, 0) }
    server("127.0.0.1:8080").await; 
}

async fn server(addr: &str) {
    let dir_server = warp::path("server").and(warp::fs::dir("../www"));
    let index_server = warp::path("server")
        .and(warp::path::end())
        .and(warp::fs::file("www/index.htm"));

    let dir_wasm = warp::fs::dir("../www/");
    let index_wasm = warp::path::end().and(warp::fs::file("www/index.htm"));

    let ws = warp::path("api")
        .and(warp::ws())
        .map(|ws: Ws| ws.on_upgrade(serve_client));

    let serve = dir_server
        .or(index_server)
        .or(dir_wasm)
        .or(ws)
        .or(index_wasm);

    let addr = addr.parse::<std::net::SocketAddr>().unwrap();
    warp::serve(serve).run(addr).await;
}

async fn serve_client(ws: WebSocket) {
    let (ws_tx, mut ws_rx) = ws.split();

    let (tx, rx) = futures::channel::mpsc::unbounded();

    tokio::spawn(rx.map(Ok).forward(ws_tx));

    let stop = Arc::new(AtomicBool::new(false));

    while let Some(m) = ws_rx.next().await {
        match m {
            Ok(m) => {
                if m.is_text() {
                    let request = m.to_str().expect("error parsing json!").to_owned();
                    if request == "\"STOP\"" {
                        println!("stop asked");
                        stop.store(true, Ordering::Release);
                    } else {
                        let tx = tx.clone();
                        let stop = stop.clone();
                        let fun = move || {
                            round_eliminator_lib::serial::request_json(&request, |s, send_to_client| {
                                if stop.load(Ordering::Acquire) {
                                    panic!("stopping thread");
                                }
                                if send_to_client {
                                    tx.unbounded_send(Message::text(s))
                                        .expect("unbounded_send failed!");
                                }
                            });
                        };
                        tokio::task::spawn_blocking(fun);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error while receiving message from websocket: {:?}", e);
                return;
            }
        }
    }
}
