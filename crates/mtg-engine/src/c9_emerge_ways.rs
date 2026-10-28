//! The ways emerge and harmonize give a card (CR 702.119, 702.180): an alternative cost whose first part is the creature
//! that reduces it, sacrificed for emerge and tapped, if the caster chooses, for harmonize.

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::{Amount, CastWay};
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use crate::state::GameState;
use crate::ways::{Way, cast_cost_with_x, cost_words};
use crate::zones::Zone;

/// The variant of the harmonize way that taps a creature, apart from the variants of the costs a card lets its caster
/// choose between (`announce/required.rs`).
pub(crate) const C9_EMERGE_TAPPED: u8 = 0x80;

/// "a creature": the creature the first part of the way takes.
fn c9_emerge_creature() -> Selector {
    Selector(vec![Pred::Type(CardType::Creature)])
}

/// `cost` with `first` before its other items.
fn c9_emerge_with(first: CostItem, cost: &Cost) -> Cost {
    Cost(std::iter::once(first).chain(cost.0.iter().cloned()).collect())
}

/// The ways emerge or harmonize give the card `id` where it is: emerge from a hand, harmonize from a graveyard as it is
/// and with a creature tapped. Nothing for any other keyword or zone, or for a cost the engine does not pay.
pub(crate) fn c9_emerge_ways(
    state: &GameState,
    id: ObjectId,
    keyword: &Keyword,
    instant: bool,
) -> Vec<Way> {
    let Some(zone) = state.object(id).map(crate::object::GameObject::zone) else {
        return Vec::new();
    };
    match keyword {
        Keyword::Emerge(cost) if matches!(zone, Zone::Hand(_)) => {
            let sacrifice = CostItem::Sacrifice {
                filter: c9_emerge_creature(),
                count: Amount::Fixed(1),
            };
            let cost = c9_emerge_with(sacrifice, cost);
            c9_emerge_way(CastWay::Emerge, "Emerge", &cost, 0, instant)
                .into_iter()
                .collect()
        }
        Keyword::Harmonize(cost) if matches!(zone, Zone::Graveyard(_)) => {
            let tap = CostItem::TapPermanents {
                filter: c9_emerge_creature(),
                count: Amount::Fixed(1),
            };
            let tapped = c9_emerge_with(tap, cost);
            [
                c9_emerge_way(CastWay::Harmonize, "Harmonize", cost, 0, instant),
                c9_emerge_way(
                    CastWay::Harmonize,
                    "Harmonize",
                    &tapped,
                    C9_EMERGE_TAPPED,
                    instant,
                ),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        _ => Vec::new(),
    }
}

/// The way `way` for `cost`, its `{X}` named first in the label.
fn c9_emerge_way(
    way: CastWay,
    name: &str,
    cost: &Cost,
    variant: u8,
    instant: bool,
) -> Option<Way> {
    let (mana, parts, x) = cast_cost_with_x(cost)?;
    let symbols = "{X}".repeat(usize::try_from(x).unwrap_or(0));
    let words = cost_words(&mana, &parts);
    let label = if symbols.is_empty() {
        format!("{name} {words}")
    } else if mana.mana_value() > 0 {
        format!("{name} {symbols}{words}")
    } else {
        format!("{name} {symbols}, {words}")
    };
    Some(Way {
        alternative: Some(way),
        label,
        mana: Some(mana),
        instant,
        parts,
        variant,
    })
}

/// How many `{X}` the harmonize cost among `keywords` holds ("Harmonize {X}{R}{R}"), announced in place of those of the
/// printed cost.
pub(crate) fn c9_emerge_harmonize_x(keywords: &[&Keyword]) -> u32 {
    keywords
        .iter()
        .find_map(|keyword| match keyword {
            Keyword::Harmonize(cost) => cast_cost_with_x(cost).map(|(_, _, x)| x),
            _ => None,
        })
        .unwrap_or(0)
}
