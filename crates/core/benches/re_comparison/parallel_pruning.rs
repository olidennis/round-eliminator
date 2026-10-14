// Included in the temporary engine module by investigate.py. This experiment
// parallelizes expensive dominance rejection against an immutable antichain.
// Surviving candidates are committed in their original order, so observers and
// mutations of the antichain remain on the requesting thread.

type PruningCache<T> = std::sync::Mutex<std::collections::VecDeque<Configuration<T>>>;

// Global pruning needs ordered results and cancellation, but no per-row
// progress messages. Compare direct Rayon collection with the progress helper's
// coordinator/channel dispatch to isolate that scheduling cost.
fn map_pruning<R: Send>(
    count: usize,
    cancellation: &Cancellation,
    parallel: bool,
    work: impl Fn(usize) -> Result<R, Cancelled> + Sync,
) -> Result<Vec<R>, Cancelled> {
    let _dispatch = crate::profile_hooks::timer(31);
    cancellation.check()?;
    #[cfg(not(target_arch = "wasm32"))]
    if crate::profile_hooks::enabled("direct-pruning") && parallel {
        use rayon::prelude::*;
        let results: Vec<_> = (0..count)
            .into_par_iter()
            .map(|index| {
                cancellation.check()?;
                work(index)
            })
            .collect();
        return results.into_iter().collect();
    }
    map_with_progress(count, cancellation, parallel, work, |_| {
        cancellation.check()
    })
}

