//! Platform-independent hashing used for state fingerprints.

use std::hash::{Hash, Hasher};

const MULTIPLIER: u64 = 0x517c_c1b7_2722_0a95;

/// A fixed-algorithm hasher: integers are widened and fed as little-endian words.
#[derive(Debug, Default, Clone)]
pub struct StableHasher(u64);

impl StableHasher {
    fn mix(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(MULTIPLIER);
    }
}

impl Hasher for StableHasher {
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for chunk in chunks {
            self.mix(u64::from_le_bytes(*chunk));
        }
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.mix(u64::from_le_bytes(word) ^ ((rest.len() as u64) << 56));
        }
    }

    fn write_u8(&mut self, i: u8) {
        self.mix(u64::from(i));
    }

    fn write_u16(&mut self, i: u16) {
        self.mix(u64::from(i));
    }

    fn write_u32(&mut self, i: u32) {
        self.mix(u64::from(i));
    }

    fn write_u64(&mut self, i: u64) {
        self.mix(i);
    }

    fn write_u128(&mut self, i: u128) {
        self.write(&i.to_le_bytes());
    }

    fn write_usize(&mut self, i: usize) {
        self.mix(i as u64);
    }

    fn finish(&self) -> u64 {
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// Hashes `value` with [`StableHasher`].
pub fn stable_hash<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut hasher = StableHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    //! Goal: the state fingerprint is deterministic and sensitive to every input it covers.
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn known_values_are_pinned() {
        // Pinned so an accidental algorithm change (which would orphan stored fingerprints) fails loudly.
        assert_eq!(stable_hash("abc"), 17_505_106_049_313_230_061);
        assert_ne!(stable_hash(&0u64), stable_hash(&1u64));
    }

    #[test]
    fn length_prefix_separates_concatenations() {
        assert_ne!(
            stable_hash(&vec![1u8, 2]),
            stable_hash(&vec![vec![1u8], vec![2u8]])
        );
        assert_ne!(stable_hash(&("ab", "c")), stable_hash(&("a", "bc")));
    }

    proptest! {
        #[test]
        fn equal_inputs_hash_equal_and_single_changes_are_detected(
            xs in prop::collection::vec(any::<u32>(), 1..40), idx in any::<prop::sample::Index>()
        ) {
            prop_assert_eq!(stable_hash(&xs), stable_hash(&xs.clone()));
            let mut ys = xs.clone();
            let i = idx.index(ys.len());
            ys[i] = ys[i].wrapping_add(1);
            prop_assert_ne!(stable_hash(&xs), stable_hash(&ys));
        }
    }
}
