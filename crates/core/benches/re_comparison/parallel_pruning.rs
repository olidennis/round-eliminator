// LLM: Included in the temporary engine module by investigate.py. This experiment
// LLM: parallelizes expensive dominance rejection against an immutable antichain.
// LLM: Surviving candidates are committed in their original order, so observers and
// LLM: mutations of the antichain remain on the requesting thread.
/// LLM: The benchmark prototype's reusable dominator queues, used to compare parallel
/// LLM: rejection and pruning policies with ordered insertion.
type PruningCache<T> = std::sync::Mutex<std::collections::VecDeque<Configuration<T>>>;

// LLM: Global pruning needs ordered results and cancellation, but no per-row
// LLM: progress messages. Compare direct Rayon collection with the progress helper's
// LLM: coordinator/channel dispatch to isolate that scheduling cost.
/// LLM: Map prototype pruning work using the benchmark's selected parallel scheduling policy.
/// LLM: Used only in the retained pruning investigation.
fn map_pruning<R: Send>(
    count: usize,
    cancellation: &Cancellation,
    parallel: bool,
    work: impl Fn(usize) -> Result<R, Cancelled> + Sync,
) -> Result<Vec<R>, Cancelled> {
    // LLM: Prepare `_dispatch` for the following computation; it stores the  dispatch for this
    // LLM: stage.
    let _dispatch = crate::profile_hooks::timer(31);
    cancellation.check()?;
    #[cfg(not(target_arch = "wasm32"))]
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if crate::profile_hooks::enabled("direct-pruning") && parallel {
        // LLM: Prepare the local constants, types, or helper imports used by the following
        // LLM: work.
        use rayon::prelude::*;
        let results: Vec<_> = (0..count)
            .into_par_iter()
            .map(|index| {
                cancellation.check()?;
                work(index)
            })
            .collect();
        // LLM: Return the result of this completed search or conversion to its caller.
        return results.into_iter().collect();
    }
    // LLM: Observe cooperative interruption before continuing this unit of work.
    map_with_progress(count, cancellation, parallel, work, |_| {
        cancellation.check()
    })
}

