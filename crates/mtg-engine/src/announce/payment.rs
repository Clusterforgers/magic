//! Mana abilities and payment (CR 601.2g, 601.2h), after which the spell or ability is on the stack (CR 601.2i).
//!
//! A total cost with mana asks `PayMana`, which can still be backed out of; the events of the cast or activation follow
//! the payment (`cast.rs`). A cost the player cannot pay rewinds the announcement. An ability with no mana to pay, and
//! a triggered ability, go on the stack at once.

use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;

use super::x::XCost;
use super::{Ctx, Next, What, Work};
use crate::abilities::PutOnStack;
use crate::cast::{activation_events, payment_decision, r114_roll_asks, spell_cast};
use crate::choose::Choices;
use crate::decision::DecisionKind;
use crate::events::Event;
use crate::help::Offered;
use crate::mana::{ManaOption, can_afford_with};
use crate::r281_mana_at_priority::r281_while_paying;
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
        (What::Spell(spell), Some(cost)) => pay(
            ctx,
            *spell,
            cost,
            total.sources,
            total.help,
            (total.life, true),
            total.spend,
        ),
        // An attack is paid for by `attack_tax::next` and never gets here.
        (What::Spell(_) | What::Attack(..), None) | (What::Attack(..) | What::Block(_), _) => {
            Next::Drop
        }
        (What::Ability(ability), mana) => {
            // A die the cost rolls is rolled first, so the rules that change a roll can ask (CR 706.2, 601.2h); the
            // payment that follows can no longer be backed out of once a question showed the result.
            let rolled = &work.done.r114_rolled;
            let roll_life =
                match r114_roll_asks(ctx.state, ctx.player, *ability, work.choices, rolled) {
                    Some(Err(decision)) => return Next::Ask(decision),
                    Some(Ok(life)) => life,
                    None => 0,
                };
            match mana {
                Some(cost) => pay(
                    ctx,
                    ability.object,
                    cost,
                    total.sources,
                    total.help,
                    (total.life + roll_life, rolled.is_empty()),
                    total.spend,
                ),
                None => activate(ctx, *ability, work),
            }
        }
    }
}

/// The events of an activation that has no mana to pay: it goes on the stack at once.
fn activate(ctx: &Ctx<'_>, ability: crate::abilities::AbilityRef, work: &Work<'_>) -> Next {
    activation_events(
        ctx.state,
        ctx.player,
        ability,
        work.choices,
        &work.done.r114_rolled,
    )
    .map_or(Next::Drop, |mut events| {
        crate::r281_mana_at_priority::r281_resolve_at_once(ctx.state, &mut events);
        Next::Commit(events)
    })
}

/// What the payment of the cost being announced is for: the spell or the ability, with whether the cost has {X}.
pub(super) fn spend(ctx: &Ctx<'_>, way: Option<CastWay>) -> Spend {
    let x = XCost::of(ctx.state, ctx.player, ctx.what, ctx.ability);
    let has_x = x.is_some();
    match ctx.what {
        // Only an `{X}` of the mana cost is mana for X: "pay X life" is paid with life.
        What::Spell(spell) => Spend::for_spell(
            ctx.state,
            *spell,
            ctx.player,
            matches!(x, Some(XCost::Mana { .. })),
        )
        .cast_as(way),
        What::Ability(ability) => {
            let spend = Spend::for_ability(ctx.state, ability.object, has_x);
            match ctx.ability {
                Some(rules) => spend.of_ability(ctx.state, *ability, rules),
                None => spend,
            }
        }
        What::Trigger(_) | What::Attack(..) | What::Block(_) => {
            Spend::other().r111_by(ctx.state, ctx.player)
        }
    }
}

/// The payment decision for `cost` on behalf of `object`, if it can be paid; nothing is paid until it is answered, so
/// the player can back out unless `can_cancel` is false (a die the cost rolled has been seen). `reserved_life` is the life
/// other parts of the cost pay.
fn pay(
    ctx: &Ctx<'_>,
    object: ObjectId,
    cost: ManaCost,
    sources: Vec<ManaOption>,
    help: Offered,
    (reserved_life, can_cancel): (u32, bool),
    paying: Spend,
) -> Next {
    let reserved_life =
        crate::r4707_pay_costs::r4707_reserved_life(ctx.state, ctx.player, reserved_life);
    match payment_decision(
        ctx.state,
        ctx.player,
        object,
        cost,
        sources,
        help,
        paying,
        ctx.can_cancel() && can_cancel,
        reserved_life,
    ) {
        Some(mut options)
            if can_afford_with(
                &options.cost,
                &options.pool,
                &options.sources,
                options.help(),
                options.life,
                &options.spend,
            ) =>
        {
            if let What::Ability(ability) = ctx.what {
                options.ability = Some(Box::new(*ability));
            }
            options.announced = match (ctx.what, &ctx.granted) {
                (What::Spell(_), None) => r281_while_paying(ctx.state, ctx.player, None),
                (What::Ability(ability), None) => {
                    r281_while_paying(ctx.state, ctx.player, Some(*ability))
                }
                _ => Vec::new(),
            };
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
