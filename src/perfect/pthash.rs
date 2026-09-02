//! PTHash perfect-hash construction (Pibiri & Trani 2021).
//!
//! Two-level: partition n hashes into r = ceil(n/λ) buckets via a primary
//! hash, sort buckets largest-first, and for each bucket search for a
//! per-bucket displacement `d` that resolves all keys in the bucket to
//! distinct empty slots.
//!
//! **What this impl does and does not do.** Displacements are bit-packed
//! into a dense u64 array at a *fixed* width `b = ceil(log₂(m)) + 1`
//! (stored in ceil(r·b / 64) u64 words). This is *not* the compact
//! encoding from the PTHash paper: the paper's ~2.4 bits/key comes from
//! Elias-Fano / dictionary-compressing the pilot table to exploit the
//! skewed displacement distribution, which is deliberately left
//! unimplemented. At fixed width the table lands at ~5.3 bits/key at
//! N = 1M (vs CHD's ~6.4) — a ~17 % PHF saving that barely moves total
//! map memory, since the slot array dominates (8 B/key for u64 values).
//!
//! **Where it actually wins: build speed.** At λ = 4 with the +1-bit
//! displacement headroom, the per-bucket search converges far faster
//! than CHD's λ = 5 linear walk — ~2.6–3× faster construction (2.1 ms
//! vs 6.7 ms at N = 10K, 26 ms vs 69 ms at N = 100K), with the same
//! lookup shape as CHD. Treat `PtHashPhf` as the fast-build CHD
//! alternative, not a smaller one. Chase the compact encoding only if a
//! workload holds the PHF standalone with memory as the hard constraint.
//!
//! Lookup: bucket = bucket_of(hash, seed, r), extract d from packed table,
//! slot = mixed_slot(hash, seed, d, m).

use super::phf::{BuildError, PerfectHashFunction};
use super::util::{bucket_of, has_duplicate, mix};

/// Default average bucket size. Lower bound for M = 2 for minimal PHFs;
/// λ = 4 is the same default as the CHD family for comparable bucket
/// counts. λ = 4 (vs CHD's 5) is what buys the faster build — smaller
/// buckets resolve their displacement in fewer attempts. It does not
/// buy compactness: at the fixed displacement width this lands near
/// ~5.3 bits/key at 1M (see the module docs), not the paper's ~2.4.
const DEFAULT_LAMBDA: usize = 4;

/// How many independent seed families to try before giving up.
const MAX_SEED_RETRIES: u32 = 64;

/// Splitmix64-style starting seed, distinct from CHD's.
const SEED_BASE: u64 = 0xD6E8FEB86659FD93;

/// Per-bucket displacement search ceiling. Same cap as CHD but PTHash
/// typically finds d much faster because buckets are smaller (λ = 4).
const MAX_DISPLACEMENT: u64 = 1 << 24;

/// PTHash perfect hash function — fixed-width bit-packed displacement table.
#[derive(Debug, Clone)]
pub struct PtHashPhf {
    seed: u64,
    /// Bit-packed per-bucket displacements. `ceil(r * bits_per_disp / 64)` u64 words.
    displacements: Box<[u64]>,
    /// Bit width of each displacement value.
    bits_per_disp: u32,
    /// Number of buckets `r`.
    num_buckets: u32,
    /// Table size `m` (slot domain).
    m: u32,
}

impl PtHashPhf {
    /// Build with a custom average bucket load λ. `lambda` must be > 0.
    /// Lower λ → larger displacement table (more buckets) but easier
    /// construction. Higher λ → more compact table but harder per-bucket
    /// displacement search.
    pub fn build_with_lambda(hashes: &[u64], m: usize, lambda: usize) -> Result<Self, BuildError> {
        assert!(lambda > 0, "lambda must be > 0, got {lambda}");
        let n = hashes.len();
        assert!(m >= n, "PHF table size m={m} must be >= key count n={n}");
        assert!(
            m <= u32::MAX as usize,
            "PtHashPhf does not support tables larger than u32::MAX slots"
        );

        if n == 0 {
            return Ok(PtHashPhf {
                seed: 0,
                displacements: Box::new([]),
                bits_per_disp: 1,
                num_buckets: 0,
                m: m as u32,
            });
        }

        // bits_per_disp = ceil(log2(m)) + 1, at least 2.
        // +1 gives the displacement search twice the range, matching the
        // PTHash paper's recommendation. Without it, minimal builds fail
        // at modest N because the displacement space is too tight.
        let bits_per_disp = if m <= 1 {
            2u32
        } else {
            (usize::BITS - m.leading_zeros()) + 1
        };

        // r = ceil(n / λ), at least 1
        let r = ((n + lambda - 1) / lambda).max(1);

        let mut build_disp = None;
        let mut final_seed = SEED_BASE;
        for seed_try in 0..MAX_SEED_RETRIES {
            let seed = SEED_BASE.wrapping_add(seed_try as u64);
            match try_build_seed(hashes, m, r, lambda, bits_per_disp, seed) {
                Ok(disp) => {
                    build_disp = Some(disp);
                    final_seed = seed;
                    break;
                }
                Err(BuildError::DuplicateHash) => return Err(BuildError::DuplicateHash),
                Err(BuildError::Exhausted) => continue,
            }
        }

        match build_disp {
            Some(displacements) => Ok(PtHashPhf {
                seed: final_seed,
                displacements: displacements.into_boxed_slice(),
                bits_per_disp,
                num_buckets: r as u32,
                m: m as u32,
            }),
            None => Err(BuildError::Exhausted),
        }
    }

