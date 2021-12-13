
use std::sync::mpsc::{Sender};

pub struct EventHandler {
    tx: Option<Sender<(String,usize,usize)>>
}


impl EventHandler {
    pub fn null() -> Self {
        Self{ tx : None}
    }

    pub fn notify<S : AsRef<str>> (&self, s : S, x : usize, t : usize) {
        let s = s.as_ref();
        self.tx.as_ref().map(|tx|tx.send((s.to_string(),x,t)).expect("Stopped"));
    }

    //pub fn check_stop(&self) {
    //    self.notify("",0,0);
    //}
}