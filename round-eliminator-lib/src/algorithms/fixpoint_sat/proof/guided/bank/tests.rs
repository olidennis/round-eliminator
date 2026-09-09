use super::*;

fn synthetic(n: usize) -> Bank {
    Bank::new((0..n).map(|i| vec![Term::Terminal(i as Label)]).collect())
}

#[test]
fn lazy_sweeps_cover_all_pairs_and_the_first_round_covers_the_archive() {
    let control = SearchControl::default();
    for n in [0, 1, 6, 7, 12, 13, 25, 36, 503] {
        let mut bank = synthetic(n);
        let mut seen = HashSet::new();
        let mut pairs = HashSet::new();
        let mut count = 0;
        while let Some(ids) = bank.next_batch(&control).unwrap() {
            count += 1;
            assert!(ids.len() <= BATCH_SIZE);
            seen.extend(ids.iter().copied());
            for &a in &ids {
                for &b in &ids {
                    pairs.insert((a, b));
                }
            }
            if count == n.div_ceil(BATCH_SIZE / 2).div_ceil(2) {
                assert_eq!(seen.len(), n, "First round missed a block for {n}");
            }
        }
        assert_eq!(pairs.len(), n * n);
        assert!(bank.focus.is_empty());
        assert!(bank.next_batch(&control).unwrap().is_none());
    }
}

#[test]
fn lazy_scheduling_checks_cancellation_before_work() {
    let mut bank = synthetic(600);
    let control = SearchControl::default();
    control.stop();
    assert!(bank.next_batch(&control).is_err());
    assert!(bank.tried.is_empty());
}

#[test]
fn new_fragments_get_a_priority_turn_without_restarting_breadth() {
    let control = SearchControl::default();
    let mut bank = synthetic(504);
    let first = bank.next_batch(&control).unwrap().unwrap();
    assert!(first.contains(&503));
    let progress = (bank.sweep.round, bank.sweep.pair);
    bank.retain(vec![Term::Terminal(504)], true);
    let mut found = false;
    for _ in 0..2 {
        found |= bank.next_batch(&control).unwrap().unwrap().contains(&504);
    }
    assert!(found);
    assert!(bank.sweep.pair > progress.1);
    assert_eq!(bank.sweep.round, progress.0);
    assert!(bank.focus.len() <= bank.blocks.len());
}

#[test]
fn rotation_preserves_pinned_terms_and_invalidates_pair_versions() {
    let control = SearchControl::default();
    let mut bank = synthetic(1);
    bank.retain(vec![Term::Terminal(1)], false);
    bank.pinned = 2; // Emulate a pinned bootstrap after the original input.
    bank.replace_next = 2;
    bank.fragment_limit = 3;
    bank.retain(vec![Term::Terminal(2)], true);
    bank.retain(vec![Term::Terminal(3)], true);
    while bank.next_batch(&control).unwrap().is_some() {}
    for label in 4..100 {
        bank.retain(vec![Term::Terminal(label)], true);
        assert_eq!(bank.tuples.len(), 4);
        assert_eq!(bank.known.len(), 4);
        assert!(bank.tuples[0] == vec![Term::Terminal(0)]);
        assert!(bank.tuples[1] == vec![Term::Terminal(1)]);
        let batch = bank.next_batch(&control).unwrap().unwrap();
        assert!(batch
            .iter()
            .any(|&i| bank.tuples[i] == vec![Term::Terminal(label)]));
    }
    assert_eq!(bank.rotations, 96);
    assert!(bank.new_selected >= 98);
    assert_eq!(bank.tried.len(), 1);
}

#[test]
fn continuous_imports_do_not_starve_the_breadth_sweep() {
    let control = SearchControl::default();
    let mut bank = synthetic(504);
    let mut covered = HashSet::new();
    for i in 0..1000 {
        bank.retain(vec![Term::Terminal(504 + i)], true);
        let batch = bank.next_batch(&control).unwrap().unwrap();
        covered.extend(batch.into_iter().filter(|&id| id < 504));
        if covered.len() == 504 {
            break;
        }
    }
    assert_eq!(covered.len(), 504);
    assert!(bank.focus.len() <= bank.blocks.len());
}
