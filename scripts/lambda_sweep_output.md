# Lambda Sweep — Bucketed Perfect-Hash Family

**Date:** 2026-06-12
**SLOTS_PER_BUCKET:** 16
**N sizes:** 100K, 1M, 10M
**Single-level λ:** 6, 8, 10, 12
**Multi-level:** λ₀ ∈ 6, 8, 10, 12, 14, λ₁ = 4
**Hasher:** foldhash::fast::RandomState

## Multi-Level Results (λ₁ = 4)

| λ₀ | N | PHF | T(PHF) | Buckets L0 | Buckets L1 | Overflow | Has L1 | PHF bits/K | Map | T(Map) | Bytes | B/K | Tag b/K |
|----|---|-----|--------|------------|------------|----------|--------|------------|-----|--------|-------|-----|---------|
| 6 | 100K | OK | 1.2 ms | 16667 | 9 | 2 (0.0%) | ✓ | 0.2 | OK | 1.9 ms | 4.3 MiB | 45.4 | 21.3 |
| 6 | 1M | OK | 16.9 ms | 166667 | 152 | 35 (0.0%) | ✓ | 0.2 | OK | 39.5 ms | 43.3 MiB | 45.4 | 21.4 |
| 6 | 10M | OK | 220.0 ms | 1666667 | 1246 | 285 (0.0%) | ✓ | 0.2 | OK | 538.1 ms | 432.9 MiB | 45.4 | 21.3 |
| 8 | 100K | OK | 1.2 ms | 12500 | 181 | 41 (0.3%) | ✓ | 0.1 | OK | 1.7 ms | 3.3 MiB | 34.6 | 16.3 |
| 8 | 1M | OK | 14.8 ms | 125000 | 1921 | 434 (0.3%) | ✓ | 0.1 | OK | 34.6 ms | 33.0 MiB | 34.6 | 16.3 |
| 8 | 10M | OK | 223.3 ms | 1250000 | 20534 | 4635 (0.4%) | ✓ | 0.1 | OK | 508.7 ms | 329.6 MiB | 34.6 | 16.3 |
| 10 | 100K | OK | 1.3 ms | 10000 | 1328 | 296 (3.0%) | ✓ | 0.1 | OK | 2.0 ms | 2.9 MiB | 30.5 | 14.4 |
| 10 | 1M | OK | 16.1 ms | 100000 | 12301 | 2725 (2.7%) | ✓ | 0.1 | OK | 29.6 ms | 29.1 MiB | 30.5 | 14.4 |
| 10 | 10M | OK | 222.4 ms | 1000000 | 122108 | 27092 (2.7%) | ✓ | 0.1 | OK | 557.8 ms | 291.0 MiB | 30.5 | 14.4 |
| 12 | 100K | OK | 1.6 ms | 8334 | 3953 | 863 (10.4%) | ✓ | 0.1 | OK | 2.4 ms | 3.2 MiB | 33.1 | 15.6 |
| 12 | 1M | OK | 22.3 ms | 83334 | 39249 | 8521 (10.2%) | ✓ | 0.1 | OK | 37.3 ms | 31.7 MiB | 33.2 | 15.6 |
| 12 | 10M | OK | 270.1 ms | 833334 | 387377 | 84062 (10.1%) | ✓ | 0.1 | OK | 529.0 ms | 316.8 MiB | 33.2 | 15.6 |
| 14 | 100K | OK | 1.9 ms | 7143 | 8232 | 1737 (24.3%) | ✓ | 0.1 | OK | 3.1 ms | 4.0 MiB | 41.6 | 19.6 |
| 14 | 1M | OK | 24.7 ms | 71429 | 82438 | 17382 (24.3%) | ✓ | 0.1 | OK | 44.0 ms | 39.9 MiB | 41.9 | 19.7 |
| 14 | 10M | OK | 335.1 ms | 714286 | 826434 | 174226 (24.4%) | ✓ | 0.1 | OK | 638.7 ms | 399.1 MiB | 41.8 | 19.7 |

## Single-Level Results

