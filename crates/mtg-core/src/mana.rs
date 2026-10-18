//! Mana types, cost symbols and mana costs (CR 107.4 and 202).

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::color::{Color, ColorSet};

/// Life paid for one Phyrexian symbol (CR 107.4f).
pub const PHYREXIAN_LIFE: u32 = 2;

/// A kind of mana that can sit in a mana pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

/// The ten colour pairs in the orientation hybrid symbols print them: the five allied pairs, then the five enemy pairs.
const PAIRS: [(Color, Color); 10] = [
    (Color::White, Color::Blue),
    (Color::Blue, Color::Black),
    (Color::Black, Color::Red),
    (Color::Red, Color::Green),
    (Color::Green, Color::White),
    (Color::White, Color::Black),
    (Color::Blue, Color::Red),
    (Color::Black, Color::Green),
    (Color::Red, Color::White),
    (Color::Green, Color::Blue),
];

/// Where the pair `{a, b}` sits in [`PAIRS`] (either way round); `None` for one colour twice.
fn pair_position(a: Color, b: Color) -> Option<usize> {
    PAIRS
        .iter()
        .position(|&(x, y)| (x, y) == (a, b) || (x, y) == (b, a))
}

/// One symbol of a mana cost (CR 107.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ManaSymbol {
    /// A generic requirement such as `{2}`, payable with any mana.
    Generic(u32),
    /// A colored requirement such as `{G}`.
    Colored(Color),
    /// A colorless-only requirement `{C}`.
    Colorless,
    /// A two-colour hybrid symbol such as `{W/U}`: one mana of either colour (CR 107.4e).
    Hybrid(Color, Color),
    /// A monocolored hybrid symbol such as `{2/W}`: one mana of the colour or two generic mana (CR 107.4e).
    MonoHybrid(Color),
    /// A Phyrexian symbol such as `{W/P}`: one mana of the colour or 2 life (CR 107.4f).
    Phyrexian(Color),
    /// A hybrid Phyrexian symbol such as `{G/U/P}`: one mana of either colour or 2 life.
    HybridPhyrexian(Color, Color),
    /// A colorless hybrid symbol such as `{C/W}`: one mana of the colour or one colorless mana.
    ColorlessHybrid(Color),
    /// The colorless Phyrexian symbol `{C/P}`: one colorless mana or 2 life.
    ColorlessPhyrexian,
}

/// One way to pay a single cost symbol (CR 601.2b: the choice for a hybrid or Phyrexian symbol is made when casting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolPayment {
    /// One mana of this colour.
    Color(Color),
    /// One colorless mana.
    Colorless,
    /// This much generic mana.
    Generic(u32),
    /// This much life.
    Life(u32),
}

impl ManaSymbol {
    /// What the symbol adds to a mana value (CR 202.3): its number for `{2}`, 2 for a monocolored hybrid, else 1.
    pub const fn mana_value(self) -> u32 {
        match self {
            ManaSymbol::Generic(n) => n,
            ManaSymbol::MonoHybrid(_) => 2,
            _ => 1,
        }
    }

    /// The colours the symbol gives a card (CR 105.2): every colour it names, Phyrexian and hybrid ones included.
    pub fn colors(self) -> ColorSet {
        match self {
            ManaSymbol::Generic(_) | ManaSymbol::Colorless | ManaSymbol::ColorlessPhyrexian => {
                ColorSet::EMPTY
            }
            ManaSymbol::Colored(c)
            | ManaSymbol::MonoHybrid(c)
            | ManaSymbol::Phyrexian(c)
            | ManaSymbol::ColorlessHybrid(c) => ColorSet::EMPTY.with(c),
            ManaSymbol::Hybrid(a, b) | ManaSymbol::HybridPhyrexian(a, b) => {
                ColorSet::EMPTY.with(a).with(b)
            }
        }
    }

    /// Whether the symbol has exactly one way to pay it: generic, colored or colorless.
    pub const fn is_plain(self) -> bool {
        matches!(
            self,
            ManaSymbol::Generic(_) | ManaSymbol::Colored(_) | ManaSymbol::Colorless
        )
    }