fn parallel_prune<T: Clone + Ord + Send + Sync>(
    results: Vec<Vec<Candidate<T>>>,
    current: &[Configuration<T>],
    recent: &std::collections::VecDeque<Configuration<T>>,
    caches: &mut Vec<PruningCache<T>>,
    seen: &mut BTreeSet<Configuration<T>>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<(Vec<Vec<Candidate<T>>>, bool), Cancelled> {
    let _prefilter = crate::profile_hooks::timer(28);
    let dedup = crate::profile_hooks::timer(30);
    let parallel_history = crate::profile_hooks::enabled("parallel-history");
    let borrowed = crate::profile_hooks::enabled("borrowed-history");
    let raw: Vec<_> = results.into_iter().flatten().collect();
    let mut indices: Vec<_> = (0..raw.len()).collect();
    if crate::profile_hooks::enabled("sort-history") {
        let compare = |&a: &usize, &b: &usize| {
            raw[a]
                .configuration
                .cmp(&raw[b].configuration)
                .then(a.cmp(&b))
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            use rayon::prelude::*;
            indices.par_sort_unstable_by(compare);
        }
        #[cfg(target_arch = "wasm32")]
        indices.sort_unstable_by(compare);
        indices.dedup_by(|a, b| raw[*a].configuration == raw[*b].configuration);
        crate::profile_hooks::add(6, (raw.len() - indices.len()) as u64);
        indices.sort_unstable();
    }
    let mut retained = vec![false; raw.len()];
    for &index in &indices {
        retained[index] = true;
    }
    let known = if parallel_history {
        #[cfg(not(target_arch = "wasm32"))]
        let workers = rayon::current_num_threads();
        #[cfg(target_arch = "wasm32")]
        let workers = 1;
        let size = indices.len().div_ceil(workers).max(1);
        let history = &*seen;
        map_pruning(
            indices.len().div_ceil(size),
            cancellation,
            indices.len() >= 32,
            |chunk| {
                let _pruning = crate::profile_hooks::pruning();
                let start = chunk * size;
                let end = (start + size).min(indices.len());
                let values: Vec<_> = indices[start..end]
                    .iter()
                    .map(|&i| history.contains(&raw[i].configuration))
                    .collect();
                crate::profile_hooks::add(40, (end - start) as u64);
                Ok(values)
            },
        )?
        .into_iter()
        .flatten()
        .collect::<Vec<bool>>()
    } else {
        vec![false; indices.len()]
    };
    for (&index, &known) in indices.iter().zip(&known) {
        if known {
            retained[index] = false;
            crate::profile_hooks::inc(6);
        }
    }
    let mut fresh = Vec::new();
    let mut clones = 0;
    let mut queries = 0;
    for (index, candidate) in raw.into_iter().enumerate() {
        cancellation.check()?;
        if !retained[index] {
            continue;
        }
        if borrowed {
            queries += 1;
            if seen.contains(&candidate.configuration) {
                crate::profile_hooks::inc(6);
                continue;
            }
        }
        clones += 1;
        queries += 1;
        if seen.insert(candidate.configuration.clone()) {
            fresh.push(candidate);
        } else {
            crate::profile_hooks::inc(6);
        }
    }
    crate::profile_hooks::add(39, clones);
    crate::profile_hooks::add(40, queries);
    drop(dedup);
    // Serial insertion below already has a cheap dominator cache. Reserve
    // dispatch for batches with enough remaining work to amortize its cost.
    if fresh.len().saturating_mul(current.len()) < 4096 || fresh.len() < 32 {
        return Ok((vec![fresh], false));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let workers = rayon::current_num_threads();
    #[cfg(target_arch = "wasm32")]
    let workers = 1;
    caches.resize_with(workers, || std::sync::Mutex::new(recent.clone()));
    let chunk_size = fresh.len().div_ceil(workers).max(1);
    let chunks = fresh.len().div_ceil(chunk_size);
    let caches = &*caches;
    crate::profile_hooks::inc(26);
    let rejected = map_pruning(chunks, cancellation, true, |index| {
        let _pruning = crate::profile_hooks::pruning();
        crate::profile_hooks::inc(27);
        let mut cache = caches[index].lock().unwrap();
        let start = index * chunk_size;
        let end = (start + chunk_size).min(fresh.len());
        let mut rejected = Vec::with_capacity(end - start);
        let (mut cache_checks, mut cache_rejections, mut scan_checks, mut scan_rejections) =
            (0, 0, 0, 0);
        let promote = crate::profile_hooks::enabled("promote-cache");
        for candidate in &fresh[start..end] {
            cancellation.check()?;
            crate::profile_hooks::inc(24);
            let mut dominated = false;
            for position in 0..cache.len() {
                let old = &cache[position];
                cache_checks += 1;
                if dominates(old, &candidate.configuration, operations, cancellation)? {
                    dominated = true;
                    cache_rejections += 1;
                    if promote && position > 0 {
                        let old = cache.remove(position).unwrap();
                        cache.push_front(old);
                    }
                    break;
                }
            }
            if !dominated {
                for old in current {
                    scan_checks += 1;
                    if dominates(old, &candidate.configuration, operations, cancellation)? {
                        // Cache entries remain sound even after their removal
                        // from the antichain: a stronger surviving row covers
                        // them by transitivity. Each task owns a distinct lock.
                        cache.push_front(old.clone());
                        cache.truncate((current.len() / 25).max(1));
                        dominated = true;
                        scan_rejections += 1;
                        break;
                    }
                }
            }
            if dominated {
                crate::profile_hooks::inc(25);
            }
            rejected.push(dominated);
        }
        crate::profile_hooks::add(33, cache_checks);
        crate::profile_hooks::add(34, cache_rejections);
        crate::profile_hooks::add(35, scan_checks);
        crate::profile_hooks::add(36, scan_rejections);
        Ok(rejected)
    })?;
    let survivors = fresh
        .into_iter()
        .zip(rejected.into_iter().flatten())
        .filter_map(|(candidate, rejected)| (!rejected).then_some(candidate))
        .collect();
    Ok((vec![survivors], true))
}

// Without a derivation observer, compute the antichain of the entire batch at
// once. Both rejection and removal tests run in the shared pool. The immutable
// snapshot avoids a contended mutable antichain; each distinct configuration
// has the same maximality regardless of insertion order.
fn parallel_merge<T: Clone + Ord + Send + Sync>(
    results: Vec<Vec<Candidate<T>>>,
    current: &mut Vec<Configuration<T>>,
    recent: &std::collections::VecDeque<Configuration<T>>,
    caches: &mut Vec<PruningCache<T>>,
    seen: &mut BTreeSet<Configuration<T>>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    let (pruned, checked) = parallel_prune(
        results,
        current,
        recent,
        caches,
        seen,
        operations,
        cancellation,
    )?;
    let candidates: Vec<_> = pruned.into_iter().flatten().collect();
    let reduced = crate::profile_hooks::enabled("reduce-new");
    let candidates = if reduced {
        reduce_new(candidates, operations, cancellation)?
    } else {
        candidates
    };
    if candidates.is_empty() {
        return Ok(false);
    }
    let _commit = crate::profile_hooks::timer(29);
    #[cfg(not(target_arch = "wasm32"))]
    let workers = rayon::current_num_threads();
    #[cfg(target_arch = "wasm32")]
    let workers = 1;
    let chunk_size = (current.len() + candidates.len()).div_ceil(workers).max(1);
    let count = current.len() + candidates.len();
    let parallel = current.len().saturating_mul(candidates.len()) >= 256;
    let removed = map_pruning(
        count.div_ceil(chunk_size),
        cancellation,
        parallel,
        |chunk| {
            let _pruning = crate::profile_hooks::pruning();
            let start = chunk * chunk_size;
            let end = (start + chunk_size).min(count);
            let mut removed = Vec::with_capacity(end - start);
            let (mut removal_checks, mut new_checks) = (0, 0);
            for index in start..end {
                cancellation.check()?;
                let mut dominated = false;
                if index < current.len() {
                    for candidate in &candidates {
                        removal_checks += 1;
                        if dominates(
                            &candidate.configuration,
                            &current[index],
                            operations,
                            cancellation,
                        )? {
                            dominated = true;
                            break;
                        }
                    }
                } else {
                    let offered = &candidates[index - current.len()].configuration;
                    // Small undispatched batches skipped the initial rejection
                    // pass, so they still need comparison with existing rows.
                    if !checked {
                        for old in current.iter() {
                            if dominates(old, offered, operations, cancellation)? {
                                dominated = true;
                                break;
                            }
                        }
                    }
                    if !dominated && !reduced {
                        for (other, candidate) in candidates.iter().enumerate() {
                            if other == index - current.len() {
                                continue;
                            }
                            new_checks += 1;
                            if dominates(
                                &candidate.configuration,
                                offered,
                                operations,
                                cancellation,
                            )? {
                                dominated = true;
                                break;
                            }
                        }
                    }
                }
                removed.push(dominated);
            }
            crate::profile_hooks::add(37, removal_checks);
            crate::profile_hooks::add(38, new_checks);
            Ok(removed)
        },
    )?;
    let _apply = crate::profile_hooks::timer(32);
    let mut removed = removed.into_iter().flatten();
    current.retain(|_| !removed.next().unwrap());
    let mut changed = false;
    for candidate in candidates {
        if !removed.next().unwrap() {
            crate::profile_hooks::inc(13);
            current.push(candidate.configuration);
            changed = true;
        }
    }
    drop(_apply);
    if changed && crate::profile_hooks::enabled("refresh-cache") {
        let _refresh = crate::profile_hooks::timer(42);
        let active: BTreeSet<_> = current.iter().collect();
        let mut checks = 0;
        for cache in caches {
            let mut cache = cache.lock().unwrap();
            let mut updated = std::collections::VecDeque::new();
            for old in cache.drain(..) {
                let replacement = if active.contains(&&old) {
                    None
                } else {
                    let mut replacement = None;
                    for value in current.iter() {
                        checks += 1;
                        if dominates(value, &old, operations, cancellation)? {
                            replacement = Some(value.clone());
                            break;
                        }
                    }
                    replacement
                };
                let value = replacement.unwrap_or(old);
                if !updated.contains(&value) {
                    updated.push_back(value);
                }
            }
            *cache = updated;
        }
        crate::profile_hooks::add(41, checks);
    }
    Ok(changed)
}

// First compute the maximum of the new batch. Only its maxima need to be
// compared against old rows; discarded candidates are covered by them.
fn reduce_new<T: Clone + Ord>(
    candidates: Vec<Candidate<T>>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<Vec<Candidate<T>>, Cancelled> {
    let mut kept: Vec<usize> = Vec::new();
    let mut cache: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut checks = 0;
    for own in 0..candidates.len() {
        cancellation.check()?;
        let offered = &candidates[own].configuration;
        let mut dominated = false;
        for &index in &cache {
            checks += 1;
            if dominates(
                &candidates[index].configuration,
                offered,
                operations,
                cancellation,
            )? {
                dominated = true;
                break;
            }
        }
        if !dominated {
            for &index in &kept {
                checks += 1;
                if dominates(
                    &candidates[index].configuration,
                    offered,
                    operations,
                    cancellation,
                )? {
                    cache.push_front(index);
                    cache.truncate((kept.len() / 25).max(1));
                    dominated = true;
                    break;
                }
            }
        }
        if dominated {
            continue;
        }
        let mut next = Vec::new();
        for index in kept {
            checks += 1;
            if !dominates(
                offered,
                &candidates[index].configuration,
                operations,
                cancellation,
            )? {
                next.push(index);
            }
        }
        next.push(own);
        kept = next;
    }
    crate::profile_hooks::add(38, checks);
    let mut retained = vec![false; candidates.len()];
    for index in kept {
        retained[index] = true;
    }
    Ok(candidates
        .into_iter()
        .zip(retained)
        .filter_map(|(candidate, keep)| keep.then_some(candidate))
        .collect())
}

#[cfg(test)]
mod parallel_pruning_tests {
    use super::*;
    use std::collections::VecDeque;

    type Row = Configuration<LabelSet<u16>>;

    fn row(labels: Vec<u16>) -> Row {
        Configuration::new(vec![(LabelSet::new(labels), 2)]).unwrap()
    }

    fn roots() -> Vec<Row> {
        (0..16)
            .flat_map(|a| ((a + 1)..16).map(move |b| row(vec![a, b])))
            .take(64)
            .collect()
    }

    fn batches() -> Vec<Vec<Row>> {
        let first = roots()
            .into_iter()
            .flat_map(|root| {
                let labels: Vec<_> = root.multiplicities()[0].0.iter().collect();
                let mut larger = labels.clone();
                larger.push(32 + labels[0] % 4);
                [row(vec![labels[0]]), row(larger), root, row(vec![])]
            })
            .collect();
        let second = (0..16)
            .flat_map(|a| {
                ((a + 1)..16).map(move |b| {
                    Configuration::new(vec![
                        (LabelSet::new(vec![a]), 1),
                        (LabelSet::new(vec![b]), 1),
                    ])
                    .unwrap()
                })
            })
            .collect();
        vec![first, second]
    }

    fn candidates(rows: &[Row]) -> Vec<Vec<Candidate<LabelSet<u16>>>> {
        vec![
            rows.iter()
                .cloned()
                .map(|configuration| Candidate {
                    configuration,
                    combination: None,
                })
                .collect(),
        ]
    }

    #[test]
    fn parallel_batch_antichains_match_serial_insertion_with_stale_caches() {
        let operations = SetOperations { allow_empty: true };
        let cancellation = Cancellation::default();
        let mut serial = roots();
        let mut parallel = serial.clone();
        let mut serial_seen: BTreeSet<_> = serial.iter().cloned().collect();
        let mut parallel_seen = serial_seen.clone();
        let mut recent = VecDeque::new();
        let mut caches = Vec::new();
        for rows in batches() {
            for candidate in &rows {
                if serial_seen.insert(candidate.clone()) {
                    insert(
                        &mut serial,
                        &mut recent,
                        candidate.clone(),
                        &operations,
                        &cancellation,
                    )
                    .unwrap();
                }
            }
            parallel_merge(
                candidates(&rows),
                &mut parallel,
                &VecDeque::new(),
                &mut caches,
                &mut parallel_seen,
                &operations,
                &cancellation,
            )
            .unwrap();
            assert_eq!(
                serial.iter().collect::<BTreeSet<_>>(),
                parallel.iter().collect::<BTreeSet<_>>()
            );
            assert_eq!(serial_seen, parallel_seen);
        }
        assert!(!caches.is_empty());
        cancellation.cancel();
        assert!(
            parallel_merge(
                candidates(&[row(vec![99])]),
                &mut parallel,
                &VecDeque::new(),
                &mut caches,
                &mut parallel_seen,
                &operations,
                &cancellation
            )
            .is_err()
        );
    }

    #[test]
    fn parallel_rejection_preserves_ordered_admissions() {
        let operations = SetOperations { allow_empty: true };
        let cancellation = Cancellation::default();
        let mut serial = roots();
        let mut parallel = serial.clone();
        let mut serial_seen: BTreeSet<_> = serial.iter().cloned().collect();
        let mut parallel_seen = serial_seen.clone();
        let mut serial_recent = VecDeque::new();
        let mut parallel_recent = VecDeque::new();
        let mut caches = Vec::new();
        for rows in batches() {
            let mut expected_admissions = Vec::new();
            for candidate in &rows {
                if serial_seen.insert(candidate.clone())
                    && insert(
                        &mut serial,
                        &mut serial_recent,
                        candidate.clone(),
                        &operations,
                        &cancellation,
                    )
                    .unwrap()
                {
                    expected_admissions.push(candidate.clone());
                }
            }
            let mut admissions = Vec::new();
            let (pruned, _) = parallel_prune(
                candidates(&rows),
                &parallel,
                &parallel_recent,
                &mut caches,
                &mut parallel_seen,
                &operations,
                &cancellation,
            )
            .unwrap();
            for candidate in pruned.into_iter().flatten() {
                if insert(
                    &mut parallel,
                    &mut parallel_recent,
                    candidate.configuration.clone(),
                    &operations,
                    &cancellation,
                )
                .unwrap()
                {
                    admissions.push(candidate.configuration);
                }
            }
            assert_eq!(admissions, expected_admissions);
            assert_eq!(serial, parallel);
        }
    }
}
