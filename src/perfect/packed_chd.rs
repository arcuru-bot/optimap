//! Fixed-width packed CHD-style perfect-hash construction.
//!
//! Hashes are partitioned into `r = ceil(n / λ)` buckets, the buckets are
//! processed largest-first, and each bucket receives the first linear
//! displacement `d` that maps all of its keys to distinct empty slots.
//! Lookup reads that displacement from a fixed-width packed table and uses
//! the same mixed-displacement slot function as [`super::ChdPhf`].
//!
//! This is deliberately named for the algorithm it implements. It is not
//! PTHash: it has neither PTHash's pilot/XOR search nor its compact pilot
//! encoding. The separate [`super::ChdPhf::build_with_profile_pilot`]
//! experiment implements the pilot search.
//!
//! The default `λ = 4` trades a larger bucket table for smaller buckets
//! than the default `ChdPhf` configuration. The fixed-width table uses
//! about 5.3 bits/key at one million keys versus about 6.4 bits/key for
//! `ChdPhf`; the slot array still dominates total map memory. These are
//! layout calculations, not allocator measurements or timing results.

use super::phf::{BuildError, PerfectHashFunction};
use super::util::{bucket_of, has_duplicate, mix};

/// Default average bucket size.
///
/// `λ = 4` (versus [`super::ChdPhf`]'s default 5) trades a larger packed
/// displacement table for smaller buckets and fewer construction attempts.
const DEFAULT_LAMBDA: usize = 4;

/// How many independent seed families to try before giving up.
const MAX_SEED_RETRIES: u32 = 1024;

/// Splitmix64-style starting seed, distinct from CHD's.
const SEED_BASE: u64 = 0xD6E8FEB86659FD93;

/// Per-bucket displacement search ceiling.
const MAX_DISPLACEMENT: u64 = 1 << 24;

/// CHD-style perfect hash function with fixed-width packed displacements.
#[derive(Debug, Clone)]
pub struct PackedChdPhf {
    seed: u64,
    /// Bit-packed per-bucket displacements. `ceil(r * bits_per_disp / 64)` u64 words.
    displacements: Box<[u64]>,
    /// Bit width of each displacement value.
    bits_per_disp: u32,
    /// Number of buckets `r`.
    num_buckets: u32,
    /// Number of hashes used to build this PHF.
    num_keys: u32,
    /// Table size `m` (slot domain).
    m: u32,
}

