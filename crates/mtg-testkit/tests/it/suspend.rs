//! Goal: suspending is a special action from the hand at the timing the card is cast at (the stack stays empty, a card without mana to pay or at the wrong time is not offered), the card waits in exile with its time counters in the open, each upkeep of its owner and no other removes one, the last one lets the owner cast it without paying its mana cost with every question of the cast asked inside the resolution (a spell with no legal target, or a declined cast, stays in exile), a creature cast this way can attack at once, several suspended cards in one upkeep are all cast, and a replay of the log rebuilds the game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::{CastWay, CounterKind};
use mtg_core::card::CardType;
use mtg_engine::Event;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{lands, resolve};
use crate::common::{name_of, names, pid};

fn time() -> CounterKind {
    CounterKind::Other("time".to_owned())
}

/// The id of the card called `name` in a zone.
fn find(driver: &Driver, zone: Zone, name: &str) -> mtg_core::ids::ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// The time counters on the card called `name` in exile.
fn counters(driver: &Driver, name: &str) -> u32 {
    let id = find(driver, Zone::Exile, name);
    driver.state().object(id).unwrap().card_counters_of(&time())
}

/// The cards seat 0 may suspend now.
fn suspendable(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return Vec::new();
    };
    options
        .suspendable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

/// Suspends the card called `name` from seat 0's hand and pays for it automatically.
fn suspend(driver: &mut Driver, name: &str) -> Result<Vec<Event>, ScriptError> {
    let id = find(driver, Zone::Hand(pid(0)), name);
    let events = driver.answer(Answer::Priority(PriorityAction::Suspend(id)))?;
    if driver.decision()?.kind.name() == "pay-mana" {
        return driver.answer(Answer::PayMana(PayChoice::Auto));
    }
    Ok(events)
}

/// Answers whatever the cast of a suspended card asks, aiming at `target`, until the stack is empty and seat 0 has priority.
fn settle(driver: &mut Driver, target: usize) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match driver.decision()?.kind.name() {
            "yes-no" => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            "choose-targets" => {
                let target = driver.target_player(target);
                driver.choose_targets(&[target])?;
            }
            "pay-mana" => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            "priority" if driver.state().zone(Zone::Stack).is_empty() => return Ok(()),
            "priority" => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the suspended cards".to_owned()))
}

/// Seat 0 holds `cards` and the lands, in the first main phase of turn 1.
fn game(cards: &[&str], lands: Vec<mtg_engine::PresetPermanent>) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .library(0, vec![forest(); 12])
            .hand_named(0, cards.iter().copied())
            .battlefield(0, lands),
    )
}

/// Plays on to the upkeep of `turn` and resolves what it puts on the stack, aiming casts at seat 1.
fn upkeep(driver: &mut Driver, turn: u32) -> Result<(), ScriptError> {
    driver.idle_until(turn, Step::Upkeep)?;
    settle(driver, 1)
}

#[rstest]
#[case::one_counter("Rift Bolt", 1)]
#[case::four_counters("Ancestral Vision", 4)]
fn suspending_exiles_the_card_with_its_counters_and_never_uses_the_stack(
    #[case] card: &str,
    #[case] time_counters: u32,
) -> Result<(), ScriptError> {
    let land = if card == "Ancestral Vision" {
        island()
    } else {
        mountain()
    };
    let mut driver = game(&[card], vec![ready(land)])?;
    assert_eq!(suspendable(&driver), [card]);
    let events = suspend(&mut driver, card)?;
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ZoneChanged {
            to: Zone::Exile,
            ..
        }
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::SpellCast { .. }))
    );
    assert_eq!(
        names(driver.state(), Zone::Exile),
        [card],
        "the card is open on the table"
    );
    assert_eq!(counters(&driver, card), time_counters);
    assert!(driver.state().zone(Zone::Stack).is_empty());
    assert_eq!(
        driver.decision()?.player,
        pid(0),
        "the player keeps priority"
    );
    assert!(suspendable(&driver).is_empty());
    Ok(())
}

#[test]
fn a_card_is_offered_only_with_the_mana_for_its_cost_and_at_the_timing_of_a_sorcery()
-> Result<(), ScriptError> {
    let driver = game(&["Rift Bolt"], Vec::new())?;
    assert!(suspendable(&driver).is_empty(), "no mana");
    let driver = game(&["Rift Bolt"], lands(1, island))?;
    assert!(suspendable(&driver).is_empty(), "the wrong colour");
    let late = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Upkeep)
            .hand_named(0, ["Rift Bolt"])
            .battlefield(0, lands(1, mountain)),
    )?;
    assert!(suspendable(&late).is_empty(), "not a main phase");
    Ok(())
}

