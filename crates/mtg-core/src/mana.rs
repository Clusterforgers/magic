//! Mana types, symbols and (hybrid-free) mana costs.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::color::{Color, ColorSet};

/// A kind of mana that can sit in a mana pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ManaType {
    /// White mana.
    White,
    /// Blue mana.
    Blue,
    /// Black mana.
    Black,
    /// Red mana.
    Red,
    /// Green mana.
    Green,
    /// Colorless mana (not the same as generic).
    Colorless,
}

impl ManaType {
    /// All mana types in WUBRG then colorless order.
    pub const ALL: [ManaType; 6] = [
        ManaType::White,
        ManaType::Blue,
        ManaType::Black,
        ManaType::Red,
        ManaType::Green,
        ManaType::Colorless,
    ];

    /// Position in [`ManaType::ALL`].
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The color this mana carries, if any.
    pub const fn color(self) -> Option<Color> {
        match self {
            ManaType::White => Some(Color::White),
            ManaType::Blue => Some(Color::Blue),
            ManaType::Black => Some(Color::Black),
            ManaType::Red => Some(Color::Red),
            ManaType::Green => Some(Color::Green),
            ManaType::Colorless => None,
        }
    }

    /// The single-letter symbol used in costs (`W`, `U`, `B`, `R`, `G`, `C`).
    pub const fn symbol(self) -> char {
        match self {
            ManaType::White => 'W',
            ManaType::Blue => 'U',
            ManaType::Black => 'B',
            ManaType::Red => 'R',
            ManaType::Green => 'G',
            ManaType::Colorless => 'C',
        }
    }
}

impl From<Color> for ManaType {
    fn from(color: Color) -> Self {
        match color {
            Color::White => ManaType::White,
            Color::Blue => ManaType::Blue,
            Color::Black => ManaType::Black,
            Color::Red => ManaType::Red,
            Color::Green => ManaType::Green,
        }
    }
}

/// One symbol of a mana cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ManaSymbol {
    /// A colored requirement such as `{G}`.
    Colored(Color),
    /// A colorless-only requirement `{C}`.
    Colorless,
    /// A generic requirement such as `{2}`, payable with any mana.
    Generic(u32),
}

/// Why a mana cost string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseManaCostError {
    /// Text outside `{...}` or an unterminated brace.
    #[error("malformed mana cost {0:?}")]
    Malformed(String),
    /// A symbol this engine does not model yet (hybrid, Phyrexian, X, snow...).
    #[error("unsupported mana symbol {{{0}}}")]
    Unsupported(String),
}

/// A hybrid-free mana cost, stored as requirement counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ManaCost {
    generic: u32,
    colorless: u32,
    colored: [u32; 5],
}

impl ManaCost {
    /// The empty cost `{0}`.
    pub const ZERO: ManaCost = ManaCost {
        generic: 0,
        colorless: 0,
        colored: [0; 5],
    };

    /// Returns the cost with one more symbol added.
    #[must_use]
    pub fn with(mut self, symbol: ManaSymbol) -> Self {
        match symbol {
            ManaSymbol::Colored(color) => self.colored[color.index()] += 1,
            ManaSymbol::Colorless => self.colorless += 1,
            ManaSymbol::Generic(n) => self.generic += n,
        }
        self
    }

    /// Builds a cost from symbols.
    pub fn from_symbols(symbols: impl IntoIterator<Item = ManaSymbol>) -> Self {
        symbols.into_iter().fold(ManaCost::ZERO, ManaCost::with)
    }

    /// Parses `{2}{G}{C}` style text (an empty string is the zero cost).
    pub fn parse(text: &str) -> Result<Self, ParseManaCostError> {
        let mut cost = ManaCost::ZERO;
        let mut rest = text;
        while !rest.is_empty() {
            let (symbol, tail) = rest
                .strip_prefix('{')
                .and_then(|r| r.split_once('}'))
                .ok_or_else(|| ParseManaCostError::Malformed(text.to_owned()))?;
            cost = cost.with(parse_symbol(symbol)?);
            rest = tail;
        }
        Ok(cost)
    }

    /// Generic requirement (payable with any mana).
    pub const fn generic(&self) -> u32 {
        self.generic
    }