impl PackedChdPhf {
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
            "PackedChdPhf does not support tables larger than u32::MAX slots"
        );
        if has_duplicate(hashes) {
            return Err(BuildError::DuplicateHash);
        }

        if n == 0 {
            return Ok(PackedChdPhf {
                seed: 0,
                displacements: Box::new([]),
                bits_per_disp: 1,
                num_buckets: 0,
                num_keys: 0,
                m: m as u32,
            });
        }

        // bits_per_disp = ceil(log2(m)) + 1. The extra bit gives the
        // displacement search twice the range; without it, minimal builds
        // fail at modest N because the displacement space is too tight.
        let bits_per_disp = displacement_width(m);

        // r = ceil(n / λ), at least 1
        let r = n.div_ceil(lambda).max(1);

        let mut build_disp = None;
        let mut final_seed = SEED_BASE;
        for seed_try in 0..MAX_SEED_RETRIES {
            let seed = SEED_BASE.wrapping_add(seed_try as u64);
            match try_build_seed(hashes, m, r, bits_per_disp, seed) {
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
            Some(displacements) => Ok(PackedChdPhf {
                seed: final_seed,
                displacements: displacements.into_boxed_slice(),
                bits_per_disp,
                num_buckets: r as u32,
                num_keys: n as u32,
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

impl PerfectHashFunction for PackedChdPhf {
    fn build(hashes: &[u64], m: usize) -> Result<Self, BuildError> {
        Self::build_with_lambda(hashes, m, DEFAULT_LAMBDA)
    }

    #[inline]
    fn index(&self, hash: u64) -> usize {
        if self.num_keys == 0 {
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
        if self.num_keys == 0 {
            return 0.0;
        }
        let meta = 64.0 + 32.0 + 32.0 + 32.0 + 32.0; // seed + num_buckets + num_keys + bits_per_disp + m
        let disp_bits = self.displacements.len() as f64 * 64.0;
        (meta + disp_bits) / self.num_keys as f64
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

/// Largest displacement tried for one bucket before retrying with a new seed.
#[inline]
fn displacement_search_limit(bits_per_disp: u32) -> u64 {
    ((1u64 << bits_per_disp) - 1).min(MAX_DISPLACEMENT)
}

/// `ceil(log2(m)) + 1`, with the `m = 1` minimum handled directly.
#[inline]
fn displacement_width(m: usize) -> u32 {
    if m <= 1 { 1 } else { (m - 1).ilog2() + 2 }
}

/// One packed CHD-style construction attempt under a fixed `seed`.
fn try_build_seed(
    hashes: &[u64],
    m: usize,
    r: usize,
    bits_per_disp: u32,
    seed: u64,
) -> Result<Vec<u64>, BuildError> {
    let n = hashes.len();
    let m_u64 = m as u64;
    let r_u64 = r as u64;
    let num_words = r
        .checked_mul(bits_per_disp as usize)
        .expect("packed CHD displacement size overflows usize before allocation")
        .div_ceil(64);
    let num_starts = r
        .checked_add(1)
        .expect("packed CHD bucket count overflows usize before allocation");

    // Phase 1: bucket assignment + counting
    let mut bucket_of_key = vec![0u32; n];
    let mut bucket_sizes = vec![0u32; r];
    for (i, &h) in hashes.iter().enumerate() {
        let b = bucket_of(h, seed, r_u64);
        bucket_of_key[i] = b as u32;
        bucket_sizes[b] += 1;
    }

    // Phase 2: counting sort for flat bucket-major layout
    let mut bucket_start = vec![0u32; num_starts];
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
    let mut disps = vec![0u64; num_words];
    let max_disp = displacement_search_limit(bits_per_disp);
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
        let phf = PackedChdPhf::build(&[], 0).unwrap();
        assert_eq!(phf.capacity(), 0);
    }

    #[test]
    fn empty_with_spare_capacity_has_in_range_index() {
        let phf = PackedChdPhf::build(&[], 8).unwrap();
        assert_eq!(phf.capacity(), 8);
        assert_eq!(phf.bytes_on_heap(), 0);
        assert_eq!(phf.bits_per_key(), 0.0);
        assert!(phf.index(42) < 8);
    }

    #[test]
    fn build_single() {
        let phf = PackedChdPhf::build(&[42], 1).unwrap();
        assert_eq!(phf.index(42), 0);
    }

    #[test]
    fn minimal_small_round_trip() {
        for &n in &[2usize, 8, 64, 256, 1024] {
            let hashes = make_hashes(n);
            let phf = PackedChdPhf::build(&hashes, n).expect("build should succeed at λ=4, m=n");
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
        let phf = PackedChdPhf::build(&hashes, n).expect("minimal build at N=50K");
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
        let phf = PackedChdPhf::build(&hashes, m).expect("non-minimal build succeeds");
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
        let err = PackedChdPhf::build(&hashes, 5).unwrap_err();
        assert_eq!(err, BuildError::DuplicateHash);
    }

    #[test]
    fn bits_per_key_sane() {
        let n = 10_000;
        let hashes = make_hashes(n);
        let phf = PackedChdPhf::build(&hashes, n).unwrap();
        let bpk = phf.bits_per_key();
        assert!(
            bpk > 0.0 && bpk < 10.0,
            "bits_per_key {bpk} outside expected range"
        );
    }

    #[test]
    fn displacement_width_matches_documented_formula() {
        assert_eq!(displacement_width(1), 1);
        assert_eq!(displacement_width(8), 4);
        assert_eq!(displacement_width(9), 5);
    }

    #[test]
    fn displacement_search_is_bounded_before_seed_retry() {
        assert_eq!(displacement_search_limit(4), 15);
        assert_eq!(displacement_search_limit(32), MAX_DISPLACEMENT);
    }

    #[test]
    #[should_panic(expected = "packed CHD displacement size overflows usize before allocation")]
    fn oversized_packed_table_fails_before_allocation() {
        let _ = try_build_seed(&[], 0, usize::MAX, 2, SEED_BASE);
    }

    #[test]
    fn packed_backends_round_trip_and_account_for_owned_arrays() {
        use crate::map::DefaultHashBuilder;
        use crate::perfect::{
            PerfectMap, PerfectMapConfig, PerfectMapSparse, PerfectMapUnchecked, PerfectSet,
        };

        let entries: Vec<(u64, u64)> = (0..32).map(|k| (k, k * 31)).collect();
        let map = PerfectMap::<_, _, PackedChdPhf>::from_entries(
            entries.clone(),
            DefaultHashBuilder::default(),
        )
        .unwrap();
        let unchecked = PerfectMapUnchecked::<_, _, PackedChdPhf>::from_entries(
            entries.clone(),
            DefaultHashBuilder::default(),
        )
        .unwrap();
        let set = PerfectSet::<_, PackedChdPhf>::from_keys(0u64..32, DefaultHashBuilder::default())
            .unwrap();
        let sparse = PerfectMapSparse::<_, _, PackedChdPhf>::from_entries_with(
            entries.clone(),
            DefaultHashBuilder::default(),
            &PerfectMapConfig::default().with_load_factor(1.5),
        )
        .unwrap();
        for (k, v) in &entries {
            assert_eq!(map.get(k), Some(v));
            assert_eq!(unchecked.get_unchecked(k), v);
            assert!(set.contains(k));
            assert_eq!(sparse.get(k), Some(v));
        }
        assert_eq!(map.get(&32), None);
        assert!(!set.contains(&32));
        assert_eq!(sparse.get(&32), None);
        // Eight buckets × six bits occupy one u64; slack widens the
        // sparse displacements to seven bits, still within one u64.
        assert_eq!(map.bytes_used(), 32 * 16 + 8);
        assert_eq!(unchecked.bytes_used(), 32 * 8 + 8);
        assert_eq!(set.bytes_used(), 32 * 8 + 8);
        assert_eq!(
            sparse.bytes_used(),
            48 * std::mem::size_of::<Option<(u64, u64)>>() + 8
        );
    }

    #[test]
    fn large_lambda_does_not_overflow_bucket_rounding() {
        let phf = PackedChdPhf::build_with_lambda(&[42], 1, usize::MAX).unwrap();
        assert_eq!(phf.index(42), 0);
    }

    #[test]
    fn bits_per_key_uses_actual_key_count() {
        let hashes = make_hashes(3);
        let phf = PackedChdPhf::build_with_lambda(&hashes, 3, 4).unwrap();
        let expected = (192.0 + (phf.displacements.len() * 64) as f64) / 3.0;
        assert_eq!(phf.bits_per_key(), expected);
    }
}
