extern crate futures01;

use crate::auto::AutomaticSimplifications;
use crate::autolb::AutoLb;
use crate::autoub::AutoUb;
use crate::problem::Problem;
use warp::Filter;
use warp::ws::{Message, WebSocket};
use futures::future::{FutureExt, TryFutureExt};
use futures::compat::Stream01CompatExt;
use futures::stream::StreamExt;
use futures01::stream::Stream;
use futures01::sync::mpsc;
use futures01::Future;

pub fn file(name: &str, iter: usize) {
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    let mut p = Problem::from_line_separated_text(&data).unwrap();
    println!("{}", p.as_result());

    for _ in 0..iter {
        println!("-------------------------");
        p = p.speedup().unwrap();
        println!("{}", p.as_result());
    }
}

pub fn autolb(name: &str, labels: usize, iter: usize) {
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    let p = Problem::from_line_separated_text(&data).unwrap();
    let auto = AutomaticSimplifications::<AutoLb>::new(p, iter, labels);
    //auto.run(|x|println!("{}",x));
    for x in auto {
        println!("{}", x.unwrap());
    }
}

pub fn autoub(name: &str, labels: usize, iter: usize) {
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    let p = Problem::from_line_separated_text(&data).unwrap();
    let auto = AutomaticSimplifications::<AutoUb>::new(p, iter, labels);
    //auto.run(|x|println!("{}",x));
    for x in auto {
        println!("{}", x.unwrap());
    }
}

pub fn server(addr : &str) {
    let dir = warp::fs::dir("static/");
    let index = warp::path::end().and(warp::fs::file("static/index.htm"));
    let ws = warp::path("api")
        .and(warp::ws2())
        .map(|ws: warp::ws::Ws2,| {
            ws.on_upgrade(move |socket| {
                serve_client(socket).boxed().compat()
            })
        });

    let serve = dir.or(ws).or(index);

    let addr = addr.parse::<std::net::SocketAddr>().unwrap();
    warp::serve(serve).run(addr);
}

async fn serve_client(ws: WebSocket) -> Result<(),()> {
    let (ws_tx,ws_rx) = ws.split();
    let mut ws_rx = ws_rx.compat();

    let (tx, rx) = mpsc::unbounded();
    warp::spawn(
        rx.map_err(|()| -> warp::Error { unreachable!("unbounded rx never errors") })
            .forward(ws_tx)
            .map(|_tx_rx| ())
            .map_err(|ws_err| eprintln!("websocket send error: {}", ws_err)),
    );

    while let Some(m) = ws_rx.next().await {
        let m = m.map_err(|_|())?;
        crate::simpleapi::request_json(m.to_str().unwrap(), |s|{  tx.unbounded_send(Message::text(s)).unwrap(); });
    }
    Ok(())
}