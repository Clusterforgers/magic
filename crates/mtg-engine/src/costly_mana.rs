//! Mana abilities that cost mana (a filter land's `{1}, {T}`, a Signet, a Coffers) used at priority: offered while
//! floating mana or the player's other sources can pay the cost, which is then paid automatically, pool first.

use mtg_core::ids::PlayerId;

use crate::cast;
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::help::Offered;
use crate::mana::{ManaOption, ManaPool, can_afford};
use crate::spend::Spend;
use crate::state::GameState;

/// The sources that may pay `option`'s cost: every other permanent's mana abilities.
fn payers(all: &[ManaOption], option: &ManaOption) -> Vec<ManaOption> {
    all.iter()
        .filter(|other| other.source.object != option.source.object)
        .cloned()
        .collect()
}

/// The mana abilities of `all` offered at priority: none that takes other permanents or cards, and one that costs mana
/// only while it is [`payable`].
pub(crate) fn at_priority(
    state: &GameState,
    all: &[ManaOption],
    pool: &ManaPool,
    life: i32,
) -> Vec<ManaOption> {
    all.iter()
        .filter(|option| {
            option.takes.is_empty()
                && (!option.is_costly() || payable(state, option, all, pool, life))
        })
        .cloned()
        .collect()
}

/// Whether `option`, a mana ability that costs mana, can be activated now from `pool` and the other sources in `all`.
fn payable(
    state: &GameState,
    option: &ManaOption,
    all: &[ManaOption],
    pool: &ManaPool,
    life: i32,
) -> bool {
    let spend = Spend::for_ability(state, option.source.object, false);
    can_afford(&option.cost, pool, &payers(all, option), life, &spend)
}

/// The events of paying `option`'s mana cost automatically, before the ability itself is used.
pub(crate) fn cost_events(
    state: &GameState,
    player: PlayerId,
    option: &ManaOption,
    all: &[ManaOption],
) -> Result<Vec<Event>, EngineError> {
    let spend = Spend::for_ability(state, option.source.object, false);
    let options = cast::payment_decision(
        state,
        player,
        option.source.object,
        option.cost.clone(),
        payers(all, option),
        Offered::default(),
        spend,
        false,
        0,
    )
    .ok_or(IllegalAnswer::NotOffered("mana ability"))?;
    cast::auto_pay_events(state, player, &options)
}
