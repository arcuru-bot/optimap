//! Memory-footprint table for the perfect-hash family.
//!
//! Prints a table of `bytes_used()` for each perfect-hash variant at
//! N ∈ {10K, 100K, 1M}, alongside hashbrown and `HighTag128_TombMap`
//! approximations. All u64 → u64 (or u64-only for sets).

mod bench_helpers;

use bench_helpers::make_random_keys;
use criterion::{Criterion, criterion_group, criterion_main};

use optimap::matrix_types::HighTag128_TombMap;
use optimap::perfect::{PerfectMapConfig, PtHashPhf};
use optimap::{
    PerfectMap, PerfectMapBucketed, PerfectMapMultilevelBucketed, PerfectMapSparse,
    PerfectMapUnchecked, PerfectSet, PerfectSetBucketed, PerfectSetMultilevelBucketed,
};

const SIZES: &[usize] = &[10_000, 100_000, 1_000_000];

fn print_memory_table(n: usize) {
    let keys = make_random_keys(n, 42);
    let entries: Vec<(u64, u64)> = keys.iter().map(|&k| (k, k.wrapping_mul(31))).collect();

    // ── Perfect-hash variants ──────────────────────────────────────────

    // PerfectMap (dense minimal, CHD)
    let pm = PerfectMap::<u64, u64>::from_iter_perfect(entries.iter().copied()).unwrap();
    let pm_bytes = pm.bytes_used();

    // PerfectMapUnchecked (no stored keys)
    let pmu = PerfectMapUnchecked::<u64, u64>::from_iter_perfect(entries.iter().copied()).unwrap();
    let pmu_bytes = pmu.bytes_used();

    // PerfectSet (dense minimal keys only)
    let ps = PerfectSet::<u64>::from_iter_perfect(keys.iter().copied()).unwrap();
    let ps_bytes = ps.bytes_used();

    // PerfectMapSparse (default config: load_factor 1.10)
    let pms = PerfectMapSparse::<u64, u64>::from_iter_perfect(entries.iter().copied()).unwrap();
    let pms_bytes = pms.bytes_used();

    // PerfectMapBucketed
    let pmb = PerfectMapBucketed::<u64, u64>::from_iter_perfect(entries.iter().copied()).unwrap();
    let pmb_bytes = pmb.bytes_used();

    // PerfectSetBucketed
    let psb = PerfectSetBucketed::<u64>::from_iter_perfect(keys.iter().copied()).unwrap();
    let psb_bytes = psb.bytes_used();

    // PerfectMapMultilevelBucketed
    let pmml = PerfectMapMultilevelBucketed::<u64, u64>::from_iter_perfect(entries.iter().copied())
        .unwrap();
    let pmml_bytes = pmml.bytes_used();

    // PerfectSetMultilevelBucketed
    let psml =
        PerfectSetMultilevelBucketed::<u64>::from_iter_perfect(keys.iter().copied()).unwrap();
    let psml_bytes = psml.bytes_used();

    // ── PTHash-backed variants ────────────────────────────────────────

    // PerfectMap (dense minimal, PTHash)
    let pm_pt = PerfectMap::<u64, u64, PtHashPhf>::from_entries(
        entries.clone(),
        foldhash::fast::RandomState::default(),
    )
    .unwrap();
    let pm_pt_bytes = pm_pt.bytes_used();

    // PerfectMapUnchecked (PTHash)
    let pmu_pt = PerfectMapUnchecked::<u64, u64, PtHashPhf>::from_entries(
        entries.clone(),
        foldhash::fast::RandomState::default(),
    )
    .unwrap();
    let pmu_pt_bytes = pmu_pt.bytes_used();

    // PerfectSet (PTHash)
    let ps_pt = PerfectSet::<u64, PtHashPhf>::from_keys(
        keys.clone(),
        foldhash::fast::RandomState::default(),
    )
    .unwrap();
    let ps_pt_bytes = ps_pt.bytes_used();

    // PerfectMapSparse (PTHash, load_factor=1.50 — non-minimal, generous slack)
    let config = PerfectMapConfig::default().with_load_factor(1.50);
    let pms_pt = PerfectMapSparse::<u64, u64, PtHashPhf>::from_entries_with(
        entries.clone(),
        foldhash::fast::RandomState::default(),
        &config,
    )
    .unwrap();
    let pms_pt_bytes = pms_pt.bytes_used();

    // ── Mutable hash maps ──────────────────────────────────────────────

    // hashbrown
    let mut hb: hashbrown::HashMap<u64, u64> = entries.iter().copied().collect();
    hb.shrink_to_fit();
    let hb_bytes = hb.capacity() * std::mem::size_of::<(u64, u64)>();

    // HighTag128_TombMap
    let mut tomb = HighTag128_TombMap::<u64, u64>::with_capacity(n);
    for &(k, v) in &entries {
        tomb.insert(k, v);
    }
    let tomb_bytes = tomb.capacity() * std::mem::size_of::<(u64, u64)>();

    // ── Print table ────────────────────────────────────────────────────

    println!();
    println!("=== Perfect Hash Memory Footprint (u64→u64) ===");
    println!("N={n}");
    println!(
        "  PerfectMap                    :  total={pm_bytes:>12} bytes,  bytes/key={:.2}",
        pm_bytes as f64 / n as f64
    );
    println!(
        "  PerfectMapUnchecked           :  total={pmu_bytes:>12} bytes,  bytes/key={:.2}",
        pmu_bytes as f64 / n as f64
    );
    println!(
        "  PerfectSet                    :  total={ps_bytes:>12} bytes,  bytes/key={:.2}",
        ps_bytes as f64 / n as f64
    );
    println!(
        "  PerfectMapSparse (LF=1.10)    :  total={pms_bytes:>12} bytes,  bytes/key={:.2}",
        pms_bytes as f64 / n as f64
    );
    println!(
        "  PerfectMapBucketed            :  total={pmb_bytes:>12} bytes,  bytes/key={:.2}",
        pmb_bytes as f64 / n as f64
    );
    println!(
        "  PerfectSetBucketed            :  total={psb_bytes:>12} bytes,  bytes/key={:.2}",
        psb_bytes as f64 / n as f64
    );
    println!(
        "  PerfectMapMultilevelBucketed  :  total={pmml_bytes:>12} bytes,  bytes/key={:.2}",
        pmml_bytes as f64 / n as f64
    );
    println!(
        "  PerfectSetMultilevelBucketed  :  total={psml_bytes:>12} bytes,  bytes/key={:.2}",
        psml_bytes as f64 / n as f64
    );
    println!(
        "  PerfectMap (PTHash)           :  total={pm_pt_bytes:>12} bytes,  bytes/key={:.2}",
        pm_pt_bytes as f64 / n as f64
    );
    println!(
        "  PerfectMapUnchecked (PTHash)  :  total={pmu_pt_bytes:>12} bytes,  bytes/key={:.2}",
        pmu_pt_bytes as f64 / n as f64
    );
    println!(
        "  PerfectSet (PTHash)           :  total={ps_pt_bytes:>12} bytes,  bytes/key={:.2}",
        ps_pt_bytes as f64 / n as f64
    );
    println!(
        "  PerfectMapSparse (PTHash)     :  total={pms_pt_bytes:>12} bytes,  bytes/key={:.2}",
        pms_pt_bytes as f64 / n as f64
    );
    println!(
        "  hashbrown                     :  total={hb_bytes:>12} bytes,  bytes/key={:.2}",
        hb_bytes as f64 / n as f64
    );
    println!(
        "  HighTag128_Tomb               :  total={tomb_bytes:>12} bytes,  bytes/key={:.2}",
        tomb_bytes as f64 / n as f64
    );
}

fn bench_memory(c: &mut Criterion) {
    // Print tables outside benchmark closure so they appear before criterion output.
    for &n in SIZES {
        print_memory_table(n);
    }

    // Minimal benchmark to satisfy criterion_main. We don't care about
    // timing — the printout above is the output of interest.
    c.bench_function("perfect_memory/noop", |b| b.iter(|| {}));
}

criterion_group!(benches, bench_memory);
criterion_main!(benches);
