//! Paying to ignore an effect: "Any player may pay {2} for that player to ignore this effect until end of turn."
//! (`GameRule::PayToIgnore`). Paying is a special action of any player with priority that does not use the stack (CR
//! 116.2); the payer is logged with the source, and the source's effects leave them out until end of turn through
//! `Condition::PaidToIgnore`.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::decision::PaidIgnore;
use crate::events::Event;
use crate::mana::{ManaOption, ManaPool, can_afford};
use crate::mana_takes::for_card;
use crate::spend::Spend;
use crate::state::GameState;

/// The price for `player` to ignore the effects of `source`, while its rule is in force and they have not paid this
/// turn.
pub(crate) fn offer(state: &GameState, player: PlayerId, source: ObjectId) -> Option<ManaCost> {
    crate::rules::pay_to_ignore(state)
        .iter()
        .find(|(found, _)| found.id == source && !state.r6703_ignored.contains(&(player, *found)))
        .map(|(_, cost)| cost.clone())
}

/// The sources whose effects `player` may ignore now by paying, with the price, when the mana they could make pays it.
pub(crate) fn ignorable(
    state: &GameState,
    player: PlayerId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<PaidIgnore> {
    let mut found: Vec<PaidIgnore> = Vec::new();
    for (source, _) in crate::rules::pay_to_ignore(state) {
        if found.iter().any(|offered| offered.object == source.id) {
            continue;
        }
        let Some(cost) = offer(state, player, source.id) else {
            continue;
        };
        if can_afford(
            &cost,
            pool,
            &for_card(state, sources, source.id),
            life,
            &Spend::other(),
        ) {
            found.push(PaidIgnore {
                object: source.id,
                cost,
            });
        }
    }
    found
}

/// The event of `player` ignoring the effects of `source`; none when they may not now.
pub(crate) fn events(state: &GameState, player: PlayerId, source: ObjectId) -> Option<Vec<Event>> {
    offer(state, player, source)?;
    let source = state.object(source)?.reference();
    Some(vec![Event::IgnoredEffect { player, source }])
}
