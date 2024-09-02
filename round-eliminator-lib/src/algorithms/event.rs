use chrono::{NaiveTime, Utc,Duration};

pub struct EventHandler<'a> {
    tx: Option<EventFunc<'a>>,
    last_msg : String,
    last_time : NaiveTime
}

type EventFunc<'a> = Box<dyn FnMut((String, usize, usize)) + 'a>;

impl<'a> EventHandler<'a> {
    pub fn null() -> Self {
        Self { tx: None, last_msg : String::new(), last_time : Utc::now().time() - Duration::seconds(1) }
    }

    pub fn with<T>(f: T) -> Self
    where
        T: FnMut((String, usize, usize)) + 'a,
    {
        Self {
            tx: Some(Box::new(f)),
            last_msg : String::new(), last_time : Utc::now().time() - Duration::seconds(1)
        }
    }

    pub fn notify<S: AsRef<str>>(&mut self, s: S, x: usize, t: usize) {
        let s = s.as_ref();
        if let Some(tx) = self.tx.as_mut() {
            if self.last_msg != s || (Utc::now().time() - self.last_time).num_milliseconds() > 300 {
                self.last_msg = String::from(s);
                self.last_time = Utc::now().time();
                tx((s.to_string(), x, t));
            }
        }
    }
}