    /// Whether 2 life can pay the symbol.
    pub const fn can_pay_life(self) -> bool {
        matches!(
            self,
            ManaSymbol::Phyrexian(_)
                | ManaSymbol::HybridPhyrexian(_, _)
                | ManaSymbol::ColorlessPhyrexian
        )
    }

    /// Every way to pay the symbol, the mana ones first and life last.
    pub fn payments(self) -> Vec<SymbolPayment> {
        use SymbolPayment::{Color as Mana, Colorless, Generic, Life};
        match self {
            ManaSymbol::Generic(n) => vec![Generic(n)],
            ManaSymbol::Colored(c) => vec![Mana(c)],
            ManaSymbol::Colorless => vec![Colorless],
            ManaSymbol::Hybrid(a, b) => vec![Mana(a), Mana(b)],
            ManaSymbol::MonoHybrid(c) => vec![Mana(c), Generic(2)],
            ManaSymbol::Phyrexian(c) => vec![Mana(c), Life(PHYREXIAN_LIFE)],
            ManaSymbol::HybridPhyrexian(a, b) => vec![Mana(a), Mana(b), Life(PHYREXIAN_LIFE)],
            ManaSymbol::ColorlessHybrid(c) => vec![Mana(c), Colorless],
            ManaSymbol::ColorlessPhyrexian => vec![Colorless, Life(PHYREXIAN_LIFE)],
        }
    }

    /// The symbol in its one canonical form: a colour pair in printed orientation, or a plain colour for a pair of one.
    fn normalized(self) -> ManaSymbol {
        let pair = |a, b| {
            PAIRS
                .iter()
                .find(|&&(x, y)| (x, y) == (a, b) || (x, y) == (b, a))
                .copied()
        };
        match self {
            ManaSymbol::Hybrid(a, b) => {
                pair(a, b).map_or(ManaSymbol::Colored(a), |(x, y)| ManaSymbol::Hybrid(x, y))
            }
            ManaSymbol::HybridPhyrexian(a, b) => pair(a, b)
                .map_or(ManaSymbol::Phyrexian(a), |(x, y)| {
                    ManaSymbol::HybridPhyrexian(x, y)
                }),
            other => other,
        }
    }

    /// Sort key of the canonical order: generic, colorless, colours, then the symbols with a choice.
    fn rank(self) -> (u8, usize) {
        let pair = |a, b| pair_position(a, b).unwrap_or(0);
        match self {
            ManaSymbol::Generic(_) => (0, 0),
            ManaSymbol::Colorless => (1, 0),
            ManaSymbol::Colored(c) => (2, c.index()),
            ManaSymbol::ColorlessHybrid(c) => (3, c.index()),
            ManaSymbol::Hybrid(a, b) => (4, pair(a, b)),
            ManaSymbol::MonoHybrid(c) => (5, c.index()),
            ManaSymbol::Phyrexian(c) => (6, c.index()),
            ManaSymbol::ColorlessPhyrexian => (7, 0),
            ManaSymbol::HybridPhyrexian(a, b) => (8, pair(a, b)),
        }
    }
}

fn letter(color: Color) -> char {
    ManaType::from(color).symbol()
}

impl fmt::Display for ManaSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ManaSymbol::Generic(n) => write!(f, "{{{n}}}"),
            ManaSymbol::Colored(c) => write!(f, "{{{}}}", letter(c)),
            ManaSymbol::Colorless => f.write_str("{C}"),
            ManaSymbol::Hybrid(a, b) => write!(f, "{{{}/{}}}", letter(a), letter(b)),
            ManaSymbol::MonoHybrid(c) => write!(f, "{{2/{}}}", letter(c)),
            ManaSymbol::Phyrexian(c) => write!(f, "{{{}/P}}", letter(c)),
            ManaSymbol::HybridPhyrexian(a, b) => {
                write!(f, "{{{}/{}/P}}", letter(a), letter(b))
            }
            ManaSymbol::ColorlessHybrid(c) => write!(f, "{{C/{}}}", letter(c)),
            ManaSymbol::ColorlessPhyrexian => f.write_str("{C/P}"),
        }
    }
}

