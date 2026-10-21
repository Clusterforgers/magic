//! Mana abilities and payment (CR 601.2g, 601.2h), after which the spell or ability is on the stack (CR 601.2i).
//!
//! A total cost with mana asks `PayMana`, which can still be backed out of; the events of the cast or activation follow
//! the payment (`cast.rs`). A cost the player cannot pay rewinds the announcement. An ability with no mana to pay, and
//! a triggered ability, go on the stack at once.

use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;

use super::{Ctx, Next, What, Work};
use crate::abilities::PutOnStack;
use crate::cast::{activation_events, payment_decision};
use crate::choose::Choices;
use crate::decision::DecisionKind;
use crate::events::Event;
use crate::mana::{ManaOption, can_afford};
use crate::state::GameState;
use crate::triggers::PendingTrigger;

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    let Some(total) = work.total.take() else {
        return Some(Next::Drop);
    };
    Some(match (ctx.what, total.mana) {
        (What::Trigger(waiting), _) => {
            Next::Commit(vec![trigger_event(ctx.state, waiting, work.choices)])
        }
        (What::Spell(spell), Some(cost)) => pay(ctx, *spell, cost, total.sources),
        (What::Spell(_), None) => Next::Drop,
        (What::Ability(ability), Some(cost)) => pay(ctx, ability.object, cost, total.sources),
        (What::Ability(ability), None) => {
            activation_events(ctx.state, ctx.player, *ability, work.choices)
                .map_or(Next::Drop, Next::Commit)
        }
    })
}

/// The payment decision for `cost` on behalf of `object`, if it can be paid; nothing is paid until it is answered, so
/// the player can back out.
fn pay(ctx: &Ctx<'_>, object: ObjectId, cost: ManaCost, sources: Vec<ManaOption>) -> Next {
    match payment_decision(ctx.state, ctx.player, object, cost, sources, true) {
        Some(options)
            if can_afford(&options.cost, &options.pool, &options.sources, options.life) =>
        {
            ctx.ask(DecisionKind::PayMana(options))
        }
        _ => Next::Drop,
    }
}

/// The event that puts a waiting triggered ability on the stack with the targets its controller chose.
fn trigger_event(state: &GameState, waiting: &PendingTrigger, choices: &Choices) -> Event {
    let t = &waiting.trigger;
    Event::AbilityPutOnStack(Box::new(PutOnStack {
        object: state.next_object(),
        source: t.source,
        snapshot: t.snapshot.clone(),
        ability: t.ability,
        origin: t.origin,
        controller: t.controller,
        choices: choices.clone(),
        cause: Some(t.cause.clone()),
        delayed: t.delayed,
    }))
}
