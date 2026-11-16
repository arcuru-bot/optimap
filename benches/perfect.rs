//! Perfect-hash family benchmarks vs hashbrown and `HighTag128_TombMap`.
//!
//! Measures:
//!
//! 1. **Construction cost** — building the map/set from N entries. The
//!    perfect-hash side pays an O(N · λ) displacement search up front;
//!    mutable hash maps pay incremental insert + resize.
//! 2. **Hit-lookup cost** — looking up keys that are in the set. The
//!    perfect side does one PHF computation + one indirect (+ one key
//!    compare for `PerfectMap` / `PerfectSet`); the mutable side does a
//!    probe loop.
//! 3. **Miss-lookup cost** — looking up keys NOT in the set. Skipped for
//!    `PerfectMapUnchecked`: its contract returns garbage on miss, so
//!    "miss" isn't a measurable distinct path.
//!
//! Variants:
//!
//! - `PerfectMap` — CHD-MPH, dense, stored keys, miss-safe
//! - `PerfectMapSparse` — CHD with `load_factor = 1.10` (default), sparse `Option`-wrapped storage, miss-safe
//! - `PerfectMapUnchecked` — CHD-MPH, no key compare, hit-only
//! - `PerfectSet` — CHD-MPH exact membership
//! - `PerfectMapBucketed` — bucketed PHF, SIMD tag scan, miss-safe
//! - `PerfectSetBucketed` — bucketed PHF exact membership
//! - `PerfectMapMultilevelBucketed` — two-level bucketed, λ₀=8 + λ₁=4
//! - `PerfectSetMultilevelBucketed` — two-level bucketed set
//! - `hashbrown::HashMap` — the reference incumbent
//! - `HighTag128_TombMap` — `MapType::Tomb`, OptiMap's `Auto` default
//!
//! The u64 construction grid remains N ∈ {10K, 100K, 1M}; hit and miss lookup
//! use 10 points from 10K through 5M. A bounded String hit row covers 10K,
//! 100K, 300K, and 1M.

mod bench_helpers;

use bench_helpers::{make_miss_keys, make_random_keys};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::time::Duration;
#[cfg(feature = "hasher-ahash")]
type BenchHasher = ahash::RandomState;
#[cfg(not(feature = "hasher-ahash"))]
type BenchHasher = foldhash::fast::RandomState;

use optimap::matrix_types::HighTag128_TombMap;
use optimap::perfect::PackedChdPhf;
use optimap::{
    PerfectMap, PerfectMapBucketed, PerfectMapMultilevelBucketed, PerfectMapSparse,
    PerfectMapUnchecked, PerfectSet, PerfectSetBucketed, PerfectSetMultilevelBucketed,
};

const SIZES: &[usize] = &[10_000, 100_000, 1_000_000];
const SIZES_LOOKUP: &[usize] = &[
    10_000, 100_000, 200_000, 300_000, 500_000, 700_000, 1_000_000, 1_500_000, 2_000_000, 5_000_000,
];
const SIZES_LOOKUP_STRING: &[usize] = &[10_000, 100_000, 300_000, 1_000_000];

fn sample_size_for(n: usize) -> usize {
    if n >= 1_000_000 {
        10
    } else if n >= 100_000 {
        20
    } else {
        30
    }
}

fn cap_large_lookup_timing(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    n: usize,
) {
    // Shorten sampling at large N; this does not bound PHF construction time.
    if n >= 1_500_000 {
        group.warm_up_time(Duration::from_millis(500));
        group.measurement_time(Duration::from_secs(2));
    }
}

// ── Construction ──────────────────────────────────────────────────────────

