//! Paying for a cast or activation: the payment decision and the events a valid payment produces.
//!
//! What a player has announced lives outside game state until it is paid for (CR 601.2), so backing out is free and
//! leaves no trace in the log (`announce`). The payment puts the spell or ability on the stack with the events of
//! its cast or activation between the life paid and the mana.

use mtg_cards::ability::Ability;
use mtg_cards::cost::CostItem;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::abilities::{AbilityRef, PutOnStack};
use crate::ability_reader;
use crate::announce::{Pending, Special, What, companion, foretell, suspend, turn_up};
use crate::answers::Outcome;
use crate::choose::{Choice, Choices};
use crate::decision::{PayChoice, PayOptions};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::{Event, ZoneCause};
use crate::help::Offered;
use crate::loyalty;
use crate::mana::{
    ManaOption, Payment, auto_payment_with, check_payment_with, complete_payment_with,
    max_life_payment,
};
use crate::mana_ability;
use crate::mana_takes::{assignment, without_taken};
use crate::object::PermanentState;
use crate::parts::{self, Part};
use crate::price::Price;
use crate::snapshot::snapshot;
use crate::spend::Spend;
use crate::state::GameState;
use crate::zones::Zone;

/// The events of activating an ability: the tap and the sacrifice, discard or exile of the source that its cost asks,
/// the activation and the ability on the stack. The mana of its cost is paid around them by [`pay_answer`].
///
/// The source is photographed before the costs, so the ability knows what it was (CR 113.7a); a token sacrificed
/// for it has ceased to exist by then, and the ability still has the rules it was activated from.
pub(crate) fn activation_events(
    state: &GameState,
    player: PlayerId,
    ability: AbilityRef,
    choices: &Choices,
) -> Option<Vec<Event>> {
    let source = state.object(ability.object)?;
    let rules = ability_reader::ability(state, ability)?;
    let mut before = snapshot(state, ability.object)?;
    let mut events = Vec::new();
    let mut shots = Vec::new();
    match &*rules {
        Ability::Activated { cost, .. } => {
            let price = Price::announced(cost).ok()?.at(choices.x.unwrap_or(0));
            shots = parts::photographed(state, &price.parts, choices.cost_picks());
            if price.tap {
                events.push(Event::PermanentTapped {
                    object: ability.object,
                });
            }
            // The other parts of the cost, in the order it prints them, with the objects the player picked for them.
            let mut picks = choices.cost_picks().iter();
            let mut kinds = choices.cost_kinds().iter();
            for part in &price.parts {
                let picked = if matches!(part, Part::Pick(_)) {
                    picks.next()?.as_slice()
                } else {
                    &[]
                };
                events.extend(part.w2f_events(state, (player, ability.object), picked, &mut kinds));
                if let Part::Counters { counter, count } = part {
                    parts::after_removal(&mut before.counters, counter, *count);
                }
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
                // Cycling is the discard of its cost; it is logged beside it, for what triggers on cycling.
                if matches!(
                    ability_reader::keyword_adding(state, ability),
                    Some(Keyword::Cycling(_) | Keyword::Typecycling { .. })
                ) {
                    events.push(Event::Cycled {
                        player,
                        object: ability.object,
                    });
                }
            }
            if cost.0.contains(&CostItem::ExileSelf) {
                events.push(Event::ZoneChanged {
                    object: ability.object,
                    from: source.zone(),
                    to: Zone::Exile,
                    cause: ZoneCause::Exiled,
                });
            }
        }
        // The loyalty counters of the cost are paid first (CR 606.4); the source stays, and the ability remembers it as it
        // stands once they are paid.
        Ability::Loyalty { cost, .. } => {
            let delta = loyalty::paid_change(*cost, choices.x)?;
            if let Some(paid) = loyalty::payment(source, *cost, choices.x) {
                events.push(paid);
                loyalty::after_payment(&mut before.counters, delta);
            }
        }
        _ => return None,
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
        choices: choices.clone().with_cost_shots(shots),
        cause: None,
        delayed: None,
    })));
    Some(events)
}