/// Why a mana cost string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseManaCostError {
    /// Text outside `{...}` or an unterminated brace.
    #[error("malformed mana cost {0:?}")]
    Malformed(String),
    /// A symbol this engine does not model (X, snow, and the silver-border ones).
    #[error("unsupported mana symbol {{{0}}}")]
    Unsupported(String),
}

/// A mana cost: its symbols in one canonical order, so equal costs are equal values (the order a card prints
/// its symbols in has no rules meaning).
///
/// Generic symbols are merged into one, `{0}` is dropped (the empty cost prints as `{0}`), and a colour pair
/// is written the way cards print it. On the wire it is the list of its symbols.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(from = "Vec<ManaSymbol>")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ManaCost(Vec<ManaSymbol>);

/// A cost with every choice made (CR 601.2b): the plain cost left to pay with mana, and the life paid instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Resolution {
    /// What is left to pay with mana: generic, colored and colorless symbols only.
    pub cost: ManaCost,
    /// Life paid in place of Phyrexian symbols.
    pub life: u32,
}

impl From<Vec<ManaSymbol>> for ManaCost {
    fn from(symbols: Vec<ManaSymbol>) -> Self {
        ManaCost::from_symbols(symbols)
    }
}

impl ManaCost {
    /// The empty cost `{0}`.
    pub const ZERO: ManaCost = ManaCost(Vec::new());

    /// Returns the cost with one more symbol added.
    #[must_use]
    pub fn with(mut self, symbol: ManaSymbol) -> Self {
        match symbol.normalized() {
            ManaSymbol::Generic(0) => {}
            ManaSymbol::Generic(n) => match self.0.first_mut() {
                Some(ManaSymbol::Generic(have)) => *have = have.saturating_add(n),
                _ => self.0.insert(0, ManaSymbol::Generic(n)),
            },
            other => {
                let rank = other.rank();
                let at = self.0.partition_point(|s| s.rank() <= rank);
                self.0.insert(at, other);
            }
        }
        self
    }

    /// Builds a cost from symbols.
    pub fn from_symbols(symbols: impl IntoIterator<Item = ManaSymbol>) -> Self {
        symbols.into_iter().fold(ManaCost::ZERO, ManaCost::with)
    }

    /// Parses `{2}{G}{C}` style text, with hybrid and Phyrexian symbols (an empty string is the zero cost).
    ///
    /// A symbol that is not modelled (`{X}`, `{S}`, ...) is an error, never skipped.
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

    /// Parses a card-level cost as Scryfall writes it, one cost per face joined by ` // ` (`{2}{B} // {B}`).
    pub fn parse_faces(text: &str) -> Result<Vec<Self>, ParseManaCostError> {
        text.split(" // ").map(ManaCost::parse).collect()
    }

    /// The symbols in canonical order.
    pub fn symbols(&self) -> &[ManaSymbol] {
        &self.0
    }

    /// Whether every symbol is generic, colored or colorless, so the cost is paid with mana alone and in one way.
    pub fn is_plain(&self) -> bool {
        self.0.iter().all(|s| s.is_plain())
    }

    /// Generic requirement of the plain symbols (payable with any mana); hybrid ones are in [`resolutions`](Self::resolutions).
    pub fn generic(&self) -> u32 {
        self.0
            .iter()
            .map(|s| match s {
                ManaSymbol::Generic(n) => *n,
                _ => 0,
            })
            .fold(0, u32::saturating_add)
    }

