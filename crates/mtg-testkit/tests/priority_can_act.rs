//! Goal: `PriorityOptions::can_act` is true exactly when the holder has a land to play, a spell it can pay for at this timing or an ability that is not a mana ability; mana abilities alone never count, and a game in which every idle holder is passed for still ends.

mod common;

use std::sync::Arc;

use common::pid;
use mtg_core::card::{AbilityDef, CardDefinition, CardType, ManaAbility};
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use mtg_engine::decision::{DecisionKind, PriorityOptions};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, elf, forest, free_creature, giant, instant, mountain, wastes};
use mtg_testkit::{Driver, PlayOptions, Scenario, ScriptError, fuzz_setup, play_game_with, ready};
use mtg_testkit::{sick, tapped};
use rstest::rstest;

type Prelude = fn(&mut Driver) -> Result<(), ScriptError>;

// Has the shape of every `Prelude`, so a case can name it like the others.
#[allow(clippy::unnecessary_wraps)]
fn nothing(_: &mut Driver) -> Result<(), ScriptError> {
    Ok(())
}

fn play_a_forest(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.play_land("Forest").map(|_| ())
}

fn tap_one_forest(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.tap_for_mana("Forest").map(|_| ())
}

fn tap_both_forests(driver: &mut Driver) -> Result<(), ScriptError> {
    tap_one_forest(driver)?;
    tap_one_forest(driver)
}

fn put_a_spell_on_the_stack(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.cast("Ghoul").map(|_| ())
}

fn mana_elf() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Mana Elf")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_ability(AbilityDef::Mana(ManaAbility {
                produces: ManaType::Green,
            })),
    )
}

/// The options of `holder` at its first priority decision after `prelude`; whoever has priority before it passes.
fn options_of(
    scenario: Scenario,
    holder: usize,
    prelude: Prelude,
) -> Result<PriorityOptions, ScriptError> {
    let mut driver = Driver::from_scenario(scenario)?;
    prelude(&mut driver)?;
    for _ in 0..8 {
        if driver.decision()?.player == pid(holder) {
            break;
        }
        driver.pass()?;
    }
    let decision = driver.decision()?;
    match &decision.kind {
        DecisionKind::Priority(options) if decision.player == pid(holder) => Ok(options.clone()),
        other => Err(ScriptError::UnexpectedDecision {
            wanted: "priority for the holder",
            got: other.name().to_owned(),
        }),
    }
}

/// Seat 0's own first main phase, so it holds priority first.
fn own_turn() -> Scenario {
    Scenario::new(2)
}

/// Seat 1's turn, so seat 0 gets priority once seat 1 has passed.
fn their_turn() -> Scenario {
    Scenario::new(2).at(2, 1, Step::Main1)
}

fn forests(count: usize) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(forest())).collect()
}

