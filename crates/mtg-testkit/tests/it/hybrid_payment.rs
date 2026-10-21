//! Goal: casting a spell with hybrid and Phyrexian symbols taps the mana and takes the life that were chosen, in one step and before the spell goes on the stack; the rules' refusals (too little life, a payment that does not fit) change nothing; and a cast that is cancelled mid-payment leaves no trace.

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::mana::Payment;
use mtg_engine::player::LossReason;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event, GameResult, PresetPermanent};
use mtg_testkit::cards::{
    colorless_hybrid_elf, colorless_phyrexian_elf, double_phyrexian_ogre, forest, hybrid_bear,
    hybrid_phyrexian_elf, island, mono_hybrid_bear, mountain, phyrexian_bear, plains, swamp,
    wastes,
};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

type Card = fn() -> Arc<CardDefinition>;

fn land(name: &str) -> PresetPermanent {
    ready(match name {
        "Forest" => forest(),
        "Plains" => plains(),
        "Island" => island(),
        "Swamp" => swamp(),
        "Mountain" => mountain(),
        _ => wastes(),
    })
}

/// Player 0 holds `card` and has `lands` in play, at `life`.
fn driver(card: Card, lands: &[&str], life: i32) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [card()])
            .battlefield(0, lands.iter().map(|name| land(name)))
            .life(0, life),
    )
}

fn castable(driver: &Driver) -> Result<bool, ScriptError> {
    match &driver.decision()?.kind {
        mtg_engine::DecisionKind::Priority(options) => Ok(!options.castable.is_empty()),
        other => Err(ScriptError::UnexpectedDecision {
            wanted: "priority",
            got: other.name().to_owned(),
        }),
    }
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

fn tapped_count(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id)?.permanent())
        .filter(|p| p.tapped())
        .count()
}

#[rstest]
#[case::phyrexian_by_colour(phyrexian_bear, &["Forest", "Forest"], &["Forest", "Forest"], 0)]
#[case::phyrexian_by_life(phyrexian_bear, &["Forest"], &["Forest"], 2)]
#[case::phyrexian_by_life_with_the_colour_untouched(phyrexian_bear, &["Forest", "Forest"], &["Forest"], 2)]
#[case::hybrid_first_colour(hybrid_bear, &["Forest", "Forest"], &["Forest", "Forest"], 0)]
#[case::hybrid_each_colour(hybrid_bear, &["Forest", "Plains"], &["Forest", "Plains"], 0)]
#[case::hybrid_second_colour(hybrid_bear, &["Plains", "Plains"], &["Plains", "Plains"], 0)]
#[case::hybrid_with_extra_lands_in_play(hybrid_bear, &["Plains", "Forest", "Island"], &["Plains", "Forest"], 0)]
#[case::mono_hybrid_by_colour(mono_hybrid_bear, &["Forest", "Island"], &["Forest"], 0)]
#[case::mono_hybrid_by_generic(mono_hybrid_bear, &["Island", "Mountain"], &["Island", "Mountain"], 0)]
#[case::hybrid_phyrexian_first_colour(hybrid_phyrexian_elf, &["Forest"], &["Forest"], 0)]
#[case::hybrid_phyrexian_second_colour(hybrid_phyrexian_elf, &["Island"], &["Island"], 0)]
#[case::hybrid_phyrexian_by_life(hybrid_phyrexian_elf, &["Plains"], &[], 2)]
#[case::colorless_hybrid_by_colorless(colorless_hybrid_elf, &["Wastes"], &["Wastes"], 0)]
#[case::colorless_hybrid_by_colour(colorless_hybrid_elf, &["Forest"], &["Forest"], 0)]
#[case::colorless_phyrexian_by_colorless(colorless_phyrexian_elf, &["Wastes"], &["Wastes"], 0)]
#[case::colorless_phyrexian_by_life(colorless_phyrexian_elf, &["Forest"], &[], 2)]
#[case::two_phyrexian_by_mana(double_phyrexian_ogre, &["Swamp", "Swamp", "Forest"], &["Swamp", "Swamp", "Forest"], 0)]
#[case::two_phyrexian_one_by_life(double_phyrexian_ogre, &["Swamp", "Forest"], &["Swamp", "Forest"], 2)]
#[case::two_phyrexian_both_by_life(double_phyrexian_ogre, &["Forest"], &["Forest"], 4)]
fn a_spell_is_cast_with_the_mana_and_life_that_were_chosen(
    #[case] card: Card,
    #[case] lands: &[&str],
    #[case] taps: &[&str],
    #[case] life: u32,
) -> Result<(), ScriptError> {
    let mut driver = driver(card, lands, 20)?;
    assert!(castable(&driver)?, "the cast is offered");
    let events = driver.cast_paying(&card().name, taps, life)?;
    assert_eq!(driver.state().stack().len(), 1, "the spell is on the stack");
    driver.expect_life(0, 20 - i32::try_from(life).unwrap_or(0))?;
    assert_eq!(tapped_count(&driver), taps.len());
    if life == 0 {
        assert_eq!(life_paid(&events), Vec::<u32>::new());
    } else {
        assert_eq!(
            life_paid(&events),
            [life],
            "one payment for all the symbols"
        );
        assert!(
            matches!(events[0], Event::LifePaid { player, amount } if player == pid(0) && amount == life),
            "life comes first: {events:?}"
        );
        assert!(matches!(events[1], Event::SpellCast { .. }));
    }
    driver.expect_no_event("damage", |e| matches!(e, Event::DamageDealt { .. }))
}