| λ | N | PHF | T(PHF) | Buckets | PHF bits/K | Map | T(Map) | Bytes | B/K | Tag b/K |
|---|----|-----|--------|---------|------------|-----|--------|-------|-----|---------|
| 6 | 100K | OK | 3.3 ms | 16667 | 0.0 | OK | 2.8 ms | 4.3 MiB | 45.3 | 21.3 |
| 6 | 1M | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 6 | 10M | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 8 | 100K | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 8 | 1M | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 8 | 10M | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 10 | 100K | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 10 | 1M | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 10 | 10M | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 12 | 100K | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 12 | 1M | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |
| 12 | 10M | FAIL | 0.00 ms | 0 | 0.0 | FAIL | 0.00 ms | 0 B | — | 0.0 |

## Failures

| λ₀ | λ₁ | N | Component | Error |
|----|----|---|-----------|-------|
| 6 | — | 1000000 | PHF | PHF: Exhausted |
| 6 | — | 10000000 | PHF | PHF: Exhausted |
| 8 | — | 100000 | PHF | PHF: Exhausted |
| 8 | — | 1000000 | PHF | PHF: Exhausted |
| 8 | — | 10000000 | PHF | PHF: Exhausted |
| 10 | — | 100000 | PHF | PHF: Exhausted |
| 10 | — | 1000000 | PHF | PHF: Exhausted |
| 10 | — | 10000000 | PHF | PHF: Exhausted |
| 12 | — | 100000 | PHF | PHF: Exhausted |
| 12 | — | 1000000 | PHF | PHF: Exhausted |
| 12 | — | 10000000 | PHF | PHF: Exhausted |

## Recommendation

### Reliability Summary

Highest λ that succeeds at each N:

| N | Single-level λ | Multi-level λ₀ |
|---|:---:|:---:|
| 100K | 6 | 14 |
| 1M | ≤4 (default) | 14 |
| 10M | ≤4 (default) | 14 |

**Key finding:** The single-level bucketed PHF exhausts its 64-seed retry budget
for λ ≥ 6 at N ≥ 1M. Only λ = 4 (the current default) and N = 100K at λ = 6
build successfully. The multi-level design avoids this by accepting overflow
at level 0 (fixed seed) and retrying only at level 1 where the key count is
much smaller. Multi-level succeeds across the full sweep range.

### Memory Comparison (N = 100K, λ/λ₀ = 6)

| Variant | Bytes/key | Tag bits/key | Build time |
|---------|-----------|--------------|------------|
| Single λ=6 | 45.3 | 21.3 | 2.8 ms |
| Multi λ₀=6 | 45.4 | 21.3 | 1.9 ms |

Multi-level is 31.9% faster than single-level (fixed L0 seed vs seed retry).
Both variants are within 1% of memory parity.

### Multi-Level Memory Scaling (N = 10M)

| λ₀ | Bytes/key | Tag bits/K | Overflow % | Build time |
|-----|-----------|------------|------------|------------|
| 6 | 45.4 | 21.3 | 0.0% | 538.1 ms |
| 8 | 34.6 | 16.3 | 0.4% | 508.7 ms |
| 10 | 30.5 | 14.4 | 2.7% | 557.8 ms |
| 12 | 33.2 | 15.6 | 10.1% | 529.0 ms |
| 14 | 41.8 | 19.7 | 24.4% | 638.7 ms |

### Tuning Advice

- **Single-level** is reliable only at λ ≤ 4 with the current 64-seed retry budget.
  At λ = 4 and N = 10M: ~2.5M buckets, ~32 tag bits/key (default).

- **Multi-level** is the safe choice for λ₀ ≥ 6 at any N.
  - λ₀ = 6 is near memory parity with single-level λ = 4 but works reliably.
  - λ₀ = 8 is the sweet spot: ~35 B/K, 16 tag bits/K, < 0.5% overflow.
  - λ₀ = 10 is aggressive: ~30 B/K but 2.7% overflow to level 1.
  - λ₀ = 12–14 trades memory for significant level-1 overhead (10–25% overflow).
  λ₀ = 14 at N=10M: ~42 B/K (level-1 dominates), no memory advantage over λ₀=8.

- **Recommendation for default:** Keep single-level λ = 4 as the safe default.
  For the multi-level family, λ₀ = 8 with λ₁ = 4 gives the best balance of
  memory efficiency (~35 B/K at N=10M) and low construction complexity.
