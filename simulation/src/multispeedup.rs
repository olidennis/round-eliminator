use crate::problem::Normalized;
use crate::problem::GenericProblem;
use crate::problem::ResultProblem;

fn has_periodic_point(
  curr_normalized_problem: &Normalized,
  prev_normalized_problems: &Vec<Normalized>
) -> bool {
  for prev_problem in prev_normalized_problems {
      if curr_normalized_problem == prev_problem {
          return true;
      }
  }
  return false;
}

pub fn do_multiple_speedups(
  mut p: GenericProblem,
  iter: usize,
  merge : bool,
  find_periodic_point: bool
) -> (Vec<ResultProblem>, bool, bool) {
  // Save normalized representations of problems derived on each iteration.
  // Used to search for periodic points later on.
  let mut derived_normalized_problems = Vec::new();
  let mut results = Vec::new();
  let mut found_periodic_point = false;
  let mut found_zero_round = false;

  if find_periodic_point {
      derived_normalized_problems.push(p.normalize());  
  }
  if p.is_zero_rounds() {
    found_zero_round = true;
    return (results, found_periodic_point, found_zero_round);
  }

  for _ in 0..iter {
      p = p.speedup().unwrap();
    
      if merge && !p.mergeable.as_ref().unwrap().is_empty() {
          p = p.merge_equal();  
      }
      results.push(p.as_result());

      let normalized_p = p.normalize();
      found_periodic_point = find_periodic_point &&
        has_periodic_point(&normalized_p, &derived_normalized_problems);
      derived_normalized_problems.push(normalized_p);
      
      if p.is_zero_rounds() {
        found_zero_round = true;
        break;
      }
      if found_periodic_point {
        break;
      }
    }

    return (results, found_periodic_point, found_zero_round);
}

