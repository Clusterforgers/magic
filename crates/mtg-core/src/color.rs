//! Colors and color sets.

use serde::{Deserialize, Serialize};

/// One of the five Magic colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Color {
    /// White.
    White,
    /// Blue.
    Blue,
    /// Black.
    Black,
    /// Red.
    Red,
    /// Green.
    Green,
}

impl Color {
    /// All colors in WUBRG order.
    pub const ALL: [Color; 5] = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];

    /// Position in WUBRG order.
    pub const fn index(self) -> usize {
        self as usize
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }

    /// Parses a WUBRG letter.
    pub const fn from_symbol(symbol: char) -> Option<Color> {
        match symbol {
            'W' => Some(Color::White),
            'U' => Some(Color::Blue),
            'B' => Some(Color::Black),
            'R' => Some(Color::Red),
            'G' => Some(Color::Green),
            _ => None,
        }
    }
}

/// A set of colors (colorless is the empty set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(transparent)]
pub struct ColorSet(u8);

impl ColorSet {
    /// The empty (colorless) set.
    pub const EMPTY: ColorSet = ColorSet(0);

    /// Returns the set with `color` added.
    #[must_use]
    pub const fn with(self, color: Color) -> ColorSet {
        ColorSet(self.0 | color.bit())
    }

    /// Whether `color` is in the set.
    pub const fn contains(self, color: Color) -> bool {
        self.0 & color.bit() != 0
    }

    /// Whether every color of `self` is in `other` (commander color identity check).
    pub const fn is_subset_of(self, other: ColorSet) -> bool {
        self.0 & !other.0 == 0
    }

    /// Number of colors in the set.
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether the set is empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Colors in the set in WUBRG order.
    pub fn iter(self) -> impl Iterator<Item = Color> {
        Color::ALL.into_iter().filter(move |c| self.contains(*c))
    }
}

impl FromIterator<Color> for ColorSet {
    fn from_iter<I: IntoIterator<Item = Color>>(iter: I) -> Self {
        iter.into_iter().fold(ColorSet::EMPTY, ColorSet::with)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: color-set algebra is correct, since color identity validation depends on it.
    use super::*;
    use rstest::rstest;

    fn set(symbols: &str) -> ColorSet {
        symbols.chars().filter_map(Color::from_symbol).collect()
    }

    #[rstest]
    #[case("", "", true)]
    #[case("", "WUBRG", true)]
    #[case("WU", "WUB", true)]
    #[case("WUB", "WU", false)]
    #[case("R", "G", false)]
    #[case("WUBRG", "WUBRG", true)]
    fn subset_relation(#[case] a: &str, #[case] b: &str, #[case] expected: bool) {
        assert_eq!(set(a).is_subset_of(set(b)), expected);
    }

    #[test]
    fn every_color_roundtrips_through_a_set() {
        for color in Color::ALL {
            let s = ColorSet::EMPTY.with(color);
            assert_eq!(s.len(), 1);
            assert!(Color::ALL.iter().filter(|c| s.contains(**c)).eq([&color]));
        }
    }

    #[test]
    fn duplicates_do_not_grow_the_set() {
        assert_eq!(set("WWW").len(), 1);
        assert!(set("").is_empty());
    }
}
