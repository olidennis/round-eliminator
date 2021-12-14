use crate::{problem::Problem, algorithms::event::EventHandler};



pub fn request_json<F>(_req: &str, mut f: F)
where
    F: FnMut(String),
{
    /*
    let req: Request = serde_json::from_str(req).unwrap();
    let handler = |resp: Response| {
        let s = serde_json::to_string(&resp).unwrap();
        f(s);
    };
    request(req, handler);*/

    let mut eh = EventHandler::with(|x : (String,usize,usize)|{
        // TEMPORARY, for testing
        f(format!("\"{} {} {}\"",x.0,x.1,x.2));
    });
    let mut p = Problem::from_string("A AB AB\n\nA B").unwrap();
    p.compute_diagram(&mut eh);
    let _ = p.speedup(&mut eh);
    drop(eh);

    f("{\"msg\" : \"this is still not implemented\"}".into());
    f("\"Done\"".into());
}
