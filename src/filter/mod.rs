//! Approximate membership query filters (AMQ / probabilistic set).
//!
//! Two filter types sharing the same Graf-Lemire segmented construction:
//!
//! | Filter | Construction | Best at |
//! |--------|:------------:|---------|
//! | [`BinaryFuse8`] | Peel-queue (O(n)) | Fast build, ~9 bits/key |
//! | [`Xor8`] | Sorted-peel (O(n log n)) | Same output, simpler retry |
//!
//! Keys are pre-hashed `u64` values; hashing is the caller's responsibility.
//! Internal position mapping uses splitmix64 mixing with per-segment seeds.
//!
//! ## Quick start
//!
//! ```
//! use optimap::BinaryFuse8;
//! use std::hash::{BuildHasher, Hasher};
//!
//! let keys: Vec<u64> = (0..1000u64)
//!     .map(|k| {
//!         let mut h = foldhash::fast::FixedState::with_seed(42).build_hasher();
//!         h.write(&k.to_le_bytes());
//!         h.finish()
//!     })
//!     .collect();
//!
//! let filter = BinaryFuse8::build(&keys);
//! assert!(filter.contains(keys[0]));
//! assert!(!filter.contains(u64::MAX));
//! ```

/// Splitmix64 finalizer — avalanche mixing.
#[inline(always)]
fn mix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

/// Map hash to position in `[0, segment_length)` using seed.
/// `segment_length` is always a power of 2, so modulo is a bitmask.
#[inline(always)]
fn hash_to_position(hash: u64, seed: u64, segment_length: u32) -> usize {
    let mixed = mix64(hash ^ seed);
    (mixed as u32 & (segment_length - 1)) as usize
}

/// 8-bit fingerprint from hash (different mixing so independent of position).
#[inline(always)]
fn fingerprint(hash: u64) -> u8 {
    mix64(hash.wrapping_mul(0x9e3779b97f4a7c15)) as u8
}

/// Compute segment length from `n` (number of keys).
///
/// Each segment is a power of 2 so that `hash_to_position` uses a bitmask
/// instead of a division.
fn compute_segment_length(n: usize) -> u32 {
    if n == 0 {
        return 0;
    }
    let total_needed = ((1.23 * n as f64).floor() as usize) + 32;
    let total = total_needed + 2;
    let seg = (total / 3) + if !total.is_multiple_of(3) { 1 } else { 0 };
    seg.next_power_of_two() as u32
}

// ── BinaryFuse8 ───────────────────────────────────────────────────────────

/// A Graf-Lemire BinaryFuse8 filter: ~9 bits/key, O(n) construction via peel-queue.
#[derive(Clone)]
pub struct BinaryFuse8 {
    fingerprints: Box<[u8]>,
    segment_length: u32,
    seed: u64,
    n: usize,
}

impl BinaryFuse8 {
    /// Build from pre-hashed keys. Retries with different seeds if construction fails.
    pub fn build(hashes: &[u64]) -> Self {
        for seed in 0u64.. {
            if let Some(filter) = Self::try_build_with_seed(hashes, seed) {
                return filter;
            }
        }
        unreachable!("construction eventually succeeds")
    }