    /// Requirement of the plain symbols for mana of exactly this type; generic and hybrid symbols are not included.
    pub fn required(&self, mana: ManaType) -> u32 {
        let count = self
            .0
            .iter()
            .filter(|s| match (s, mana.color()) {
                (ManaSymbol::Colored(c), Some(wanted)) => *c == wanted,
                (ManaSymbol::Colorless, None) => true,
                _ => false,
            })
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// The mana value of the cost (CR 202.3): `{2/W}` counts 2, every other symbol its own value.
    pub fn mana_value(&self) -> u32 {
        self.0
            .iter()
            .map(|s| s.mana_value())
            .fold(0, u32::saturating_add)
    }

    /// Colors appearing in the cost (CR 105.2): a hybrid symbol gives both of its colours, a Phyrexian one its colour.
    pub fn colors(&self) -> ColorSet {
        self.0.iter().fold(ColorSet::EMPTY, |set, s| {
            s.colors().iter().fold(set, ColorSet::with)
        })
    }

    /// How many symbols 2 life can pay.
    pub fn life_symbols(&self) -> u32 {
        let count = self.0.iter().filter(|s| s.can_pay_life()).count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// Every way to pay the cost with its choices made, each as a plain cost plus the life paid.
    ///
    /// The list has no repeats, takes colours before generic mana before life for each symbol, and holds
    /// just the cost itself with no life when it is plain. Equal symbols are interchangeable, so the list is
    /// counted in splits of a symbol's copies among its ways, not in a way per copy.
    pub fn resolutions(&self) -> Vec<Resolution> {
        let mut groups: Vec<(ManaSymbol, usize)> = Vec::new();
        for &symbol in &self.0 {
            match groups.last_mut() {
                Some((last, count)) if *last == symbol => *count += 1,
                _ => groups.push((symbol, 1)),
            }
        }
        let mut found = Vec::new();
        expand(&groups, ManaCost::ZERO, 0, &mut found);
        let mut seen = HashSet::new();
        found.retain(|r| seen.insert((r.cost.clone(), r.life)));
        found
    }
}

/// Splits `total` copies of a symbol among `ways`, the first way taking the most first; `f` sees each split.
fn splits(total: usize, ways: usize, f: &mut impl FnMut(&[usize])) {
    fn go(left: usize, ways: usize, chosen: &mut Vec<usize>, f: &mut impl FnMut(&[usize])) {
        if ways == 1 {
            chosen.push(left);
            f(chosen);
            chosen.pop();
            return;
        }
        for take in (0..=left).rev() {
            chosen.push(take);
            go(left - take, ways - 1, chosen, f);
            chosen.pop();
        }
    }
    go(total, ways.max(1), &mut Vec::new(), f);
}

fn expand(groups: &[(ManaSymbol, usize)], cost: ManaCost, life: u32, found: &mut Vec<Resolution>) {
    let Some((&(symbol, copies), later)) = groups.split_first() else {
        found.push(Resolution { cost, life });
        return;
    };
    let ways = symbol.payments();
    splits(copies, ways.len(), &mut |split| {
        let mut next = cost.clone();
        let mut paid = life;
        for (way, &n) in ways.iter().zip(split) {
            for _ in 0..n {
                match *way {
                    SymbolPayment::Color(c) => next = next.with(ManaSymbol::Colored(c)),
                    SymbolPayment::Colorless => next = next.with(ManaSymbol::Colorless),
                    SymbolPayment::Generic(g) => next = next.with(ManaSymbol::Generic(g)),
                    SymbolPayment::Life(l) => paid = paid.saturating_add(l),
                }
            }
        }
        expand(later, next, paid, found);
    });
}

fn color_of(part: &str) -> Option<Color> {
    let mut chars = part.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Color::from_symbol(c),
        _ => None,
    }
}

fn parse_symbol(symbol: &str) -> Result<ManaSymbol, ParseManaCostError> {
    let unsupported = || ParseManaCostError::Unsupported(symbol.to_owned());
    let parts: Vec<&str> = symbol.split('/').collect();
    let parsed = match parts.as_slice() {
        [one] if !one.is_empty() && one.bytes().all(|b| b.is_ascii_digit()) => {
            one.parse().ok().map(ManaSymbol::Generic)
        }
        ["C"] => Some(ManaSymbol::Colorless),
        [one] => color_of(one).map(ManaSymbol::Colored),
        ["2", c] => color_of(c).map(ManaSymbol::MonoHybrid),
        ["C", "P"] => Some(ManaSymbol::ColorlessPhyrexian),
        ["C", c] => color_of(c).map(ManaSymbol::ColorlessHybrid),
        [c, "P"] => color_of(c).map(ManaSymbol::Phyrexian),
        [a, b] => match (color_of(a), color_of(b)) {
            (Some(a), Some(b)) if a != b => Some(ManaSymbol::Hybrid(a, b)),
            _ => None,
        },
        [a, b, "P"] => match (color_of(a), color_of(b)) {
            (Some(a), Some(b)) if a != b => Some(ManaSymbol::HybridPhyrexian(a, b)),
            _ => None,
        },
        _ => None,
    };
    parsed.ok_or_else(unsupported)
}

impl fmt::Display for ManaCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("{0}");
        }
        self.0.iter().try_for_each(|symbol| write!(f, "{symbol}"))
    }
}

