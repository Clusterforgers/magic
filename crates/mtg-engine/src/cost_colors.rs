//! Costs whose colors are drawn anew (`GameRule::CostColorsDrawn`, the Delves uber boss): each round a player under the
//! rule has a derangement of their commanders' color identity drawn from the game seed, their seat and the round, and
//! each color of that identity in the cost of a spell they cast stands for the color it is drawn to.
//!
//! Generic, colorless and snow symbols and colors outside the identity stay as they are; a hybrid or Phyrexian symbol
//! has each of its colors drawn, so it keeps its shape and its choices. A cost keeps its mana value and stays inside the
//! identity, so a card the deck may hold stays castable with the mana the deck makes. An identity of fewer than two
//! colors has no other color to draw, so its costs stay.

use std::collections::BTreeMap;

use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::{ManaCost, ManaSymbol};

use crate::state::GameState;

/// The colors of `player`'s costs this round, each color of their commanders' identity to the one it stands for; `None`
/// while the rule does not apply to them or their identity has fewer than two colors.
pub fn drawn(state: &GameState, player: PlayerId) -> Option<BTreeMap<Color, Color>> {
    if !crate::rules::cost_colors_drawn(state, player) {
        return None;
    }
    let identity: Vec<Color> = crate::commanders::dk25_identity(state, player)
        .iter()
        .collect();
    derangement(
        &identity,
        roll(state.seed(), player.index(), state.turn().round()),
    )
}

/// The cost `player` pays for a spell of mana cost `cost` now: `cost` with its colors drawn, or `cost` itself while the
/// rule does not change it.
pub fn redrawn(state: &GameState, player: PlayerId, cost: &ManaCost) -> ManaCost {
    match drawn(state, player) {
        Some(colors) => recolored(cost, &colors),
        None => cost.clone(),
    }
}

/// `cost` with each color of its symbols changed as `colors` says; a color it does not name stays.
pub fn recolored(cost: &ManaCost, colors: &BTreeMap<Color, Color>) -> ManaCost {
    let to = |color: Color| colors.get(&color).copied().unwrap_or(color);
    ManaCost::from_symbols(cost.symbols().iter().map(|symbol| match *symbol {
        ManaSymbol::Colored(c) => ManaSymbol::Colored(to(c)),
        ManaSymbol::Hybrid(a, b) => ManaSymbol::Hybrid(to(a), to(b)),
        ManaSymbol::MonoHybrid(c) => ManaSymbol::MonoHybrid(to(c)),
        ManaSymbol::Phyrexian(c) => ManaSymbol::Phyrexian(to(c)),
        ManaSymbol::HybridPhyrexian(a, b) => ManaSymbol::HybridPhyrexian(to(a), to(b)),
        ManaSymbol::ColorlessHybrid(c) => ManaSymbol::ColorlessHybrid(to(c)),
        other @ (ManaSymbol::Generic(_)
        | ManaSymbol::Colorless
        | ManaSymbol::ColorlessPhyrexian
        | ManaSymbol::Snow) => other,
    }))
}

