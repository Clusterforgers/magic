//! Conversion between color sets and the bitmasks stored in Postgres.

use mtg_core::color::{Color, ColorSet};

use crate::error::DbError;

/// Highest valid mask: all five colors set.
const MASK_MAX: i16 = 0b1_1111;

/// The bit stored for `color`; fixed by the schema, not by `mtg_core`'s representation.
const fn bit(color: Color) -> i16 {
    match color {
        Color::White => 1,
        Color::Blue => 2,
        Color::Black => 4,
        Color::Red => 8,
        Color::Green => 16,
    }
}

/// Encodes a color set as the schema's bitmask (W=1, U=2, B=4, R=8, G=16).
pub fn to_mask(set: ColorSet) -> i16 {
    Color::ALL
        .into_iter()
        .filter(|&color| set.contains(color))
        .fold(0, |mask, color| mask | bit(color))
}

/// Decodes a stored bitmask; masks outside `0..=31` are corrupt.
pub fn from_mask(column: &'static str, mask: i16) -> Result<ColorSet, DbError> {
    if !(0..=MASK_MAX).contains(&mask) {
        return Err(DbError::Corrupt {
            column,
            value: mask.to_string(),
        });
    }
    Ok(Color::ALL
        .into_iter()
        .filter(|&color| mask & bit(color) != 0)
        .collect())
}

/// WUBRG letter of a color.
const fn symbol(color: Color) -> char {
    match color {
        Color::White => 'W',
        Color::Blue => 'U',
        Color::Black => 'B',
        Color::Red => 'R',
        Color::Green => 'G',
    }
}

/// Serde adapter storing a color set as an array of WUBRG letters.
pub mod letters {
    use mtg_core::color::{Color, ColorSet};
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::symbol;

    /// Serializes the set as letters in WUBRG order.
    pub fn serialize<S: Serializer>(set: &ColorSet, serializer: S) -> Result<S::Ok, S::Error> {
        Color::ALL
            .into_iter()
            .filter(|&color| set.contains(color))
            .map(symbol)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    /// Reads letters back into a set, rejecting anything but W, U, B, R and G.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<ColorSet, D::Error> {
        Vec::<char>::deserialize(deserializer)?
            .into_iter()
            .map(|letter| {
                Color::from_symbol(letter)
                    .ok_or_else(|| D::Error::custom(format!("unknown color letter {letter:?}")))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the color bitmask stored in Postgres round-trips every color set and matches the documented bit assignment.
    use proptest::prelude::*;
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    use super::*;

    fn set(symbols: &str) -> ColorSet {
        symbols.chars().filter_map(Color::from_symbol).collect()
    }

    #[rstest]
    #[case("", 0)]
    #[case("W", 1)]
    #[case("U", 2)]
    #[case("B", 4)]
    #[case("R", 8)]
    #[case("G", 16)]
    #[case("WU", 3)]
    #[case("UR", 10)]
    #[case("WUBRG", 31)]
    fn documented_bit_assignment(#[case] symbols: &str, #[case] mask: i16) {
        assert_eq!(to_mask(set(symbols)), mask);
        assert_eq!(from_mask("colors", mask).unwrap(), set(symbols));
    }

    #[rstest]
    #[case(-1)]
    #[case(32)]
    #[case(i16::MAX)]
    fn out_of_range_masks_are_corrupt(#[case] mask: i16) {
        assert!(matches!(
            from_mask("colors", mask),
            Err(DbError::Corrupt {
                column: "colors",
                ..
            })
        ));
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Holder(#[serde(with = "letters")] ColorSet);

    #[rstest]
    #[case("", "[]")]
    #[case("RU", r#"["U","R"]"#)]
    #[case("GWUBR", r#"["W","U","B","R","G"]"#)]
    fn letters_are_wubrg_ordered(#[case] symbols: &str, #[case] json: &str) {
        let holder = Holder(set(symbols));
        assert_eq!(serde_json::to_string(&holder).unwrap(), json);
        assert_eq!(serde_json::from_str::<Holder>(json).unwrap(), holder);
    }

    #[test]
    fn unknown_letters_are_rejected() {
        assert!(serde_json::from_str::<Holder>(r#"["X"]"#).is_err());
        assert!(serde_json::from_str::<Holder>(r#"["w"]"#).is_err());
    }

    proptest! {
        #[test]
        fn every_mask_round_trips(mask in 0i16..=31) {
            prop_assert_eq!(to_mask(from_mask("colors", mask).unwrap()), mask);
        }

        #[test]
        fn subset_matches_the_sql_expression(a in 0i16..=31, b in 0i16..=31) {
            let sql_subset = a & !b == 0;
            let model_subset = from_mask("c", a).unwrap().is_subset_of(from_mask("c", b).unwrap());
            prop_assert_eq!(sql_subset, model_subset);
        }
    }
}
