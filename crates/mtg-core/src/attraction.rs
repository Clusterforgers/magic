//! The lights of an Attraction card (CR 717.1): the numbers from 1 to 6 lit up on it, which differ between printings.

use serde::{Deserialize, Serialize};

/// The numbers lit up on an Attraction, as bits: bit `n` is the number `n` (1 to 6).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Lights(pub u8);

impl Lights {
    /// The lights of these numbers; a number outside 1 to 6 is no light.
    pub fn of(numbers: impl IntoIterator<Item = u32>) -> Self {
        Self(
            numbers
                .into_iter()
                .filter(|number| (1..=6).contains(number))
                .fold(0, |bits, number| bits | (1 << number)),
        )
    }

    /// Whether this number is lit up.
    pub fn lit(self, number: u32) -> bool {
        (1..=6).contains(&number) && self.0 & (1 << number) != 0
    }

    /// The numbers lit up, ascending.
    pub fn numbers(self) -> Vec<u8> {
        (1u8..=6)
            .filter(|number| self.lit(u32::from(*number)))
            .collect()
    }

    /// Whether no number is lit up.
    #[allow(clippy::trivially_copy_pass_by_ref)] // serde hands over a reference
    pub fn is_empty(&self) -> bool {
        self.numbers().is_empty()
    }
}

#[cfg(test)]
mod tests {
    //! Goal: lights hold exactly the numbers from 1 to 6 they are made of, in any order, and nothing else.
    use rstest::rstest;

    use super::Lights;

    #[rstest]
    #[case::two_and_six(&[2, 6], &[2, 6])]
    #[case::three_lights(&[6, 3, 2], &[2, 3, 6])]
    #[case::out_of_range(&[0, 7, 4], &[4])]
    #[case::none(&[], &[])]
    #[case::twice(&[5, 5], &[5])]
    fn the_lights_are_the_numbers_from_one_to_six(#[case] given: &[u32], #[case] lit: &[u8]) {
        let lights = Lights::of(given.iter().copied());
        assert_eq!(lights.numbers(), lit);
        for number in 0..=8 {
            assert_eq!(
                lights.lit(number),
                lit.iter().any(|n| u32::from(*n) == number),
                "{number}"
            );
        }
        assert_eq!(lights.is_empty(), lit.is_empty());
    }
}