    fn try_build_with_seed(hashes: &[u64], seed: u64) -> Option<Self> {
        let n = hashes.len();
        if n == 0 {
            return Some(Self {
                fingerprints: Box::new([]),
                segment_length: 0,
                seed: 0,
                n: 0,
            });
        }

        let sl = compute_segment_length(n);
        let array_len = 3 * sl as usize;

        let mut counts = vec![0u16; array_len];
        let mut xor_sums: Vec<usize> = vec![0; array_len];

        // Pre-compute all positions.
        let mut h0 = vec![0usize; n];
        let mut h1 = vec![0usize; n];
        let mut h2 = vec![0usize; n];

        for i in 0..n {
            let hash = hashes[i];
            let s0 = seed;
            let s1 = seed.wrapping_add(1);
            let s2 = seed.wrapping_add(2);
            h0[i] = hash_to_position(hash, s0, sl);
            h1[i] = sl as usize + hash_to_position(hash, s1, sl);
            h2[i] = 2 * sl as usize + hash_to_position(hash, s2, sl);
            counts[h0[i]] += 1;
            counts[h1[i]] += 1;
            counts[h2[i]] += 1;
            xor_sums[h0[i]] ^= i;
            xor_sums[h1[i]] ^= i;
            xor_sums[h2[i]] ^= i;
        }

        // Track which keys have been peeled to avoid re-processing duplicates.
        let mut peeled = vec![false; n];

        // Initialize queue with keys that have a degree-1 position.
        let mut queue = std::collections::VecDeque::with_capacity(n);
        for i in 0..n {
            if counts[h0[i]] == 1 || counts[h1[i]] == 1 || counts[h2[i]] == 1 {
                queue.push_back(i);
            }
        }

        // Peel: repeatedly remove keys with a singleton position.
        struct StackEntry {
            key_idx: usize,
            singleton_pos: usize,
        }
        let mut stack: Vec<StackEntry> = Vec::with_capacity(n);

        while let Some(i) = queue.pop_front() {
            if peeled[i] {
                continue;
            }

            // Determine which position is the singleton.
            let which: u8 = if counts[h0[i]] == 1 {
                0
            } else if counts[h1[i]] == 1 {
                1
            } else {
                2
            };

            let positions = [h0[i], h1[i], h2[i]];
            let singleton_pos = positions[which as usize];

            peeled[i] = true;
            stack.push(StackEntry {
                key_idx: i,
                singleton_pos,
            });

            counts[h0[i]] -= 1;
            counts[h1[i]] -= 1;
            counts[h2[i]] -= 1;
            xor_sums[h0[i]] ^= i;
            xor_sums[h1[i]] ^= i;
            xor_sums[h2[i]] ^= i;

            // Check each of the 3 positions for newly-singleton keys.
            for &pos in &positions {
                if counts[pos] == 1 {
                    let remaining = xor_sums[pos];
                    if !peeled[remaining] {
                        queue.push_back(remaining);
                    }
                }
            }
        }

        if stack.len() != n {
            return None;
        }

        // Assign fingerprints in reverse stack order.
        let mut fp = vec![0u8; array_len];

        for entry in stack.iter().rev() {
            let i = entry.key_idx;
            let hash = hashes[i];
            let fp_hash = fingerprint(hash);
            fp[entry.singleton_pos] = fp_hash ^ fp[h0[i]] ^ fp[h1[i]] ^ fp[h2[i]];
        }

        // Verify: every key's triple XOR should equal its fingerprint.
        for i in 0..n {
            let fp_hash = fingerprint(hashes[i]);
            if fp[h0[i]] ^ fp[h1[i]] ^ fp[h2[i]] != fp_hash {
                return None;
            }
        }

        Some(Self {
            fingerprints: fp.into_boxed_slice(),
            segment_length: sl,
            seed,
            n,
        })
    }

    /// Check if the filter probably contains the given hash.
    #[inline]
    pub fn contains(&self, hash: u64) -> bool {
        if self.n == 0 {
            return false;
        }
        let sl = self.segment_length;
        let s0 = self.seed;
        let s1 = self.seed.wrapping_add(1);
        let s2 = self.seed.wrapping_add(2);
        let h0 = hash_to_position(hash, s0, sl);
        let h1 = sl as usize + hash_to_position(hash, s1, sl);
        let h2 = 2 * sl as usize + hash_to_position(hash, s2, sl);
        self.fingerprints[h0] ^ self.fingerprints[h1] ^ self.fingerprints[h2] == fingerprint(hash)
    }

    /// Number of keys in the filter.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Whether the filter contains no keys.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Number of heap bytes used by the fingerprint array.
    pub fn bytes_on_heap(&self) -> usize {
        self.fingerprints.len()
    }

    /// Bits per key stored.
    pub fn bits_per_key(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            (self.fingerprints.len() * 8) as f64 / self.n as f64
        }
    }
}

impl core::fmt::Debug for BinaryFuse8 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BinaryFuse8")
            .field("len", &self.n)
            .field("segment_length", &self.segment_length)
            .field("seed", &self.seed)
            .field("bits_per_key", &self.bits_per_key())
            .finish()
    }
}

// ── Xor8 ──────────────────────────────────────────────────────────────────

/// An Xor8 filter: same layout as BinaryFuse8, but constructed via sorted-peel.
#[derive(Clone)]
pub struct Xor8 {
    fingerprints: Box<[u8]>,
    segment_length: u32,
    seed: u64,
    n: usize,
}

