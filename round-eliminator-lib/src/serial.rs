

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
    f("{\"msg\" : \"this is still not implemented\"}".into());
    f("\"Done\"".into());
}