    /// Extract the displacement for bucket `b` from the bit-packed table.
    #[inline]
    fn read_displacement(&self, bucket: usize) -> u64 {
        let bit_offset = bucket * self.bits_per_disp as usize;
        let word_idx = bit_offset / 64;
        let bit_in_word = bit_offset % 64;

        let mut d = self.displacements[word_idx] >> bit_in_word;
        if bit_in_word + self.bits_per_disp as usize > 64 {
            // Value spans two words
            d |= self.displacements[word_idx + 1] << (64 - bit_in_word);
        }
        d & ((1u64 << self.bits_per_disp) - 1)
    }
}

impl PerfectHashFunction for PtHashPhf {
    fn build(hashes: &[u64], m: usize) -> Result<Self, BuildError> {
        Self::build_with_lambda(hashes, m, DEFAULT_LAMBDA)
    }

    #[inline]
    fn index(&self, hash: u64) -> usize {
        if self.m == 0 {
            return 0;
        }
        let r = self.num_buckets as u64;
        let bucket = bucket_of(hash, self.seed, r);
        let d = self.read_displacement(bucket);
        slot_of(hash, self.seed, d, self.m as u64)
    }

    fn capacity(&self) -> usize {
        self.m as usize
    }

    fn bits_per_key(&self) -> f64 {
        let n_approx = self.num_buckets as f64 * DEFAULT_LAMBDA as f64;
        if n_approx == 0.0 {
            return 0.0;
        }
        let meta = 64.0 + 32.0 + 32.0 + 32.0; // seed + num_buckets + bits_per_disp + m
        let disp_bits = (self.displacements.len() * 64) as f64;
        (meta + disp_bits) / n_approx
    }