impl Xor8 {
    /// Build from pre-hashed keys. Retries with different seeds if construction fails.
    pub fn build(hashes: &[u64]) -> Self {
        for seed in 0u64.. {
            if let Some(filter) = Self::try_build_with_seed(hashes, seed) {
                return filter;
            }
        }
        unreachable!("construction eventually succeeds")
    }

    fn try_build_with_seed(hashes: &[u64], seed: u64) -> Option<Self> {
        let n = hashes.len();
        if n == 0 {
            return Some(Self {
                fingerprints: Box::new([]),
                segment_length: 0,
                seed: 0,
                n: 0,
            });
        }

        let sl = compute_segment_length(n);
        let array_len = 3 * sl as usize;

        let mut counts = vec![0u16; array_len];
        let mut xor_sums: Vec<usize> = vec![0; array_len];

        let mut h0 = vec![0usize; n];
        let mut h1 = vec![0usize; n];
        let mut h2 = vec![0usize; n];

        for i in 0..n {
            let hash = hashes[i];
            let s0 = seed;
            let s1 = seed.wrapping_add(1);
            let s2 = seed.wrapping_add(2);
            h0[i] = hash_to_position(hash, s0, sl);
            h1[i] = sl as usize + hash_to_position(hash, s1, sl);
            h2[i] = 2 * sl as usize + hash_to_position(hash, s2, sl);
            counts[h0[i]] += 1;
            counts[h1[i]] += 1;
            counts[h2[i]] += 1;
            xor_sums[h0[i]] ^= i;
            xor_sums[h1[i]] ^= i;
            xor_sums[h2[i]] ^= i;
        }

        // Sort keys by min(counts[h0], counts[h1], counts[h2]) ascending.
        let mut indices: Vec<usize> = (0..n).collect();
        indices.sort_by_key(|&i| {
            let (a, b, c) = (counts[h0[i]], counts[h1[i]], counts[h2[i]]);
            a.min(b).min(c)
        });

        struct StackEntry {
            key_idx: usize,
            singleton_pos: usize,
        }
        let mut stack: Vec<StackEntry> = Vec::with_capacity(n);
        let mut visited = vec![false; n];

        // Repeated passes: each pass peels keys with degree-1 positions.
        loop {
            let mut progress = false;
            for &i in &indices {
                if visited[i] {
                    continue;
                }
                let cs = [counts[h0[i]], counts[h1[i]], counts[h2[i]]];
                let min_deg = cs.iter().min().copied().unwrap();
                if min_deg != 1 {
                    continue;
                }

                visited[i] = true;
                progress = true;

                let which: u8 = if counts[h0[i]] == 1 {
                    0
                } else if counts[h1[i]] == 1 {
                    1
                } else {
                    2
                };
                let positions = [h0[i], h1[i], h2[i]];
                let singleton_pos = positions[which as usize];

                stack.push(StackEntry {
                    key_idx: i,
                    singleton_pos,
                });

                counts[h0[i]] -= 1;
                counts[h1[i]] -= 1;
                counts[h2[i]] -= 1;
                xor_sums[h0[i]] ^= i;
                xor_sums[h1[i]] ^= i;
                xor_sums[h2[i]] ^= i;
            }
            if !progress {
                break;
            }
        }

        if stack.len() != n {
            return None;
        }

        // Assign fingerprints in reverse stack order.
        let mut fp = vec![0u8; array_len];

        for entry in stack.iter().rev() {
            let i = entry.key_idx;
            let hash = hashes[i];
            let fp_hash = fingerprint(hash);
            fp[entry.singleton_pos] = fp_hash ^ fp[h0[i]] ^ fp[h1[i]] ^ fp[h2[i]];
        }

        // Verify: every key's triple XOR should equal its fingerprint.
        for i in 0..n {
            let fp_hash = fingerprint(hashes[i]);
            if fp[h0[i]] ^ fp[h1[i]] ^ fp[h2[i]] != fp_hash {
                return None;
            }
        }

        Some(Self {
            fingerprints: fp.into_boxed_slice(),
            segment_length: sl,
            seed,
            n,
        })
    }