#[test]
fn a_counter_is_removed_in_each_upkeep_of_the_owner_and_the_last_one_casts_the_card_for_nothing()
-> Result<(), ScriptError> {
    let mut driver = game(&["Ancestral Vision"], lands(1, island))?;
    suspend(&mut driver, "Ancestral Vision")?;
    for (turn, left) in [(3, 3), (5, 2), (7, 1)] {
        driver.idle_until(turn - 1, Step::Upkeep)?;
        assert_eq!(
            counters(&driver, "Ancestral Vision"),
            left + 1,
            "the opponent's upkeep removes none"
        );
        upkeep(&mut driver, turn)?;
        assert_eq!(counters(&driver, "Ancestral Vision"), left);
    }
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.idle_until(9, Step::Upkeep)?;
    resolve(&mut driver)?;
    let DecisionKind::Priority(_) = driver.decision()?.kind else {
        panic!("the second trigger waits on the stack");
    };
    resolve(&mut driver)?;
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        panic!("the cast is asked: {}", driver.decision()?.kind.name());
    };
    assert_eq!(
        question.text,
        "Cast Ancestral Vision without paying its mana cost?"
    );
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    let target = driver.target_player(0);
    let events = driver.choose_targets(&[target])?;
    let way = events.iter().find_map(|event| match event {
        Event::SpellCast { choices, .. } => Some(choices.way),
        _ => None,
    });
    assert_eq!(way, Some(Some(CastWay::Suspend)), "nothing is paid for it");
    settle(&mut driver, 0)?;
    assert!(driver.state().zone(Zone::Exile).is_empty());
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + 1 + 3,
        "the draw step and three cards"
    );
    Ok(())
}

#[test]
fn rift_bolt_hits_when_its_counter_is_gone_and_a_declined_cast_leaves_it_in_exile()
-> Result<(), ScriptError> {
    let mut driver = game(&["Rift Bolt"], lands(1, mountain))?;
    suspend(&mut driver, "Rift Bolt")?;
    upkeep(&mut driver, 3)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Rift Bolt"]
    );

    let mut driver = game(&["Rift Bolt"], lands(1, mountain))?;
    suspend(&mut driver, "Rift Bolt")?;
    driver.idle_until(3, Step::Upkeep)?;
    resolve(&mut driver)?;
    resolve(&mut driver)?;
    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Rift Bolt"]);
    assert_eq!(counters(&driver, "Rift Bolt"), 0);
    upkeep(&mut driver, 5).ok();
    assert_eq!(
        names(driver.state(), Zone::Exile),
        ["Rift Bolt"],
        "no counters, so nothing triggers again"
    );
    Ok(())
}

/// A sorcery that needs a creature to target, with suspend 1.
fn needs_a_creature() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
    )
    .expect("a spell in the DSL");
    Arc::new(
        CardDefinition::new("Hunter")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_testkit::cards::cost(2, &[]))
            .with_rules(spell)
            .with_rules(Ability::Keyword(Keyword::Suspend {
                time: 1,
                cost: Cost::mana("{0}".parse().unwrap()),
            })),
    )
}

#[test]
fn a_suspended_spell_that_cannot_be_cast_stays_in_exile() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [needs_a_creature()])
            .library(0, vec![forest(); 12]),
    )?;
    suspend(&mut driver, "Hunter")?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Hunter"]);
    driver.idle_until(3, Step::Upkeep)?;
    resolve(&mut driver)?;
    resolve(&mut driver)?;
    assert!(
        !matches!(driver.decision()?.kind, DecisionKind::YesNo(_)),
        "nothing is asked when there is nothing to target"
    );
    assert_eq!(names(driver.state(), Zone::Exile), ["Hunter"]);
    assert_eq!(counters(&driver, "Hunter"), 0);
    Ok(())
}

#[test]
fn a_creature_cast_from_suspend_can_attack_the_turn_it_arrives() -> Result<(), ScriptError> {
    let mut driver = game(&["Durkwood Baloth"], lands(1, forest))?;
    suspend(&mut driver, "Durkwood Baloth")?;
    assert_eq!(counters(&driver, "Durkwood Baloth"), 5);
    for turn in [3, 5, 7, 9] {
        upkeep(&mut driver, turn)?;
    }
    upkeep(&mut driver, 11)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Durkwood Baloth".to_owned()));
    driver.idle_until(11, Step::DeclareAttackers)?;
    driver.attack(&[("Durkwood Baloth", 1)])?;
    Ok(())
}

#[test]
fn two_suspended_cards_are_both_cast_in_one_upkeep() -> Result<(), ScriptError> {
    let mut driver = game(&["Rift Bolt", "Rift Bolt"], lands(2, mountain))?;
    suspend(&mut driver, "Rift Bolt")?;
    suspend(&mut driver, "Rift Bolt")?;
    assert_eq!(
        names(driver.state(), Zone::Exile),
        ["Rift Bolt", "Rift Bolt"]
    );
    upkeep(&mut driver, 3)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 14);
    assert!(driver.state().zone(Zone::Exile).is_empty());
    Ok(())
}

#[test]
fn a_replay_of_the_log_rebuilds_a_game_with_a_suspended_card() -> Result<(), ScriptError> {
    let mut driver = game(&["Rift Bolt"], lands(1, mountain))?;
    suspend(&mut driver, "Rift Bolt")?;
    upkeep(&mut driver, 3)?;
    let rebuilt = mtg_engine::Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.engine().state_hash());
    Ok(())
}
