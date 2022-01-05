pub struct EventHandler<'a> {
    tx: Option<EventFunc<'a>>,
}

type EventFunc<'a> = Box<dyn FnMut((String, usize, usize)) + 'a>;

impl<'a> EventHandler<'a> {
    pub fn null() -> Self {
        Self { tx: None }
    }

    pub fn with<T>(f: T) -> Self
    where
        T: FnMut((String, usize, usize)) + 'a,
    {
        Self {
            tx: Some(Box::new(f)),
        }
    }

    pub fn notify<S: AsRef<str>>(&mut self, s: S, x: usize, t: usize) {
        let s = s.as_ref();
        if let Some(tx) = self.tx.as_mut() {
            tx((s.to_string(), x, t));
        }
    }
}
