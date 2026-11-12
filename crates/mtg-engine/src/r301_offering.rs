//! Offering (CR 702.48): a spell with "Fox offering" may be cast from its owner's hand any time they could cast an
//! instant by sacrificing a Fox as it is announced, its total cost reduced by the sacrificed permanent's mana cost,
//! colored mana included (CR 118.7c, 118.7d). Before the permanent is picked the one that takes off the most is assumed,
//! so a cast is offered when some permanent makes it affordable.

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::{Amount, CastWay};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol};

use crate::parts::{Part, Payer};
use crate::price::Price;
use crate::state::GameState;
use crate::ways::Way;
use crate::zones::Zone;

/// The way offering gives the card `id` in its owner's hand; none for another keyword or zone.
pub(crate) fn r301_offering_way(state: &GameState, id: ObjectId, keyword: &Keyword) -> Option<Way> {
    let Keyword::Offering(quality) = keyword else {
        return None;
    };
    let card = state.object(id)?;
    if !matches!(card.zone(), Zone::Hand(_)) {
        return None;
    }
    let printed = card.card().mana_cost.as_ref()?;
    let price = Price::announced(&Cost(vec![CostItem::Sacrifice {
        filter: quality.clone(),
        count: Amount::one(),
    }]))
    .ok()?;
    let words = price
        .parts
        .iter()
        .map(Part::text)
        .collect::<Vec<_>>()
        .join(", ");
    Some(Way {
        alternative: Some(CastWay::Offering),
        label: format!("Offering {printed}, {words}"),
        mana: None,
        instant: true,
        variant: 0,
        parts: price.parts,
    })
}

/// `cost` reduced by the mana cost `by` (CR 118.7): generic mana by its generic part; each colored or colorless symbol
/// by that symbol when the cost holds one, else by one generic mana; a hybrid symbol by whichever half the cost holds,
/// else by its generic or one generic mana; a Phyrexian symbol as its color. The cost never goes below nothing.
pub(crate) fn r301_reduced_by(cost: &ManaCost, by: &ManaCost) -> ManaCost {
    let mut rest: Vec<ManaSymbol> = cost
        .symbols()
        .iter()
        .copied()
        .filter(|symbol| !matches!(symbol, ManaSymbol::Generic(_)))
        .collect();
    let mut generic = cost.generic();
    for symbol in by.symbols() {
        let less = match *symbol {
            ManaSymbol::Generic(n) => n,
            ManaSymbol::Colored(color) | ManaSymbol::Phyrexian(color) => {
                u32::from(!take(ManaSymbol::Colored(color), &mut rest))
            }
            ManaSymbol::Colorless | ManaSymbol::ColorlessPhyrexian => {
                u32::from(!take(ManaSymbol::Colorless, &mut rest))
            }
            ManaSymbol::Hybrid(a, b) | ManaSymbol::HybridPhyrexian(a, b) => u32::from(
                !take(ManaSymbol::Colored(a), &mut rest)
                    && !take(ManaSymbol::Colored(b), &mut rest),
            ),
            ManaSymbol::MonoHybrid(color) => {
                if take(ManaSymbol::Colored(color), &mut rest) {
                    0
                } else {
                    2
                }
            }
            ManaSymbol::ColorlessHybrid(color) => u32::from(
                !take(ManaSymbol::Colored(color), &mut rest)
                    && !take(ManaSymbol::Colorless, &mut rest),
            ),
            ManaSymbol::Snow => 1,
        };
        generic = generic.saturating_sub(less);
    }
    ManaCost::from_symbols(rest).with(ManaSymbol::Generic(generic))
}

/// Takes one `wanted` symbol out of `rest`; whether there was one.
fn take(wanted: ManaSymbol, rest: &mut Vec<ManaSymbol>) -> bool {
    match rest.iter().position(|symbol| *symbol == wanted) {
        Some(at) => {
            rest.remove(at);
            true
        }
        None => false,
    }
}

/// The mana cost of a permanent as it stands; nothing for one without.
fn mana_cost_of(state: &GameState, object: ObjectId) -> ManaCost {
    state
        .object(object)
        .and_then(crate::object::GameObject::mana_cost)
        .cloned()
        .unwrap_or(ManaCost::ZERO)
}

/// `cost` of the spell `id` cast by offering, reduced by the permanent picked for the sacrifice, or before one is
/// picked by the one that leaves the least to pay.
pub(crate) fn r301_offering_cost(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    parts: &[Part],
    picked: Option<&[ObjectId]>,
    cost: &ManaCost,
) -> ManaCost {
    if let Some(picked) = picked.filter(|picked| !picked.is_empty()) {
        return picked.iter().fold(cost.clone(), |left, object| {
            r301_reduced_by(&left, &mana_cost_of(state, *object))
        });
    }
    let Some(Part::Pick(pick)) = parts.first() else {
        return cost.clone();
    };
    let payer = Payer {
        state,
        player,
        source: id,
        barred: &[id],
        granter: None,
    };
    pick.candidates(&payer)
        .into_iter()
        .map(|object| r301_reduced_by(cost, &mana_cost_of(state, object)))
        .min_by_key(ManaCost::mana_value)
        .unwrap_or_else(|| cost.clone())
}

#[cfg(test)]
mod tests {
    //! Goal: an offering reduces generic mana by generic mana and each colored symbol by the same symbol, or by one generic mana when the cost has none of that color, never below nothing.
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::r301_reduced_by;

    #[rstest]
    #[case::fox_of_the_same_color("{5}{W}{W}", "{1}{W}", "{4}{W}")]
    #[case::another_color_takes_generic("{5}{W}{W}", "{1}{G}", "{3}{W}{W}")]
    #[case::bigger_than_the_cost("{1}{R}", "{4}{R}{R}", "{0}")]
    #[case::colorless_card("{4}{B}", "{3}", "{1}{B}")]
    #[case::nothing_to_take("{3}{U}", "{0}", "{3}{U}")]
    #[case::hybrid_takes_the_half_present("{2}{G}", "{R/G}", "{2}")]
    fn r301_an_offering_reduces_the_cost(#[case] cost: &str, #[case] by: &str, #[case] left: &str) {
        let parse = |text: &str| ManaCost::parse(text).unwrap();
        let reduced = r301_reduced_by(&parse(cost), &parse(by));
        assert_eq!(reduced.mana_value(), parse(left).mana_value(), "{reduced}");
        assert_eq!(reduced.colors(), parse(left).colors(), "{reduced}");
    }
}
