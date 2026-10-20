//! Casting a spell and activating an ability: the announcement, the payment decision and the events a valid
//! payment produces.
//!
//! What a player has begun lives outside game state until it is paid for (CR 601.2), so backing out is free and
//! leaves no trace in the log. The decisions come in the order of the rules: targets, then the payment; the
//! other announcements (mode, X, division, optional costs) join before and after the targets
//! (`docs/cards-dsl.md`, Execution).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::cost::CostItem;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::abilities::{AbilityRef, PutOnStack};
use crate::ability_reader;
use crate::answers::Outcome;
use crate::choose::{Choice, Choices, announced_specs, asks, modal_of, mode_options, options_for};
use crate::decision::{Decision, DecisionKind, PayChoice, PayOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::{Event, ZoneCause};
use crate::mana::{
    ManaOption, Payment, auto_payment, check_payment, complete_payment, max_life_payment,
};
use crate::object::PermanentState;
use crate::price::Price;
use crate::priority::mana_options;
use crate::resolve::spell_ability;
use crate::snapshot::snapshot;
use crate::state::GameState;
use crate::triggers::PendingTrigger;
use crate::zones::Zone;

/// What is being put on the stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum What {
    /// A spell cast from a hand.
    Spell(ObjectId),
    /// An activated ability of a permanent.
    Ability(AbilityRef),
    /// A triggered ability that waits in the queue; its targets are chosen as it goes on the stack (CR 603.3d).
    Trigger(Box<PendingTrigger>),
}

/// A cast or activation that has begun and is not paid for yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pending {
    pub(crate) player: PlayerId,
    pub(crate) what: What,
    /// What the player has announced so far.
    pub(crate) choices: Choices,
    /// Whether the modes are settled: chosen, or there are none to choose. Choosing none is a choice too.
    pub(crate) modes_chosen: bool,
}

impl Pending {
    pub(crate) fn new(player: PlayerId, what: What) -> Self {
        Pending {
            player,
            what,
            choices: Choices::default(),
            modes_chosen: false,
        }
    }

    /// The announcement of a triggered ability that waits in the queue.
    pub(crate) fn trigger(player: PlayerId, waiting: PendingTrigger) -> Self {
        Pending::new(player, What::Trigger(Box::new(waiting)))
    }
}

/// What the cast or activation needs next.
pub(crate) enum Next {
    /// A decision from the player.
    Ask(Decision),
    /// Nothing more to ask: these events pay for it and put it on the stack.
    Commit(Vec<Event>),
    /// It cannot go on, so nothing happens.
    Drop,
}

/// The object whose abilities and targets are being announced.
const fn source_of(what: &What) -> ObjectId {
    match what {
        What::Spell(spell) => *spell,
        What::Ability(ability) => ability.object,
        What::Trigger(waiting) => waiting.trigger.source.id,
    }
}

/// The rules of the spell or ability being announced.
fn rules_of(state: &GameState, what: &What) -> Option<Arc<Ability>> {
    let object = state.object(source_of(what));
    match what {
        What::Spell(spell) => object
            .and_then(|_| spell_ability(state, *spell))
            .map(|(_, rules)| rules),
        What::Ability(address) => object.and_then(|_| ability_reader::ability(state, *address)),
        What::Trigger(waiting) => Some(Arc::clone(&waiting.rules)),
    }
}