    /// Requirement for mana of exactly this type; generic is not included.
    pub const fn required(&self, mana: ManaType) -> u32 {
        match mana.color() {
            Some(color) => self.colored[color.index()],
            None => self.colorless,
        }
    }

    /// Total number of mana that must be paid (the mana value of the cost).
    pub fn mana_value(&self) -> u32 {
        self.generic + self.colorless + self.colored.iter().sum::<u32>()
    }

    /// Colors appearing in the cost.
    pub fn colors(&self) -> ColorSet {
        Color::ALL
            .into_iter()
            .filter(|c| self.colored[c.index()] > 0)
            .collect()
    }
}

fn parse_symbol(symbol: &str) -> Result<ManaSymbol, ParseManaCostError> {
    if let Ok(n) = symbol.parse::<u32>() {
        return Ok(ManaSymbol::Generic(n));
    }
    let mut chars = symbol.chars();
    match (chars.next(), chars.next()) {
        (Some('C'), None) => Ok(ManaSymbol::Colorless),
        (Some(c), None) => Color::from_symbol(c)
            .map(ManaSymbol::Colored)
            .ok_or_else(|| ParseManaCostError::Unsupported(symbol.to_owned())),
        _ => Err(ParseManaCostError::Unsupported(symbol.to_owned())),
    }
}

impl fmt::Display for ManaCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mana_value() == 0 {
            return f.write_str("{0}");
        }
        if self.generic > 0 {
            write!(f, "{{{}}}", self.generic)?;
        }
        for _ in 0..self.colorless {
            f.write_str("{C}")?;
        }
        for color in Color::ALL {
            let symbol = ManaType::from(color).symbol();
            for _ in 0..self.colored[color.index()] {
                write!(f, "{{{symbol}}}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: mana cost parsing, totals and display agree, since payment and deck import rely on them.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case("", 0, 0)]
    #[case("{0}", 0, 0)]
    #[case("{2}{G}", 3, 1)]
    #[case("{W}{W}{U}{U}{B}{B}{R}{R}{G}{G}", 10, 5)]
    #[case("{C}{C}{1}", 3, 0)]
    #[case("{10}", 10, 0)]
    fn parses_value_and_colors(#[case] text: &str, #[case] value: u32, #[case] colors: u32) {
        let cost = ManaCost::parse(text).unwrap();
        assert_eq!(cost.mana_value(), value);
        assert_eq!(cost.colors().len(), colors);
    }

    #[rstest]
    #[case("G")]
    #[case("{G")]
    #[case("{G}x")]
    #[case("{}")]
    fn malformed_text_is_rejected(#[case] text: &str) {
        assert!(ManaCost::parse(text).is_err());
    }

    #[rstest]
    #[case("{X}")]
    #[case("{W/U}")]
    #[case("{G/P}")]
    #[case("{S}")]
    fn unmodelled_symbols_are_rejected(#[case] text: &str) {
        assert!(matches!(
            ManaCost::parse(text),
            Err(ParseManaCostError::Unsupported(_))
        ));
    }

    #[test]
    fn requirements_split_colored_colorless_and_generic() {
        let cost = ManaCost::parse("{3}{C}{R}{R}").unwrap();
        assert_eq!(cost.generic(), 3);
        assert_eq!(cost.required(ManaType::Colorless), 1);
        assert_eq!(cost.required(ManaType::Red), 2);
        assert_eq!(cost.required(ManaType::Green), 0);
    }

    proptest! {
        #[test]
        fn display_then_parse_roundtrips(
            generic in 0u32..8, colorless in 0u32..4, colored in prop::array::uniform5(0u32..4)
        ) {
            let mut cost = ManaCost::ZERO.with(ManaSymbol::Generic(generic));
            for _ in 0..colorless {
                cost = cost.with(ManaSymbol::Colorless);
            }
            for (color, n) in Color::ALL.into_iter().zip(colored) {
                for _ in 0..n {
                    cost = cost.with(ManaSymbol::Colored(color));
                }
            }
            let reparsed = ManaCost::parse(&cost.to_string()).unwrap();
            prop_assert_eq!(reparsed.mana_value(), cost.mana_value());
            if cost.mana_value() > 0 {
                prop_assert_eq!(reparsed, cost);
            }
        }
    }
}