fn bench_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("perfect/construction");

    for &n in SIZES {
        let keys = make_random_keys(n, 42);
        let entries: Vec<(u64, u64)> = keys.iter().map(|&k| (k, k.wrapping_mul(31))).collect();

        group.throughput(Throughput::Elements(n as u64));
        group.sample_size(sample_size_for(n));

        group.bench_with_input(BenchmarkId::new("PerfectMap", n), &entries, |b, e| {
            b.iter(|| {
                let m = PerfectMap::<u64, u64>::from_iter_perfect(e.iter().copied()).unwrap();
                black_box(m);
            });
        });

        group.bench_with_input(BenchmarkId::new("PerfectMapSparse", n), &entries, |b, e| {
            b.iter(|| {
                let m = PerfectMapSparse::<u64, u64>::from_iter_perfect(e.iter().copied()).unwrap();
                black_box(m);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("PerfectMapUnchecked", n),
            &entries,
            |b, e| {
                b.iter(|| {
                    let m = PerfectMapUnchecked::<u64, u64>::from_iter_perfect(e.iter().copied())
                        .unwrap();
                    black_box(m);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("PerfectSet", n), &keys, |b, k| {
            b.iter(|| {
                let s = PerfectSet::<u64>::from_iter_perfect(k.iter().copied()).unwrap();
                black_box(s);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("PerfectMap (packed CHD)", n),
            &entries,
            |b, e| {
                b.iter(|| {
                    let m = PerfectMap::<u64, u64, PackedChdPhf>::from_entries(
                        e.iter().copied(),
                        BenchHasher::default(),
                    )
                    .unwrap();
                    black_box(m);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectMapUnchecked (packed CHD)", n),
            &entries,
            |b, e| {
                b.iter(|| {
                    let m = PerfectMapUnchecked::<u64, u64, PackedChdPhf>::from_entries(
                        e.iter().copied(),
                        BenchHasher::default(),
                    )
                    .unwrap();
                    black_box(m);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectSet (packed CHD)", n),
            &keys,
            |b, k| {
                b.iter(|| {
                    let s = PerfectSet::<u64, PackedChdPhf>::from_keys(
                        k.iter().copied(),
                        BenchHasher::default(),
                    )
                    .unwrap();
                    black_box(s);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectMapBucketed", n),
            &entries,
            |b, e| {
                b.iter(|| {
                    let m = PerfectMapBucketed::<u64, u64>::from_iter_perfect(e.iter().copied())
                        .unwrap();
                    black_box(m);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("PerfectSetBucketed", n), &keys, |b, k| {
            b.iter(|| {
                let s = PerfectSetBucketed::<u64>::from_iter_perfect(k.iter().copied()).unwrap();
                black_box(s);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("PerfectMapMultilevelBucketed", n),
            &entries,
            |b, e| {
                b.iter(|| {
                    let m = PerfectMapMultilevelBucketed::<u64, u64>::from_iter_perfect(
                        e.iter().copied(),
                    )
                    .unwrap();
                    black_box(m);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectSetMultilevelBucketed", n),
            &keys,
            |b, k| {
                b.iter(|| {
                    let s =
                        PerfectSetMultilevelBucketed::<u64>::from_iter_perfect(k.iter().copied())
                            .unwrap();
                    black_box(s);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("hashbrown", n), &entries, |b, e| {
            b.iter(|| {
                let m: hashbrown::HashMap<u64, u64> = e.iter().copied().collect();
                black_box(m);
            });
        });

        group.bench_with_input(BenchmarkId::new("HighTag128_Tomb", n), &entries, |b, e| {
            b.iter(|| {
                let mut m = HighTag128_TombMap::<u64, u64>::with_capacity(e.len());
                for &(k, v) in e {
                    m.insert(k, v);
                }
                black_box(m);
            });
        });
    }

    group.finish();
}

// ── Hit lookup ────────────────────────────────────────────────────────────

fn bench_lookup_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("perfect/lookup_hit");

    for &n in SIZES_LOOKUP {
        let keys = make_random_keys(n, 42);
        let entries: Vec<(u64, u64)> = keys.iter().map(|&k| (k, k.wrapping_mul(31))).collect();

        // Build only selected cases, outside the timed lookup loop.
        group.throughput(Throughput::Elements(n as u64));
        group.sample_size(sample_size_for(n));
        cap_large_lookup_timing(&mut group, n);

        group.bench_with_input(BenchmarkId::new("PerfectMap", n), &keys, |b, ks| {
            let pm = PerfectMap::<u64, u64>::from_iter_perfect(entries.iter().copied()).unwrap();
            b.iter(|| {
                let mut sum = 0u64;
                for &k in ks {
                    sum = sum.wrapping_add(*pm.get(&k).unwrap_or(&0));
                }
                black_box(sum);
            });
        });

        group.bench_with_input(BenchmarkId::new("PerfectMapSparse", n), &keys, |b, ks| {
            let pms =
                PerfectMapSparse::<u64, u64>::from_iter_perfect(entries.iter().copied()).unwrap();
            b.iter(|| {
                let mut sum = 0u64;
                for &k in ks {
                    sum = sum.wrapping_add(*pms.get(&k).unwrap_or(&0));
                }
                black_box(sum);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("PerfectMapUnchecked", n),
            &keys,
            |b, ks| {
                let pmu =
                    PerfectMapUnchecked::<u64, u64>::from_iter_perfect(entries.iter().copied())
                        .unwrap();
                b.iter(|| {
                    let mut sum = 0u64;
                    for &k in ks {
                        sum = sum.wrapping_add(*pmu.get_unchecked(&k));
                    }
                    black_box(sum);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("PerfectSet", n), &keys, |b, ks| {
            let ps = PerfectSet::<u64>::from_iter_perfect(keys.iter().copied()).unwrap();
            b.iter(|| {
                let mut count = 0u64;
                for &k in ks {
                    if ps.contains(&k) {
                        count += 1;
                    }
                }
                black_box(count);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("PerfectMap (packed CHD)", n),
            &keys,
            |b, ks| {
                let pm_packed = PerfectMap::<u64, u64, PackedChdPhf>::from_entries(
                    entries.iter().copied(),
                    BenchHasher::default(),
                )
                .unwrap();
                b.iter(|| {
                    let mut sum = 0u64;
                    for &k in ks {
                        sum = sum.wrapping_add(*pm_packed.get(&k).unwrap_or(&0));
                    }
                    black_box(sum);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectMapUnchecked (packed CHD)", n),
            &keys,
            |b, ks| {
                let pmu_packed = PerfectMapUnchecked::<u64, u64, PackedChdPhf>::from_entries(
                    entries.iter().copied(),
                    BenchHasher::default(),
                )
                .unwrap();
                b.iter(|| {
                    let mut sum = 0u64;
                    for &k in ks {
                        sum = sum.wrapping_add(*pmu_packed.get_unchecked(&k));
                    }
                    black_box(sum);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectSet (packed CHD)", n),
            &keys,
            |b, ks| {
                let ps_packed = PerfectSet::<u64, PackedChdPhf>::from_keys(
                    keys.iter().copied(),
                    BenchHasher::default(),
                )
                .unwrap();
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if ps_packed.contains(&k) {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("PerfectMapBucketed", n), &keys, |b, ks| {
            let pmb =
                PerfectMapBucketed::<u64, u64>::from_iter_perfect(entries.iter().copied()).unwrap();
            b.iter(|| {
                let mut sum = 0u64;
                for &k in ks {
                    sum = sum.wrapping_add(*pmb.get(&k).unwrap_or(&0));
                }
                black_box(sum);
            });
        });

        group.bench_with_input(BenchmarkId::new("PerfectSetBucketed", n), &keys, |b, ks| {
            let psb = PerfectSetBucketed::<u64>::from_iter_perfect(keys.iter().copied()).unwrap();
            b.iter(|| {
                let mut count = 0u64;
                for &k in ks {
                    if psb.contains(&k) {
                        count += 1;
                    }
                }
                black_box(count);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("PerfectMapMultilevelBucketed", n),
            &keys,
            |b, ks| {
                let pmml = PerfectMapMultilevelBucketed::<u64, u64>::from_iter_perfect(
                    entries.iter().copied(),
                )
                .unwrap();
                b.iter(|| {
                    let mut sum = 0u64;
                    for &k in ks {
                        sum = sum.wrapping_add(*pmml.get(&k).unwrap_or(&0));
                    }
                    black_box(sum);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectSetMultilevelBucketed", n),
            &keys,
            |b, ks| {
                let psml =
                    PerfectSetMultilevelBucketed::<u64>::from_iter_perfect(keys.iter().copied())
                        .unwrap();
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if psml.contains(&k) {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("hashbrown", n), &keys, |b, ks| {
            let hb: hashbrown::HashMap<u64, u64> = entries.iter().copied().collect();
            b.iter(|| {
                let mut sum = 0u64;
                for &k in ks {
                    sum = sum.wrapping_add(*hb.get(&k).unwrap_or(&0));
                }
                black_box(sum);
            });
        });

        group.bench_with_input(BenchmarkId::new("HighTag128_Tomb", n), &keys, |b, ks| {
            let mut tomb = HighTag128_TombMap::<u64, u64>::with_capacity(n);
            for &(k, v) in &entries {
                tomb.insert(k, v);
            }
            b.iter(|| {
                let mut sum = 0u64;
                for &k in ks {
                    sum = sum.wrapping_add(*tomb.get(&k).unwrap_or(&0));
                }
                black_box(sum);
            });
        });
    }

    group.finish();
}

// ── Miss lookup ───────────────────────────────────────────────────────────

fn bench_lookup_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("perfect/lookup_miss");

    for &n in SIZES_LOOKUP {
        let keys = make_random_keys(n, 42);
        let miss_keys = make_miss_keys(n);
        let entries: Vec<(u64, u64)> = keys.iter().map(|&k| (k, k.wrapping_mul(31))).collect();

        group.throughput(Throughput::Elements(n as u64));
        group.sample_size(sample_size_for(n));
        cap_large_lookup_timing(&mut group, n);

        group.bench_with_input(BenchmarkId::new("PerfectMap", n), &miss_keys, |b, ks| {
            let pm = PerfectMap::<u64, u64>::from_iter_perfect(entries.iter().copied()).unwrap();
            b.iter(|| {
                let mut count = 0u64;
                for &k in ks {
                    if pm.get(&k).is_none() {
                        count += 1;
                    }
                }
                black_box(count);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("PerfectMapSparse", n),
            &miss_keys,
            |b, ks| {
                let pms = PerfectMapSparse::<u64, u64>::from_iter_perfect(entries.iter().copied())
                    .unwrap();
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if pms.get(&k).is_none() {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("PerfectSet", n), &miss_keys, |b, ks| {
            let ps = PerfectSet::<u64>::from_iter_perfect(keys.iter().copied()).unwrap();
            b.iter(|| {
                let mut count = 0u64;
                for &k in ks {
                    if !ps.contains(&k) {
                        count += 1;
                    }
                }
                black_box(count);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("PerfectMap (packed CHD)", n),
            &miss_keys,
            |b, ks| {
                let pm_packed = PerfectMap::<u64, u64, PackedChdPhf>::from_entries(
                    entries.iter().copied(),
                    BenchHasher::default(),
                )
                .unwrap();
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if pm_packed.get(&k).is_none() {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectSet (packed CHD)", n),
            &miss_keys,
            |b, ks| {
                let ps_packed = PerfectSet::<u64, PackedChdPhf>::from_keys(
                    keys.iter().copied(),
                    BenchHasher::default(),
                )
                .unwrap();
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if !ps_packed.contains(&k) {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectMapBucketed", n),
            &miss_keys,
            |b, ks| {
                let pmb =
                    PerfectMapBucketed::<u64, u64>::from_iter_perfect(entries.iter().copied())
                        .unwrap();
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if pmb.get(&k).is_none() {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectSetBucketed", n),
            &miss_keys,
            |b, ks| {
                let psb =
                    PerfectSetBucketed::<u64>::from_iter_perfect(keys.iter().copied()).unwrap();
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if !psb.contains(&k) {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectMapMultilevelBucketed", n),
            &miss_keys,
            |b, ks| {
                let pmml = PerfectMapMultilevelBucketed::<u64, u64>::from_iter_perfect(
                    entries.iter().copied(),
                )
                .unwrap();
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if pmml.get(&k).is_none() {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("PerfectSetMultilevelBucketed", n),
            &miss_keys,
            |b, ks| {
                let psml =
                    PerfectSetMultilevelBucketed::<u64>::from_iter_perfect(keys.iter().copied())
                        .unwrap();
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if !psml.contains(&k) {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("hashbrown", n), &miss_keys, |b, ks| {
            let hb: hashbrown::HashMap<u64, u64> = entries.iter().copied().collect();
            b.iter(|| {
                let mut count = 0u64;
                for &k in ks {
                    if hb.get(&k).is_none() {
                        count += 1;
                    }
                }
                black_box(count);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("HighTag128_Tomb", n),
            &miss_keys,
            |b, ks| {
                let mut tomb = HighTag128_TombMap::<u64, u64>::with_capacity(n);
                for &(k, v) in &entries {
                    tomb.insert(k, v);
                }
                b.iter(|| {
                    let mut count = 0u64;
                    for &k in ks {
                        if tomb.get(&k).is_none() {
                            count += 1;
                        }
                    }
                    black_box(count);
                });
            },
        );
    }

    group.finish();
}

// ── String hit lookup ─────────────────────────────────────────────────────

fn bench_lookup_hit_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("perfect/lookup_hit_str");

    for &n in SIZES_LOOKUP_STRING {
        let random_keys = make_random_keys(n, 42);
        let keys: Vec<String> = random_keys.iter().map(|k| format!("{k:020}")).collect();
        let entries: Vec<(String, u64)> = keys
            .iter()
            .cloned()
            .zip(random_keys.iter().copied())
            .map(|(s, k)| (s, k.wrapping_mul(31)))
            .collect();

        group.throughput(Throughput::Elements(n as u64));
        group.sample_size(sample_size_for(n));
        cap_large_lookup_timing(&mut group, n);

        group.bench_with_input(BenchmarkId::new("PerfectMap", n), &keys, |b, ks| {
            let pm = PerfectMap::<String, u64>::from_iter_perfect(entries.iter().cloned()).unwrap();
            b.iter(|| {
                let mut sum = 0u64;
                for k in ks {
                    sum = sum.wrapping_add(*pm.get(k).unwrap_or(&0));
                }
                black_box(sum);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("PerfectMapUnchecked", n),
            &keys,
            |b, ks| {
                let pmu =
                    PerfectMapUnchecked::<String, u64>::from_iter_perfect(entries.iter().cloned())
                        .unwrap();
                b.iter(|| {
                    let mut sum = 0u64;
                    for k in ks {
                        sum = sum.wrapping_add(*pmu.get_unchecked(k));
                    }
                    black_box(sum);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("hashbrown", n), &keys, |b, ks| {
            let hb: hashbrown::HashMap<String, u64> = entries.iter().cloned().collect();
            b.iter(|| {
                let mut sum = 0u64;
                for k in ks {
                    sum = sum.wrapping_add(*hb.get(k).unwrap_or(&0));
                }
                black_box(sum);
            });
        });

        group.bench_with_input(BenchmarkId::new("HighTag128_Tomb", n), &keys, |b, ks| {
            let mut tomb = HighTag128_TombMap::<String, u64>::with_capacity(n);
            for (k, v) in &entries {
                tomb.insert(k.clone(), *v);
            }
            b.iter(|| {
                let mut sum = 0u64;
                for k in ks {
                    sum = sum.wrapping_add(*tomb.get(k).unwrap_or(&0));
                }
                black_box(sum);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_construction,
    bench_lookup_hit,
    bench_lookup_miss,
    bench_lookup_hit_string
);
criterion_main!(benches);
