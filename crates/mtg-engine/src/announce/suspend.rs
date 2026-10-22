//! Suspending a card (CR 702.62a): a special action that does not use the stack. The announcement is only the payment of
//! the suspend cost, which can be backed out of; the events that follow it exile the card face up with its time counters.
//! The payment is shared with foretell (`announce/foretell.rs`).

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use super::Next;
use crate::cast::payment_decision;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::events::{Event, ZoneCause};
use crate::help::Offered;
use crate::keywords::suspend::time_counter;
use crate::mana::can_afford;
use crate::object::ObjectRef;
use crate::priority::mana_options;
use crate::spend::Spend;
use crate::state::GameState;
use crate::ways;
use crate::zones::Zone;

/// The payment decision of the suspend cost of `card`, or nothing when it cannot be suspended now.
pub(super) fn next(state: &GameState, player: PlayerId, card: ObjectId) -> Next {
    let Some((_, cost)) = ways::suspend_of(state, card) else {
        return Next::Drop;
    };
    pay(state, player, card, cost, || events(state, card))
}

/// The payment decision of `cost` for a special action on `card`, which completes with `events` once it is paid. A cost of
/// nothing needs no payment decision, and one that cannot be paid drops the action.
pub(super) fn pay(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    cost: ManaCost,
    events: impl FnOnce() -> Option<Vec<Event>>,
) -> Next {
    if cost.mana_value() == 0 {
        return events().map_or(Next::Drop, Next::Commit);
    }
    let options = payment_decision(
        state,
        player,
        card,
        cost,
        mana_options(state, player),
        Offered::default(),
        Spend::other(),
        true,
        0,
    );
    match options {
        Some(options)
            if can_afford(
                &options.cost,
                &options.pool,
                &options.sources,
                options.life,
                &options.spend,
            ) =>
        {
            Next::Ask(Decision {
                player,
                kind: DecisionKind::PayMana(options),
            })
        }
        _ => Next::Drop,
    }
}

/// The events of suspending `card`: it is exiled face up and takes its time counters. They come between the life and the
/// mana of the payment.
pub(crate) fn events(state: &GameState, card: ObjectId) -> Option<Vec<Event>> {
    let (time, _) = ways::suspend_of(state, card)?;
    let object = state.object(card)?;
    let Zone::Hand(_) = object.zone() else {
        return None;
    };
    Some(vec![
        Event::ZoneChanged {
            object: card,
            from: object.zone(),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        },
        Event::CountersChanged(CountersChanged {
            on: Target::Object(ObjectRef {
                id: card,
                counter: object.counter() + 1,
            }),
            counter: time_counter(),
            delta: i32::try_from(time).ok()?,
        }),
    ])
}