/// LLM: Prune dominated configurations in parallel in the retained experimental
/// LLM: implementation. Used to measure global-pruning alternatives against serial insertion.
fn parallel_prune<T: Clone + Ord + Send + Sync>(
    results: Vec<Vec<Candidate<T>>>,
    current: &[Configuration<T>],
    recent: &std::collections::VecDeque<Configuration<T>>,
    caches: &mut Vec<PruningCache<T>>,
    seen: &mut BTreeSet<Configuration<T>>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<(Vec<Vec<Candidate<T>>>, bool), Cancelled> {
    // LLM: Prepare `_prefilter` for the following computation; it stores the  prefilter for
    // LLM: this stage.
    let _prefilter = crate::profile_hooks::timer(28);
    let dedup = crate::profile_hooks::timer(30);
    let parallel_history = crate::profile_hooks::enabled("parallel-history");
    let borrowed = crate::profile_hooks::enabled("borrowed-history");
    let raw: Vec<_> = results.into_iter().flatten().collect();
    let mut indices: Vec<_> = (0..raw.len()).collect();
    // LLM: Handle this condition before continuing with the remaining search alternatives.
    if crate::profile_hooks::enabled("sort-history") {
        // LLM: Prepare `compare` for the following computation; it stores the compare for this
        // LLM: stage.
        let compare = |&a: &usize, &b: &usize| {
            raw[a]
                .configuration
                .cmp(&raw[b].configuration)
                .then(a.cmp(&b))
        };
        #[cfg(not(target_arch = "wasm32"))]
        // LLM: Update the current working state before continuing this operation.
        {
            use rayon::prelude::*;
            indices.par_sort_unstable_by(compare);
        }
        #[cfg(target_arch = "wasm32")]
        // LLM: Restore deterministic canonical ordering before the value is stored or
        // LLM: returned.
        indices.sort_unstable_by(compare);
        indices.dedup_by(|a, b| raw[*a].configuration == raw[*b].configuration);
        crate::profile_hooks::add(6, (raw.len() - indices.len()) as u64);
        indices.sort_unstable();
    }
    // LLM: Prepare `retained` for the following computation; it stores the retained for this
    // LLM: stage.
    let mut retained = vec![false; raw.len()];
    for &index in &indices {
        retained[index] = true;
    }
    // LLM: Prepare `known` for the following computation; it stores the known for this stage.
    let known = if parallel_history {
        #[cfg(not(target_arch = "wasm32"))]
        // LLM: Prepare `workers` for the following computation; it stores the workers for this
        // LLM: stage.
        let workers = rayon::current_num_threads();
        #[cfg(target_arch = "wasm32")]
        // LLM: Prepare `workers` for the following computation; it stores the workers for this
        // LLM: stage.
        let workers = 1;
        let size = indices.len().div_ceil(workers).max(1);
        let history = &*seen;
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        map_pruning(
            indices.len().div_ceil(size),
            cancellation,
            indices.len() >= 32,
            |chunk| {
                // LLM: Prepare `_pruning` for the following computation; it stores the
                // LLM: pruning for this stage.
                let _pruning = crate::profile_hooks::pruning();
                let start = chunk * size;
                let end = (start + size).min(indices.len());
                let values: Vec<_> = indices[start..end]
                    .iter()
                    .map(|&i| history.contains(&raw[i].configuration))
                    .collect();
                crate::profile_hooks::add(40, (end - start) as u64);
                // LLM: Return the result of this completed search or conversion to its caller.
                Ok(values)
            },
        )?
        .into_iter()
        .flatten()
        .collect::<Vec<bool>>()
    } else {
        vec![false; indices.len()]
    };
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for (&index, &known) in indices.iter().zip(&known) {
        if known {
            retained[index] = false;
            crate::profile_hooks::inc(6);
        }
    }
    let mut fresh = Vec::new();
    let mut clones = 0;
    // LLM: Prepare `queries` for the following computation; it stores the queries for this
    // LLM: stage.
    let mut queries = 0;
    for (index, candidate) in raw.into_iter().enumerate() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        if !retained[index] {
            continue;
        }
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if borrowed {
            queries += 1;
            if seen.contains(&candidate.configuration) {
                crate::profile_hooks::inc(6);
                continue;
            }
        }
        clones += 1;
        // LLM: Update the current working state before continuing this operation.
        queries += 1;
        if seen.insert(candidate.configuration.clone()) {
            fresh.push(candidate);
        } else {
            crate::profile_hooks::inc(6);
        }
    }
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    crate::profile_hooks::add(39, clones);
    crate::profile_hooks::add(40, queries);
    drop(dedup);
    // LLM: Serial insertion below already has a cheap dominator cache. Reserve
    // LLM: dispatch for batches with enough remaining work to amortize its cost.

    if fresh.len().saturating_mul(current.len()) < 4096 || fresh.len() < 32 {
        return Ok((vec![fresh], false));
    }
    #[cfg(not(target_arch = "wasm32"))]
    // LLM: Prepare `workers` for the following computation; it stores the workers for this
    // LLM: stage.
    let workers = rayon::current_num_threads();
    #[cfg(target_arch = "wasm32")]
    // LLM: Prepare `workers` for the following computation; it stores the workers for this
    // LLM: stage.
    let workers = 1;
    caches.resize_with(workers, || std::sync::Mutex::new(recent.clone()));
    let chunk_size = fresh.len().div_ceil(workers).max(1);
    let chunks = fresh.len().div_ceil(chunk_size);
    let caches = &*caches;
    crate::profile_hooks::inc(26);
    // LLM: Check containment before admitting a row or marking a weaker row redundant.
    let rejected = map_pruning(chunks, cancellation, true, |index| {
        // LLM: Prepare `_pruning` for the following computation; it stores the  pruning for
        // LLM: this stage.
        let _pruning = crate::profile_hooks::pruning();
        crate::profile_hooks::inc(27);
        let mut cache = caches[index].lock().unwrap();
        let start = index * chunk_size;
        let end = (start + chunk_size).min(fresh.len());
        let mut rejected = Vec::with_capacity(end - start);
        let (mut cache_checks, mut cache_rejections, mut scan_checks, mut scan_rejections) =
            (0, 0, 0, 0);
        // LLM: Prepare `promote` for the following computation; it stores the promote for this
        // LLM: stage.
        let promote = crate::profile_hooks::enabled("promote-cache");
        for candidate in &fresh[start..end] {
            // LLM: Observe cooperative interruption before continuing this unit of work.
            cancellation.check()?;
            crate::profile_hooks::inc(24);
            let mut dominated = false;
            // LLM: Check containment before admitting a row or marking a weaker row redundant.
            for position in 0..cache.len() {
                // LLM: Prepare `old` for the following computation; it stores the old for this
                // LLM: stage.
                let old = &cache[position];
                cache_checks += 1;
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                if dominates(old, &candidate.configuration, operations, cancellation)? {
                    // LLM: Update the current working state before continuing this operation.
                    dominated = true;
                    cache_rejections += 1;
                    if promote && position > 0 {
                        let old = cache.remove(position).unwrap();
                        cache.push_front(old);
                    }
                    break;
                }
            }
            // LLM: Check containment before admitting a row or marking a weaker row redundant.
            if !dominated {
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                for old in current {
                    // LLM: Update the current working state before continuing this operation.
                    scan_checks += 1;
                    if dominates(old, &candidate.configuration, operations, cancellation)? {
                        // LLM: Cache entries remain sound even after their removal
                        // LLM: from the antichain: a stronger surviving row covers
                        // LLM: them by transitivity. Each task owns a distinct lock.

                        cache.push_front(old.clone());
                        cache.truncate((current.len() / 25).max(1));
                        dominated = true;
                        scan_rejections += 1;
                        break;
                    }
                }
            }
            // LLM: Handle this condition before continuing with the remaining search
            // LLM: alternatives.
            if dominated {
                crate::profile_hooks::inc(25);
            }
            rejected.push(dominated);
        }
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        crate::profile_hooks::add(33, cache_checks);
        crate::profile_hooks::add(34, cache_rejections);
        crate::profile_hooks::add(35, scan_checks);
        crate::profile_hooks::add(36, scan_rejections);
        Ok(rejected)
    })?;
    // LLM: Prepare `survivors` for the following computation; it stores the survivors for this
    // LLM: stage.
    let survivors = fresh
        .into_iter()
        .zip(rejected.into_iter().flatten())
        .filter_map(|(candidate, rejected)| (!rejected).then_some(candidate))
        .collect();
    Ok((vec![survivors], true))
}

