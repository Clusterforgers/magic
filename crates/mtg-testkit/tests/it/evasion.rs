//! Goal: a creature that can't be blocked, or can't be blocked by some creatures, is offered no blocker it can't be blocked by, as a static ability and as the effect of a spell until the cleanup step, against blockers of every power and keyword, tapped and summoning-sick ones, with the power of a blocker judged as the layers leave it when blockers are declared, with menace, and with blockers already declared staying; combat damage that creatures you control deal can't be prevented by a shield, protection or Fog (Questing Beast) and damage of other creatures can; Questing Beast hurts a planeswalker the damaged player controls for that much and only for combat damage to a player, and its trigger is removed on resolution, with nothing else hurt, when the planeswalker it targets leaves first; and whole bot games at two, three and four seats with these cards keep every invariant and replay.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::combat::to_blockers;
use common::keywords::{Unit, card as unit_card};
use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_engine::{EngineError, Event, GameSetup, PresetPermanent, replay};
use mtg_testkit::cards::{cost, free_creature, keyworded, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::evasion_cards::{EVASION_CARDS, evader, sneak};
use mtg_testkit::{
    Driver, Scenario, ScriptError, evasion_fuzz_setup, play_game, ready, sick, tapped,
    with_counters,
};
use rstest::rstest;

const SMALL: &str =
    r#"{"CantBeBlockedBy":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]}"#;
const FLIERS: &str = r#"{"CantBeBlockedBy":[{"HasKeyword":"Flying"}]}"#;
const EXCEPT_FLIERS: &str =
    r#"{"CantBeBlockedBy":[{"Not":{"AnyOf":[{"HasKeyword":"Flying"},{"HasKeyword":"Reach"}]}}]}"#;

/// The names of the (blocker, attacker) pairs the pending blockers decision offers; none when it is not asked.
fn offered(driver: &Driver) -> Vec<(String, String)> {
    let Ok(decision) = driver.decision() else {
        return Vec::new();
    };
    let DecisionKind::DeclareBlockers(options) = &decision.kind else {
        return Vec::new();
    };
    let mut pairs: Vec<(String, String)> = options
        .blockers
        .iter()
        .flat_map(|blocker| {
            blocker.can_block.iter().map(|attacker| {
                (
                    name_of(driver.state(), blocker.object),
                    name_of(driver.state(), *attacker),
                )
            })
        })
        .collect();
    pairs.sort();
    pairs
}

fn pairs(blockers: &[&str]) -> Vec<(String, String)> {
    blockers
        .iter()
        .map(|name| ((*name).to_owned(), "A".to_owned()))
        .collect()
}

/// Player 0 about to declare attackers with `attacker`, player 1 holding `blockers`.
fn duel(
    attacker: Arc<CardDefinition>,
    blockers: Vec<PresetPermanent>,
) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(attacker)])
            .battlefield(1, blockers),
    )
}

fn units(list: &[Unit]) -> Vec<PresetPermanent> {
    list.iter().map(|unit| ready(unit_card(*unit))).collect()
}

fn attack_and_ask(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.attack(&[("A", 1)])?;
    to_blockers(driver, 1)
}

#[rstest]
#[case::unblockable_by_power_one(r#""CantBeBlocked""#, &[], &[("B", 1, 3, "")], &[])]
#[case::unblockable_by_power_three(r#""CantBeBlocked""#, &[], &[("B", 3, 3, "flying reach")], &[])]
#[case::small_by_power_one(SMALL, &[], &[("B", 1, 3, "")], &[])]
#[case::small_by_power_two(SMALL, &[], &[("B", 2, 3, "")], &[])]
#[case::small_by_power_three(SMALL, &[], &[("B", 3, 3, "")], &["B"])]
#[case::small_among_all(SMALL, &[], &[("B1", 1, 3, ""), ("B2", 2, 3, ""), ("B3", 3, 3, "")], &["B3"])]
#[case::fliers_by_a_flier(FLIERS, &[], &[("B", 1, 3, "flying")], &[])]
#[case::fliers_by_the_ground(FLIERS, &[], &[("B", 1, 3, "")], &["B"])]
#[case::fliers_by_reach(FLIERS, &[], &[("B", 1, 3, "reach")], &["B"])]
#[case::except_fliers_by_the_ground(EXCEPT_FLIERS, &[], &[("B", 3, 3, "")], &[])]
#[case::except_fliers_by_a_flier(EXCEPT_FLIERS, &[], &[("B", 1, 3, "flying")], &["B"])]
#[case::except_fliers_by_reach(EXCEPT_FLIERS, &[], &[("B", 1, 3, "reach")], &["B"])]
#[case::small_and_menace_with_one_big(SMALL, &[Keyword::Menace], &[("B1", 1, 3, ""), ("B2", 3, 3, "")], &[])]
#[case::small_and_menace_with_two_big(SMALL, &[Keyword::Menace], &[("B1", 3, 3, ""), ("B2", 4, 3, "")], &["B1", "B2"])]
fn an_attacker_is_offered_only_the_blockers_that_may_block_it(
    #[case] mods: &str,
    #[case] keywords: &[Keyword],
    #[case] blockers: &[Unit],
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let attacker = evader("A", 2, 2, &format!("[{mods}]"), keywords).unwrap();
    let mut driver = duel(attacker, units(blockers))?;
    attack_and_ask(&mut driver)?;
    assert_eq!(offered(&driver), pairs(expected));
    Ok(())
}

