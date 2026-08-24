//! Isolate the cold `with_capacity` + fill path from allocation and warm fill.
//!
//! Usage: `cargo run --release --example profile_cold_fill -- [runs] [design] [case]`.
//! `design` is `tomb`, `hb`, or `all`; `case` is `allocation_only`,
//! `warm_fill_only`, `cold_allocate_and_fill`, or `all`. Defaults run both
//! designs and all cases at N=10K. `warm_fill_only` reuses a page-faulted
//! allocation after `clear()`; the other cases time allocation only or
//! allocation plus fill.

use optimap::Map;
use optimap::matrix_types::HighTag128_TombMap;
use std::time::{Duration, Instant};

const N: usize = 10_000;

fn main() {
    let mut args = std::env::args().skip(1);
    let runs = args
        .next()
        .map(|value| value.parse().expect("runs must be an integer"))
        .unwrap_or(101);
    assert!(runs > 0, "runs must be positive");
    let design = args.next().unwrap_or_else(|| "all".into());
    let case = args.next().unwrap_or_else(|| "all".into());
    let keys: Vec<u64> = (0..N)
        .map(|i| (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .collect();

    println!("design,case,n,runs,median_us");
    match design.as_str() {
        "tomb" => profile::<HighTag128_TombMap<u64, u64>>("Tomb", &keys, runs, &case),
        "hb" => profile::<hashbrown::HashMap<u64, u64>>("hashbrown", &keys, runs, &case),
        "all" => {
            profile::<HighTag128_TombMap<u64, u64>>("Tomb", &keys, runs, &case);
            profile::<hashbrown::HashMap<u64, u64>>("hashbrown", &keys, runs, &case);
        }
        _ => panic!("design must be tomb|hb|all"),
    }
}

fn profile<M: Map<u64, u64>>(name: &str, keys: &[u64], runs: usize, selected: &str) {
    for case in [
        "allocation_only",
        "warm_fill_only",
        "cold_allocate_and_fill",
    ] {
        if selected != "all" && selected != case {
            continue;
        }
        let elapsed = match case {
            "allocation_only" => measure(runs, || allocate_only::<M>(keys.len())),
            "warm_fill_only" => measure(runs, || warm_fill::<M>(keys)),
            "cold_allocate_and_fill" => measure(runs, || cold_fill::<M>(keys)),
            _ => unreachable!(),
        };
        report(name, case, runs, elapsed);
    }
    assert!(
        selected == "all"
            || [
                "allocation_only",
                "warm_fill_only",
                "cold_allocate_and_fill"
            ]
            .contains(&selected),
        "case must be allocation_only|warm_fill_only|cold_allocate_and_fill|all"
    );
}

fn report(name: &str, case: &str, runs: usize, elapsed: Duration) {
    println!(
        "{name},{case},{N},{runs},{:.3}",
        elapsed.as_secs_f64() * 1_000_000.0
    );
}

fn measure(runs: usize, mut operation: impl FnMut() -> Duration) -> Duration {
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        samples.push(operation());
    }
    let median = samples.len() / 2;
    *samples.select_nth_unstable(median).1
}

fn allocate_only<M: Map<u64, u64>>(n: usize) -> Duration {
    let start = Instant::now();
    let map = M::with_capacity(n);
    std::hint::black_box(map.capacity());
    start.elapsed()
}

fn cold_fill<M: Map<u64, u64>>(keys: &[u64]) -> Duration {
    let start = Instant::now();
    let mut map = M::with_capacity(keys.len());
    fill(&mut map, keys);
    std::hint::black_box(map.len());
    start.elapsed()
}

fn warm_fill<M: Map<u64, u64>>(keys: &[u64]) -> Duration {
    let mut map = M::with_capacity(keys.len());
    fill(&mut map, keys);
    map.clear();
    let start = Instant::now();
    fill(&mut map, keys);
    std::hint::black_box(map.len());
    start.elapsed()
}

fn fill<M: Map<u64, u64>>(map: &mut M, keys: &[u64]) {
    for (i, &key) in keys.iter().enumerate() {
        map.insert(key, i as u64);
    }
}