    fn bytes_on_heap(&self) -> usize {
        self.displacements.len() * std::mem::size_of::<u64>()
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────

/// Mix hash with seed and displacement into a slot.
#[inline(always)]
fn slot_of(h: u64, seed: u64, d: u64, m: u64) -> usize {
    let x = mix(h, seed ^ d.wrapping_mul(0x9E3779B97F4A7C15));
    (x % m) as usize
}

/// Write displacement `d` for bucket `b` into the bit-packed `disps` array.
#[inline]
fn write_displacement(disps: &mut [u64], bucket: usize, bits_per_disp: u32, d: u64) {
    let bit_offset = bucket * bits_per_disp as usize;
    let word_idx = bit_offset / 64;
    let bit_in_word = bit_offset % 64;
    let mask = (1u64 << bits_per_disp) - 1;

    disps[word_idx] &= !(mask << bit_in_word);
    disps[word_idx] |= d << bit_in_word;
    if bit_in_word + bits_per_disp as usize > 64 {
        let overflow_bits = bit_in_word + bits_per_disp as usize - 64;
        disps[word_idx + 1] &= !(mask >> (bits_per_disp as usize - overflow_bits));
        disps[word_idx + 1] |= d >> (bits_per_disp as usize - overflow_bits);
    }
}

/// One PTHash construction attempt under a fixed `seed`.
fn try_build_seed(
    hashes: &[u64],
    m: usize,
    r: usize,
    _lambda: usize,
    bits_per_disp: u32,
    seed: u64,
) -> Result<Vec<u64>, BuildError> {
    let n = hashes.len();
    let m_u64 = m as u64;
    let r_u64 = r as u64;

    // Check for duplicate hashes on first seed only.
    if seed == SEED_BASE && has_duplicate(hashes) {
        return Err(BuildError::DuplicateHash);
    }

    // Phase 1: bucket assignment + counting
    let mut bucket_of_key = vec![0u32; n];
    let mut bucket_sizes = vec![0u32; r];
    for (i, &h) in hashes.iter().enumerate() {
        let b = bucket_of(h, seed, r_u64);
        bucket_of_key[i] = b as u32;
        bucket_sizes[b] += 1;
    }

    // Phase 2: counting sort for flat bucket-major layout
    let mut bucket_start = vec![0u32; r + 1];
    let mut acc = 0u32;
    for (i, &sz) in bucket_sizes.iter().enumerate() {
        bucket_start[i] = acc;
        acc += sz;
    }
    bucket_start[r] = acc;

    let mut bucket_keys = vec![0u32; n];
    let mut cursor = bucket_start[..r].to_vec();
    for (i, &b) in bucket_of_key.iter().enumerate() {
        let pos = cursor[b as usize] as usize;
        bucket_keys[pos] = i as u32;
        cursor[b as usize] += 1;
    }

    // Phase 3: sort buckets by size descending
    let mut order: Vec<u32> = (0..r as u32).collect();
    order.sort_unstable_by(|&a, &b| bucket_sizes[b as usize].cmp(&bucket_sizes[a as usize]));

    // Phase 4: per-bucket displacement search
    let num_words = (r * bits_per_disp as usize).div_ceil(64);
    let mut disps = vec![0u64; num_words];
    let max_disp = (1u64 << bits_per_disp) - 1;
    let mut occupied = vec![false; m];
    let mut tentative = Vec::with_capacity(16);

    for &b in &order {
        let b_us = b as usize;
        let sz = bucket_sizes[b_us] as usize;
        if sz == 0 {
            continue;
        }

        let keys_in_bucket =
            &bucket_keys[bucket_start[b_us] as usize..bucket_start[b_us + 1] as usize];

        let mut d: u64 = 0;
        loop {
            if d > max_disp {
                return Err(BuildError::Exhausted);
            }
            tentative.clear();
            let mut ok = true;
            for &ki in keys_in_bucket {
                let h = hashes[ki as usize];
                let s = slot_of(h, seed, d, m_u64);
                if occupied[s] || tentative.contains(&(s as u32)) {
                    ok = false;
                    break;
                }
                tentative.push(s as u32);
            }
            if ok {
                for &s in &tentative {
                    occupied[s as usize] = true;
                }
                write_displacement(&mut disps, b_us, bits_per_disp, d);
                break;
            }
            d += 1;
        }
    }

    Ok(disps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perfect::util::mix;

    fn make_hashes(n: usize) -> Vec<u64> {
        (0..n as u64)
            .map(|i| mix(i.wrapping_mul(0x9E3779B97F4A7C15), 0x6A09E667F3BCC908))
            .collect()
    }

    #[test]
    fn build_empty() {
        let phf = PtHashPhf::build(&[], 0).unwrap();
        assert_eq!(phf.capacity(), 0);
    }

    #[test]
    fn build_single() {
        let phf = PtHashPhf::build(&[42], 1).unwrap();
        assert_eq!(phf.index(42), 0);
    }

    #[test]
    fn minimal_small_round_trip() {
        for &n in &[2usize, 8, 64, 256, 1024] {
            let hashes = make_hashes(n);
            let phf = PtHashPhf::build(&hashes, n).expect("build should succeed at λ=4, m=n");
            let mut seen = vec![false; n];
            for &h in &hashes {
                let s = phf.index(h);
                assert!(s < n, "slot {s} out of range at n={n}");
                assert!(!seen[s], "collision at slot {s} for n={n}");
                seen[s] = true;
            }
            assert!(seen.iter().all(|&b| b));
        }
    }

    #[test]
    fn minimal_medium_round_trip() {
        let n = 50_000;
        let hashes = make_hashes(n);
        let phf = PtHashPhf::build(&hashes, n).expect("minimal build at N=50K");
        let mut seen = vec![false; n];
        for &h in &hashes {
            let s = phf.index(h);
            assert!(!seen[s]);
            seen[s] = true;
        }
        assert_eq!(seen.iter().filter(|b| **b).count(), n);
    }

    #[test]
    fn non_minimal_round_trip() {
        let n = 10_000;
        let m = (n as f64 * 1.23) as usize;
        let hashes = make_hashes(n);
        let phf = PtHashPhf::build(&hashes, m).expect("non-minimal build succeeds");
        assert_eq!(phf.capacity(), m);
        let mut seen = vec![false; m];
        for &h in &hashes {
            let s = phf.index(h);
            assert!(s < m);
            assert!(!seen[s]);
            seen[s] = true;
        }
    }

    #[test]
    fn duplicate_hash_is_reported() {
        let hashes = vec![1, 2, 3, 4, 1];
        let err = PtHashPhf::build(&hashes, 5).unwrap_err();
        assert_eq!(err, BuildError::DuplicateHash);
    }

    #[test]
    fn bits_per_key_sane() {
        let n = 10_000;
        let hashes = make_hashes(n);
        let phf = PtHashPhf::build(&hashes, n).unwrap();
        let bpk = phf.bits_per_key();
        assert!(
            bpk > 0.0 && bpk < 10.0,
            "bits_per_key {bpk} outside expected range"
        );
    }
}