/// The payment decision for `cost`, which `player` pays with `sources` for `object`: the spell cast, the source of
/// the ability activated or the source of the effect that asks, `reserved_life` of the life being paid by other parts of
/// the cost. `None` when the player is not in the game.
// Every argument is a different thing a payment depends on; the three callers each have them all.
#[allow(clippy::too_many_arguments)]
pub(crate) fn payment_decision(
    state: &GameState,
    player: PlayerId,
    object: ObjectId,
    cost: ManaCost,
    sources: Vec<ManaOption>,
    help: Offered,
    spend: Spend,
    can_cancel: bool,
    reserved_life: u32,
) -> Option<PayOptions> {
    let seat = state.player(player)?;
    // The life another part of the cost pays is not there for the mana.
    let life = seat
        .life()
        .saturating_sub(i32::try_from(reserved_life).unwrap_or(i32::MAX));
    let pool = seat.mana().clone();
    let narrowed = match crate::mana_takes::for_card(state, &sources, object) {
        std::borrow::Cow::Owned(narrowed) => Some(narrowed),
        std::borrow::Cow::Borrowed(_) => None,
    };
    let sources = narrowed.unwrap_or(sources);
    Some(PayOptions {
        spell: object,
        auto_available: auto_payment_with(&cost, &pool, &sources, help.help(), life, &spend)
            .is_some(),
        help: Box::new(help),
        spend: Box::new(spend),
        can_cancel,
        life,
        max_life: max_life_payment(&cost, life),
        cost,
        pool,
        sources,
    })
}

/// How a mana source is used when the game is not at hand: the source and the permanents its cost takes.
type Activate<'a> = &'a dyn Fn(&ManaOption, &[Vec<ObjectId>]) -> Result<Vec<Event>, EngineError>;

/// The offered permanents that are tapped (or gone) and the offered cards that left the graveyard in `state`; no payment may use them.
fn unusable(state: &GameState, player: PlayerId, options: &PayOptions) -> Vec<ObjectId> {
    let tapped = |id: &ObjectId| {
        state
            .object(*id)
            .and_then(|o| o.permanent())
            .is_none_or(PermanentState::tapped)
    };
    let mut gone: Vec<ObjectId> = options
        .sources
        .iter()
        .filter(|o| !o.tapless)
        .map(|o| o.source.object)
        .chain(options.help.convoke.iter().map(|o| o.creature))
        .chain(options.help.improvise.iter().copied())
        .filter(tapped)
        .collect();
    gone.extend(
        options
            .help
            .delve
            .iter()
            .copied()
            .filter(|card| !state.zone(Zone::Graveyard(player)).contains(card)),
    );
    gone
}

/// The events of using a mana source when the game is not at hand: the tap and the mana it adds.
#[cfg(test)]
fn tap_events(player: PlayerId, option: &ManaOption) -> Vec<Event> {
    let mut events = vec![Event::PermanentTapped {
        object: option.source.object,
    }];
    events.extend((0..option.count).map(|_| Event::ManaAdded {
        player,
        mana: option.produces,
        source: Some(option.source.object),
        only: option.only.clone(),
        lasts: mtg_cards::spend::ManaLasts::Step,
    }));
    events
}