// LLM: Without a derivation observer, compute the antichain of the entire batch at
// LLM: once. Both rejection and removal tests run in the shared pool. The immutable
// LLM: snapshot avoids a contended mutable antichain; each distinct configuration
// LLM: has the same maximality regardless of insertion order.
/// LLM: Combine the prototype's local pruning results and remove remaining global dominance.
/// LLM: Used after parallel candidate processing in benchmark experiments.
fn parallel_merge<T: Clone + Ord + Send + Sync>(
    results: Vec<Vec<Candidate<T>>>,
    current: &mut Vec<Configuration<T>>,
    recent: &std::collections::VecDeque<Configuration<T>>,
    caches: &mut Vec<PruningCache<T>>,
    seen: &mut BTreeSet<Configuration<T>>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<bool, Cancelled> {
    // LLM: Update the current working state before continuing this operation.
    let (pruned, checked) = parallel_prune(
        results,
        current,
        recent,
        caches,
        seen,
        operations,
        cancellation,
    )?;
    // LLM: Track alternatives still compatible with the selected support or current antichain.
    let candidates: Vec<_> = pruned.into_iter().flatten().collect();
    let reduced = crate::profile_hooks::enabled("reduce-new");
    let candidates = if reduced {
        reduce_new(candidates, operations, cancellation)?
    } else {
        candidates
    };
    // LLM: Handle the empty case explicitly before the general traversal or allocation path.
    if candidates.is_empty() {
        return Ok(false);
    }
    let _commit = crate::profile_hooks::timer(29);
    #[cfg(not(target_arch = "wasm32"))]
    // LLM: Prepare `workers` for the following computation; it stores the workers for this
    // LLM: stage.
    let workers = rayon::current_num_threads();
    #[cfg(target_arch = "wasm32")]
    // LLM: Prepare `workers` for the following computation; it stores the workers for this
    // LLM: stage.
    let workers = 1;
    let chunk_size = (current.len() + candidates.len()).div_ceil(workers).max(1);
    let count = current.len() + candidates.len();
    let parallel = current.len().saturating_mul(candidates.len()) >= 256;
    // LLM: Check containment before admitting a row or marking a weaker row redundant.
    let removed = map_pruning(
        count.div_ceil(chunk_size),
        cancellation,
        parallel,
        |chunk| {
            // LLM: Prepare `_pruning` for the following computation; it stores the  pruning
            // LLM: for this stage.
            let _pruning = crate::profile_hooks::pruning();
            let start = chunk * chunk_size;
            let end = (start + chunk_size).min(count);
            let mut removed = Vec::with_capacity(end - start);
            let (mut removal_checks, mut new_checks) = (0, 0);
            // LLM: Check containment before admitting a row or marking a weaker row redundant.
            for index in start..end {
                // LLM: Observe cooperative interruption before continuing this unit of work.
                cancellation.check()?;
                let mut dominated = false;
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                if index < current.len() {
                    // LLM: Check containment before admitting a row or marking a weaker row
                    // LLM: redundant.
                    for candidate in &candidates {
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
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
                    // LLM: Prepare `offered` for the following computation; it stores the
                    // LLM: offered for this stage.
                    let offered = &candidates[index - current.len()].configuration;
                    // LLM: Small undispatched batches skipped the initial rejection
                    // LLM: pass, so they still need comparison with existing rows.

                    if !checked {
                        // LLM: Check containment before admitting a row or marking a weaker
                        // LLM: row redundant.
                        for old in current.iter() {
                            if dominates(old, offered, operations, cancellation)? {
                                dominated = true;
                                break;
                            }
                        }
                    }
                    // LLM: Check containment before admitting a row or marking a weaker row
                    // LLM: redundant.
                    if !dominated && !reduced {
                        // LLM: Check containment before admitting a row or marking a weaker
                        // LLM: row redundant.
                        for (other, candidate) in candidates.iter().enumerate() {
                            // LLM: Handle this condition before continuing with the remaining
                            // LLM: search alternatives.
                            if other == index - current.len() {
                                continue;
                            }
                            new_checks += 1;
                            // LLM: Check containment before admitting a row or marking a
                            // LLM: weaker row redundant.
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
                // LLM: Retain the newly constructed choices in the result for this stage.
                removed.push(dominated);
            }
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
            crate::profile_hooks::add(37, removal_checks);
            crate::profile_hooks::add(38, new_checks);
            Ok(removed)
        },
    )?;
    // LLM: Prepare `_apply` for the following computation; it stores the  apply for this
    // LLM: stage.
    let _apply = crate::profile_hooks::timer(32);
    let mut removed = removed.into_iter().flatten();
    current.retain(|_| !removed.next().unwrap());
    let mut changed = false;
    // LLM: Visit the remaining alternatives for this stage, stopping when the required search
    // LLM: condition is settled.
    for candidate in candidates {
        if !removed.next().unwrap() {
            crate::profile_hooks::inc(13);
            current.push(candidate.configuration);
            changed = true;
        }
    }
    drop(_apply);
    // LLM: Check containment before admitting a row or marking a weaker row redundant.
    if changed && crate::profile_hooks::enabled("refresh-cache") {
        // LLM: Prepare `_refresh` for the following computation; it stores the  refresh for
        // LLM: this stage.
        let _refresh = crate::profile_hooks::timer(42);
        let active: BTreeSet<_> = current.iter().collect();
        let mut checks = 0;
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        for cache in caches {
            // LLM: Retain reusable artifacts or dominators so proven work need not be
            // LLM: repeated.
            let mut cache = cache.lock().unwrap();
            let mut updated = std::collections::VecDeque::new();
            // LLM: Check containment before admitting a row or marking a weaker row redundant.
            for old in cache.drain(..) {
                // LLM: Check containment before admitting a row or marking a weaker row
                // LLM: redundant.
                let replacement = if active.contains(&&old) {
                    None
                } else {
                    // LLM: Prepare `replacement` for the following computation; it stores the
                    // LLM: replacement for this stage.
                    let mut replacement = None;
                    for value in current.iter() {
                        checks += 1;
                        if dominates(value, &old, operations, cancellation)? {
                            replacement = Some(value.clone());
                            break;
                        }
                    }
                    // LLM: Update the current working state before continuing this operation.
                    replacement
                };
                // LLM: Prepare `value` for the following computation; it stores the value for
                // LLM: this stage.
                let value = replacement.unwrap_or(old);
                if !updated.contains(&value) {
                    updated.push_back(value);
                }
            }
            // LLM: Update the current working state before continuing this operation.
            *cache = updated;
        }
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        crate::profile_hooks::add(41, checks);
    }
    // LLM: Return the result of this completed search or conversion to its caller.
    Ok(changed)
}

// LLM: First compute the maximum of the new batch. Only its maxima need to be
// LLM: compared against old rows; discarded candidates are covered by them.
/// LLM: Filter and combine newly generated prototype candidates into the global antichain.
/// LLM: Used by the comparison harness to isolate pruning work.
fn reduce_new<T: Clone + Ord>(
    candidates: Vec<Candidate<T>>,
    operations: &impl Operations<T>,
    cancellation: &Cancellation,
) -> Result<Vec<Candidate<T>>, Cancelled> {
    // LLM: Prepare `kept` for the following computation; it stores the kept for this stage.
    let mut kept: Vec<usize> = Vec::new();
    let mut cache: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut checks = 0;
    // LLM: Check containment before admitting a row or marking a weaker row redundant.
    for own in 0..candidates.len() {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        cancellation.check()?;
        let offered = &candidates[own].configuration;
        let mut dominated = false;
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        for &index in &cache {
            // LLM: Update the current working state before continuing this operation.
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
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        if !dominated {
            // LLM: Check containment before admitting a row or marking a weaker row redundant.
            for &index in &kept {
                // LLM: Update the current working state before continuing this operation.
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
        // LLM: Handle this condition before continuing with the remaining search alternatives.
        if dominated {
            continue;
        }
        let mut next = Vec::new();
        // LLM: Check containment before admitting a row or marking a weaker row redundant.
        for index in kept {
            // LLM: Update the current working state before continuing this operation.
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
        // LLM: Retain the newly constructed choices in the result for this stage.
        next.push(own);
        kept = next;
    }
    // LLM: Delegate to the common implementation so this entry point shares its semantics and
    // LLM: checks.
    crate::profile_hooks::add(38, checks);
    let mut retained = vec![false; candidates.len()];
    for index in kept {
        retained[index] = true;
    }
    // LLM: Return the result of this completed search or conversion to its caller.
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

    /// LLM: The counted bit-set configuration type used by pruning test and benchmark
    /// LLM: fixtures.
    type Row = Configuration<LabelSet<u16>>;

    /// LLM: Construct one counted bit-set row for a pruning fixture. Used to compare serial
    /// LLM: and parallel antichains with the same algebra.
    fn row(labels: Vec<u16>) -> Row {
        // LLM: Delegate to the common implementation so this entry point shares its semantics
        // LLM: and checks.
        Configuration::new(vec![(LabelSet::new(labels), 2)]).unwrap()
    }

    /// LLM: Prepare the initial counted rows for the pruning comparison. Used to seed
    /// LLM: equivalent serial and batch admission tests.
    fn roots() -> Vec<Row> {
        // LLM: Update the current working state before continuing this operation.
        (0..16)
            .flat_map(|a| ((a + 1)..16).map(move |b| row(vec![a, b])))
            .take(64)
            .collect()
    }

    /// LLM: Generate deterministic batches of candidate rows, including dominated
    /// LLM: alternatives. Used to compare pruning implementations across repeated admissions.
    fn batches() -> Vec<Vec<Row>> {
        // LLM: Prepare `first` for the following computation; it stores the first for this
        // LLM: stage.
        let first = roots()
            .into_iter()
            .flat_map(|root| {
                let labels: Vec<_> = root.multiplicities()[0].0.iter().collect();
                let mut larger = labels.clone();
                larger.push(32 + labels[0] % 4);
                [row(vec![labels[0]]), row(larger), root, row(vec![])]
            })
            .collect();
        // LLM: Prepare `second` for the following computation; it stores the second for this
        // LLM: stage.
        let second = (0..16)
            .flat_map(|a| {
                // LLM: Update the current working state before continuing this operation.
                ((a + 1)..16).map(move |b| {
                    Configuration::new(vec![
                        (LabelSet::new(vec![a]), 1),
                        (LabelSet::new(vec![b]), 1),
                    ])
                    .unwrap()
                })
            })
            .collect();
        // LLM: Update the current working state before continuing this operation.
        vec![first, second]
    }

    /// LLM: Wrap fixture rows with the payload needed by admission tests. Used to verify
    /// LLM: ordered tracing or pruning equivalence.
    fn candidates(rows: &[Row]) -> Vec<Vec<Candidate<LabelSet<u16>>>> {
        // LLM: Update the current working state before continuing this operation.
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

    /// LLM: Regression test: verify that parallel batch antichains match serial insertion with
    /// LLM: stale caches. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn parallel_batch_antichains_match_serial_insertion_with_stale_caches() {
        // LLM: Prepare `operations` for the following computation; it stores the operations
        // LLM: for this stage.
        let operations = SetOperations { allow_empty: true };
        let cancellation = Cancellation::default();
        let mut serial = roots();
        let mut parallel = serial.clone();
        let mut serial_seen: BTreeSet<_> = serial.iter().cloned().collect();
        let mut parallel_seen = serial_seen.clone();
        let mut recent = VecDeque::new();
        let mut caches = Vec::new();
        // LLM: Check each relevant row independently, preserving its exact degree and
        // LLM: condensed choices.
        for rows in batches() {
            // LLM: Check each relevant row independently, preserving its exact degree and
            // LLM: condensed choices.
            for candidate in &rows {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
                if serial_seen.insert(candidate.clone()) {
                    // LLM: Delegate to the common implementation so this entry point shares
                    // LLM: its semantics and checks.
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
            // LLM: Delegate to the common implementation so this entry point shares its
            // LLM: semantics and checks.
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
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert_eq!(
                serial.iter().collect::<BTreeSet<_>>(),
                parallel.iter().collect::<BTreeSet<_>>()
            );
            assert_eq!(serial_seen, parallel_seen);
        }
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
        assert!(!caches.is_empty());
        cancellation.cancel();
        // LLM: Assert that interruption propagates as cancellation and does not produce a
        // LLM: completed mathematical result.
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

    /// LLM: Regression test: verify that parallel rejection preserves ordered admissions. Used
    /// LLM: by the test suite to protect this behavior while implementations change.
    #[test]
    fn parallel_rejection_preserves_ordered_admissions() {
        // LLM: Prepare `operations` for the following computation; it stores the operations
        // LLM: for this stage.
        let operations = SetOperations { allow_empty: true };
        let cancellation = Cancellation::default();
        let mut serial = roots();
        let mut parallel = serial.clone();
        let mut serial_seen: BTreeSet<_> = serial.iter().cloned().collect();
        let mut parallel_seen = serial_seen.clone();
        let mut serial_recent = VecDeque::new();
        let mut parallel_recent = VecDeque::new();
        // LLM: Prepare `caches` for the following computation; it stores the caches for this
        // LLM: stage.
        let mut caches = Vec::new();
        for rows in batches() {
            // LLM: Prepare `expected_admissions` for the following computation; it stores the
            // LLM: expected admissions for this stage.
            let mut expected_admissions = Vec::new();
            for candidate in &rows {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
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
            // LLM: Prepare `admissions` for the following computation; it stores the
            // LLM: admissions for this stage.
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
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for candidate in pruned.into_iter().flatten() {
                // LLM: Handle this condition before continuing with the remaining search
                // LLM: alternatives.
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
            // LLM: Compare the implementation result with the independently prepared reference
            // LLM: for this fixture.
            assert_eq!(admissions, expected_admissions);
            assert_eq!(serial, parallel);
        }
    }
}