#[cfg(test)]
mod tests {
    //! Goal: mana cost parsing, values, colours and display agree for every symbol form, and costs are canonical values, since payment, deck import and the card pips rely on them.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    use Color::{Black, Blue, Green, Red, White};

    fn colors_of(text: &str) -> ColorSet {
        text.chars().filter_map(Color::from_symbol).collect()
    }

    #[rstest]
    #[case("", 0, "")]
    #[case("{0}", 0, "")]
    #[case("{2}{G}", 3, "G")]
    #[case("{W}{W}{U}{U}{B}{B}{R}{R}{G}{G}", 10, "WUBRG")]
    #[case("{C}{C}{1}", 3, "")]
    #[case("{10}", 10, "")]
    #[case::two_colour_hybrid("{W/U}", 1, "WU")]
    #[case::hybrid_in_the_other_order("{U/W}", 1, "WU")]
    #[case::enemy_hybrid("{B/G}", 1, "BG")]
    #[case::monocolored_hybrid("{2/W}", 2, "W")]
    #[case::phyrexian("{W/P}", 1, "W")]
    #[case::hybrid_phyrexian("{G/U/P}", 1, "UG")]
    #[case::colorless_hybrid("{C/W}", 1, "W")]
    #[case::colorless_phyrexian("{C/P}", 1, "")]
    #[case::mixed("{2}{W/P}{G/U/P}", 4, "WUG")]
    #[case::boros_reckoner("{R/W}{R/W}{R/W}", 3, "RW")]
    #[case::reaper_king("{2/W}{2/U}{2/B}{2/R}{2/G}", 10, "WUBRG")]
    #[case::kozilek_compleated("{8}{C/P}{C/P}", 10, "")]
    fn parses_value_and_colors(#[case] text: &str, #[case] value: u32, #[case] colors: &str) {
        let cost = ManaCost::parse(text).unwrap();
        assert_eq!(cost.mana_value(), value);
        assert_eq!(cost.colors(), colors_of(colors));
    }

    #[rstest]
    #[case("{W/U}", "{W/U}")]
    #[case("{U/W}", "{W/U}")]
    #[case("{G/W}", "{G/W}")]
    #[case("{W/G}", "{G/W}")]
    #[case("{R/W}", "{R/W}")]
    #[case("{2/W}", "{2/W}")]
    #[case("{W/P}", "{W/P}")]
    #[case("{U/G/P}", "{G/U/P}")]
    #[case("{C/W}", "{C/W}")]
    #[case("{C/P}", "{C/P}")]
    #[case("{G}{2}{W/P}", "{2}{G}{W/P}")]
    #[case("{G/U/P}{W/P}{2}", "{2}{W/P}{G/U/P}")]
    #[case("{1}{1}", "{2}")]
    #[case("{0}{0}", "{0}")]
    #[case("", "{0}")]
    fn display_is_the_canonical_text(#[case] text: &str, #[case] shown: &str) {
        assert_eq!(ManaCost::parse(text).unwrap().to_string(), shown);
    }

    #[test]
    fn the_example_cost_round_trips() {
        let cost = ManaCost::parse("{2}{W/P}{G/U/P}").unwrap();
        assert_eq!(cost.to_string(), "{2}{W/P}{G/U/P}");
        assert_eq!(
            cost.symbols(),
            [
                ManaSymbol::Generic(2),
                ManaSymbol::Phyrexian(White),
                ManaSymbol::HybridPhyrexian(Green, Blue),
            ]
        );
    }

    #[rstest]
    #[case("G")]
    #[case("{G")]
    #[case("{G}x")]
    #[case("x{G}")]
    #[case("{G}}")]
    #[case("{G} {G}")]
    fn malformed_text_is_rejected(#[case] text: &str) {
        assert!(matches!(
            ManaCost::parse(text),
            Err(ParseManaCostError::Malformed(_))
        ));
    }

