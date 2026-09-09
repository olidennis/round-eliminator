use super::{ProcessTime, ThreadTime};
use std::marker::PhantomData;
use std::time::Duration;

#[test]
fn process_clock_regression_does_not_panic() {
    let before = ProcessTime(Duration::from_millis(1001));
    let after = ProcessTime(Duration::from_millis(1000));
    assert_eq!(after.duration_since(before), Duration::ZERO);
}

#[test]
fn thread_clock_regression_does_not_panic() {
    let before = ThreadTime(Duration::from_millis(1001), PhantomData);
    let after = ThreadTime(Duration::from_millis(1000), PhantomData);
    assert_eq!(after.duration_since(before), Duration::ZERO);
}

#[test]
fn normal_cpu_time_deltas_are_preserved() {
    let start = Duration::from_millis(900);
    let end = Duration::from_millis(1200);
    let expected = Duration::from_millis(300);
    assert_eq!(ProcessTime(end).duration_since(ProcessTime(start)), expected);
    assert_eq!(ThreadTime(end, PhantomData).duration_since(ThreadTime(start, PhantomData)), expected);
    assert_eq!(ProcessTime(start).duration_since(ProcessTime(start)), Duration::ZERO);
    assert_eq!(ThreadTime(start, PhantomData).duration_since(ThreadTime(start, PhantomData)), Duration::ZERO);
}