    /// Check if the filter probably contains the given hash.
    #[inline]
    pub fn contains(&self, hash: u64) -> bool {
        if self.n == 0 {
            return false;
        }
        let sl = self.segment_length;
        let s0 = self.seed;
        let s1 = self.seed.wrapping_add(1);
        let s2 = self.seed.wrapping_add(2);
        let h0 = hash_to_position(hash, s0, sl);
        let h1 = sl as usize + hash_to_position(hash, s1, sl);
        let h2 = 2 * sl as usize + hash_to_position(hash, s2, sl);
        self.fingerprints[h0] ^ self.fingerprints[h1] ^ self.fingerprints[h2] == fingerprint(hash)
    }

    /// Number of keys in the filter.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Whether the filter contains no keys.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Number of heap bytes used by the fingerprint array.
    pub fn bytes_on_heap(&self) -> usize {
        self.fingerprints.len()
    }

    /// Bits per key stored.
    pub fn bits_per_key(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            (self.fingerprints.len() * 8) as f64 / self.n as f64
        }
    }
}

impl core::fmt::Debug for Xor8 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Xor8")
            .field("len", &self.n)
            .field("segment_length", &self.segment_length)
            .field("seed", &self.seed)
            .field("bits_per_key", &self.bits_per_key())
            .finish()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate n pseudo-random u64 hashes.
    fn random_hashes(n: usize) -> Vec<u64> {
        let mut state = 42u64;
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                state
            })
            .collect()
    }

    #[test]
    fn round_trip_bf8() {
        let hashes = random_hashes(1000);
        let filter = BinaryFuse8::build(&hashes);
        eprintln!(
            "BinaryFuse8 1000-key: bits_per_key={:.2}, seg_len={}",
            filter.bits_per_key(),
            filter.segment_length
        );
        for &h in &hashes {
            assert!(filter.contains(h), "missing key {h}");
        }
    }

    #[test]
    fn round_trip_xor8() {
        let hashes = random_hashes(1000);
        let filter = Xor8::build(&hashes);
        eprintln!(
            "Xor8 1000-key: bits_per_key={:.2}, seg_len={}",
            filter.bits_per_key(),
            filter.segment_length
        );
        for &h in &hashes {
            assert!(filter.contains(h), "missing key {h}");
        }
    }

    #[test]
    fn fpr_check_bf8() {
        let hashes = random_hashes(1000);
        let filter = BinaryFuse8::build(&hashes);

        let inserted: std::collections::HashSet<u64> = hashes.iter().copied().collect();
        let mut state = 9999u64;
        let mut false_positives = 0;
        let test_count = 10000usize;
        for _ in 0..test_count {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            if inserted.contains(&state) {
                continue; // skip any accidental collision with inserted keys
            }
            if filter.contains(state) {
                false_positives += 1;
            }
        }
        let fpr = false_positives as f64 / test_count as f64;
        eprintln!(
            "BinaryFuse8 FPR: {}/{} = {:.6}",
            false_positives, test_count, fpr
        );
        assert!(fpr < 0.01, "BinaryFuse8 FPR {:.4} too high", fpr);
    }

    #[test]
    fn fpr_check_xor8() {
        let hashes = random_hashes(1000);
        let filter = Xor8::build(&hashes);

        let inserted: std::collections::HashSet<u64> = hashes.iter().copied().collect();
        let mut state = 9999u64;
        let mut false_positives = 0;
        let test_count = 10000usize;
        for _ in 0..test_count {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            if inserted.contains(&state) {
                continue;
            }
            if filter.contains(state) {
                false_positives += 1;
            }
        }
        let fpr = false_positives as f64 / test_count as f64;
        eprintln!("Xor8 FPR: {}/{} = {:.6}", false_positives, test_count, fpr);
        assert!(fpr < 0.01, "Xor8 FPR {:.4} too high", fpr);
    }

    #[test]
    fn empty_filter_bf8() {
        let filter = BinaryFuse8::build(&[]);
        assert_eq!(filter.len(), 0);
        assert_eq!(filter.bits_per_key(), 0.0);
        assert!(!filter.contains(42));
        assert!(!filter.contains(0));
        assert_eq!(filter.bytes_on_heap(), 0);
    }

    #[test]
    fn empty_filter_xor8() {
        let filter = Xor8::build(&[]);
        assert_eq!(filter.len(), 0);
        assert_eq!(filter.bits_per_key(), 0.0);
        assert!(!filter.contains(42));
        assert!(!filter.contains(0));
        assert_eq!(filter.bytes_on_heap(), 0);
    }

    #[test]
    fn singleton_bf8() {
        let hashes = vec![0xDEADBEEF_CAFEBABE_u64];
        let filter = BinaryFuse8::build(&hashes);
        assert!(filter.contains(0xDEADBEEF_CAFEBABE));
        assert!(!filter.contains(0x1234_5678_9ABC_DEF0));
    }

    #[test]
    fn singleton_xor8() {
        let hashes = vec![0xDEADBEEF_CAFEBABE_u64];
        let filter = Xor8::build(&hashes);
        assert!(filter.contains(0xDEADBEEF_CAFEBABE));
        assert!(!filter.contains(0x1234_5678_9ABC_DEF0));
    }

    #[test]
    fn determinism_bf8() {
        let hashes = random_hashes(500);
        let filter1 = BinaryFuse8::build(&hashes);
        let filter2 = BinaryFuse8::build(&hashes);
        assert_eq!(filter1.fingerprints, filter2.fingerprints);
        assert_eq!(filter1.seed, filter2.seed);
        assert_eq!(filter1.segment_length, filter2.segment_length);
    }

    #[test]
    fn determinism_xor8() {
        let hashes = random_hashes(500);
        let filter1 = Xor8::build(&hashes);
        let filter2 = Xor8::build(&hashes);
        assert_eq!(filter1.fingerprints, filter2.fingerprints);
        assert_eq!(filter1.seed, filter2.seed);
        assert_eq!(filter1.segment_length, filter2.segment_length);
    }

    #[test]
    fn clone_debug_bf8() {
        let hashes = random_hashes(100);
        let filter = BinaryFuse8::build(&hashes);
        let cloned = filter.clone();
        assert_eq!(filter.fingerprints, cloned.fingerprints);
        assert_eq!(filter.seed, cloned.seed);

        let dbg = format!("{filter:?}");
        assert!(dbg.contains("BinaryFuse8"));
        assert!(dbg.contains("len"));
        assert!(dbg.contains("segment_length"));
        assert!(dbg.contains("seed"));
        assert!(dbg.contains("bits_per_key"));
        assert!(!dbg.contains("fingerprint"));
    }

    #[test]
    fn clone_debug_xor8() {
        let hashes = random_hashes(100);
        let filter = Xor8::build(&hashes);
        let cloned = filter.clone();
        assert_eq!(filter.fingerprints, cloned.fingerprints);
        assert_eq!(filter.seed, cloned.seed);

        let dbg = format!("{filter:?}");
        assert!(dbg.contains("Xor8"));
        assert!(dbg.contains("len"));
        assert!(dbg.contains("segment_length"));
        assert!(dbg.contains("seed"));
        assert!(dbg.contains("bits_per_key"));
        assert!(!dbg.contains("fingerprint"));
    }

    #[test]
    fn large_filter_bf8() {
        let hashes = random_hashes(100_000);
        let filter = BinaryFuse8::build(&hashes);

        // All keys should be present.
        for &h in &hashes {
            assert!(filter.contains(h), "missing key in 100K filter");
        }

        let bpk = filter.bits_per_key();
        eprintln!(
            "BinaryFuse8 100K-key: bits_per_key={bpk:.2}, seg_len={}",
            filter.segment_length
        );
        // bits_per_key depends on power-of-2 segment rounding (~9.84 asymptotically,
        // but can spike to ~16 at specific n just past a power-of-2 boundary).
        assert!(
            bpk < 18.0,
            "BinaryFuse8 bits_per_key = {bpk:.2}, expected < 18"
        );
    }

    #[test]
    fn large_filter_xor8() {
        let hashes = random_hashes(100_000);
        let filter = Xor8::build(&hashes);

        // All keys should be present.
        for &h in &hashes {
            assert!(filter.contains(h), "missing key in 100K filter");
        }

        let bpk = filter.bits_per_key();
        eprintln!(
            "Xor8 100K-key: bits_per_key={bpk:.2}, seg_len={}",
            filter.segment_length
        );
        assert!(bpk < 18.0, "Xor8 bits_per_key = {bpk:.2}, expected < 18");
    }
}