#[rstest]
#[case::ready(0, true)]
#[case::summoning_sick(1, true)]
#[case::tapped(2, false)]
fn a_blocker_is_judged_by_its_power_whether_it_is_sick_or_tapped(
    #[case] state: u8,
    #[case] offered_to_block: bool,
) -> Result<(), ScriptError> {
    let attacker = evader("A", 2, 2, &format!("[{SMALL}]"), &[]).unwrap();
    let big = free_creature("B", 3, 3);
    let blocker = match state {
        0 => ready(big),
        1 => sick(big),
        _ => tapped(big),
    };
    let mut driver = duel(attacker, vec![blocker])?;
    attack_and_ask(&mut driver)?;
    assert_eq!(
        offered(&driver),
        pairs(if offered_to_block { &["B"] } else { &[] })
    );
    Ok(())
}

#[rstest]
#[case::grown_past_the_limit(1, 2, true)]
#[case::shrunk_to_the_limit(3, -1, false)]
#[case::grown_but_still_small(1, 1, false)]
fn the_power_of_a_blocker_is_judged_as_the_layers_leave_it_when_blocks_are_declared(
    #[case] power: i32,
    #[case] change: i32,
    #[case] offered_to_block: bool,
) -> Result<(), ScriptError> {
    let attacker = evader("A", 2, 2, &format!("[{SMALL}]"), &[]).unwrap();
    let mut driver = duel(attacker, vec![ready(free_creature("B", power, 5))])?;
    driver.attack(&[("A", 1)])?;
    let blocker = driver.id_of("B")?;
    driver.add_effect(
        driver.on(blocker),
        Modification::ModifyPowerToughness {
            power: change,
            toughness: 0,
        },
        Duration::UntilEndOfTurn,
    )?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(
        offered(&driver),
        pairs(if offered_to_block { &["B"] } else { &[] })
    );
    Ok(())
}

#[test]
fn a_block_that_was_not_offered_is_refused_and_changes_nothing() -> Result<(), ScriptError> {
    let attacker = evader("A", 2, 2, &format!("[{SMALL}]"), &[]).unwrap();
    let mut driver = duel(
        attacker,
        vec![
            ready(free_creature("Small", 1, 5)),
            ready(free_creature("Big", 3, 5)),
        ],
    )?;
    attack_and_ask(&mut driver)?;
    assert_eq!(offered(&driver), pairs(&["Big"]));
    let (small, attacker) = (driver.id_of("Small")?, driver.id_of("A")?);
    let before = driver.engine().state_hash();
    let refused = driver.answer(Answer::DeclareBlockers(vec![(small, attacker)]));
    assert_eq!(
        refused.err(),
        Some(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::NotOffered("blocker")
        )))
    );
    assert_eq!(driver.engine().state_hash(), before);
    driver.block(&[("Big", "A")])?;
    Ok(())
}

#[test]
fn losing_the_ability_before_blockers_lets_everything_block_and_gaining_it_after_changes_nothing()
-> Result<(), ScriptError> {
    // An unblockable attacker that loses its abilities after it attacks can be blocked.
    let attacker = evader("A", 2, 2, r#"["CantBeBlocked"]"#, &[]).unwrap();
    let mut driver = duel(attacker, vec![ready(free_creature("B", 1, 5))])?;
    driver.attack(&[("A", 1)])?;
    let id = driver.id_of("A")?;
    driver.add_effect(
        driver.on(id),
        Modification::LoseAbilities,
        Duration::UntilEndOfTurn,
    )?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver), pairs(&["B"]));
    // Blocked, it gains the restriction: it stays blocked, deals its damage to the blocker and none to the player.
    driver.block(&[("B", "A")])?;
    driver.add_effect(
        driver.on(id),
        Modification::CantBeBlocked,
        Duration::UntilEndOfTurn,
    )?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 20)?;
    let blocker = driver.id_of("B")?;
    let marked = driver
        .state()
        .object(blocker)
        .and_then(|o| o.permanent())
        .map(mtg_engine::object::PermanentState::damage);
    assert_eq!(marked, Some(2));
    Ok(())
}

