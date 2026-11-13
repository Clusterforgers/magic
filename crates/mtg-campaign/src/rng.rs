//! A small seeded random stream (`SplitMix64`), keyed by the run's seed and a label, so a draw never depends on the
//! order in which other draws happened and a reload never rerolls anything.

/// A deterministic random stream.
#[derive(Debug, Clone)]
pub struct Stream {
    state: u64,
}

impl Stream {
    /// The stream of `seed` for one purpose, named by `label` and two numbers (a node, a seat).
    pub fn new(seed: u64, label: &str, a: u64, b: u64) -> Stream {
        let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
        for byte in label.bytes() {
            state = mix(state ^ u64::from(byte));
        }
        state = mix(state ^ a.wrapping_mul(0xA24B_AED4_963E_E407));
        state = mix(state ^ b.wrapping_mul(0x9FB2_1C65_1E98_DF25));
        Stream { state }
    }

    /// The next number.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.state)
    }

    /// A number below `n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        let n = n as u64;
        usize::try_from(self.next_u64() % n).unwrap_or(0)
    }

    /// Whether a chance of `percent` in 100 comes up.
    pub fn chance(&mut self, percent: u32) -> bool {
        self.next_u64() % 100 < u64::from(percent)
    }

    /// `count` distinct items of `items`, in drawn order.
    pub fn pick<T: Clone>(&mut self, items: &[T], count: usize) -> Vec<T> {
        let mut left: Vec<T> = items.to_vec();
        let mut out = Vec::new();
        while out.len() < count && !left.is_empty() {
            let at = self.below(left.len());
            out.push(left.swap_remove(at));
        }
        out
    }
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    //! Goal: a stream is a function of its seed and keys, differs between keys and picks distinct items.
    use super::*;

    #[test]
    fn streams_repeat_for_the_same_keys_and_differ_otherwise() {
        let draw = |seed, label, a| {
            let mut s = Stream::new(seed, label, a, 0);
            (0..4).map(|_| s.next_u64()).collect::<Vec<_>>()
        };
        assert_eq!(draw(7, "map", 1), draw(7, "map", 1));
        assert_ne!(draw(7, "map", 1), draw(7, "map", 2));
        assert_ne!(draw(7, "map", 1), draw(7, "loot", 1));
        assert_ne!(draw(7, "map", 1), draw(8, "map", 1));
    }

    #[test]
    fn a_pick_is_distinct_and_never_longer_than_the_items() {
        let mut s = Stream::new(1, "pick", 0, 0);
        for count in 0..6 {
            let mut got = s.pick(&[1, 2, 3, 4], count);
            assert_eq!(got.len(), count.min(4));
            got.sort_unstable();
            got.dedup();
            assert_eq!(got.len(), count.min(4));
        }
    }
}
