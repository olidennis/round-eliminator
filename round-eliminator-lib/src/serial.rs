use serde::{Deserialize, Serialize};

use crate::{algorithms::event::EventHandler, problem::Problem};

pub fn request_json<F>(req: &str, f: F)
where
    F: Fn(String),
{
    
    let req: Request = serde_json::from_str(req).unwrap();
    let handler = |resp: Response| {
        let s = serde_json::to_string(&resp).unwrap();
        f(s);
    };

    let mut eh = EventHandler::with(|x: (String, usize, usize)| {
        let resp = Response::Event(x.0, x.1, x.2);
        handler(resp);
    });

    match req {
        Request::Ping => {
            handler(Response::Pong);
            return;
        }
        Request::NewProblem(active, passive) => match Problem::from_string_active_passive(active, passive) {
            Ok(r) => handler(Response::P(r)),
            Err(s) => handler(Response::E(s.into())),
        },
        Request::Speedup(problem) => {
            handler(Response::P(problem.speedup(&mut eh)));
        }
        _ => { unimplemented!() }
    }


    handler(Response::Done);
}


#[derive(Deserialize, Serialize)]
pub enum Request {
    NewProblem(String, String),
    Speedup(Problem),
    Ping,
}

#[derive(Deserialize, Serialize)]
pub enum Response {
    Done,
    Pong,
    Event(String, usize, usize),
    P(Problem),
    E(String)
}