/// Plays on until the stack is empty and a player has priority.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[test]
fn a_spell_makes_a_creature_unblockable_until_the_cleanup_step() -> Result<(), ScriptError> {
    let filler = || (0..10).map(|_| free_creature("Filler", 1, 1));
    let scenario = Scenario::new(2)
        .at(2, 0, Step::Main1)
        .library(0, filler())
        .library(1, filler())
        .hand(0, [sneak("Sneak").unwrap()])
        .battlefield(0, [ready(mountain()), ready(free_creature("A", 2, 2))])
        .battlefield(1, [ready(free_creature("B", 1, 5))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("A")?;
    driver.cast_at("Sneak", &[target])?;
    settle(&mut driver)?;
    driver.idle_until(2, Step::DeclareAttackers)?;
    attack_and_ask(&mut driver)?;
    assert_eq!(offered(&driver), Vec::new(), "unblockable this turn");
    // The effect is gone in the next turn of its controller: the creature can be blocked again.
    driver.idle_until(4, Step::DeclareAttackers)?;
    attack_and_ask(&mut driver)?;
    assert_eq!(offered(&driver), pairs(&["B"]));
    Ok(())
}

// Damage that can't be prevented (Questing Beast)

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(common::pid(seat)).unwrap().life()
}

/// Plays on to the end of the combat, answering passively and blocking as `blocks` says.
fn through_combat(driver: &mut Driver, blocks: &[(&str, &str)]) -> Result<(), ScriptError> {
    for _ in 0..120 {
        if driver.state().turn().step() == Step::Main2
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(blocks)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat".into()))
}

fn big_attacker(name: &str) -> Arc<CardDefinition> {
    if name == "Questing Beast" {
        card(name).unwrap()
    } else {
        keyworded("Ogre", 4, 4, &[])
    }
}

#[rstest]
#[case::fog_against_the_beast("Questing Beast", "fog", 16)]
#[case::fog_against_another_creature("Ogre", "fog", 20)]
#[case::a_shield_against_the_beast("Questing Beast", "shield", 16)]
#[case::a_shield_against_another_creature("Ogre", "shield", 20)]
#[case::nothing_against_the_beast("Questing Beast", "none", 16)]
fn combat_damage_of_questing_beast_passes_fog_and_shields(
    #[case] attacker: &str,
    #[case] prevention: &str,
    #[case] life_left: i32,
) -> Result<(), ScriptError> {
    let filler = || (0..10).map(|_| free_creature("Filler", 1, 1));
    let spell = match prevention {
        "fog" => Some("Fog"),
        "shield" => Some("Mending Hands"),
        _ => None,
    };
    let scenario = Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .library(0, filler())
        .library(1, filler())
        .battlefield(0, [ready(big_attacker(attacker))])
        .battlefield(
            1,
            [
                ready(mtg_testkit::cards::forest()),
                ready(mtg_testkit::cards::plains()),
            ],
        )
        .hand(1, spell.map(|name| card(name).unwrap()));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[(
        if attacker == "Ogre" {
            "Ogre"
        } else {
            "Questing Beast"
        },
        1,
    )])?;
    // The spell is cast once the blockers are declared; a shield is made for the defending player.
    driver.pass()?;
    if let Some(name) = spell {
        if name == "Fog" {
            driver.cast("Fog")?;
        } else {
            let me = driver.target_player(1);
            driver.cast_at("Mending Hands", &[me])?;
        }
        settle(&mut driver)?;
    }
    through_combat(&mut driver, &[])?;
    assert_eq!(life(&driver, 1), life_left);
    Ok(())
}

#[rstest]
#[case::the_beast("Questing Beast", 4)]
#[case::another_creature("Ogre", 0)]
fn protection_does_not_stop_the_combat_damage_of_questing_beast(
    #[case] attacker: &str,
    #[case] marked: u32,
) -> Result<(), ScriptError> {
    let guard = keyworded(
        "Guard",
        3,
        9,
        &[
            Keyword::Protection(Selector(vec![Pred::Color(Color::Green)])),
            Keyword::Indestructible,
        ],
    );
    let ogre = Arc::new(
        CardDefinition::new("Ogre")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[Color::Green]))
            .with_power_toughness(4, 4),
    );
    let attacker_card = if attacker == "Ogre" {
        ogre
    } else {
        card(attacker).unwrap()
    };
    let scenario = Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .library(0, (0..10).map(|_| free_creature("Filler", 1, 1)))
        .library(1, (0..10).map(|_| free_creature("Filler", 1, 1)))
        .battlefield(0, [ready(attacker_card)])
        .battlefield(1, [ready(guard)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[(attacker, 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("Guard", attacker)])?;
    driver.idle_until(2, Step::EndCombat)?;
    let guard = driver.id_of("Guard")?;
    let dealt = driver
        .state()
        .object(guard)
        .and_then(|o| o.permanent())
        .map(mtg_engine::object::PermanentState::damage);
    assert_eq!(dealt, Some(marked));
    Ok(())
}

// The trigger on a planeswalker

fn loyalty_of(driver: &Driver, name: &str) -> Option<u32> {
    let id = driver.id_of(name).ok()?;
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .map(|p| p.counters_of(&CounterKind::Loyalty))
}

fn walker(name: &str, loyalty: u32) -> PresetPermanent {
    with_counters(ready(card(name).unwrap()), CounterKind::Loyalty, loyalty)
}

#[rstest]
#[case::no_planeswalker(&[], &[], 2, &[])]
#[case::one_planeswalker(&[("Jace Beleren", 3)], &[], 2, &[("Jace Beleren", 0)])]
#[case::two_planeswalkers_one_is_hurt(&[("Jace Beleren", 6), ("Garruk Wildspeaker", 6)], &[], 2, &[("Jace Beleren", 2)])]
#[case::a_planeswalker_of_another_player(&[], &[("Jace Beleren", 3)], 3, &[("Jace Beleren", 3)])]
fn questing_beast_hurts_a_planeswalker_the_damaged_player_controls(
    #[case] defender: &[(&str, u32)],
    #[case] bystander: &[(&str, u32)],
    #[case] seats: usize,
    #[case] expected: &[(&str, u32)],
) -> Result<(), ScriptError> {
    let filler = || (0..10).map(|_| free_creature("Filler", 1, 1));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Questing Beast").unwrap())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, filler());
    }
    scenario = scenario.battlefield(1, defender.iter().map(|(name, n)| walker(name, *n)));
    if seats > 2 {
        scenario = scenario.battlefield(2, bystander.iter().map(|(name, n)| walker(name, *n)));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Questing Beast", 1)])?;
    to_blockers(&mut driver, 1)?;
    through_combat(&mut driver, &[])?;
    assert_eq!(life(&driver, 1), 16, "the player takes the damage first");
    // Exactly the walkers `expected` names lost 4 loyalty (or died), the rest are as they were.
    let hurt: BTreeSet<&str> = defender
        .iter()
        .filter(|(name, was)| loyalty_of(&driver, name) != Some(*was))
        .map(|(name, _)| *name)
        .collect();
    let want: BTreeSet<&str> = if defender.is_empty() {
        BTreeSet::new()
    } else {
        // One of the walkers the damaged player controls takes all 4; the bot picks the first legal target.
        BTreeSet::from([defender[0].0])
    };
    assert_eq!(hurt, want);
    for (name, loyalty) in expected {
        if defender.len() == 1 || seats > 2 {
            assert_eq!(loyalty_of(&driver, name).unwrap_or(0), *loyalty, "{name}");
        }
    }
    Ok(())
}

#[test]
fn combat_damage_to_a_creature_or_a_planeswalker_does_not_trigger_the_beast()
-> Result<(), ScriptError> {
    let filler = || (0..10).map(|_| free_creature("Filler", 1, 1));
    let scenario = Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .library(0, filler())
        .library(1, filler())
        .battlefield(0, [ready(card("Questing Beast").unwrap())])
        .battlefield(
            1,
            [
                ready(free_creature("Wall", 3, 9)),
                walker("Jace Beleren", 6),
                walker("Garruk Wildspeaker", 6),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Questing Beast", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("Wall", "Questing Beast")])?;
    let events: Vec<Event> = {
        let mut seen = Vec::new();
        for _ in 0..60 {
            if driver.state().turn().step() == Step::Main2 {
                break;
            }
            seen.extend(driver.answer_passively()?);
        }
        seen
    };
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::AbilityTriggered(_))),
        "{events:?}"
    );
    assert_eq!(loyalty_of(&driver, "Jace Beleren"), Some(6));
    assert_eq!(loyalty_of(&driver, "Garruk Wildspeaker"), Some(6));
    Ok(())
}

