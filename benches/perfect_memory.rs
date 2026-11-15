//! Storage-accounting table for the perfect-hash family.
//!
//! Prints each perfect-hash variant's `bytes_used()` accounting at
//! N ∈ {10K, 100K, 1M}. The mutable-map row is a layout estimate, not an
//! allocator-backed footprint measurement; hashbrown's bucket count is not
//! exposed by its public API, so it is reported as unavailable. All u64 → u64
//! (or u64-only for sets). Accounting includes owned array storage, not
//! inline structs, allocations owned by keys/values, or allocator overhead.

mod bench_helpers;

use bench_helpers::make_random_keys;
use criterion::{Criterion, criterion_group, criterion_main};
#[cfg(feature = "hasher-ahash")]
type BenchHasher = ahash::RandomState;
#[cfg(not(feature = "hasher-ahash"))]
type BenchHasher = foldhash::fast::RandomState;

use optimap::matrix_types::HighTag128_TombMap;
use optimap::perfect::{PackedChdPhf, PerfectMapConfig};
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

    // ── Packed-CHD-backed variants ────────────────────────────────────────

    // PerfectMap (dense minimal, packed CHD)
    let pm_packed =
        PerfectMap::<u64, u64, PackedChdPhf>::from_entries(entries.clone(), BenchHasher::default())
            .unwrap();
    let pm_packed_bytes = pm_packed.bytes_used();

    // PerfectMapUnchecked (packed CHD)
    let pmu_packed = PerfectMapUnchecked::<u64, u64, PackedChdPhf>::from_entries(
        entries.clone(),
        BenchHasher::default(),
    )
    .unwrap();
    let pmu_packed_bytes = pmu_packed.bytes_used();

    // PerfectSet (packed CHD)
    let ps_packed =
        PerfectSet::<u64, PackedChdPhf>::from_keys(keys.clone(), BenchHasher::default()).unwrap();
    let ps_packed_bytes = ps_packed.bytes_used();

    // PerfectMapSparse (packed CHD, load_factor=1.50 — non-minimal, generous slack)
    let config = PerfectMapConfig::default().with_load_factor(1.50);
    let pms_packed = PerfectMapSparse::<u64, u64, PackedChdPhf>::from_entries_with(
        entries.clone(),
        BenchHasher::default(),
        &config,
    )
    .unwrap();
    let pms_packed_bytes = pms_packed.bytes_used();

    // ── Mutable hash maps ──────────────────────────────────────────────

    // HighTag128_TombMap
    let mut tomb = HighTag128_TombMap::<u64, u64>::with_capacity(n);
    for &(k, v) in &entries {
        tomb.insert(k, v);
    }
    // Layout estimate: one metadata byte per reported capacity slot; excludes allocator overhead.
    let tomb_bytes = tomb.capacity() * (std::mem::size_of::<(u64, u64)>() + 1);

    // ── Print table ────────────────────────────────────────────────────

    println!();
    println!("=== Perfect Hash Storage Accounting (u64→u64) ===");
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
        "  PerfectMap (packed CHD)           :  total={pm_packed_bytes:>12} bytes,  bytes/key={:.2}",
        pm_packed_bytes as f64 / n as f64
    );
    println!(
        "  PerfectMapUnchecked (packed CHD)  :  total={pmu_packed_bytes:>12} bytes,  bytes/key={:.2}",
        pmu_packed_bytes as f64 / n as f64
    );
    println!(
        "  PerfectSet (packed CHD)           :  total={ps_packed_bytes:>12} bytes,  bytes/key={:.2}",
        ps_packed_bytes as f64 / n as f64
    );
    println!(
        "  PerfectMapSparse (packed CHD)     :  total={pms_packed_bytes:>12} bytes,  bytes/key={:.2}",
        pms_packed_bytes as f64 / n as f64
    );
    println!(
        "  hashbrown                     :  layout estimate unavailable (bucket count is not public API)"
    );
    println!(
        "  HighTag128_Tomb (layout est.) :  total={tomb_bytes:>12} bytes,  bytes/key={:.2}",
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