#[rstest]
#[case::mana_when_it_can(phyrexian_bear, &["Forest", "Forest"], 0, 2)]
#[case::life_only_when_it_must(phyrexian_bear, &["Forest"], 2, 1)]
#[case::the_colour_before_life(hybrid_phyrexian_elf, &["Island", "Plains"], 0, 1)]
#[case::life_for_the_off_colour(hybrid_phyrexian_elf, &["Plains"], 2, 0)]
#[case::both_phyrexian_by_mana(double_phyrexian_ogre, &["Swamp", "Swamp", "Mountain"], 0, 3)]
#[case::one_by_mana_one_by_life(double_phyrexian_ogre, &["Swamp", "Mountain"], 2, 2)]
#[case::both_by_life(double_phyrexian_ogre, &["Mountain"], 4, 1)]
#[case::hybrid_taps_what_pays(hybrid_bear, &["Plains", "Forest", "Island"], 0, 2)]
#[case::mono_hybrid_prefers_the_colour(mono_hybrid_bear, &["Forest", "Island", "Island"], 0, 1)]
fn automatic_payment_takes_mana_before_life_and_as_little_life_as_it_can(
    #[case] card: Card,
    #[case] lands: &[&str],
    #[case] life: u32,
    #[case] taps: usize,
) -> Result<(), ScriptError> {
    let mut driver = driver(card, lands, 20)?;
    let events = driver.cast(&card().name)?;
    assert_eq!(life_paid(&events).iter().sum::<u32>(), life);
    assert_eq!(tapped_count(&driver), taps);
    driver.expect_life(0, 20 - i32::try_from(life).unwrap_or(0))
}

#[rstest]
#[case::hybrid_with_both_colours_short(hybrid_bear, &["Forest", "Island"], 20, false)]
#[case::hybrid_with_one_land(hybrid_bear, &["Forest"], 20, false)]
#[case::hybrid_needs_no_life(hybrid_bear, &["Forest", "Plains"], 1, true)]
#[case::mono_hybrid_one_land(mono_hybrid_bear, &["Island"], 20, false)]
#[case::mono_hybrid_two_lands(mono_hybrid_bear, &["Island", "Island"], 20, true)]
#[case::colorless_hybrid_wrong_colour(colorless_hybrid_elf, &["Island"], 20, false)]
#[case::phyrexian_with_enough_life(phyrexian_bear, &["Island"], 2, true)]
#[case::phyrexian_with_too_little_life(phyrexian_bear, &["Island"], 1, false)]
#[case::phyrexian_with_the_colour_needs_no_life(phyrexian_bear, &["Forest", "Forest"], 1, true)]
#[case::colorless_phyrexian_no_lands(colorless_phyrexian_elf, &[], 2, true)]
#[case::colorless_phyrexian_no_lands_no_life(colorless_phyrexian_elf, &[], 1, false)]
#[case::two_phyrexian_with_life_for_both(double_phyrexian_ogre, &["Forest"], 4, true)]
#[case::two_phyrexian_with_life_for_one(double_phyrexian_ogre, &["Forest"], 3, false)]
#[case::two_phyrexian_with_life_for_one_and_a_swamp(double_phyrexian_ogre, &["Forest", "Swamp"], 3, true)]
#[case::the_generic_part_needs_a_land(double_phyrexian_ogre, &[], 20, false)]
fn a_spell_is_offered_only_when_the_mana_and_life_can_pay_it(
    #[case] card: Card,
    #[case] lands: &[&str],
    #[case] life: i32,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = driver(card, lands, life)?;
    assert_eq!(castable(&driver)?, offered);
    Ok(())
}

