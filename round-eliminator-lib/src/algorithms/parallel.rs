use rayon::iter::ParallelIterator;

use crate::serial::SendOnlyNonWasm;

use super::event::EventHandler;



pub trait CollectWithProgress{
    type Item: SendOnlyNonWasm;

    fn collect_with_progress(self, eh : &mut EventHandler, msg : &'static str, total : usize) -> Vec<Self::Item>;
}

#[cfg(not(target_arch = "wasm32"))]
impl<T,U> CollectWithProgress for T where T : ParallelIterator<Item=Option<U>>, U : SendOnlyNonWasm {
    type Item = U;

    fn collect_with_progress(self, eh : &mut EventHandler, msg : &'static str, total : usize) -> Vec<Self::Item> {
        crossbeam::scope(|s| {

            let (progress_tx, progress_rx) =  crossbeam_channel::unbounded();
        
            let handle = s.spawn(move |_|{
                let mut result = vec![];
                let mut received = 0;
    
                while let Ok(r) = progress_rx.recv() {
                    received += 1;
                    if let Some(r) = r {
                        result.push(r);
                    }
                    eh.notify(msg, received, total);
                }

                result
            });



            self.for_each(|x|{
                progress_tx.send(x).unwrap();
            });
            drop(progress_tx);

            handle.join().unwrap()
        }).unwrap()
    }
}