//! Casting a spell: the payment decision and the events a valid payment produces.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::decision::{PayChoice, PayOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::mana::{Payment, auto_payment, check_payment, complete_payment, max_life_payment};
use crate::object::PermanentState;
use crate::priority::mana_options;
use crate::state::GameState;

/// The payment decision for casting `spell`, or `None` if it has no cost to pay.
pub(crate) fn pay_options(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
) -> Option<PayOptions> {
    let cost = state.object(spell)?.card().mana_cost.clone()?;
    let seat = state.player(player)?;
    let (pool, life) = (*seat.mana(), seat.life());
    let sources = mana_options(state, player);
    Some(PayOptions {
        spell,
        auto_available: auto_payment(&cost, &pool, &sources, life).is_some(),
        // The cast only starts to change the game when the payment is accepted, so backing out is free.
        can_cancel: true,
        life,
        max_life: max_life_payment(&cost, life),
        cost,
        pool,
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
        options.life,
        payment,
    )?;
    let mut events = Vec::new();
    // Life is part of the cost, paid in the same step as the mana, so it comes before the spell goes on the stack.
    if payment.life > 0 {
        events.push(Event::LifePaid {
            player,
            amount: payment.life,
        });
    }
    events.push(Event::SpellCast {
        player,
        object: spell,
    });
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
            options.life,
            &partial,
        )
        .map_err(IllegalAnswer::from)?,
        PayChoice::Auto => {
            auto_payment(&options.cost, &options.pool, &options.sources, options.life)
                .ok_or(IllegalAnswer::NotOffered("automatic payment"))?
        }
    };
    let events = payment_events(player, options.spell, options, tapped, &payment)?;
    Ok(Outcome::Events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: each payment choice is validated as a whole before it produces events, life is paid in the same step and before the spell goes on the stack, and cancelling follows the options' flag.
    use mtg_core::mana::{ManaCost, ManaType};
    use rstest::rstest;

    use super::*;
    use crate::mana::{ManaOption, ManaPool, ManaSource};

    fn source(object: u64, produces: ManaType) -> ManaOption {
        ManaOption {
            source: ManaSource {
                object: ObjectId(object),
                ability: 0,
            },
            produces,
        }
    }

    fn options_for(
        cost: &str,
        sources: Vec<ManaOption>,
        life: i32,
        can_cancel: bool,
    ) -> PayOptions {
        let cost = ManaCost::parse(cost).unwrap();
        PayOptions {
            spell: ObjectId(9),
            auto_available: crate::mana::auto_payment(&cost, &ManaPool::default(), &sources, life)
                .is_some(),
            can_cancel,
            life,
            max_life: max_life_payment(&cost, life),
            cost,
            pool: ManaPool::default(),
            sources,
        }
    }

    fn options(can_cancel: bool) -> PayOptions {
        options_for(
            "{1}{G}",
            vec![source(1, ManaType::Green), source(2, ManaType::White)],
            20,
            can_cancel,
        )
    }

    fn taps(objects: &[u64]) -> Payment {
        Payment {
            taps: objects
                .iter()
                .map(|o| ManaSource {
                    object: ObjectId(*o),
                    ability: 0,
                })
                .collect(),
            ..Payment::default()
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

    fn events_of(outcome: Result<Outcome, EngineError>) -> Vec<Event> {
        match outcome {
            Ok(Outcome::Events(events)) => events,
            other => panic!("the payment must be accepted, got {other:?}"),
        }
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
        let events = events_of(outcome(&options(true), &[], choice));
        assert!(matches!(events[0], Event::SpellCast { .. }));
        assert!(
            !events.iter().any(|e| matches!(e, Event::LifePaid { .. })),
            "a plain cost takes no life"
        );
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

    fn life_paid(events: &[Event]) -> Vec<u32> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::LifePaid { amount, .. } => Some(*amount),
                _ => None,
            })
            .collect()
    }

    #[rstest]
    #[case::manual(PayChoice::Manual(Payment { life: 2, ..taps(&[1]) }))]
    #[case::complete_that_keeps_the_life(PayChoice::Complete(Payment { life: 2, ..taps(&[]) }))]
    #[case::complete_that_adds_the_life(PayChoice::Complete(taps(&[])))]
    #[case::auto(PayChoice::Auto)]
    fn life_is_paid_once_and_before_the_spell_is_cast(#[case] choice: PayChoice) {
        // {1}{B/P} with a Forest and no Swamp: the Forest pays the generic part, 2 life pays the symbol.
        let options = options_for("{1}{B/P}", vec![source(1, ManaType::Green)], 20, true);
        let events = events_of(outcome(&options, &[], choice));
        assert_eq!(life_paid(&events), [2]);
        assert!(
            matches!(
                events[..],
                [
                    Event::LifePaid {
                        player: PlayerId(0),
                        amount: 2
                    },
                    Event::SpellCast { .. },
                    ..
                ]
            ),
            "{events:?}"
        );
        let cast_at = events
            .iter()
            .position(|e| matches!(e, Event::SpellCast { .. }));
        let tapped_at = events
            .iter()
            .position(|e| matches!(e, Event::PermanentTapped { .. }));
        assert!(
            cast_at < tapped_at,
            "the mana follows the cast as it always did"
        );
    }

    #[test]
    fn a_payment_with_only_life_spends_no_mana() {
        let options = options_for("{G/P}{G/P}", vec![], 20, true);
        let events = events_of(outcome(&options, &[], PayChoice::Auto));
        assert_eq!(life_paid(&events), [4], "one payment for both symbols");
        assert!(matches!(
            events[..],
            [Event::LifePaid { .. }, Event::SpellCast { .. }]
        ));
    }

    #[rstest]
    #[case::too_much_for_the_cost(PayChoice::Manual(Payment { life: 4, ..taps(&[1]) }))]
    #[case::an_odd_amount(PayChoice::Manual(Payment { life: 1, ..taps(&[1]) }))]
    #[case::more_than_the_life_total(PayChoice::Manual(Payment { life: 2, ..taps(&[1]) }))]
    #[case::life_and_enough_mana(PayChoice::Manual(Payment { life: 2, ..taps(&[1, 2]) }))]
    fn life_that_does_not_fit_is_refused_before_anything_happens(#[case] choice: PayChoice) {
        // One Phyrexian symbol and a life total of 1: no amount of life can be paid.
        let options = options_for(
            "{1}{B/P}",
            vec![source(1, ManaType::Green), source(2, ManaType::Black)],
            1,
            true,
        );
        assert!(matches!(
            outcome(&options, &[], choice),
            Err(EngineError::Illegal(IllegalAnswer::Payment(_)))
        ));
    }

    #[test]
    fn automatic_payment_is_not_offered_when_it_would_take_the_last_life() {
        let options = options_for("{B/P}", vec![], 2, true);
        assert!(!options.auto_available);
        assert_eq!(options.max_life, 2, "paying it by hand is still allowed");
        assert_eq!(
            outcome(&options, &[], PayChoice::Auto).unwrap_err(),
            EngineError::Illegal(IllegalAnswer::NotOffered("automatic payment"))
        );
        let by_hand = events_of(outcome(
            &options,
            &[],
            PayChoice::Manual(Payment {
                life: 2,
                ..Payment::default()
            }),
        ));
        assert_eq!(life_paid(&by_hand), [2]);
    }
}