#[test]
fn a_spell_that_is_not_offered_cannot_be_cast_and_nothing_changes() -> Result<(), ScriptError> {
    let mut driver = driver(phyrexian_bear, &["Island"], 1)?;
    let (hash, log) = (driver.engine().state_hash(), driver.engine().events().len());
    let result = driver.begin_cast("Phyrexian Bear");
    assert!(matches!(result, Err(ScriptError::NoSuchCard { .. })));
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(driver.engine().events().len(), log);
    Ok(())
}

#[test]
fn the_payment_decision_says_the_life_total_and_the_most_life_that_may_be_paid()
-> Result<(), ScriptError> {
    let mut driver = driver(double_phyrexian_ogre, &["Forest", "Swamp"], 3)?;
    driver.begin_cast("Double Phyrexian Ogre")?;
    let options = driver.pay_options()?;
    assert_eq!(
        options.cost,
        ManaCost::parse("{1}{B/P}{B/P}").map_err(|e| ScriptError::Expectation(e.to_string()))?
    );
    assert_eq!(options.life, 3);
    assert_eq!(options.max_life, 2, "an odd life total pays in steps of 2");
    assert!(options.auto_available);
    assert!(options.can_cancel);
    Ok(())
}

#[rstest]
#[case::more_than_the_cost_takes(6, &["Forest"], "BadLife")]
#[case::an_odd_amount(1, &["Forest", "Swamp"], "BadLife")]
#[case::more_than_the_life_total(4, &["Forest"], "NotEnoughLife")]
fn life_the_cost_does_not_take_or_the_payer_does_not_have_is_refused_before_anything_changes(
    #[case] life: u32,
    #[case] taps: &[&str],
    #[case] why: &str,
) -> Result<(), ScriptError> {
    // {1}{B/P}{B/P} at a life total of 3: one symbol may be paid with life (2), not both (4).
    let mut driver = driver(double_phyrexian_ogre, &["Forest", "Swamp", "Mountain"], 3)?;
    driver.begin_cast("Double Phyrexian Ogre")?;
    assert_eq!(driver.pay_options()?.max_life, 2);
    let (hash, log) = (driver.engine().state_hash(), driver.engine().events().len());
    let refused = driver.pay_tapping(taps, life);
    let Err(ScriptError::Engine(EngineError::Illegal(error))) = refused else {
        panic!("the payment must be refused, got {refused:?}");
    };
    assert!(format!("{error:?}").contains(why), "{error:?}");
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(driver.engine().events().len(), log);
    driver.expect_decision(0, "pay-mana")?;
    Ok(())
}

#[test]
fn a_payment_that_pays_for_the_same_symbol_twice_is_refused() -> Result<(), ScriptError> {
    // {G/P}'s mana and its 2 life together overpay a cost of {1}{G/P}.
    let mut driver = driver(phyrexian_bear, &["Forest", "Forest"], 20)?;
    driver.begin_cast("Phyrexian Bear")?;
    let hash = driver.engine().state_hash();
    let refused = driver.pay_tapping(&["Forest", "Forest"], 2);
    assert!(
        matches!(refused, Err(ScriptError::Engine(EngineError::Illegal(_)))),
        "{refused:?}"
    );
    assert_eq!(driver.engine().state_hash(), hash);
    driver.expect_life(0, 20)
}

#[test]
fn paying_all_of_ones_life_is_allowed_but_never_automatic_and_loses_the_game()
-> Result<(), ScriptError> {
    let mut driver = driver(colorless_phyrexian_elf, &[], 2)?;
    assert!(
        castable(&driver)?,
        "CR 119.4: a player may pay all their life"
    );
    driver.begin_cast("Colorless Phyrexian Elf")?;
    let options = driver.pay_options()?;
    assert!(!options.auto_available, "automatic payment keeps one life");
    assert_eq!(options.max_life, 2);
    let refused = driver.answer(Answer::PayMana(PayChoice::Auto));
    assert!(matches!(
        refused,
        Err(ScriptError::Engine(EngineError::Illegal(_)))
    ));
    driver.answer(Answer::PayMana(PayChoice::Manual(Payment {
        life: 2,
        ..Payment::default()
    })))?;
    driver.expect_life(0, 0)?;
    driver.expect_event("the payer loses", |e| {
        matches!(e, Event::PlayerLost { player, reason: LossReason::ZeroLife } if *player == pid(0))
    })?;
    assert_eq!(driver.engine().result(), Some(GameResult::Winner(pid(1))));
    Ok(())
}

