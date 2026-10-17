//! Goal: land drops obey timing and the one-per-turn limit, and summoning sickness gates attacking and tap abilities exactly as the rules say.

mod common;

use std::sync::Arc;

use common::pid;
use mtg_core::card::{AbilityDef, CardDefinition, CardType, ManaAbility};
use mtg_core::mana::ManaType;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{forest, free_creature, instant};
use mtg_testkit::{ready, sick, tapped, Driver, Scenario, ScriptError};
use rstest::rstest;

fn may_play_a_land(driver: &Driver) -> Result<bool, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::Priority(o) => Ok(!o.playable_lands.is_empty()),
        other => Err(ScriptError::UnexpectedDecision { wanted: "priority", got: other.name().to_owned() }),
    }
}

fn mana_elf() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Mana Elf")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_ability(AbilityDef::Mana(ManaAbility { produces: ManaType::Green })),
    )
}

#[rstest]
#[case::first_main_phase(Step::Main1, 0, false, false, true)]
#[case::second_main_phase(Step::Main2, 0, false, false, true)]
#[case::upkeep(Step::Upkeep, 0, false, false, false)]
#[case::draw(Step::Draw, 0, false, false, false)]
#[case::beginning_of_combat(Step::BeginCombat, 0, false, false, false)]
#[case::end_step(Step::End, 0, false, false, false)]
#[case::opponents_turn(Step::Main1, 1, false, false, false)]
#[case::spell_on_the_stack(Step::Main1, 0, true, false, false)]
#[case::land_already_played(Step::Main1, 0, false, true, false)]
fn land_play_timing_table(
    #[case] step: Step,
    #[case] holder_seat: usize,
    #[case] stack_busy: bool,
    #[case] already_played: bool,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2).at(2, 0, step).hand(0, [forest(), forest(), instant("Trick", 0, &[])]),
    )?;
    if already_played {
        driver.play_land("Forest")?;
    }
    if stack_busy {
        driver.cast("Trick")?;
    }
    if holder_seat == 1 {
        driver.pass()?;
    }
    assert_eq!(driver.decision()?.player, pid(holder_seat));
    assert_eq!(may_play_a_land(&driver)?, offered);
    Ok(())
}

#[test]
fn a_second_land_drop_is_rejected_without_changing_anything() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [forest(), forest()]))?;
    driver.play_land("Forest")?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).to_vec();
    let before = driver.engine().state_hash();
    let result = driver.answer(Answer::Priority(PriorityAction::PlayLand(hand[0])));
    assert!(matches!(result, Err(ScriptError::Engine(EngineError::Illegal(_)))));
    assert_eq!(driver.engine().state_hash(), before);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))), hand.as_slice());
    Ok(())
}

#[test]
fn land_drops_come_back_every_turn() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [forest(), forest(), forest()]))?;
    for turn in [1, 3, 5] {
        driver.idle_until(turn, Step::Main1)?;
        assert!(may_play_a_land(&driver)?, "turn {turn}");
        driver.play_land("Forest")?;
        assert!(!may_play_a_land(&driver)?, "turn {turn} after the drop");
    }
    driver.expect_zone(Zone::Battlefield, &["Forest", "Forest", "Forest"])
}

#[test]
fn a_new_land_taps_for_mana_immediately() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [forest()]))?;
    driver.play_land("Forest")?;
    driver.tap_for_mana("Forest")?;
    assert_eq!(driver.state().player(pid(0)).map(|p| p.mana().get(ManaType::Green)), Some(1));
    Ok(())
}

#[rstest]
#[case::ready(ready(free_creature("Bear", 2, 2)), true)]
#[case::summoning_sick(sick(free_creature("Bear", 2, 2)), false)]
#[case::tapped(tapped(free_creature("Bear", 2, 2)), false)]
fn only_ready_untapped_creatures_may_attack(
    #[case] creature: mtg_engine::PresetPermanent,
    #[case] may_attack: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2).at(2, 0, Step::DeclareAttackers).battlefield(0, [creature]),
    )?;
    if may_attack {
        driver.expect_decision(0, "declare-attackers")
    } else {
        driver.expect_event("NothingToDo(Attack)", |e| matches!(e, Event::NothingToDo { .. }))
    }
}

#[test]
fn a_creature_cast_this_turn_cannot_attack_until_next_turn() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).hand(0, [free_creature("Ghoul", 1, 1)]))?;
    driver.cast("Ghoul")?;
    driver.pass()?;
    driver.pass()?;
    driver.idle_until(1, Step::Main2)?;
    driver.expect_event("no attack possible this turn", |e| matches!(e, Event::NothingToDo { player, .. } if *player == pid(0)))?;
    driver.idle_until(3, Step::DeclareAttackers)?;
    driver.expect_decision(0, "declare-attackers")
}

#[rstest]
#[case::creature_ready(ready(mana_elf()), 1)]
#[case::creature_sick(sick(mana_elf()), 0)]
#[case::creature_tapped(tapped(mana_elf()), 0)]
#[case::sick_land_is_fine(sick(forest()), 1)]
#[case::tapped_land(tapped(forest()), 0)]
fn tap_abilities_respect_sickness_for_creatures_only(
    #[case] permanent: mtg_engine::PresetPermanent,
    #[case] offered: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [permanent]))?;
    match &driver.decision()?.kind {
        DecisionKind::Priority(o) => assert_eq!(o.mana_abilities.len(), offered),
        other => panic!("unexpected {other:?}"),
    }
    Ok(())
}

#[test]
fn sickness_ends_at_the_start_of_the_controllers_own_turn() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(3)
            .at(1, 0, Step::Cleanup)
            .battlefield(0, [sick(free_creature("Mine", 1, 1))])
            .battlefield(1, [sick(free_creature("Theirs", 1, 1))])
            .battlefield(2, [sick(free_creature("Other", 1, 1))]),
    )?;
    let state = driver.state();
    let sick_names: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.permanent().is_some_and(|p| p.summoning_sick()))
        .map(|o| o.card().name.clone())
        .collect();
    assert_eq!(state.turn().active(), pid(1));
    assert_eq!(sick_names, ["Mine", "Other"], "only the new active player's creature lost its sickness");
    Ok(())
}
