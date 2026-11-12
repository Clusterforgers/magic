//! Offering (CR 702.48): "[Quality] offering" lets its card be cast any time its controller could cast an instant by
//! sacrificing a permanent of that quality and paying the difference in mana costs between the card and the permanent
//! sacrificed, colour included. It is an alternative cost (CR 702.48b): the way sacrifices one permanent as the first part
//! of its cost, and the mana cost of what is sacrificed comes off the card's mana cost.

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::{Amount, CastWay};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol};

use crate::parts::{Part, Payer};
use crate::state::GameState;
use crate::ways::{Way, cast_cost_with_x, cost_words};
use crate::zones::Zone;

/// The way the offering of the card `id` gives where it is: from a hand, at instant timing, for its mana cost and the
/// sacrifice of one permanent of the quality; nothing for another keyword or zone.
pub(crate) fn r301c_offering_way(
    state: &GameState,
    id: ObjectId,
    keyword: &Keyword,
) -> Option<Way> {
    let Keyword::Offering(quality) = keyword else {
        return None;
    };
    let card = state.object(id)?;
    if !matches!(card.zone(), Zone::Hand(_)) {
        return None;
    }
    let sacrifice = Cost(vec![CostItem::Sacrifice {
        filter: quality.clone(),
        count: Amount::Fixed(1),
    }]);
    let (_, parts, _) = cast_cost_with_x(&sacrifice)?;
    let mana = card.mana_cost().cloned().unwrap_or(ManaCost::ZERO);
    Some(Way {
        alternative: Some(CastWay::Offering),
        label: format!("Offering {}", cost_words(&mana, &parts)),
        mana: Some(mana),
        instant: true,
        variant: 0,
        parts,
    })
}

/// `cost` less the mana cost `less`: each coloured or hybrid symbol of `less` takes off the same symbol, else as much
/// generic mana as it is worth; its generic mana takes off generic mana; nothing goes below zero.
pub(crate) fn r301c_offering_difference(cost: &ManaCost, less: &ManaCost) -> ManaCost {
    let mut symbols: Vec<ManaSymbol> = cost.symbols().to_vec();
    let mut generic_off = 0u32;
    for symbol in less.symbols() {
        if let ManaSymbol::Generic(amount) = symbol {
            generic_off = generic_off.saturating_add(*amount);
        } else if let Some(at) = symbols.iter().position(|held| held == symbol) {
            symbols.remove(at);
        } else {
            // A symbol the card lacks takes off as much generic mana as it is worth ({2/W} two).
            let worth = ManaCost::from_symbols([*symbol]).mana_value();
            generic_off = generic_off.saturating_add(worth);
        }
    }
    let generic: u32 = symbols
        .iter()
        .map(|symbol| match symbol {
            ManaSymbol::Generic(amount) => *amount,
            _ => 0,
        })
        .sum::<u32>()
        .saturating_sub(generic_off);
    let rest = symbols
        .into_iter()
        .filter(|symbol| !matches!(symbol, ManaSymbol::Generic(_)));
    let generic = (generic > 0).then_some(ManaSymbol::Generic(generic));
    ManaCost::from_symbols(generic.into_iter().chain(rest))
}

/// The permanents `player` may sacrifice for the first part of `parts`, the offering's sacrifice.
pub(crate) fn r301c_offering_candidates(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    parts: &[Part],
) -> Vec<ObjectId> {
    let Some(Part::Pick(pick)) = parts.first() else {
        return Vec::new();
    };
    pick.candidates(&Payer {
        state,
        player,
        source: id,
        barred: &[],
        granter: None,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the difference in mana costs of offering takes each symbol off the same symbol, a symbol the card lacks off its generic mana as much as it is worth, generic off generic, and never goes below nothing.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::r301c_offering_difference;

    #[rstest]
    #[case::same_colour("{4}{R}{R}", "{1}{R}", "{3}{R}")]
    #[case::both_coloured_symbols("{4}{R}{R}", "{1}{R}{R}", "{3}")]
    #[case::another_colour_takes_generic("{4}{W}{W}", "{1}{G}", "{2}{W}{W}")]
    #[case::colourless("{7}{R}{R}", "{1}", "{6}{R}{R}")]
    #[case::more_than_the_generic("{1}{U}", "{3}{B}", "{U}")]
    #[case::a_hybrid_symbol_worth_two("{5}{B}{B}", "{2/B}", "{3}{B}{B}")]
    #[case::nothing("{6}{G}{G}", "", "{6}{G}{G}")]
    fn the_difference_in_mana_costs(#[case] cost: &str, #[case] less: &str, #[case] left: &str) {
        let parse = |text: &str| {
            if text.is_empty() {
                ManaCost::ZERO
            } else {
                ManaCost::parse(text).unwrap()
            }
        };
        assert_eq!(
            r301c_offering_difference(&parse(cost), &parse(less)).to_string(),
            left
        );
    }
}