/// The events of a valid payment, with the `announced` events of what it is for between the life and the mana.
///
/// `activate` gives the events of using one mana source: its cost, the mana and what else it does (`mana_ability.rs`).
fn payment_events(
    player: PlayerId,
    options: &PayOptions,
    tapped: &[ObjectId],
    payment: &Payment,
    announced: Vec<Event>,
    activate: Activate<'_>,
) -> Result<Vec<Event>, EngineError> {
    let spent = check_payment_with(
        &options.cost,
        &options.pool,
        &options.sources,
        options.help(),
        tapped,
        options.life,
        &options.spend,
        payment,
    )
    .map_err(IllegalAnswer::from)?;
    // The permanents the sources take are the engine's to pick (`mana_takes.rs`).
    let paying: Vec<ManaOption> = payment
        .taps
        .iter()
        .filter_map(|tap| options.sources.iter().find(|o| o.source == *tap))
        .cloned()
        .collect();
    // The permanents tapped for convoke or improvise are tapped first and are not there to take.
    let helped = crate::help::Used::new(&payment.convoke, &payment.improvise, &[]);
    let paying = without_taken(&paying, &|id| helped.taps(id));
    let mut taken = assignment(&paying.iter().collect::<Vec<_>>())
        .ok_or(IllegalAnswer::NotOffered("permanents to take"))?;
    let mut events = Vec::new();
    // Life is part of the cost, paid in the same step as the mana, so it comes before the spell goes on the stack.
    if payment.life > 0 {
        events.push(Event::LifePaid {
            player,
            amount: payment.life,
        });
    }
    // The spell keeps the mana its cast spends, for "if {R} was spent to cast it" (CR 601.2h).
    let mut announced = announced;
    for event in &mut announced {
        if let Event::SpellCast { choices, .. } = event {
            choices.spent = (!spent.mana.is_empty()).then(|| Box::new(spent.mana.clone()));
        }
    }
    events.extend(announced);
    // The creatures and artifacts tapped and the cards exiled to help pay (CR 702.51, CR 702.126, CR 702.66), before the mana.
    for object in payment
        .convoke
        .iter()
        .map(|convoked| convoked.creature)
        .chain(payment.improvise.iter().copied())
    {
        events.push(Event::PermanentTapped { object });
    }
    for &card in &payment.delve {
        events.push(Event::ZoneChanged {
            object: card,
            from: Zone::Graveyard(player),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        });
    }
    // A source that costs mana pays it from the pool just before it adds its own mana. A source used again takes
    // the permanents of its next use, so each step claims one entry of `paying`.
    let mut claimed = vec![false; paying.len()];
    for step in &spent.steps {
        let option = options
            .sources
            .iter()
            .find(|o| o.source == step.source)
            .ok_or(IllegalAnswer::NotOffered("mana source"))?;
        if !step.paid.is_empty() {
            events.push(Event::ManaSpent {
                player,
                mana: step.paid.clone(),
                restricted: Vec::new(),
            });
        }
        let at = paying
            .iter()
            .enumerate()
            .position(|(at, o)| !claimed[at] && o.source == option.source);
        let picked = at
            .map(|at| {
                claimed[at] = true;
                std::mem::take(&mut taken[at])
            })
            .unwrap_or_default();
        events.extend(activate(option, &picked)?);
    }
    if !spent.mana.is_empty() {
        events.push(Event::ManaSpent {
            player,
            mana: spent.mana,
            restricted: spent.restricted,
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
    let outcome = pay_with(
        player,
        options,
        &unusable(state, player, options),
        choice,
        announced,
        &|option, taken| mana_ability::activation(state, player, option, taken),
    )?;
    Ok(match outcome {
        Outcome::Events(events) => Outcome::Events(parts::mill::in_turn(state, player, events)),
        other => other,
    })
}

/// The events of a spell being cast: `SpellCast`, and for a cast that the resolution of another spell or ability allowed
/// (`granted`) the yes that resolution reads back from its journal to go on (`resolve/prim/cast_from_zone.rs`).
pub(crate) fn spell_cast(
    player: PlayerId,
    spell: ObjectId,
    choices: &Choices,
    granted: bool,
) -> Vec<Event> {
    let mut events = vec![Event::SpellCast {
        player,
        object: spell,
        choices: choices.clone(),
    }];
    if granted {
        events.push(Event::ChoiceMade {
            player,
            choice: Choice::YesNo(true),
        });
    }
    events
}

/// The events of a spell being cast: what its way and optional costs pay besides mana (`parts`, with the objects picked
/// for them in `choices`), then `SpellCast`. `None` when a pick is missing.
pub(crate) fn spell_events(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
    parts: &[Part],
    choices: &Choices,
    granted: bool,
) -> Option<Vec<Event>> {
    let mut events = parts::pay(
        state,
        player,
        spell,
        parts,
        choices.cost_picks(),
        choices.cost_kinds(),
    )?;
    let shots = parts::photographed(state, parts, choices.cost_picks());
    events.extend(spell_cast(
        player,
        spell,
        &choices.clone().with_cost_shots(shots),
        granted,
    ));
    Some(events)
}

/// `events` of a cast with the `Unprepared` of the permanent first when the spell is a copy of its spell face: the
/// permanent stops being prepared as the spell is cast (a copy that is countered does not prepare it again).
fn unpreparing(state: &GameState, spell: ObjectId, mut events: Vec<Event>) -> Vec<Event> {
    if let Some(unprepared) = crate::prepared::unprepare(state, spell) {
        let cast = events
            .iter()
            .position(|event| matches!(event, Event::SpellCast { .. }))
            .unwrap_or(events.len());
        events.insert(cast, unprepared);
    }
    events
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
            what: What::Spell(card),
            special: Some(Special::Suspend),
            ..
        }) => suspend::events(state, *card).ok_or_else(unexpected),
        Some(Pending {
            what: What::Spell(card),
            special: Some(Special::Foretell),
            ..
        }) => foretell::events(state, *card).ok_or_else(unexpected),
        Some(Pending {
            special: Some(Special::Companion),
            ..
        }) => companion::events(state, player).ok_or_else(unexpected),
        Some(Pending {
            what: What::Spell(card),
            special: Some(Special::TurnFaceUp),
            ..
        }) => turn_up::events(state, *card).ok_or_else(unexpected),
        Some(
            pending @ Pending {
                what: What::Spell(spell),
                choices,
                granted,
                ..
            },
        ) => spell_events(
            state,
            player,
            *spell,
            &pending.cost_parts(),
            choices,
            granted.is_some(),
        )
        .map(|events| unpreparing(state, *spell, events))
        .ok_or_else(unexpected),
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
            what: What::Attack(attackers),
            ..
        }) => Ok(crate::combat::attack_events(state, player, attackers)),
        Some(Pending {
            what: What::Trigger(_),
            ..
        })
        | None => Err(unexpected()),
    }
}

