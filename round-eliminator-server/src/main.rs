use futures::channel::mpsc::UnboundedReceiver;
use warp::{Filter, ws::{WebSocket, Ws, Message}};
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    server("127.0.0.1:8080").await;
}



async fn server(addr : &str) {

    let dir_server = warp::path("server").and(warp::fs::dir("www"));
    let index_server = warp::path("server").and(warp::path::end()).and(warp::fs::file("www/index.htm"));

    let dir_wasm = warp::fs::dir("www/");
    let index_wasm = warp::path::end().and(warp::fs::file("www/index.htm"));

    let ws = warp::path("api")
        .and(warp::ws())
        .map(|ws: Ws,| {
            ws.on_upgrade(move |socket| {
                serve_client(socket)
            })
        });

    let serve = dir_server.or(index_server).or(dir_wasm).or(ws).or(index_wasm);

    let addr = addr.parse::<std::net::SocketAddr>().unwrap();
    warp::serve(serve).run(addr).await;
}

async fn serve_client(ws: WebSocket) {
    
    let (ws_tx,mut ws_rx) = ws.split();


    let (tx, rx) : (futures::channel::mpsc::UnboundedSender<Message>, UnboundedReceiver<Message>)= futures::channel::mpsc::unbounded();

    tokio::spawn(
        rx.map(|x|Ok(x))
            .forward(ws_tx)
    );


    while let Some(m) = ws_rx.next().await {
        match m {
            Ok(m) =>  
                if m.is_text() {
                    let request = m.to_str().expect("error parsing json!").to_owned();
                    let tx = tx.clone();
                    let fun = move || {
                        round_eliminator_lib::serial::request_json(&request, |s|{  tx.unbounded_send(Message::text(s)).expect("unbounded_send failed!"); });
                    };
                    tokio::task::spawn_blocking(fun);
                }
            Err(e) => { eprintln!("Error while receiving message from websocket: {:?}",e); return; }
        }
    }
    
}