#[rstest]
#[case::empty_hand_and_no_lands(own_turn(), 0, nothing, false)]
#[case::only_a_land_in_hand(own_turn().hand(0, [forest()]), 0, nothing, true)]
#[case::two_lands_in_hand_before_the_drop(own_turn().hand(0, [forest(), forest()]), 0, nothing, true)]
#[case::land_drop_already_used(own_turn().hand(0, [forest(), forest()]), 0, play_a_forest, false)]
#[case::creature_affordable(
    own_turn().hand(0, [bear()]).battlefield(0, forests(2)), 0, nothing, true
)]
#[case::creature_one_mana_short(
    own_turn().hand(0, [bear()]).battlefield(0, forests(1)), 0, nothing, false
)]
#[case::creature_without_its_colour(
    own_turn().hand(0, [bear()]).battlefield(0, [ready(mountain()), ready(mountain())]), 0, nothing, false
)]
#[case::generic_cost_paid_by_any_colour(
    own_turn().hand(0, [bear()]).battlefield(0, [ready(forest()), ready(mountain())]), 0, nothing, true
)]
#[case::colourless_mana_pays_the_generic_part_only(
    own_turn().hand(0, [bear()]).battlefield(0, [ready(wastes()), ready(wastes())]), 0, nothing, false
)]
#[case::two_spells_one_affordable(
    own_turn().hand(0, [giant(), bear()]).battlefield(0, forests(2)), 0, nothing, true
)]
#[case::two_spells_none_affordable(
    own_turn().hand(0, [giant(), giant()]).battlefield(0, forests(2)), 0, nothing, false
)]
#[case::a_land_to_play_and_a_spell_too_dear(
    own_turn().hand(0, [forest(), giant()]), 0, nothing, true
)]
#[case::tapped_lands_pay_for_nothing(
    own_turn().hand(0, [bear()]).battlefield(0, [tapped(forest()), tapped(forest())]), 0, nothing, false
)]
#[case::a_summoning_sick_land_still_pays(
    own_turn().hand(0, [bear()]).battlefield(0, [sick(forest()), sick(forest())]), 0, nothing, true
)]
#[case::a_summoning_sick_creature_makes_no_mana(
    own_turn().hand(0, [elf()]).battlefield(0, [sick(mana_elf())]), 0, nothing, false
)]
#[case::a_ready_creature_makes_mana(
    own_turn().hand(0, [elf()]).battlefield(0, [ready(mana_elf())]), 0, nothing, true
)]
#[case::floating_mana_pays_with_a_land_untapped(
    own_turn().hand(0, [bear()]).battlefield(0, forests(2)), 0, tap_one_forest, true
)]
#[case::floating_mana_pays_alone(
    own_turn().hand(0, [bear()]).battlefield(0, forests(2)), 0, tap_both_forests, true
)]
#[case::floating_mana_that_is_not_enough(
    own_turn().hand(0, [bear()]).battlefield(0, forests(1)), 0, tap_one_forest, false
)]
#[case::opponents_turn_untapped_lands_nothing_castable(
    their_turn().hand(0, [bear()]).battlefield(0, forests(2)), 0, nothing, false
)]
#[case::opponents_turn_with_an_instant_it_can_pay(
    their_turn().hand(0, [instant("Zap", 0, &[Color::Green])]).battlefield(0, forests(1)), 0, nothing, true
)]
#[case::opponents_turn_with_an_instant_it_cannot_pay(
    their_turn().hand(0, [instant("Zap", 0, &[Color::Green])]).battlefield(0, [ready(mountain())]), 0, nothing, false
)]
#[case::opponents_turn_with_only_lands_in_hand(
    their_turn().hand(0, [forest()]).battlefield(0, forests(1)), 0, nothing, false
)]
#[case::their_empty_hand_on_their_own_turn(their_turn(), 1, nothing, false)]
#[case::stack_not_empty_a_creature_waits_for_an_empty_stack(
    own_turn().hand(0, [free_creature("Ghoul", 1, 1), bear()]).battlefield(0, forests(2)),
    0, put_a_spell_on_the_stack, false
)]
#[case::stack_not_empty_an_instant_may_respond(
    own_turn().hand(0, [free_creature("Ghoul", 1, 1), instant("Zap", 0, &[Color::Green])]).battlefield(0, forests(1)),
    0, put_a_spell_on_the_stack, true
)]
#[case::stack_not_empty_lands_wait_too(
    own_turn().hand(0, [free_creature("Ghoul", 1, 1), forest()]), 0, put_a_spell_on_the_stack, false
)]
fn can_act_table(
    #[case] scenario: Scenario,
    #[case] holder: usize,
    #[case] prelude: Prelude,
    #[case] expected: bool,
) -> Result<(), ScriptError> {
    let options = options_of(scenario, holder, prelude)?;
    assert_eq!(options.can_act, expected, "{options:?}");
    assert_eq!(
        options.can_act,
        !options.playable_lands.is_empty() || !options.castable.is_empty(),
        "can_act is the lands and spells and nothing else while no card has another ability"
    );
    assert!(options.can_pass);
    Ok(())
}

#[rstest]
#[case::lands_alone(own_turn().battlefield(0, forests(3)))]
#[case::lands_and_an_uncastable_hand(own_turn().hand(0, [giant()]).battlefield(0, forests(3)))]
#[case::their_turn(their_turn().hand(0, [bear()]).battlefield(0, forests(3)))]
fn mana_abilities_alone_never_count(#[case] scenario: Scenario) -> Result<(), ScriptError> {
    let options = options_of(scenario, 0, nothing)?;
    assert!(
        !options.mana_abilities.is_empty(),
        "the holder could tap something"
    );
    assert!(!options.can_act, "{options:?}");
    Ok(())
}

#[test]
fn tapping_a_land_never_makes_an_idle_holder_busy() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(own_turn().hand(0, [giant()]).battlefield(0, forests(3)))?;
    for _ in 0..3 {
        driver.tap_for_mana("Forest")?;
        let DecisionKind::Priority(options) = &driver.decision()?.kind else {
            panic!("priority comes back after a mana ability");
        };
        assert!(!options.can_act, "{options:?}");
    }
    // The fifth mana would be needed for the 4/4; three are floating and none is left to tap.
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority is pending");
    };
    assert!(options.mana_abilities.is_empty() && options.castable.is_empty());
    Ok(())
}

#[rstest]
fn games_in_which_every_idle_holder_is_passed_for_still_end(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3, 4, 5, 6)] seed: u64,
) {
    let options = PlayOptions {
        auto_pass: true,
        ..PlayOptions::default()
    };
    let outcome = play_game_with(fuzz_setup(players, seed), seed, options)
        .unwrap_or_else(|failure| panic!("{failure}"));
    assert!(
        outcome.auto_passes > 0,
        "bot games hold plenty of priority windows with nothing to do"
    );
    assert!(outcome.auto_passes < outcome.answers);
}
