//! Bounded, in-memory deduplication for polled contract events.
//!
//! RPC polling may replay a ledger after a re-org, or two pollers may overlap.
//! [`SlidingWindowDedupFilter`] keeps recent event sequence numbers in an exact
//! hash table and uses a Bloom filter to avoid most hash table lookups. The
//! exact table is authoritative, so false positives never drop a new event.

use std::collections::{hash_map::DefaultHasher, HashSet, VecDeque};
use std::hash::{Hash, Hasher};

/// A bounded sliding-window filter for contract-event sequence numbers.
///
/// `check_and_insert` returns `true` when the sequence number was already
/// observed in the current window and should therefore be suppressed.
#[derive(Debug)]
pub struct SlidingWindowDedupFilter {
    capacity: usize,
    bloom_bits: Vec<u64>,
    seen: HashSet<u64>,
    window: VecDeque<u64>,
}

impl SlidingWindowDedupFilter {
    /// Creates a filter retaining at most `capacity` sequence numbers.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "deduplication window capacity must be positive");
        let bloom_words = capacity.saturating_mul(16).max(64).div_ceil(64);
        Self {
            capacity,
            bloom_bits: vec![0; bloom_words],
            seen: HashSet::with_capacity(capacity),
            window: VecDeque::with_capacity(capacity),
        }
    }

    /// Returns whether `sequence` is already retained, recording new values.
    pub fn check_and_insert(&mut self, sequence: u64) -> bool {
        if self.might_contain(sequence) && self.seen.contains(&sequence) {
            return true;
        }

        self.set_bloom_bits(sequence);
        self.seen.insert(sequence);
        self.window.push_back(sequence);

        if self.window.len() > self.capacity {
            let expired = self.window.pop_front().expect("window is non-empty");
            self.seen.remove(&expired);
            // A regular Bloom filter cannot remove individual values safely.
            self.rebuild_bloom();
        }
        false
    }

    /// Number of sequence numbers currently retained.
    pub fn len(&self) -> usize {
        self.window.len()
    }

    /// Whether no sequence numbers are currently retained.
    pub fn is_empty(&self) -> bool {
        self.window.is_empty()
    }

    fn rebuild_bloom(&mut self) {
        self.bloom_bits.fill(0);
        let retained: Vec<u64> = self.window.iter().copied().collect();
        for sequence in retained {
            self.set_bloom_bits(sequence);
        }
    }

    fn might_contain(&self, sequence: u64) -> bool {
        self.indices(sequence).into_iter().all(|index| {
            self.bloom_bits[index / 64] & (1_u64 << (index % 64)) != 0
        })
    }

    fn set_bloom_bits(&mut self, sequence: u64) {
        for index in self.indices(sequence) {
            self.bloom_bits[index / 64] |= 1_u64 << (index % 64);
        }
    }

    fn indices(&self, sequence: u64) -> [usize; 3] {
        let bit_len = self.bloom_bits.len() * 64;
        let first = hash(sequence, 0x9e37_79b9_7f4a_7c15);
        let second = hash(sequence, 0xc2b2_ae3d_27d4_eb4f) | 1;
        [
            (first as usize) % bit_len,
            (first.wrapping_add(second) as usize) % bit_len,
            (first.wrapping_add(second.wrapping_mul(2)) as usize) % bit_len,
        ]
    }
}

fn hash(sequence: u64, salt: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    salt.hash(&mut hasher);
    sequence.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::SlidingWindowDedupFilter;

    #[test]
    fn drops_a_duplicate_in_the_active_window() {
        let mut filter = SlidingWindowDedupFilter::new(3);
        assert!(!filter.check_and_insert(42));
        assert!(filter.check_and_insert(42));
        assert_eq!(filter.len(), 1);
    }

    #[test]
    fn accepts_an_event_after_it_falls_out_of_the_window() {
        let mut filter = SlidingWindowDedupFilter::new(2);
        assert!(!filter.check_and_insert(1));
        assert!(!filter.check_and_insert(2));
        assert!(!filter.check_and_insert(3));
        assert_eq!(filter.len(), 2);
        assert!(!filter.check_and_insert(1));
    }

    #[test]
    fn accepts_bloom_collisions_not_present_in_the_hash_table() {
        let mut filter = SlidingWindowDedupFilter::new(1);
        assert!(!filter.check_and_insert(7));
        filter.bloom_bits.fill(u64::MAX);
        assert!(!filter.check_and_insert(8));
    }

    #[test]
    #[should_panic(expected = "capacity must be positive")]
    fn rejects_a_zero_sized_window() {
        SlidingWindowDedupFilter::new(0);
    }
}