    #[rstest]
    #[case::x("{X}")]
    #[case::y("{Y}")]
    #[case::snow("{S}")]
    #[case::empty("{}")]
    #[case::lowercase("{g}")]
    #[case::lowercase_hybrid("{w/u}")]
    #[case::same_colour_twice("{W/W}")]
    #[case::same_colour_twice_phyrexian("{W/W/P}")]
    #[case::generic_hybrid_with_other_number("{3/W}")]
    #[case::generic_hybrid_with_colorless("{2/C}")]
    #[case::phyrexian_of_phyrexian("{P/P}")]
    #[case::bare_p("{P}")]
    #[case::three_colours("{W/U/B}")]
    #[case::three_colours_phyrexian("{W/U/B/P}")]
    #[case::colorless_twice("{C/C}")]
    #[case::colorless_hybrid_of_two_colours("{C/W/U}")]
    #[case::half_mana("{HW}")]
    #[case::playtest("{D}")]
    #[case::plus_sign("{+1}")]
    #[case::too_big("{99999999999}")]
    #[case::negative("{-1}")]
    #[case::decimal("{0.5}")]
    #[case::trailing_slash("{W/}")]
    #[case::leading_slash("{/W}")]
    #[case::unknown_inside_a_good_cost("{2}{X}{G}")]
    fn unmodelled_symbols_are_errors(#[case] text: &str) {
        assert!(
            matches!(
                ManaCost::parse(text),
                Err(ParseManaCostError::Unsupported(_))
            ),
            "{text} must be an error naming the symbol"
        );
    }

    #[test]
    fn an_unsupported_symbol_is_named_in_the_error() {
        assert_eq!(
            ManaCost::parse("{2}{X}{G}"),
            Err(ParseManaCostError::Unsupported("X".into()))
        );
        assert_eq!(
            ParseManaCostError::Unsupported("X".into()).to_string(),
            "unsupported mana symbol {X}"
        );
    }

    #[test]
    fn faces_are_parsed_one_by_one() {
        let faces = ManaCost::parse_faces("{2}{B} // {B}").unwrap();
        assert_eq!(
            faces,
            [
                ManaCost::parse("{2}{B}").unwrap(),
                ManaCost::parse("{B}").unwrap()
            ]
        );
        assert_eq!(
            ManaCost::parse_faces("{W/U}").unwrap(),
            [ManaCost::parse("{W/U}").unwrap()]
        );
        assert_eq!(ManaCost::parse_faces("").unwrap(), [ManaCost::ZERO]);
        assert!(ManaCost::parse_faces("{2}{B} // {X}").is_err());
        assert!(ManaCost::parse_faces("{2}{B}//{B}").is_err());
    }

    #[test]
    fn requirements_split_colored_colorless_and_generic() {
        let cost = ManaCost::parse("{3}{C}{R}{R}").unwrap();
        assert_eq!(cost.generic(), 3);
        assert_eq!(cost.required(ManaType::Colorless), 1);
        assert_eq!(cost.required(ManaType::Red), 2);
        assert_eq!(cost.required(ManaType::Green), 0);
    }

    #[test]
    fn plain_requirements_leave_out_the_symbols_with_a_choice() {
        let cost = ManaCost::parse("{1}{R}{R/W}{2/G}{G/P}{C/W}").unwrap();
        assert_eq!(cost.generic(), 1);
        assert_eq!(cost.required(ManaType::Red), 1);
        assert_eq!(cost.required(ManaType::Green), 0);
        assert_eq!(cost.required(ManaType::Colorless), 0);
        assert!(!cost.is_plain());
        assert!(ManaCost::parse("{3}{C}{R}").unwrap().is_plain());
        assert!(ManaCost::ZERO.is_plain());
    }

    #[rstest]
    #[case("{W/P}", 1)]
    #[case("{1}{B/P}{B/P}", 2)]
    #[case("{G/U/P}{C/P}{W/P}", 3)]
    #[case("{R/W}{2/W}{C/W}", 0)]
    #[case("{3}{G}", 0)]
    fn counts_the_symbols_life_can_pay(#[case] text: &str, #[case] count: u32) {
        assert_eq!(ManaCost::parse(text).unwrap().life_symbols(), count);
    }

