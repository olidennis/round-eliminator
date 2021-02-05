use procspawn;
use simulation::AutoLb;
use simulation::AutoUb;
use simulation::AutomaticSimplifications;
use simulation::GenericProblem;
use simulation::Config;
use simulation::do_multiple_speedups;

pub fn search_for_complexity(
  data: String,
  config: Config,
  labels: usize,
  iter: usize,
  merge: bool,
  autolb_features: String,
  autoub_features: String,
  timeout: u64
) -> (String, String) {
  let mut pp_search_handle = procspawn::spawn((data.clone(), config, iter, merge), |(data, config, iter, merge)| {
      let p = GenericProblem::from_line_separated_text(&data, config).unwrap();
      let (_, found_periodic_point) = do_multiple_speedups(p, iter, merge, true);
      found_periodic_point
  });

 
  let mut autolb_handle = procspawn::spawn((data.clone(), config, labels, iter, autolb_features), |(data, config, labels, iter, autolb_features)| {
      let p = GenericProblem::from_line_separated_text(&data, config).unwrap();
      let autolb_features : Vec<_> = autolb_features.split(",").collect();
      let auto = AutomaticSimplifications::<AutoLb>::new(p, iter, labels, 1000, &autolb_features);
      let mut res: i32 = -1;
      for x in auto {
          let sol = x.unwrap();
          let local_res = (sol.speedups + if sol.current().is_zero_rounds() { 0 } else { 1 }) as i32;
          if local_res > res {
              res = local_res;
          }
      }
      res
  });

  let mut autoub_handle = procspawn::spawn((data, config, labels, iter, autoub_features), |(data, config, labels, iter, autoub_features)| {
      let p = GenericProblem::from_line_separated_text(&data, config).unwrap();
      let autoub_features : Vec<_> = autoub_features.split(",").collect();
      let auto = AutomaticSimplifications::<AutoUb>::new(p, iter, labels, 1000, &autoub_features);
      let mut res: i32 = -1;
      for x in auto {
          let sol = x.unwrap();
          let local_res = sol.speedups as i32;
          if res == -1 || local_res < res {
              res = local_res;
          }
      }
      res
  });

  let mut timeout_handle = procspawn::spawn(timeout, |timeout| {
      std::thread::sleep(std::time::Duration::from_millis(timeout));
  });


  let mut multi = procspawn::MultiWait::new();
  let pp_search_id = multi.add(&mut pp_search_handle);
  let autolb_id = multi.add(&mut autolb_handle);
  let autoub_id = multi.add(&mut autoub_handle);
  let timeout_id = multi.add(&mut timeout_handle);

  let mut pp_search_handle = Some(pp_search_handle);
  let mut autolb_handle = Some(autolb_handle);
  let mut autoub_handle = Some(autoub_handle);

  let mut lower_bound = String::from("unknown");
  let mut upper_bound = String::from("unknown");

  while let Some(events) = multi.wait_events() {
      for event in events {
          if event == pp_search_id {
              let found_periodic_point = pp_search_handle.take().unwrap().join().unwrap();
              if found_periodic_point {
                  lower_bound = String::from("log n");
              }
          }
          if event == autolb_id {
              let lower_bound_res = autolb_handle.take().unwrap().join().unwrap();
              if lower_bound_res != -1 && lower_bound != "log n" {
                  lower_bound = lower_bound_res.to_string();
              }
          }
          if event == autoub_id {
              let upper_bound_res = autoub_handle.take().unwrap().join().unwrap();
              if upper_bound_res != -1 {
                  upper_bound = upper_bound_res.to_string();
              }
          }
          if event == timeout_id {
            println!("timeout!");
            pp_search_handle.take().map(|mut h|h.kill());
            autolb_handle.take().map(|mut h|h.kill());
            autoub_handle.take().map(|mut h|h.kill());
        }
      }
  }

  (lower_bound, upper_bound)
}