#[test]
fn a_cancelled_cast_leaves_no_trace_and_can_be_paid_again_with_life() -> Result<(), ScriptError> {
    let mut driver = driver(phyrexian_bear, &["Forest"], 20)?;
    let (hash, log) = (driver.engine().state_hash(), driver.engine().events().len());
    driver.begin_cast("Phyrexian Bear")?;
    assert_eq!(
        driver.engine().state_hash(),
        hash,
        "starting a cast changes nothing"
    );
    let events = driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(events, Vec::<Event>::new());
    assert_eq!(driver.engine().state_hash(), hash);
    assert_eq!(driver.engine().events().len(), log);
    driver.expect_decision(0, "priority")?;
    driver.expect_life(0, 20)?;
    let events = driver.cast_paying("Phyrexian Bear", &["Forest"], 2)?;
    assert_eq!(life_paid(&events), [2]);
    driver.expect_life(0, 18)
}

#[test]
fn a_payment_cancelled_after_a_refusal_still_changes_nothing() -> Result<(), ScriptError> {
    let mut driver = driver(double_phyrexian_ogre, &["Forest"], 20)?;
    let hash = driver.engine().state_hash();
    driver.begin_cast("Double Phyrexian Ogre")?;
    let refused = driver.pay_tapping(&[], 2);
    assert!(
        refused.is_err(),
        "the generic part has no mana: {refused:?}"
    );
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(driver.engine().state_hash(), hash);
    Ok(())
}

#[rstest]
#[case::life_chosen_mana_finished(double_phyrexian_ogre, &["Swamp", "Forest"], &[], 2, 2, 2)]
#[case::life_added_when_no_mana_can_pay(double_phyrexian_ogre, &["Forest"], &[], 0, 4, 1)]
#[case::the_chosen_land_is_kept(phyrexian_bear, &["Forest", "Forest"], &["Forest"], 0, 0, 2)]
#[case::the_chosen_land_and_life(phyrexian_bear, &["Forest", "Island"], &["Island"], 2, 2, 1)]
fn the_engine_finishes_a_partial_payment_with_life_only_as_a_last_resort(
    #[case] card: Card,
    #[case] lands: &[&str],
    #[case] chosen: &[&str],
    #[case] life: u32,
    #[case] life_in_the_end: u32,
    #[case] lands_tapped: usize,
) -> Result<(), ScriptError> {
    let mut driver = driver(card, lands, 20)?;
    driver.begin_cast(&card().name)?;
    let sources = driver.pay_options()?.sources.clone();
    let mut taps = Vec::new();
    for name in chosen {
        let source = sources
            .iter()
            .filter(|o| !taps.contains(&o.source))
            .find(|o| {
                driver
                    .state()
                    .object(o.source.object)
                    .is_some_and(|object| object.card().name == *name)
            })
            .ok_or_else(|| ScriptError::Expectation(format!("no {name} to tap")))?;
        taps.push(source.source);
    }
    let events = driver.answer(Answer::PayMana(PayChoice::Complete(Payment {
        pool: vec![],
        taps: taps.clone(),
        life,
    })))?;
    assert_eq!(life_paid(&events).iter().sum::<u32>(), life_in_the_end);
    assert_eq!(tapped_count(&driver), lands_tapped);
    for tap in taps {
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::PermanentTapped { object } if *object == tap.object))
        );
    }
    Ok(())
}

#[test]
fn floating_mana_pays_a_choice_before_a_land_is_tapped() -> Result<(), ScriptError> {
    let mut driver = driver(hybrid_bear, &["Forest", "Plains", "Island"], 20)?;
    driver.tap_for_mana("Plains")?;
    driver.tap_for_mana("Island")?;
    let events = driver.cast("Hybrid Bear")?;
    // One floating White pays one {G/W}; the Forest pays the other and the floating Blue stays in the pool.
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ManaSpent { mana, .. } if mana.contains(&ManaType::White)
    )));
    assert_eq!(driver.state().stack().len(), 1);
    Ok(())
}

#[test]
fn life_paid_is_lost_life_not_damage_and_replays_from_the_log() -> Result<(), ScriptError> {
    let mut driver = driver(phyrexian_bear, &["Forest"], 20)?;
    driver.cast("Phyrexian Bear")?;
    driver.expect_life(0, 18)?;
    driver.expect_no_event("damage", |e| matches!(e, Event::DamageDealt { .. }))?;
    let replayed = mtg_engine::replay(driver.engine().setup(), driver.engine().events())
        .map_err(|e| ScriptError::Expectation(e.to_string()))?;
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
