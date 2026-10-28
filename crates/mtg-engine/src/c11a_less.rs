//! The changes an activated ability prints of its own cost (CR 601.2f, 602.2b): "This ability costs {1} less to
//! activate for each artifact you control.", "This ability costs {1} more to activate for each card in your hand." They
//! are read as the ability is activated, by its controller: the increases are added first, then the reductions take
//! their mana off the way a spell's cost reductions do.

use mtg_cards::ability::CostChange;
use mtg_cards::cost::CostItem;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::announce::c11a_reduced;
use crate::price::Price;
use crate::resolve::Scope;
use crate::state::{GameState, Tally};

/// The most times one change applies: a count this large is more mana than any cost has.
const C11A_MOST_TIMES: i64 = 100;

/// The mana of `price` with the changes of the ability applied, activated by `player` from `source`.
pub(crate) fn c11a_mana(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    price: &Price,
) -> Option<ManaCost> {
    let mana = price.mana.clone()?;
    let Some(object) = state.object(source).filter(|_| !price.changes.is_empty()) else {
        return Some(mana);
    };
    let scope = Scope::announcing(state, player, object.reference());
    let (mut more, mut less) = (ManaCost::ZERO, ManaCost::ZERO);
    for item in &price.changes {
        let CostItem::Changed {
            change,
            mana: unit,
            per,
            condition,
        } = item
        else {
            continue;
        };
        if condition
            .as_ref()
            .is_some_and(|held| !scope.holds(held).unwrap_or(false))
        {
            continue;
        }
        let times = scope.amount(per).unwrap_or(0).clamp(0, C11A_MOST_TIMES);
        let part = match change {
            CostChange::Increase => &mut more,
            CostChange::Reduce => &mut less,
        };
        for _ in 0..times {
            *part = c11a_joined(part.clone(), unit.rest());
        }
    }
    Some(c11a_reduced(&c11a_joined(mana, &more), &less))
}

/// `cost` with every symbol of `more` added.
fn c11a_joined(cost: ManaCost, more: &ManaCost) -> ManaCost {
    more.symbols()
        .iter()
        .fold(cost, |cost, symbol| cost.with(*symbol))
}

/// Counts the spell `object` that `player` casts among the creature and artifact spells of the turn ("the first
/// creature spell you cast each turn").
pub(crate) fn c11a_tally_kinds(state: &mut GameState, player: PlayerId, object: ObjectId) {
    let counted: Vec<Tally> = [
        (CardType::Creature, Tally::C11aCreatureCast),
        (CardType::Artifact, Tally::C11aArtifactCast),
    ]
    .into_iter()
    .filter(|(kind, _)| {
        state
            .object(object)
            .is_some_and(|spell| spell.card().is_type(*kind))
    })
    .map(|(_, tally)| tally)
    .collect();
    for tally in counted {
        state.tally(player, tally);
    }
}
