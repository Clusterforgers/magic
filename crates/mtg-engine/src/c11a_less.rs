//! The reductions an activated ability prints for itself (CR 601.2f, 602.2b): "This ability costs {1} less to activate
//! for each artifact you control." The reductions are read as the ability is activated, by its controller, and take
//! their mana off the cost the way a spell's cost reductions do.

use mtg_cards::cost::CostItem;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::announce::c11a_reduced;
use crate::price::Price;
use crate::resolve::Scope;
use crate::state::GameState;

/// The most times one reduction applies: a count this large is more mana than any cost has.
const C11A_MOST_TIMES: i64 = 100;

/// The mana of `price` with the reductions of the ability applied, activated by `player` from `source`.
pub(crate) fn c11a_mana(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    price: &Price,
) -> Option<ManaCost> {
    let mana = price.mana.clone()?;
    if price.less.is_empty() {
        return Some(mana);
    }
    let Some(object) = state.object(source) else {
        return Some(mana);
    };
    let scope = Scope::announcing(state, player, object.reference());
    let mut less = ManaCost::ZERO;
    for item in &price.less {
        let CostItem::LessToActivate {
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
        for _ in 0..times {
            less = unit
                .rest()
                .symbols()
                .iter()
                .fold(less, |less, symbol| less.with(*symbol));
        }
    }
    Some(c11a_reduced(&mana, &less))
}