/// Plays combat on with no blockers until a trigger waits on the stack with a player to act.
fn until_the_trigger_waits(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..120 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::DeclareBlockers(_) => {
                driver.block(&[])?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the trigger".into()))
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn the_trigger_of_questing_beast_is_removed_when_its_planeswalker_leaves_first(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let filler = || (0..10).map(|_| free_creature("Filler", 1, 1));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Questing Beast").unwrap())])
        .battlefield(
            1,
            [
                walker("Jace Beleren", 3),
                ready(free_creature("Guard", 2, 5)),
                ready(mountain()),
            ],
        )
        .hand(1, [mtg_testkit::cards::burn("Bolt", 3)]);
    for seat in 0..seats {
        scenario = scenario.library(seat, filler());
    }
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [walker("Garruk Wildspeaker", 3)]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Questing Beast", 1)])?;
    to_blockers(&mut driver, 1)?;
    until_the_trigger_waits(&mut driver)?;
    assert_eq!(life(&driver, 1), 16, "the player took the combat damage");
    assert_eq!(loyalty_of(&driver, "Jace Beleren"), Some(3));
    // The trigger is on the stack with the only walker the damaged player controls as its target. The damaged player
    // answers by burning that walker away.
    while driver.state().player(common::pid(1)).is_some()
        && driver.decision()?.player != common::pid(1)
    {
        driver.pass()?;
    }
    let jace = driver.target_named("Jace Beleren")?;
    driver.cast_at("Bolt", &[jace])?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    let ended: Vec<ResolutionOutcome> = events
        .iter()
        .filter_map(|event| match event {
            Event::ResolutionEnded { outcome, .. } => Some(*outcome),
            _ => None,
        })
        .collect();
    assert_eq!(
        ended,
        vec![ResolutionOutcome::Resolved, ResolutionOutcome::Fizzled],
        "the Bolt resolves, then the trigger of the Beast is removed: {events:?}"
    );
    assert_eq!(loyalty_of(&driver, "Jace Beleren"), None, "the walker died");
    assert_eq!(life(&driver, 1), 16, "nothing else was hurt");
    for seat in 2..seats {
        assert_eq!(loyalty_of(&driver, "Garruk Wildspeaker"), Some(3));
        assert_eq!(life(&driver, seat), 20);
    }
    let guard = driver.id_of("Guard")?;
    let marked = driver
        .state()
        .object(guard)
        .and_then(|o| o.permanent())
        .map(mtg_engine::object::PermanentState::damage);
    assert_eq!(marked, Some(0));
    let beast = driver.id_of("Questing Beast")?;
    assert!(driver.state().object(beast).is_some());
    Ok(())
}

// Whole games

fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}

fn triggered_cards(setup: &GameSetup, events: &[Event]) -> BTreeSet<String> {
    let state = replay(setup, events).unwrap();
    events
        .iter()
        .filter_map(|event| match event {
            Event::AbilityTriggered(triggered) => state.object(triggered.source.id),
            _ => None,
        })
        .map(|object| object.card().name.clone())
        .collect()
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_evasion_games_finish_keep_every_invariant_and_trigger_the_beast(#[case] players: usize) {
    let games = games();
    let mut seen = BTreeSet::new();
    for seed in 0..games {
        let setup = evasion_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.extend(triggered_cards(&setup, &outcome.events));
    }
    assert!(
        seen.contains("Questing Beast"),
        "the Beast never triggered in {games} games of {players} seats: {seen:?}"
    );
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn a_seeded_evasion_game_is_the_same_game_every_time_and_replays_to_its_state(
    #[case] players: usize,
) {
    for seed in 0..6u64 {
        let setup = evasion_fuzz_setup(players, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.events, second.events, "seed {seed}");
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        let state = replay(&setup, &first.events).unwrap();
        assert_eq!(state.state_hash(), first.final_hash, "seed {seed}");
    }
}

#[test]
fn the_cards_of_an_evasion_deck_are_all_in_the_corpus() {
    for (name, _) in EVASION_CARDS {
        assert!(card(name).is_ok(), "{name}");
    }
}
