//! Casting a spell: the payment decision and the events a valid payment produces.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::decision::{PayChoice, PayOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::mana::{Payment, auto_payment, can_afford, check_payment};
use crate::priority::mana_options;
use crate::state::GameState;

/// The payment decision for casting `spell`, or `None` if it has no cost to pay.
pub(crate) fn pay_options(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
) -> Option<PayOptions> {
    let cost = state.object(spell)?.card().mana_cost?;
    let pool = *state.player(player)?.mana();
    let sources = mana_options(state, player);
    Some(PayOptions {
        spell,
        cost,
        pool,
        auto_available: can_afford(&cost, &pool, &sources),
        sources,
    })
}

fn payment_events(
    player: PlayerId,
    spell: ObjectId,
    options: &PayOptions,
    payment: &Payment,
) -> Result<Vec<Event>, IllegalAnswer> {
    let spent = check_payment(&options.cost, &options.pool, &options.sources, payment)?;
    let mut events = vec![Event::SpellCast {
        player,
        object: spell,
    }];
    for tap in &payment.taps {
        let produces = options
            .sources
            .iter()
            .find(|o| o.source == *tap)
            .map(|o| o.produces)
            .ok_or(IllegalAnswer::NotOffered("mana source"))?;
        events.push(Event::PermanentTapped { object: tap.object });
        events.push(Event::ManaAdded {
            player,
            mana: produces,
            source: Some(tap.object),
        });
    }
    if !spent.is_empty() {
        events.push(Event::ManaSpent {
            player,
            mana: spent,
        });
    }
    Ok(events)
}

/// Validates a payment choice and produces the casting events.
pub(crate) fn pay_answer(
    player: PlayerId,
    options: &PayOptions,
    choice: PayChoice,
) -> Result<Outcome, EngineError> {
    let payment = match choice {
        PayChoice::Cancel => return Ok(Outcome::CancelCast),
        PayChoice::Manual(payment) => payment,
        PayChoice::Auto => auto_payment(&options.cost, &options.pool, &options.sources)
            .ok_or(IllegalAnswer::NotOffered("automatic payment"))?,
    };
    let events = payment_events(player, options.spell, options, &payment)?;
    Ok(Outcome::Events(events))
}
