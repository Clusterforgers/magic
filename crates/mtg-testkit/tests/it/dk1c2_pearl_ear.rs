//! Goal: Pearl-Ear, Imperial Advisor gives the enchantment spells of its controller affinity for Auras (they cost {1} less for
//! each Aura that player controls, never below the colored mana, and only while Pearl-Ear is in play) and draws a card when
//! they cast an Aura spell that targets a modified permanent they control (an Aura, an Equipment or a counter on it), at
//! tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::attach_cards::{mighty_aura, winged_aura};
use mtg_testkit::cards::{cost, creature, forest, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready, with_counters};
use rstest::rstest;

use crate::common::{names, on_battlefield, pid};

/// A plain enchantment for `{3}{W}`.
fn dk1c2_shrine() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Shrine")
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(3, &[Color::White])),
    )
}

/// The Bear, with a `+1/+1` counter when `counter`, then `auras` Auras attached to it.
fn dk1c2_bear(counter: bool) -> PresetPermanent {
    let bear = ready(free_creature("Bear", 2, 2));
    if counter {
        with_counters(bear, CounterKind::PlusOnePlusOne, 1)
    } else {
        bear
    }
}

/// Seat 0 controls Pearl-Ear (when `pearl`), the Bear (place 1) with `auras` Winged Auras attached to it, and `lands`
/// lands; the hand holds `hand`. Seat 1 controls a Foe with `foe_auras` Winged Auras and, when `foe_counter`, a counter.
fn dk1c2_table(
    seats: usize,
    pearl: bool,
    auras: usize,
    bear_counter: bool,
    lands: usize,
    hand: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let first = if pearl {
        card("Pearl-Ear, Imperial Advisor").unwrap()
    } else {
        free_creature("Advisor", 1, 1)
    };
    let mut field = vec![ready(first), dk1c2_bear(bear_counter)];
    field.extend((0..auras).map(|_| attached_to(ready(winged_aura().unwrap()), 0, 1)));
    field.extend((0..lands).map(|_| ready(plains())));
    field.extend((0..lands).map(|_| ready(forest())));
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .library(0, [forest(), forest(), forest()])
        .battlefield(0, field)
}

#[rstest]
#[case::no_aura_costs_full(0, "{3}{W}")]
#[case::one_aura(1, "{2}{W}")]
#[case::two_auras(2, "{1}{W}")]
#[case::enough_auras_to_leave_the_color(5, "{W}")]
fn dk1c2_an_enchantment_costs_one_less_for_each_aura_you_control(
    #[values(2, 3, 4)] seats: usize,
    #[case] auras: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(dk1c2_table(
        seats,
        true,
        auras,
        false,
        2,
        vec![dk1c2_shrine()],
    ))?;
    driver.begin_cast("Shrine")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), expected);
    Ok(())
}

#[rstest]
#[case::without_pearl_ear(false, "Shrine", "{3}{W}")]
#[case::a_creature_spell_is_no_enchantment(true, "Cub", "{2}")]
fn dk1c2_only_enchantment_spells_with_pearl_ear_in_play_are_reduced(
    #[case] pearl: bool,
    #[case] spell: &str,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let hand = vec![dk1c2_shrine(), creature("Cub", 2, &[], 1, 1)];
    let mut driver = Driver::from_scenario(dk1c2_table(2, pearl, 2, false, 2, hand))?;
    driver.begin_cast(spell)?;
    assert_eq!(driver.pay_options()?.cost.to_string(), expected);
    Ok(())
}

#[test]
fn dk1c2_the_auras_another_player_controls_are_not_counted() -> Result<(), ScriptError> {
    let foe = [
        ready(free_creature("Foe", 2, 2)),
        attached_to(ready(winged_aura().unwrap()), 1, 0),
        attached_to(ready(winged_aura().unwrap()), 1, 0),
    ];
    let mut driver = Driver::from_scenario(
        dk1c2_table(2, true, 1, false, 2, vec![dk1c2_shrine()]).battlefield(1, foe),
    )?;
    driver.begin_cast("Shrine")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{2}{W}");
    Ok(())
}

/// Casts Mighty Aura on `target`, lets everything resolve and says whether a card was drawn.
fn dk1c2_drew_after_aura_on(driver: &mut Driver, target: &str) -> Result<bool, ScriptError> {
    let before = names(driver.state(), Zone::Hand(pid(0))).len();
    driver.begin_cast("Mighty Aura")?;
    let id = on_battlefield(driver.state(), target).unwrap();
    let aimed = Target::Object(driver.state().object(id).unwrap().reference());
    driver.choose_targets(&[aimed])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    // The Aura left the hand; a card drawn brings the hand back to its size.
    Ok(names(driver.state(), Zone::Hand(pid(0))).len() == before)
}

#[rstest]
#[case::an_aura_on_it(1, false, "Bear", true)]
#[case::a_counter_on_it(0, true, "Bear", true)]
#[case::a_counter_and_an_aura(2, true, "Bear", true)]
#[case::nothing_on_it(0, false, "Bear", false)]
#[case::a_modified_creature_of_another_player(0, false, "Foe", false)]
fn dk1c2_an_aura_spell_that_targets_a_modified_permanent_you_control_draws(
    #[values(2, 3, 4)] seats: usize,
    #[case] auras: usize,
    #[case] counter: bool,
    #[case] target: &str,
    #[case] draws: bool,
) -> Result<(), ScriptError> {
    // The Foe is always modified, so only whose it is tells the cases apart.
    let foe = [with_counters(
        ready(free_creature("Foe", 2, 2)),
        CounterKind::PlusOnePlusOne,
        1,
    )];
    let scenario = dk1c2_table(seats, true, auras, counter, 2, vec![mighty_aura().unwrap()])
        .battlefield(1, foe);
    let mut driver = Driver::from_scenario(scenario)?;
    let drew = dk1c2_drew_after_aura_on(&mut driver, target)?;
    assert_eq!(drew, draws, "{target}");
    Ok(())
}
