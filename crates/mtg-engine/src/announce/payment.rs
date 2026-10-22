//! Mana abilities and payment (CR 601.2g, 601.2h), after which the spell or ability is on the stack (CR 601.2i).
//!
//! A total cost with mana asks `PayMana`, which can still be backed out of; the events of the cast or activation follow
//! the payment (`cast.rs`). A cost the player cannot pay rewinds the announcement. An ability with no mana to pay, and
//! a triggered ability, go on the stack at once.

use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;

use super::x::XCost;
use super::{Ctx, Next, What, Work};
use crate::abilities::PutOnStack;
use crate::cast::{activation_events, payment_decision, spell_cast};
use crate::choose::Choices;
use crate::decision::DecisionKind;
use crate::events::Event;
use crate::help::Offered;
use crate::mana::{ManaOption, can_afford_with};
use crate::spend::Spend;
use crate::state::GameState;
use crate::triggers::PendingTrigger;

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Next {
    let Some(total) = work.total.take() else {
        return Next::Drop;
    };
    match (ctx.what, total.mana) {
        (What::Trigger(waiting), _) => {
            Next::Commit(vec![trigger_event(ctx.state, waiting, work.choices)])
        }
        // A spell a resolution lets the player cast for nothing needs no payment decision.
        (What::Spell(spell), Some(cost))
            if ctx.granted.is_some()
                && cost.mana_value() == 0
                && total.life == 0
                && ctx.needs.cost_parts(work.choices).is_empty() =>
        {
            Next::Commit(spell_cast(ctx.player, *spell, work.choices, true))
        }
        (What::Spell(spell), Some(cost)) => {
            pay(ctx, *spell, cost, total.sources, total.help, total.life)
        }
        (What::Spell(_), None) => Next::Drop,
        (What::Ability(ability), Some(cost)) => pay(
            ctx,
            ability.object,
            cost,
            total.sources,
            total.help,
            total.life,
        ),
        (What::Ability(ability), None) => {
            activation_events(ctx.state, ctx.player, *ability, work.choices)
                .map_or(Next::Drop, Next::Commit)
        }
    }
}

/// What the payment of the cost being announced is for: the spell or the ability, with whether the cost has {X}.
pub(super) fn spend(ctx: &Ctx<'_>) -> Spend {
    let has_x = XCost::of(ctx.state, ctx.what, ctx.ability).is_some();
    match ctx.what {
        What::Spell(spell) => Spend::for_spell(ctx.state, *spell, has_x),
        What::Ability(ability) => Spend::for_ability(ctx.state, ability.object, has_x),
        What::Trigger(_) => Spend::other(),
    }
}

/// The payment decision for `cost` on behalf of `object`, if it can be paid; nothing is paid until it is answered, so
/// the player can back out. `reserved_life` is the life other parts of the cost pay.
fn pay(
    ctx: &Ctx<'_>,
    object: ObjectId,
    cost: ManaCost,
    sources: Vec<ManaOption>,
    help: Offered,
    reserved_life: u32,
) -> Next {
    match payment_decision(
        ctx.state,
        ctx.player,
        object,
        cost,
        sources,
        help,
        spend(ctx),
        ctx.can_cancel(),
        reserved_life,
    ) {
        Some(options)
            if can_afford_with(
                &options.cost,
                &options.pool,
                &options.sources,
                options.help(),
                options.life,
                &options.spend,
            ) =>
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
