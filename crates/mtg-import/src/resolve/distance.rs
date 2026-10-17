//! Bounded edit distance used for fuzzy card-name suggestions.

/// Edit distance between `a` and `b` (insert, delete, substitute, swap neighbours), or `None` above `max`.
pub fn bounded_distance(a: &[char], b: &[char], max: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let width = b.len() + 1;
    let mut two_back = vec![0; width];
    let mut previous: Vec<usize> = (0..width).collect();
    let mut current = vec![0; width];

    for (i, &ca) in a.iter().enumerate() {
        current[0] = i + 1;
        let mut row_min = current[0];
        for (j, &cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            let mut best = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
            if i > 0 && j > 0 && ca == b[j - 1] && a[i - 1] == cb {
                best = best.min(two_back[j - 1] + 1);
            }
            current[j + 1] = best;
            row_min = row_min.min(best);
        }
        if row_min > max {
            return None;
        }
        std::mem::swap(&mut two_back, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }
    let distance = previous[b.len()];
    (distance <= max).then_some(distance)
}

#[cfg(test)]
mod tests {
    //! Goal: the bounded distance agrees with plain edit distance below the bound and refuses everything above it.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn distance(a: &str, b: &str) -> Option<usize> {
        bounded_distance(&chars(a), &chars(b), usize::MAX / 4)
    }

    /// Straightforward full-matrix reference implementation.
    fn reference(a: &[char], b: &[char]) -> usize {
        let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
        for (i, row) in d.iter_mut().enumerate() {
            row[0] = i;
        }
        for (j, cell) in d[0].iter_mut().enumerate() {
            *cell = j;
        }
        for i in 1..=a.len() {
            for j in 1..=b.len() {
                let mut best = (d[i - 1][j] + 1)
                    .min(d[i][j - 1] + 1)
                    .min(d[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]));
                if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                    best = best.min(d[i - 2][j - 2] + 1);
                }
                d[i][j] = best;
            }
        }
        d[a.len()][b.len()]
    }

    #[rstest]
    #[case("", "", 0)]
    #[case("a", "", 1)]
    #[case("", "abc", 3)]
    #[case("same", "same", 0)]
    #[case("kitten", "sitting", 3)]
    #[case("ab", "ba", 1)]
    #[case("lightning bolt", "lightning blot", 1)]
    #[case("lightning bolt", "ligthning bolt", 1)]
    #[case("lightning bolt", "lightnin bolt", 1)]
    #[case("lightning bolt", "lightningbolt", 1)]
    #[case("abc", "xyz", 3)]
    #[case("ca", "abc", 3)]
    fn known_distances(#[case] a: &str, #[case] b: &str, #[case] expected: usize) {
        assert_eq!(distance(a, b), Some(expected));
        assert_eq!(distance(b, a), Some(expected));
    }

    #[rstest]
    #[case("abc", "abd", 0, None)]
    #[case("abc", "abd", 1, Some(1))]
    #[case("abc", "abcdef", 2, None)]
    #[case("abc", "abcdef", 3, Some(3))]
    #[case("kitten", "sitting", 2, None)]
    #[case("kitten", "sitting", 3, Some(3))]
    fn respects_the_bound(
        #[case] a: &str,
        #[case] b: &str,
        #[case] max: usize,
        #[case] expected: Option<usize>,
    ) {
        assert_eq!(bounded_distance(&chars(a), &chars(b), max), expected);
    }

    proptest! {
        #[test]
        fn matches_reference_and_bound(a in "[a-d]{0,12}", b in "[a-d]{0,12}", max in 0usize..8) {
            let (ca, cb) = (chars(&a), chars(&b));
            let exact = reference(&ca, &cb);
            let expected = (exact <= max).then_some(exact);
            prop_assert_eq!(bounded_distance(&ca, &cb, max), expected);
        }

        #[test]
        fn is_symmetric_and_zero_only_for_equal(a in "\\PC{0,10}", b in "\\PC{0,10}") {
            let forward = distance(&a, &b);
            prop_assert_eq!(forward, distance(&b, &a));
            prop_assert_eq!(forward == Some(0), a == b);
        }

        #[test]
        fn never_exceeds_longer_length(a in "\\PC{0,10}", b in "\\PC{0,10}") {
            let longest = a.chars().count().max(b.chars().count());
            prop_assert!(distance(&a, &b).is_some_and(|d| d <= longest));
        }
    }
}