/// A number drawn from the game seed, the seat and the round alone (splitmix64), so drawing it never moves the game's
/// own random stream.
fn roll(seed: u64, seat: usize, round: u32) -> u64 {
    let mut z = seed
        ^ (u64::try_from(seat).unwrap_or(u64::MAX).wrapping_add(1))
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(round).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The derangement of `colors` that `roll` picks among all of them: each color to another one of the list.
fn derangement(colors: &[Color], roll: u64) -> Option<BTreeMap<Color, Color>> {
    if colors.len() < 2 {
        return None;
    }
    let mut all: Vec<Vec<Color>> = Vec::new();
    permutations(colors, &mut Vec::new(), &mut all);
    all.retain(|order| order.iter().zip(colors).all(|(a, b)| a != b));
    let count = u64::try_from(all.len()).ok()?;
    let at = usize::try_from(roll % count.max(1)).ok()?;
    let order = all.get(at)?;
    Some(colors.iter().copied().zip(order.iter().copied()).collect())
}

/// Every ordering of `rest` after `head`, in lexical order of positions.
fn permutations(rest: &[Color], head: &mut Vec<Color>, out: &mut Vec<Vec<Color>>) {
    if rest.is_empty() {
        out.push(head.clone());
        return;
    }
    for at in 0..rest.len() {
        let mut left = rest.to_vec();
        let color = left.remove(at);
        head.push(color);
        permutations(&left, head, out);
        head.pop();
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a drawn cost keeps its mana value and shape, every color moves within the identity and none outside it,
    //! and the draw is the same for the same seed, seat and round.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn colors(letters: &str) -> Vec<Color> {
        Color::ALL
            .iter()
            .copied()
            .filter(|c| letters.chars().any(|l| Color::from_symbol(l) == Some(*c)))
            .collect()
    }

    #[rstest]
    #[case::two_colors_swap("WU", 1)]
    #[case::three_colors_turn("UBR", 2)]
    #[case::four_colors("WUBG", 9)]
    #[case::five_colors("WUBRG", 44)]
    fn every_derangement_moves_each_color_within_the_identity(
        #[case] letters: &str,
        #[case] ways: usize,
    ) {
        let identity = colors(letters);
        let mut seen = std::collections::BTreeSet::new();
        for roll in 0..400 {
            let map = derangement(&identity, roll).unwrap();
            assert_eq!(map.len(), identity.len());
            for (from, to) in &map {
                assert_ne!(from, to);
                assert!(identity.contains(to));
            }
            seen.insert(map.values().copied().collect::<Vec<_>>());
        }
        assert_eq!(seen.len(), ways, "every derangement is drawn");
        assert_eq!(derangement(&colors("G"), 3), None);
        assert_eq!(derangement(&[], 3), None);
    }

    #[rstest]
    #[case::plain("{2}{W}{U}", "WU", "{2}{U}{W}")]
    #[case::hybrid_keeps_both_halves("{W/U}{W/U}", "WU", "{W/U}{W/U}")]
    #[case::outside_colors_stay("{1}{R}{W}", "WU", "{1}{R}{U}")]
    #[case::phyrexian_and_mono_hybrid("{W/P}{2/U}{C}", "WU", "{U/P}{2/W}{C}")]
    fn a_cost_keeps_its_shape(#[case] cost: &str, #[case] letters: &str, #[case] want: &str) {
        let identity = colors(letters);
        let map = derangement(&identity, 0).unwrap();
        let cost = ManaCost::parse(cost).unwrap();
        let want = ManaCost::parse(want).unwrap();
        assert_eq!(recolored(&cost, &map), want);
    }

    proptest! {
        #[test]
        fn a_drawn_cost_keeps_its_mana_value_and_identity(
            seed in any::<u64>(),
            seat in 0usize..6,
            round in 0u32..40,
            generic in 0u32..5,
            pips in proptest::collection::vec(0usize..5, 0..6),
            mask in 2u8..32,
        ) {
            let identity: Vec<Color> = Color::ALL
                .iter()
                .enumerate()
                .filter(|(at, _)| mask & (1 << at) != 0)
                .map(|(_, c)| *c)
                .collect();
            prop_assume!(identity.len() >= 2);
            let cost = ManaCost::from_symbols(
                std::iter::once(ManaSymbol::Generic(generic)).chain(
                    pips.iter().map(|at| ManaSymbol::Colored(identity[at % identity.len()])),
                ),
            );
            let map = derangement(&identity, roll(seed, seat, round)).unwrap();
            prop_assert_eq!(&map, &derangement(&identity, roll(seed, seat, round)).unwrap());
            let drawn = recolored(&cost, &map);
            prop_assert_eq!(drawn.mana_value(), cost.mana_value());
            prop_assert!(drawn.colors().iter().all(|c| identity.contains(&c)));
            prop_assert_eq!(drawn.symbols().len(), cost.symbols().len());
        }
    }
}