/// The next decision of a pending cast or activation, or the events that complete it.
///
/// A modal spell or ability asks its modes first (CR 700.2) and then the targets of the modes chosen only. A target
/// specification that allows no targets, or has none to offer and needs none, is passed over. A triggered ability
/// that cannot be given the modes or targets it needs is removed (CR 603.3d).
pub(crate) fn announce(state: &GameState, pending: &mut Pending) -> Next {
    let triggered = match &pending.what {
        What::Trigger(waiting) => Some(waiting),
        _ => None,
    };
    let cause = triggered.map(|waiting| &waiting.trigger.cause);
    let held = rules_of(state, &pending.what);
    let rules = held.as_deref();
    if !pending.modes_chosen {
        if let Some(modal) = rules.and_then(modal_of) {
            let Some(options) = mode_options(
                state,
                source_of(&pending.what),
                pending.player,
                &modal,
                triggered.is_none(),
                cause,
            ) else {
                return triggered.map_or(Next::Drop, |waiting| {
                    Next::Commit(vec![crate::triggers::removing(waiting)])
                });
            };
            // Nothing to choose between when no mode may be chosen.
            if options.max > 0 {
                return Next::Ask(Decision {
                    player: pending.player,
                    kind: DecisionKind::ChooseMode(options),
                });
            }
        }
        pending.modes_chosen = true;
    }
    let specs = rules.map_or_else(Vec::new, |rules| {
        announced_specs(rules, &pending.choices.modes)
    });
    while pending.choices.targets.len() < specs.len() {
        let index = pending.choices.targets.len();
        let Some(mut options) = options_for(
            state,
            source_of(&pending.what),
            pending.player,
            index,
            specs[index],
            triggered.is_none(),
            cause,
        ) else {
            return triggered.map_or(Next::Drop, |waiting| {
                Next::Commit(vec![crate::triggers::removing(waiting)])
            });
        };
        if !asks(&options) {
            pending.choices.targets.push(Vec::new());
            continue;
        }
        options.triggered = triggered.is_some();
        return Next::Ask(Decision {
            player: pending.player,
            kind: DecisionKind::ChooseTargets(options),
        });
    }
    match &pending.what {
        What::Spell(spell) => match pay_options(state, pending.player, *spell) {
            Some(options) => Next::Ask(Decision {
                player: pending.player,
                kind: DecisionKind::PayMana(options),
            }),
            None => Next::Drop,
        },
        What::Ability(ability) => match activation_payment(state, pending.player, *ability) {
            Some(options) => Next::Ask(Decision {
                player: pending.player,
                kind: DecisionKind::PayMana(options),
            }),
            None => activation_events(state, pending.player, *ability, &pending.choices)
                .map_or(Next::Drop, Next::Commit),
        },
        What::Trigger(waiting) => {
            Next::Commit(vec![trigger_event(state, waiting, &pending.choices)])
        }
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

/// The events of activating an ability: the tap and the sacrifice, discard or exile of the source that its cost asks,
/// the activation and the ability on the stack. The mana of its cost is paid around them by [`pay_answer`].
///
/// The source is photographed before the costs, so the ability knows what it was (CR 113.7a); a token sacrificed
/// for it has ceased to exist by then, and the ability still has the rules it was activated from.
fn activation_events(
    state: &GameState,
    player: PlayerId,
    ability: AbilityRef,
    choices: &Choices,
) -> Option<Vec<Event>> {
    let source = state.object(ability.object)?;
    let rules = ability_reader::ability(state, ability)?;
    let Ability::Activated { cost, .. } = &*rules else {
        return None;
    };
    let before = snapshot(state, ability.object)?;
    let mut events = Vec::new();
    if cost.0.contains(&CostItem::Tap) {
        events.push(Event::PermanentTapped {
            object: ability.object,
        });
    }
    // The costs that move the source come before the activation, which names the source as it was (CR 113.7a): it is
    // sacrificed from the battlefield, discarded from the hand, or exiled from the zone it is in.
    if cost.0.contains(&CostItem::SacrificeSelf) {
        events.push(Event::ZoneChanged {
            object: ability.object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(source.owner()),
            cause: ZoneCause::Sacrificed,
        });
    }
    if cost.0.contains(&CostItem::DiscardSelf) {
        events.push(Event::CardDiscarded {
            player,
            object: ability.object,
        });
    }
    if cost.0.contains(&CostItem::ExileSelf) {
        events.push(Event::ZoneChanged {
            object: ability.object,
            from: source.zone(),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        });
    }
    events.push(Event::AbilityActivated {
        player,
        source: source.reference(),
        ability: ability.ability,
        origin: ability.source,
    });
    events.push(Event::AbilityPutOnStack(Box::new(PutOnStack {
        object: state.next_object(),
        source: source.reference(),
        snapshot: before,
        ability: ability.ability,
        origin: ability.source,
        controller: player,
        choices: choices.clone(),
        cause: None,
        delayed: None,
    })));
    Some(events)
}

/// The payment decision for activating `ability`, or `None` when its cost has no mana to pay.
fn activation_payment(
    state: &GameState,
    player: PlayerId,
    ability: AbilityRef,
) -> Option<PayOptions> {
    state.object(ability.object)?;
    let rules = ability_reader::ability(state, ability)?;
    let Ability::Activated { cost, .. } = &*rules else {
        return None;
    };
    let price = Price::of(cost).ok()?;
    let sources = price.sources(state, player, ability.object);
    // Nothing is paid until the payment is accepted, so backing out of the activation is free.
    payment_decision(state, player, ability.object, price.mana?, sources, true)
}

/// The payment decision for casting `spell`, or `None` if it has no cost to pay.
pub(crate) fn pay_options(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
) -> Option<PayOptions> {
    let cost = state.object(spell)?.card().mana_cost.clone()?;
    // The cast only starts to change the game when the payment is accepted, so backing out is free.
    payment_decision(
        state,
        player,
        spell,
        cost,
        mana_options(state, player),
        true,
    )
}

/// The payment decision for `cost`, which `player` pays with `sources` for `object`: the spell cast, the source of
/// the ability activated or the source of the effect that asks. `None` when the player is not in the game.
pub(crate) fn payment_decision(
    state: &GameState,
    player: PlayerId,
    object: ObjectId,
    cost: ManaCost,
    sources: Vec<ManaOption>,
    can_cancel: bool,
) -> Option<PayOptions> {
    let seat = state.player(player)?;
    let (pool, life) = (*seat.mana(), seat.life());
    Some(PayOptions {
        spell: object,
        auto_available: auto_payment(&cost, &pool, &sources, life).is_some(),
        can_cancel,
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

/// The events of a valid payment, with the `announced` events of what it is for between the life and the mana.
fn payment_events(
    player: PlayerId,
    options: &PayOptions,
    tapped: &[ObjectId],
    payment: &Payment,
    announced: Vec<Event>,
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
    events.extend(announced);
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

/// Validates a payment choice and produces the events of the cast, the activation or the effect it pays for.
///
/// `pending` is the cast or activation in progress; with none, an effect of the resolution under way asked for the
/// payment and its answer joins the journal (`resolve/prim/pay.rs`).
pub(crate) fn pay_answer(
    state: &GameState,
    player: PlayerId,
    options: &PayOptions,
    choice: PayChoice,
    pending: Option<&Pending>,
) -> Result<Outcome, EngineError> {
    let announced = match choice {
        PayChoice::Cancel => Vec::new(),
        _ => announcement(state, player, pending)?,
    };
    pay_choice(
        player,
        options,
        &unusable(state, options),
        choice,
        announced,
    )
}

/// The events that say what a payment is for, which come between the life paid and the mana: the cast of the spell,
/// the activation of the ability, or the answer "paid" to the effect that asked.
fn announcement(
    state: &GameState,
    player: PlayerId,
    pending: Option<&Pending>,
) -> Result<Vec<Event>, EngineError> {
    let unexpected = || EngineError::InvalidEvent("a payment was made for nothing".into());
    match pending {
        Some(Pending {
            what: What::Spell(spell),
            choices,
            ..
        }) => Ok(vec![Event::SpellCast {
            player,
            object: *spell,
            choices: choices.clone(),
        }]),
        Some(Pending {
            what: What::Ability(ability),
            choices,
            ..
        }) => activation_events(state, player, *ability, choices).ok_or_else(unexpected),
        None if state.resolving().is_some() => Ok(vec![Event::ChoiceMade {
            player,
            choice: Choice::YesNo(true),
        }]),
        Some(Pending {
            what: What::Trigger(_),
            ..
        })
        | None => Err(unexpected()),
    }
}

/// [`pay_answer`] with the tapped permanents and the announcing events already worked out, so it needs no game state.
fn pay_choice(
    player: PlayerId,
    options: &PayOptions,
    tapped: &[ObjectId],
    choice: PayChoice,
    announced: Vec<Event>,
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
    let events = payment_events(player, options, tapped, &payment, announced)?;
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
        let cast = Event::SpellCast {
            player: PlayerId(0),
            object: options.spell,
            choices: Choices::default(),
        };
        pay_choice(PlayerId(0), options, &tapped, choice, vec![cast])
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

#[cfg(test)]
mod announcement_tests {
    //! Goal: a cast or activation asks its questions in the order of the rules, targets before payment, passes over a specification that needs no choice, gives up on one that cannot be met, completes an activation that costs only a tap with the events that pay and announce it, asks the payment of one that costs mana (never from its own source when it taps it, announcing between the life and the mana), and pays for an effect with no way back.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
    use crate::choose::Target;
    use crate::fixtures::{creature, state};
    use crate::zones::Zone;

    fn spell(targets: &str) -> Arc<CardDefinition> {
        let rules = format!(r#"{{"Spell":{{"targets":{targets},"effects":[{{"Draw":{{}}}}]}}}}"#);
        Arc::new(
            CardDefinition::new("Spell")
                .with_type(CardType::Instant)
                .with_mana_cost(ManaCost::parse("{1}").unwrap())
                .with_rules(serde_json::from_str(&rules).unwrap()),
        )
    }

    fn pending_for(game: &GameState) -> Pending {
        let card = game.zone(Zone::Hand(PlayerId(0)))[0];
        Pending::new(PlayerId(0), What::Spell(card))
    }

    fn asks(next: &Next) -> &'static str {
        match next {
            Next::Ask(Decision {
                kind: DecisionKind::ChooseTargets(_),
                ..
            }) => "targets",
            Next::Ask(Decision {
                kind: DecisionKind::PayMana(_),
                ..
            }) => "payment",
            Next::Ask(_) => "something else",
            Next::Commit(_) => "commit",
            Next::Drop => "drop",
        }
    }

    #[rstest]
    #[case::one_specification(r#"[{"what":"Player"}]"#, 1)]
    #[case::two_specifications(r#"[{"what":"Player"},{"what":"AnyTarget"}]"#, 2)]
    fn targets_are_asked_one_specification_at_a_time_and_the_payment_follows(
        #[case] targets: &str,
        #[case] specs: usize,
    ) {
        let game = state(vec![vec![spell(targets)], vec![]], vec![vec![], vec![]]);
        let mut pending = pending_for(&game);
        for index in 0..specs {
            let next = announce(&game, &mut pending);
            let Next::Ask(Decision {
                kind: DecisionKind::ChooseTargets(options),
                player,
            }) = next
            else {
                panic!("a target decision was expected for specification {index}");
            };
            assert_eq!((options.spec, player), (index, PlayerId(0)));
            assert!(options.can_cancel, "the cast has cost nothing yet");
            pending.choices.targets.push(vec![options.legal[0]]);
        }
        assert_eq!(asks(&announce(&game, &mut pending)), "payment");
    }

    #[test]
    fn a_specification_that_needs_nothing_is_passed_over_and_one_that_cannot_be_met_ends_the_cast()
    {
        // "Up to one target creature" with no creature to target needs no decision.
        let optional = r#"[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}]"#;
        let game = state(vec![vec![spell(optional)], vec![]], vec![vec![], vec![]]);
        let mut pending = pending_for(&game);
        assert_eq!(asks(&announce(&game, &mut pending)), "payment");
        assert_eq!(pending.choices.targets, [Vec::<Target>::new()]);
        // "Target creature" with none to target cannot be announced at all.
        let required = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;
        let game = state(vec![vec![spell(required)], vec![]], vec![vec![], vec![]]);
        let mut pending = pending_for(&game);
        assert_eq!(asks(&announce(&game, &mut pending)), "drop");
        // With a creature in play it can.
        let game = state(
            vec![vec![spell(required)], vec![]],
            vec![vec![], vec![creature("Bear", 2, 2)]],
        );
        let mut pending = pending_for(&game);
        assert_eq!(asks(&announce(&game, &mut pending)), "targets");
    }

    #[test]
    fn a_spell_without_targets_goes_straight_to_the_payment() {
        let game = state(vec![vec![spell("[]")], vec![]], vec![vec![], vec![]]);
        let mut pending = pending_for(&game);
        assert_eq!(asks(&announce(&game, &mut pending)), "payment");
    }

    fn tapper() -> Arc<CardDefinition> {
        let rules = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#;
        Arc::new(
            CardDefinition::new("Tapper")
                .with_type(CardType::Artifact)
                .with_rules(serde_json::from_str(rules).unwrap()),
        )
    }

    #[test]
    fn an_activation_that_costs_a_tap_is_completed_by_the_events_that_pay_and_announce_it() {
        let game = state(vec![vec![], vec![]], vec![vec![tapper()], vec![]]);
        let source = game.zone(Zone::Battlefield)[0];
        let mut pending = Pending::new(PlayerId(0), What::Ability(AbilityRef::printed(source, 0)));
        let Next::Commit(events) = announce(&game, &mut pending) else {
            panic!("a tap needs no decision");
        };
        let [
            Event::PermanentTapped { object },
            Event::AbilityActivated {
                player,
                source: activated,
                ability,
                ..
            },
            Event::AbilityPutOnStack(put),
        ] = &events[..]
        else {
            panic!("tap, activation, ability on the stack: {events:?}");
        };
        assert_eq!((*object, *player, *ability), (source, PlayerId(0), 0));
        assert_eq!(*activated, game.object(source).unwrap().reference());
        assert_eq!(put.object, game.next_object());
        assert_eq!(put.source, *activated);
        assert_eq!(put.snapshot.zone, Zone::Battlefield);
        assert!(put.cause.is_none() && put.choices.is_empty());
    }

    fn gadget(cost: &str) -> Arc<CardDefinition> {
        let rules = format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"Draw":{{}}}}]}}}}"#);
        Arc::new(
            CardDefinition::new("Gadget")
                .with_type(CardType::Artifact)
                .with_rules(serde_json::from_str(&rules).unwrap()),
        )
    }

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::basic_land(Some(
            mtg_core::color::Color::Green,
        )))
    }

    /// Seat 0 controls a Gadget with this cost and two Forests.
    fn with_gadget(cost: &str) -> (GameState, Pending) {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![gadget(cost), forest(), forest()], vec![]],
        );
        let source = game.zone(Zone::Battlefield)[0];
        let ability = AbilityRef::printed(source, 0);
        (game, Pending::new(PlayerId(0), What::Ability(ability)))
    }

    #[test]
    fn an_activation_that_costs_mana_asks_for_the_payment_and_cannot_use_its_own_source_when_it_taps()
     {
        for (cost, sources) in [(r#""{1}""#, 2), (r#"[{"Mana":"{1}"},"Tap"]"#, 2)] {
            let (game, mut pending) = with_gadget(cost);
            let Next::Ask(Decision {
                player,
                kind: DecisionKind::PayMana(options),
            }) = announce(&game, &mut pending)
            else {
                panic!("a payment was expected for {cost}");
            };
            assert_eq!(player, PlayerId(0));
            assert_eq!(options.cost, ManaCost::parse("{1}").unwrap());
            assert_eq!(options.sources.len(), sources, "{cost}");
            assert!(options.can_cancel && options.auto_available);
            assert_eq!(options.spell, game.zone(Zone::Battlefield)[0]);
        }
    }

    #[rstest]
    #[case::only_mana(r#""{1}""#, false)]
    #[case::mana_and_tap(r#"[{"Mana":"{1}"},"Tap"]"#, true)]
    fn the_payment_of_an_activation_announces_it_between_the_life_and_the_mana(
        #[case] cost: &str,
        #[case] taps: bool,
    ) {
        let (game, mut pending) = with_gadget(cost);
        let Next::Ask(Decision {
            kind: DecisionKind::PayMana(options),
            ..
        }) = announce(&game, &mut pending)
        else {
            panic!("a payment was expected");
        };
        let Ok(Outcome::Events(events)) = pay_answer(
            &game,
            PlayerId(0),
            &options,
            PayChoice::Auto,
            Some(&pending),
        ) else {
            panic!("the payment must be accepted");
        };
        let kinds: Vec<&str> = events
            .iter()
            .map(|event| match event {
                Event::PermanentTapped { .. } => "tapped",
                Event::AbilityActivated { .. } => "activated",
                Event::AbilityPutOnStack(_) => "on the stack",
                Event::ManaAdded { .. } => "mana",
                Event::ManaSpent { .. } => "spent",
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        let mut expected = if taps { vec!["tapped"] } else { Vec::new() };
        expected.extend(["activated", "on the stack", "tapped", "mana", "spent"]);
        assert_eq!(kinds, expected);
        // The source is photographed before anything is tapped.
        let put = events.iter().find_map(|event| match event {
            Event::AbilityPutOnStack(put) => Some(put),
            _ => None,
        });
        assert!(put.is_some_and(|put| put.snapshot.zone == Zone::Battlefield));
    }

    #[test]
    fn a_payment_for_an_effect_is_logged_as_an_answer_and_cannot_be_cancelled() {
        let (game, _) = with_gadget(r#""{1}""#);
        let tapped = Vec::new();
        let options = payment_decision(
            &game,
            PlayerId(0),
            game.zone(Zone::Battlefield)[0],
            ManaCost::parse("{1}").unwrap(),
            mana_options(&game, PlayerId(0)),
            false,
        )
        .unwrap();
        // Nothing resolves here, so no effect asked for it.
        assert!(matches!(
            pay_answer(&game, PlayerId(0), &options, PayChoice::Auto, None),
            Err(EngineError::InvalidEvent(_))
        ));
        let announced = vec![Event::ChoiceMade {
            player: PlayerId(0),
            choice: Choice::YesNo(true),
        }];
        assert_eq!(
            pay_choice(
                PlayerId(0),
                &options,
                &tapped,
                PayChoice::Cancel,
                announced.clone()
            )
            .unwrap_err(),
            EngineError::Illegal(IllegalAnswer::NotOffered("cancel"))
        );
        let Ok(Outcome::Events(events)) =
            pay_choice(PlayerId(0), &options, &tapped, PayChoice::Auto, announced)
        else {
            panic!("the payment must be accepted");
        };
        assert!(matches!(
            events[0],
            Event::ChoiceMade {
                choice: Choice::YesNo(true),
                ..
            }
        ));
        assert!(matches!(events.last(), Some(Event::ManaSpent { .. })));
    }
}
