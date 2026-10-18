//! Casting a spell: the payment decision and the events a valid payment produces.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::decision::{PayChoice, PayOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::mana::{Payment, auto_payment, can_afford, check_payment, complete_payment};
use crate::object::PermanentState;
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
        // The cast only starts to change the game when the payment is accepted, so backing out is free.
        can_cancel: true,
        sources,
    })
}

/// The offered permanents that are tapped (or gone) in `state`; no payment may use them.
fn unusable(state: &GameState, options: &PayOptions) -> Vec<ObjectId> {
    options
        .sources
        .iter()
        .map(|o| o.source.object)
        .filter(|id| {
            state
                .object(*id)
                .and_then(|o| o.permanent())
                .is_none_or(PermanentState::tapped)
        })
        .collect()
}

fn payment_events(
    player: PlayerId,
    spell: ObjectId,
    options: &PayOptions,
    tapped: &[ObjectId],
    payment: &Payment,
) -> Result<Vec<Event>, IllegalAnswer> {
    let spent = check_payment(
        &options.cost,
        &options.pool,
        &options.sources,
        tapped,
        payment,
    )?;
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
    state: &GameState,
    player: PlayerId,
    options: &PayOptions,
    choice: PayChoice,
) -> Result<Outcome, EngineError> {
    pay_choice(player, options, &unusable(state, options), choice)
}

/// [`pay_answer`] with the tapped permanents already worked out, so it needs no game state.
fn pay_choice(
    player: PlayerId,
    options: &PayOptions,
    tapped: &[ObjectId],
    choice: PayChoice,
) -> Result<Outcome, EngineError> {
    let payment = match choice {
        PayChoice::Cancel if options.can_cancel => return Ok(Outcome::CancelCast),
        PayChoice::Cancel => return Err(IllegalAnswer::NotOffered("cancel").into()),
        PayChoice::Manual(payment) => payment,
        PayChoice::Complete(partial) => complete_payment(
            &options.cost,
            &options.pool,
            &options.sources,
            tapped,
            &partial,
        )
        .map_err(IllegalAnswer::from)?,
        PayChoice::Auto => auto_payment(&options.cost, &options.pool, &options.sources)
            .ok_or(IllegalAnswer::NotOffered("automatic payment"))?,
    };
    let events = payment_events(player, options.spell, options, tapped, &payment)?;
    Ok(Outcome::Events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: each payment choice is validated as a whole before it produces events, and cancelling follows the options' flag.
    use mtg_core::mana::{ManaCost, ManaType};
    use rstest::rstest;

    use super::*;
    use crate::mana::{ManaOption, ManaPool, ManaSource};

    fn options(can_cancel: bool) -> PayOptions {
        let source = |object, produces| ManaOption {
            source: ManaSource {
                object: ObjectId(object),
                ability: 0,
            },
            produces,
        };
        PayOptions {
            spell: ObjectId(9),
            cost: ManaCost::parse("{1}{G}").unwrap(),
            pool: ManaPool::default(),
            sources: vec![source(1, ManaType::Green), source(2, ManaType::White)],
            auto_available: true,
            can_cancel,
        }
    }

    fn taps(objects: &[u64]) -> Payment {
        Payment {
            pool: vec![],
            taps: objects
                .iter()
                .map(|o| ManaSource {
                    object: ObjectId(*o),
                    ability: 0,
                })
                .collect(),
        }
    }

    fn outcome(
        options: &PayOptions,
        tapped: &[u64],
        choice: PayChoice,
    ) -> Result<Outcome, EngineError> {
        let tapped: Vec<ObjectId> = tapped.iter().map(|o| ObjectId(*o)).collect();
        pay_choice(PlayerId(0), options, &tapped, choice)
    }

    #[test]
    fn cancel_follows_the_flag_and_logs_nothing() {
        assert!(matches!(
            outcome(&options(true), &[], PayChoice::Cancel),
            Ok(Outcome::CancelCast)
        ));
        assert_eq!(
            outcome(&options(false), &[], PayChoice::Cancel).unwrap_err(),
            EngineError::Illegal(IllegalAnswer::NotOffered("cancel"))
        );
    }

    #[rstest]
    #[case::manual(PayChoice::Manual(taps(&[1, 2])))]
    #[case::complete_from_nothing(PayChoice::Complete(taps(&[])))]
    #[case::complete_from_one(PayChoice::Complete(taps(&[2])))]
    #[case::auto(PayChoice::Auto)]
    fn every_way_to_pay_casts_and_taps_both_lands(#[case] choice: PayChoice) {
        let Ok(Outcome::Events(events)) = outcome(&options(true), &[], choice) else {
            panic!("the payment must be accepted");
        };
        assert!(matches!(events[0], Event::SpellCast { .. }));
        let tapped: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::PermanentTapped { object } => Some(object.0),
                _ => None,
            })
            .collect();
        assert_eq!(tapped.len(), 2);
        assert!(tapped.contains(&1) && tapped.contains(&2));
    }

    #[rstest]
    #[case::complete_with_a_tapped_land(PayChoice::Complete(taps(&[2])), &[2])]
    #[case::manual_with_a_tapped_land(PayChoice::Manual(taps(&[1, 2])), &[1])]
    #[case::complete_that_wastes_mana(PayChoice::Complete(taps(&[1, 2, 2])), &[])]
    fn rejected_payments_produce_no_events(#[case] choice: PayChoice, #[case] tapped: &[u64]) {
        assert!(matches!(
            outcome(&options(true), tapped, choice),
            Err(EngineError::Illegal(IllegalAnswer::Payment(_)))
        ));
    }
}