/// [`pay_answer`] with the tapped permanents and the announcing events already worked out, so it needs no game state.
#[cfg(test)]
pub(crate) fn pay_choice(
    player: PlayerId,
    options: &PayOptions,
    tapped: &[ObjectId],
    choice: PayChoice,
    announced: Vec<Event>,
) -> Result<Outcome, EngineError> {
    pay_with(player, options, tapped, choice, announced, &|option, _| {
        Ok(tap_events(player, option))
    })
}

/// [`pay_choice`] with the events of using each mana source given by `activate`.
fn pay_with(
    player: PlayerId,
    options: &PayOptions,
    tapped: &[ObjectId],
    choice: PayChoice,
    announced: Vec<Event>,
    activate: Activate<'_>,
) -> Result<Outcome, EngineError> {
    let payment = match choice {
        PayChoice::Cancel if options.can_cancel => return Ok(Outcome::CancelCast),
        PayChoice::Cancel => return Err(IllegalAnswer::NotOffered("cancel").into()),
        PayChoice::Manual(payment) => payment,
        PayChoice::Complete(partial) => complete_payment_with(
            &options.cost,
            &options.pool,
            &options.sources,
            options.help(),
            tapped,
            options.life,
            &options.spend,
            &partial,
        )
        .map_err(IllegalAnswer::from)?,
        PayChoice::Auto => auto_payment_with(
            &options.cost,
            &options.pool,
            &options.sources,
            options.help(),
            options.life,
            &options.spend,
        )
        .ok_or(IllegalAnswer::NotOffered("automatic payment"))?,
    };
    let events = payment_events(player, options, tapped, &payment, announced, activate)?;
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
        ManaOption::new(
            ManaSource {
                object: ObjectId(object),
                ability: 0,
            },
            produces,
        )
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
            auto_available: crate::mana::auto_payment(
                &cost,
                &ManaPool::default(),
                &sources,
                life,
                &Spend::other(),
            )
            .is_some(),
            help: Box::default(),
            spend: Box::new(Spend::other()),
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
