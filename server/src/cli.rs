extern crate futures01;
use simulation::AutomaticSimplifications;
use simulation::AutoLb;
use simulation::AutoUb;
use simulation::Problem;
use simulation::DiagramType;
use warp::Filter;
use warp::ws::{Message, WebSocket};
use futures::future::{FutureExt, TryFutureExt};
use futures::compat::Stream01CompatExt;
use futures::stream::StreamExt;
use futures01::stream::Stream;
use futures01::sync::mpsc;
use futures01::Future;
use futures_cpupool::CpuPool;

pub fn file(name: &str, iter: usize) {
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    let mut p = Problem::from_line_separated_text(&data).unwrap();
    p.compute_independent_lines();
    println!("{}", p.as_result());

    for _ in 0..iter {
        println!("-------------------------");
        p = p.speedup(DiagramType::Accurate).unwrap();
        p.compute_independent_lines();
        println!("{}", p.as_result());
    }
}

pub fn autolb(name: &str, labels: usize, iter: usize, colors:usize) {
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    let p = Problem::from_line_separated_text(&data).unwrap();
    let auto = AutomaticSimplifications::<AutoLb>::new(p, iter, labels,1000,colors,&["diag"]);
    //auto.run(|x|println!("{}",x));
    for x in auto {
        println!("{}", x.unwrap());
    }
}

pub fn autoub(name: &str, labels: usize, iter: usize, colors:usize, pred : bool) {
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    let p = Problem::from_line_separated_text(&data).unwrap();
    let mut features = vec![];
    if pred {
        features.push("pred");
    }
    let auto = AutomaticSimplifications::<AutoUb>::new(p, iter, labels,1000,colors,&features);
    //auto.run(|x|println!("{}",x));
    for x in auto {
        println!("{}", x.unwrap());
    }
}

pub fn server(addr : &str) {

    let dir_server = warp::path("server").and(warp::fs::dir("static/"));
    let index_server = warp::path("server").and(warp::path::end()).and(warp::fs::file("static/index.htm"));

    let dir_wasm = warp::fs::dir("static/");
    let index_wasm = warp::path::end().and(warp::fs::file("static/index.htm"));

    let ws = warp::path("api")
        .and(warp::ws2())
        .map(|ws: warp::ws::Ws2,| {
            ws.on_upgrade(move |socket| {
                serve_client(socket).boxed().compat().map_err(|e| {
                    eprintln!("serve_client error: {:?}", e);
                })
            })
        });

    let serve = dir_server.or(index_server).or(dir_wasm).or(ws).or(index_wasm);

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
        match m {
            Ok(m) =>  
                if m.is_text() {
                    let request = m.to_str().expect("error parsing json!").to_owned();
                    let pool = CpuPool::new(1);
                    let tx = tx.clone();
                    let fun = move || -> Result<(), ()> {
                        simulation::request_json(&request, |s|{  tx.unbounded_send(Message::text(s)).expect("unbounded_send failed!"); });
                        Ok(()) 
                    };
                    let fut = pool.spawn_fn(fun);
                    warp::spawn(fut);
                }
            Err(e) => { eprintln!("Error while receiving message from websocket: {:?}",e); return Err(()); }
        }
    }
    Ok(())
}