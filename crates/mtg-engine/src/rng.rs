//! Seeded, version-stable randomness: `ChaCha8` substreams with our own range and shuffle code.

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// A `ChaCha8` stream derived from `(seed, index)`.
///
/// Every random decision in a game (first player, each shuffle) uses its own substream selected by a
/// counter stored in game state, so the engine is a pure function of state and replays need no RNG.
/// Range reduction and shuffling are implemented here rather than taken from `rand`, whose
/// algorithms are not value-stable across releases.
#[derive(Debug, Clone)]
pub struct GameRng {
    inner: ChaCha8Rng,
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl GameRng {
    /// Creates the substream `index` of the generator keyed by `seed`.
    pub fn substream(seed: u64, index: u64) -> Self {
        let mut sm = seed;
        let mut key = [0u8; 32];
        for chunk in key.as_chunks_mut::<8>().0 {
            *chunk = splitmix64(&mut sm).to_le_bytes();
        }
        let mut inner = ChaCha8Rng::from_seed(key);
        inner.set_stream(index);
        GameRng { inner }
    }

    /// Next raw 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }

    /// Uniform value in `0..bound` without modulo bias; `bound` of 0 yields 0.
    pub fn below(&mut self, bound: u64) -> u64 {
        if bound <= 1 {
            return 0;
        }
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let x = self.next_u64();
            if x >= threshold {
                return x % bound;
            }
        }
    }

    /// Uniform index into a collection of `len` items (0 when empty).
    pub fn index(&mut self, len: usize) -> usize {
        usize::try_from(self.below(len as u64)).unwrap_or(0)
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.index(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: randomness is reproducible per (seed, index), independent across substreams and unbiased.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[test]
    fn same_seed_and_index_repeat_exactly() {
        let a: Vec<u64> = (0..16)
            .map({
                let mut r = GameRng::substream(42, 3);
                move |_| r.next_u64()
            })
            .collect();
        let b: Vec<u64> = (0..16)
            .map({
                let mut r = GameRng::substream(42, 3);
                move |_| r.next_u64()
            })
            .collect();
        assert_eq!(a, b);
    }

    #[rstest]
    #[case(1, 0, 1, 1)]
    #[case(1, 0, 2, 0)]
    #[case(1, 5, 2, 5)]
    #[case(1, 5, 1, 6)]
    fn different_seed_or_index_diverge(
        #[case] seed_a: u64,
        #[case] idx_a: u64,
        #[case] seed_b: u64,
        #[case] idx_b: u64,
    ) {
        let first = GameRng::substream(seed_a, idx_a).next_u64();
        let second = GameRng::substream(seed_b, idx_b).next_u64();
        assert_ne!(first, second);
    }

    #[test]
    fn output_is_pinned_across_versions() {
        let mut deck: Vec<u32> = (0..10).collect();
        GameRng::substream(7, 0).shuffle(&mut deck);
        assert_eq!(deck, PINNED_SHUFFLE);
    }

    const PINNED_SHUFFLE: [u32; 10] = [9, 1, 5, 2, 3, 0, 4, 8, 7, 6];

    #[test]
    fn below_is_roughly_uniform() {
        let mut rng = GameRng::substream(9, 0);
        let mut counts = [0u32; 6];
        for _ in 0..6000 {
            counts[rng.index(6)] += 1;
        }
        assert!(
            counts.iter().all(|&c| (800..1200).contains(&c)),
            "{counts:?}"
        );
    }

    proptest! {
        #[test]
        fn shuffle_is_a_permutation(seed in any::<u64>(), len in 0usize..80) {
            let mut v: Vec<usize> = (0..len).collect();
            GameRng::substream(seed, 0).shuffle(&mut v);
            v.sort_unstable();
            prop_assert_eq!(v, (0..len).collect::<Vec<_>>());
        }

        #[test]
        fn below_stays_in_range(seed in any::<u64>(), bound in 1u64..1000) {
            let mut rng = GameRng::substream(seed, 1);
            for _ in 0..20 {
                prop_assert!(rng.below(bound) < bound);
            }
        }
    }
}