    #[rstest]
    #[case(ManaSymbol::Generic(3), vec![SymbolPayment::Generic(3)])]
    #[case(ManaSymbol::Colored(Red), vec![SymbolPayment::Color(Red)])]
    #[case(ManaSymbol::Colorless, vec![SymbolPayment::Colorless])]
    #[case(ManaSymbol::Hybrid(White, Blue), vec![SymbolPayment::Color(White), SymbolPayment::Color(Blue)])]
    #[case(ManaSymbol::MonoHybrid(Black), vec![SymbolPayment::Color(Black), SymbolPayment::Generic(2)])]
    #[case(ManaSymbol::Phyrexian(Green), vec![SymbolPayment::Color(Green), SymbolPayment::Life(2)])]
    #[case(
        ManaSymbol::HybridPhyrexian(Green, Blue),
        vec![SymbolPayment::Color(Green), SymbolPayment::Color(Blue), SymbolPayment::Life(2)]
    )]
    #[case(ManaSymbol::ColorlessHybrid(White), vec![SymbolPayment::Color(White), SymbolPayment::Colorless])]
    #[case(ManaSymbol::ColorlessPhyrexian, vec![SymbolPayment::Colorless, SymbolPayment::Life(2)])]
    fn each_symbol_lists_its_ways_to_be_paid(
        #[case] symbol: ManaSymbol,
        #[case] ways: Vec<SymbolPayment>,
    ) {
        assert_eq!(symbol.payments(), ways);
        assert_eq!(
            symbol.can_pay_life(),
            ways.iter().any(|w| matches!(w, SymbolPayment::Life(_)))
        );
    }

    fn resolved(text: &str) -> Vec<(String, u32)> {
        ManaCost::parse(text)
            .unwrap()
            .resolutions()
            .into_iter()
            .map(|r| (r.cost.to_string(), r.life))
            .collect()
    }

    #[rstest]
    #[case::plain("{1}{G}", &[("{1}{G}", 0)])]
    #[case::free("", &[("{0}", 0)])]
    #[case::hybrid("{W/U}", &[("{W}", 0), ("{U}", 0)])]
    #[case::two_hybrids_split_by_count("{R/W}{R/W}", &[("{R}{R}", 0), ("{W}{R}", 0), ("{W}{W}", 0)])]
    #[case::monocolored("{2/W}", &[("{W}", 0), ("{2}", 0)])]
    #[case::phyrexian("{G/P}", &[("{G}", 0), ("{0}", 2)])]
    #[case::two_phyrexian("{1}{B/P}{B/P}", &[("{1}{B}{B}", 0), ("{1}{B}", 2), ("{1}", 4)])]
    #[case::hybrid_phyrexian("{G/U/P}", &[("{G}", 0), ("{U}", 0), ("{0}", 2)])]
    #[case::colorless_hybrid("{C/W}", &[("{W}", 0), ("{C}", 0)])]
    #[case::colorless_phyrexian("{C/P}", &[("{C}", 0), ("{0}", 2)])]
    #[case::generic_choices_merge("{2/W}{2/W}", &[("{W}{W}", 0), ("{2}{W}", 0), ("{4}", 0)])]
    fn resolutions_list_every_choice_once(#[case] text: &str, #[case] expected: &[(&str, u32)]) {
        let expected: Vec<(String, u32)> = expected
            .iter()
            .map(|(cost, life)| ((*cost).to_owned(), *life))
            .collect();
        assert_eq!(resolved(text), expected);
    }

    #[test]
    fn different_choices_that_make_the_same_payment_are_listed_once() {
        // Choosing (W, B, U) and (U, W, B) for the three symbols pays {W}{U}{B} twice over.
        let all = resolved("{W/U}{W/B}{U/B}");
        let mut unique = all.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(all.len(), unique.len(), "{all:?}");
    }

    fn color_strategy() -> impl Strategy<Value = Color> {
        (0usize..5).prop_map(|i| Color::ALL[i])
    }

    fn symbol_strategy() -> impl Strategy<Value = ManaSymbol> {
        prop_oneof![
            (0u32..6).prop_map(ManaSymbol::Generic),
            color_strategy().prop_map(ManaSymbol::Colored),
            Just(ManaSymbol::Colorless),
            (color_strategy(), color_strategy()).prop_map(|(a, b)| ManaSymbol::Hybrid(a, b)),
            color_strategy().prop_map(ManaSymbol::MonoHybrid),
            color_strategy().prop_map(ManaSymbol::Phyrexian),
            (color_strategy(), color_strategy())
                .prop_map(|(a, b)| ManaSymbol::HybridPhyrexian(a, b)),
            color_strategy().prop_map(ManaSymbol::ColorlessHybrid),
            Just(ManaSymbol::ColorlessPhyrexian),
        ]
    }

    fn cost_strategy() -> impl Strategy<Value = ManaCost> {
        prop::collection::vec(symbol_strategy(), 0..7).prop_map(ManaCost::from_symbols)
    }

    proptest! {
        #[test]
        fn display_then_parse_roundtrips(cost in cost_strategy()) {
            let reparsed = ManaCost::parse(&cost.to_string()).unwrap();
            prop_assert_eq!(reparsed, cost);
        }

        #[test]
        fn serde_roundtrips_and_is_the_symbol_list(cost in cost_strategy()) {
            let json = serde_json::to_string(&cost).unwrap();
            let back: ManaCost = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(&back, &cost);
            let list: Vec<ManaSymbol> = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(list.as_slice(), cost.symbols());
        }

        #[test]
        fn the_order_symbols_are_added_in_does_not_matter(
            symbols in prop::collection::vec(symbol_strategy(), 0..8).prop_shuffle()
        ) {
            let mut sorted = symbols.clone();
            sorted.reverse();
            prop_assert_eq!(
                ManaCost::from_symbols(symbols.clone()),
                ManaCost::from_symbols(sorted)
            );
        }

        #[test]
        fn value_and_colors_follow_the_symbols(symbols in prop::collection::vec(symbol_strategy(), 0..8)) {
            let cost = ManaCost::from_symbols(symbols.clone());
            let value: u32 = symbols.iter().map(|s| s.mana_value()).sum();
            prop_assert_eq!(cost.mana_value(), value);
            let colors: ColorSet = symbols.iter().flat_map(|s| s.colors().iter()).collect();
            prop_assert_eq!(cost.colors(), colors);
        }

        #[test]
        fn deserializing_normalizes_a_cost_written_any_way(
            symbols in prop::collection::vec(symbol_strategy(), 0..6)
        ) {
            let json = serde_json::to_string(&symbols).unwrap();
            let cost: ManaCost = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(cost, ManaCost::from_symbols(symbols));
        }

        #[test]
        fn resolutions_are_plain_bounded_and_unique(cost in cost_strategy()) {
            let all = cost.resolutions();
            prop_assert!(!all.is_empty());
            prop_assert_eq!(all[0].life, 0, "the first choice is always mana alone");
            let mono = cost.symbols().iter().filter(|s| matches!(s, ManaSymbol::MonoHybrid(_))).count();
            // Counting each symbol as one mana, then 2 per mono-hybrid paid with generic and 1 less per Phyrexian paid with life.
            let ones = cost.symbols().iter().map(|s| match s { ManaSymbol::Generic(n) => *n, _ => 1 }).sum::<u32>();
            let mut seen = HashSet::new();
            for r in &all {
                prop_assert!(r.cost.is_plain());
                prop_assert_eq!(r.life % PHYREXIAN_LIFE, 0);
                prop_assert!(r.life <= cost.life_symbols() * PHYREXIAN_LIFE);
                let paid_by_life = r.life / PHYREXIAN_LIFE;
                prop_assert!(r.cost.mana_value() + paid_by_life >= ones);
                prop_assert!(r.cost.mana_value() + paid_by_life <= ones + u32::try_from(mono).unwrap());
                prop_assert!(seen.insert((r.cost.clone(), r.life)));
            }
            if cost.is_plain() {
                prop_assert_eq!(all, vec![Resolution { cost: cost.clone(), life: 0 }]);
            }
        }
    }

    #[test]
    fn a_huge_generic_number_saturates_instead_of_overflowing() {
        let cost = ManaCost::parse("{4294967295}{4294967295}{W}").unwrap();
        assert_eq!(cost.generic(), u32::MAX);
        assert_eq!(cost.mana_value(), u32::MAX);
    }
}
