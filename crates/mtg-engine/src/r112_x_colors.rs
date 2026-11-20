//! "Spend only colored mana on X. No more than one mana of each color may be spent this way."
//! (`CostItem::R112XOneEachColor`, CR 107.3): the X of the cost is paid by X mana of X different colors. The payment
//! carries how much of the generic mana is that X (`Spend::r112_x_one_each`); each way to pay the cost then becomes one
//! way for each set of X colors, those X generic mana asked as one symbol of each color of the set.

use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol, Resolution};

use crate::spend::Spend;

/// The five colors in WUBRG order.
const COLORS: [Color; 5] = [
    Color::White,
    Color::Blue,
    Color::Black,
    Color::Red,
    Color::Green,
];

/// Every way to pay `cost` for `spend`: the ways the rules of spending allow (`r111_spend_as`), each with the X that
/// must be paid one mana of each color asked as every set of that many colors; none when the X is over five or more
/// than the generic mana of a way.
pub fn r112_resolutions(cost: &ManaCost, spend: &Spend) -> Vec<Resolution> {
    let plain = crate::r111_spend_as::r111_resolutions(cost, spend.r111_pays.as_ref());
    let x = spend.r112_x_one_each as usize;
    if x == 0 {
        return plain;
    }
    let sets = color_sets(x);
    let mut found = Vec::new();
    for resolution in plain {
        let generic: u32 = resolution
            .cost
            .symbols()
            .iter()
            .map(|symbol| match symbol {
                ManaSymbol::Generic(amount) => *amount,
                _ => 0,
            })
            .sum();
        let Some(left) = generic.checked_sub(spend.r112_x_one_each) else {
            continue;
        };
        let others: Vec<ManaSymbol> = resolution
            .cost
            .symbols()
            .iter()
            .copied()
            .filter(|symbol| !matches!(symbol, ManaSymbol::Generic(_)))
            .collect();
        for set in &sets {
            let mut symbols = others.clone();
            if left > 0 {
                symbols.push(ManaSymbol::Generic(left));
            }
            symbols.extend(set.iter().map(|color| ManaSymbol::Colored(*color)));
            found.push(Resolution {
                cost: ManaCost::from_symbols(symbols),
                life: resolution.life,
            });
        }
    }
    found
}

/// Every set of `size` different colors, in WUBRG order; none for more than five.
fn color_sets(size: usize) -> Vec<Vec<Color>> {
    let mut sets = Vec::new();
    for mask in 0u32..1 << COLORS.len() {
        if mask.count_ones() as usize == size {
            sets.push(
                (0..COLORS.len())
                    .filter(|at| mask >> at & 1 == 1)
                    .map(|at| COLORS[at])
                    .collect(),
            );
        }
    }
    sets
}

#[cfg(test)]
mod tests {
    //! Goal: an X paid one mana of each color is asked as every set of X different colors beside the rest of the cost,
    //! and an X over five or over the generic mana leaves no way to pay.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use rstest::rstest;

    fn ways(cost: &str, x: u32) -> Vec<String> {
        let spend = Spend {
            r112_x_one_each: x,
            ..Spend::default()
        };
        let mut found: Vec<String> = r112_resolutions(&ManaCost::parse(cost).unwrap(), &spend)
            .into_iter()
            .map(|r| r.cost.to_string())
            .collect();
        found.sort();
        found
    }

    #[rstest]
    #[case::none("{2}", 0, 1)]
    #[case::one("{3}", 1, 5)]
    #[case::two("{4}", 2, 10)]
    #[case::five("{7}", 5, 1)]
    #[case::six("{8}", 6, 0)]
    #[case::over_the_generic("{1}{G}", 2, 0)]
    fn r112_x_is_asked_as_each_set_of_colors(
        #[case] cost: &str,
        #[case] x: u32,
        #[case] count: usize,
    ) {
        assert_eq!(ways(cost, x).len(), count, "{cost} X={x}");
    }

    #[test]
    fn r112_the_rest_stays_generic() {
        assert!(ways("{3}", 2).contains(&"{1}{W}{U}".to_owned()));
        assert!(!ways("{3}", 2).iter().any(|way| way.contains("{W}{W}")));
    }
}
