//! A mana cost as cards print it: the symbols the engine pays plus any number of `{X}`.

use std::fmt;
use std::str::FromStr;

use mtg_core::mana::{ManaCost, ParseManaCostError};
use serde::{Deserialize, Serialize};

/// A mana cost that may hold `{X}` symbols; written as the card prints it (`{X}{R}`).
///
/// `ManaCost` in `mtg-core` refuses `{X}` until the engine learns it (roadmap, X costs), so the
/// count of X symbols lives beside the rest. When the engine gains `ManaSymbol::X` this type
/// becomes a plain `ManaCost` with the same text form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ManaCostX {
    x: u8,
    rest: ManaCost,
}

impl ManaCostX {
    /// A cost with `x` X symbols and the given other symbols.
    pub fn new(x: u8, rest: ManaCost) -> Self {
        ManaCostX { x, rest }
    }

    /// How many `{X}` symbols the cost holds.
    pub const fn x(&self) -> u8 {
        self.x
    }

    /// Whether the cost holds at least one `{X}`.
    pub const fn has_x(&self) -> bool {
        self.x > 0
    }

    /// The symbols other than `{X}`.
    pub fn rest(&self) -> &ManaCost {
        &self.rest
    }
}

impl FromStr for ManaCostX {
    type Err = ParseManaCostError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let malformed = || ParseManaCostError::Malformed(text.to_owned());
        let (mut x, mut rest, mut tail) = (0u8, String::new(), text);
        while !tail.is_empty() {
            let (symbol, after) = tail
                .strip_prefix('{')
                .and_then(|t| t.split_once('}'))
                .ok_or_else(malformed)?;
            if symbol == "X" {
                x = x.checked_add(1).ok_or_else(malformed)?;
            } else {
                rest.push('{');
                rest.push_str(symbol);
                rest.push('}');
            }
            tail = after;
        }
        Ok(ManaCostX {
            x,
            rest: ManaCost::parse(&rest)?,
        })
    }
}

impl fmt::Display for ManaCostX {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for _ in 0..self.x {
            f.write_str("{X}")?;
        }
        if self.x == 0 || !self.rest.symbols().is_empty() {
            write!(f, "{}", self.rest)?;
        }
        Ok(())
    }
}

impl TryFrom<String> for ManaCostX {
    type Error = ParseManaCostError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<ManaCostX> for String {
    fn from(cost: ManaCostX) -> String {
        cost.to_string()
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a printed cost with X reads, prints and serializes back to itself, and unsupported symbols are named.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::plain("{1}{R}", 0, "{1}{R}")]
    #[case::x("{X}{R}", 1, "{X}{R}")]
    #[case::two_x("{X}{X}{G}", 2, "{X}{X}{G}")]
    #[case::x_first_when_written_last("{R}{X}", 1, "{X}{R}")]
    #[case::only_x("{X}", 1, "{X}")]
    #[case::zero("{0}", 0, "{0}")]
    #[case::hybrid("{2}{W/U}", 0, "{2}{W/U}")]
    #[case::sorted("{G}{2}", 0, "{2}{G}")]
    fn costs_read_and_print_canonically(#[case] text: &str, #[case] x: u8, #[case] printed: &str) {
        let cost: ManaCostX = text.parse().unwrap();
        assert_eq!(cost.x(), x);
        assert_eq!(cost.to_string(), printed);
        assert_eq!(printed.parse::<ManaCostX>().unwrap(), cost);
        let json = serde_json::to_string(&cost).unwrap();
        assert_eq!(serde_json::from_str::<ManaCostX>(&json).unwrap(), cost);
    }

    #[rstest]
    #[case::snow("{S}", "S")]
    #[case::y("{2}{Y}", "Y")]
    fn unsupported_symbols_are_named(#[case] text: &str, #[case] symbol: &str) {
        assert_eq!(
            text.parse::<ManaCostX>(),
            Err(ParseManaCostError::Unsupported(symbol.to_owned()))
        );
    }

    #[rstest]
    #[case::text("R")]
    #[case::open("{R")]
    #[case::trailing("{R}x")]
    fn malformed_text_is_refused(#[case] text: &str) {
        assert!(matches!(
            text.parse::<ManaCostX>(),
            Err(ParseManaCostError::Malformed(_))
        ));
    }
}
