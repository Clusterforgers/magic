//! Goal: overload and the cast permissions play whole: an overloaded spell announces no targets and affects each object its target could be (hexproof or not, and nothing at all is no fizzle), a spell that cannot be cast for want of a legal target is still cast overloaded, a permission to cast without paying leaves the way to cast asked beside the mana cost and an X of 0, as though flash widens the timing of the controller of the permission alone and of every zone, and a permission to cast from a graveyard or the top of a library offers the card for its mana cost and the permission of another player offers nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands, pay, resolve, ways};
use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{creature, forest, free_creature, island, keyworded, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn permanent(name: &str) -> PresetPermanent {
    ready(card(name).unwrap())
}

/// The way chosen and X, as the cast logged them.
fn announced(events: &[Event]) -> mtg_engine::choose::Choices {
    events
        .iter()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.clone()),
            _ => None,
        })
        .expect("a spell was cast")
}

/// Seat 0 holds Cyclonic Rift over `count` Islands and a Bear of its own; seat 1 controls `theirs` and a Forest.
fn rift(count: usize, theirs: Vec<PresetPermanent>) -> Scenario {
    let mut mine = lands(count, island);
    mine.push(ready(free_creature("Mine", 1, 1)));
    let mut foes = theirs;
    foes.push(ready(forest()));
    Scenario::new(2)
        .hand_named(0, ["Cyclonic Rift"])
        .battlefield(0, mine)
        .battlefield(1, foes)
}

fn two_bears() -> Vec<PresetPermanent> {
    vec![
        ready(free_creature("Bear", 2, 2)),
        ready(keyworded("Hexer", 1, 1, &[Keyword::Hexproof])),
    ]
}

#[test]
fn an_overloaded_spell_asks_no_target_and_affects_each_object_it_could_target()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(rift(7, two_bears()))?;
    driver.begin_cast("Cyclonic Rift")?;
    assert_eq!(ways(&driver)?, ["Cast for {1}{U}", "Overload {6}{U}"]);
    choose_way(&mut driver, "Overload")?;
    driver.expect_decision(0, "pay-mana")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{6}{U}");
    let events = pay(&mut driver)?;
    let choices = announced(&events);
    assert_eq!(choices.way, Some(CastWay::Overload));
    assert!(choices.targets.is_empty());
    resolve(&mut driver)?;
    let mut hand = names(driver.state(), Zone::Hand(pid(1)));
    hand.sort();
    assert_eq!(
        hand,
        ["Bear", "Hexer"],
        "the hexproof creature goes too: nothing is targeted"
    );
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| *name == "Forest")
            .count(),
        1
    );
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Mine".to_owned()));
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Cyclonic Rift"]
    );
    Ok(())
}

#[test]
fn the_same_spell_cast_for_its_mana_cost_targets_one_object_and_leaves_the_others()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(rift(7, two_bears()))?;
    driver.begin_cast("Cyclonic Rift")?;
    choose_way(&mut driver, "Cast for")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("a target is chosen");
    };
    assert_eq!(
        options.legal.len(),
        1,
        "a land, my creature and a hexproof creature are no targets"
    );
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{1}{U}");
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(1))), ["Bear"]);
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Hexer".to_owned()));
    Ok(())
}

#[rstest]
#[case::two_islands_pay_only_the_mana_cost(2, two_bears(), "choose-targets")]
#[case::seven_islands_and_a_target_ask_the_way(7, two_bears(), "choose-option")]
#[case::seven_islands_and_nothing_to_target_cast_overloaded_without_asking(7, vec![ready(keyworded("Hexer", 1, 1, &[Keyword::Hexproof]))], "pay-mana")]
fn the_way_is_asked_only_when_more_than_one_can_be_used(
    #[case] count: usize,
    #[case] theirs: Vec<PresetPermanent>,
    #[case] asked: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(rift(count, theirs))?;
    driver.begin_cast("Cyclonic Rift")?;
    driver.expect_decision(0, asked)?;
    Ok(())
}

#[test]
fn nothing_to_affect_is_no_fizzle_and_a_spell_with_no_target_and_no_overload_is_not_offered()
-> Result<(), ScriptError> {
    // Overloaded with nothing of theirs but lands: the spell resolves and does nothing.
    let mut driver = Driver::from_scenario(rift(7, Vec::new()))?;
    driver.begin_cast("Cyclonic Rift")?;
    driver.expect_decision(0, "pay-mana")?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Cyclonic Rift"]
    );
    assert!(names(driver.state(), Zone::Hand(pid(1))).is_empty());
    // Two Islands cannot overload and there is no target.
    let mut driver = Driver::from_scenario(rift(2, Vec::new()))?;
    assert!(driver.begin_cast("Cyclonic Rift").is_err());
    Ok(())
}

#[test]
fn overloaded_damage_reaches_each_creature_the_caster_does_not_control() -> Result<(), ScriptError>
{
    let mut mine = lands(6, mountain);
    mine.push(ready(free_creature("Mine", 1, 1)));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Mizzium Mortars"])
            .battlefield(0, mine)
            .battlefield(
                1,
                [
                    ready(free_creature("Bear", 2, 2)),
                    ready(free_creature("Giant", 5, 5)),
                ],
            ),
    )?;
    driver.begin_cast("Mizzium Mortars")?;
    choose_way(&mut driver, "Overload")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{3}{R}{R}{R}");
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))), ["Bear"]);
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| **name == "Giant" || **name == "Mine")
            .count(),
        2,
        "the big one survives 4 damage and my own creature is not hit"
    );
    Ok(())
}

/// Seat 0 holds Mulldrifter over `count` Islands with the permanent `owner` controls, and seat 1 holds Cyclonic Rift.
fn omniscience(count: usize, permission: &str) -> Scenario {
    let mut mine = lands(count, island);
    mine.push(permanent(permission));
    Scenario::new(2)
        .hand_named(0, ["Mulldrifter"])
        .hand_named(1, ["Cyclonic Rift"])
        .battlefield(0, mine)
        .library(0, (0..4).map(|_| forest()))
}

#[test]
fn without_lands_the_free_way_is_the_only_one_and_costs_nothing() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(omniscience(0, "Omniscience"))?;
    driver.begin_cast("Mulldrifter")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{0}");
    let events = pay(&mut driver)?;
    assert_eq!(announced(&events).way, Some(CastWay::Free));
    resolve(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Mulldrifter".to_owned()));
    Ok(())
}

#[test]
fn with_lands_the_free_way_is_asked_beside_the_others_and_taps_nothing() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(omniscience(5, "Omniscience"))?;
    driver.begin_cast("Mulldrifter")?;
    assert_eq!(
        ways(&driver)?,
        [
            "Cast for {4}{U}",
            "Cast without paying its mana cost",
            "Evoke {2}{U}"
        ]
    );
    choose_way(&mut driver, "Cast without")?;
    pay(&mut driver)?;
    let state = driver.state();
    let tapped = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id)?.permanent())
        .filter(|permanent| permanent.tapped())
        .count();
    assert_eq!(tapped, 0);
    Ok(())
}

#[test]
fn the_free_way_leaves_x_at_zero_and_flashback_keeps_its_cost() -> Result<(), ScriptError> {
    let mut mine = lands(3, mountain);
    mine.push(permanent("Omniscience"));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Blaze"])
            .graveyard_named(0, ["Faithless Looting"])
            .battlefield(0, mine)
            .library(0, (0..4).map(|_| forest())),
    )?;
    driver.begin_cast("Blaze")?;
    choose_way(&mut driver, "Cast without")?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    let events = pay(&mut driver)?;
    let choices = announced(&events);
    assert_eq!((choices.way, choices.x), (Some(CastWay::Free), Some(0)));
    resolve(&mut driver)?;
    driver.expect_life(1, 20)?;
    // The permission is for cards in the hand: the graveyard card is cast for its flashback cost.
    driver.begin_cast("Faithless Looting")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{2}{R}");
    Ok(())
}

#[test]
fn a_permission_is_the_controllers_alone() -> Result<(), ScriptError> {
    // Seat 1 holds an instant and no land; the permission is seat 0's.
    let mut driver = Driver::from_scenario(omniscience(0, "Omniscience"))?;
    driver.pass()?;
    assert!(driver.begin_cast("Cyclonic Rift").is_err());
    Ok(())
}

/// Seat 0 holds `name` and the permanent `permission` over `count` Islands, on turn 2 where seat 1 is the active player.
fn opponents_turn(
    name: &str,
    permission: Option<(usize, &str)>,
    count: usize,
) -> Result<Driver, ScriptError> {
    let mut mine = lands(count, island);
    let mut theirs = Vec::new();
    match permission {
        Some((0, given)) => mine.push(permanent(given)),
        Some((_, given)) => theirs.push(permanent(given)),
        None => {}
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 1, Step::Main1)
            .hand_named(0, [name])
            .battlefield(0, mine)
            .battlefield(1, theirs)
            .library(0, (0..4).map(|_| forest())),
    )?;
    driver.pass()?;
    Ok(driver)
}

#[rstest]
#[case::with_the_orrery(Some((0, "Vedalken Orrery")), true)]
#[case::without_it(None, false)]
#[case::an_orrery_the_opponent_controls(Some((1, "Vedalken Orrery")), false)]
fn as_though_flash_lets_a_creature_be_cast_on_an_opponents_turn_for_its_controller_alone(
    #[case] permission: Option<(usize, &str)>,
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let mut driver = opponents_turn("Mulldrifter", permission, 5)?;
    assert_eq!(driver.begin_cast("Mulldrifter").is_ok(), castable);
    Ok(())
}

#[test]
fn as_though_flash_reaches_the_graveyard_too() -> Result<(), ScriptError> {
    let mut mine = lands(0, island);
    mine.extend((0..3).map(|_| ready(mountain())));
    mine.push(permanent("Vedalken Orrery"));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 1, Step::Main1)
            .graveyard_named(0, ["Faithless Looting"])
            .battlefield(0, mine)
            .library(0, (0..4).map(|_| forest())),
    )?;
    driver.pass()?;
    driver.begin_cast("Faithless Looting")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{2}{R}");
    Ok(())
}

/// A permanent that lets its controller cast the cards `what` picks from `from`.
fn key(from: &str, what: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Key")
            .with_type(CardType::Artifact)
            .with_rules(
                serde_json::from_str(&format!(
                    r#"{{"Rule":{{"rule":{{"MayCast":{{"from":"{from}","what":{what}}}}}}}}}"#
                ))
                .unwrap(),
            ),
    )
}

fn graveyard_table(from: &str, mountains: usize) -> Scenario {
    let mut mine = lands(mountains, mountain);
    mine.push(ready(key(from, r#"[{"Type":"Creature"}]"#)));
    Scenario::new(2)
        .graveyard(
            0,
            [creature("Bear", 2, &[], 2, 2), creature("Ox", 2, &[], 0, 4)],
        )
        .graveyard(1, [creature("Foe", 1, &[], 1, 1)])
        .battlefield(0, mine)
}

#[rstest]
#[case::two_lands_pay_a_creature_from_the_graveyard("Graveyard", 2, true)]
#[case::one_land_does_not("Graveyard", 1, false)]
#[case::the_permission_names_a_graveyard_not_a_hand("Hand", 2, false)]
fn a_permission_to_cast_from_the_graveyard_offers_the_card_for_its_mana_cost(
    #[case] from: &str,
    #[case] mountains: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(graveyard_table(from, mountains))?;
    assert_eq!(driver.begin_cast("Bear").is_ok(), offered);
    if offered {
        driver.expect_decision(0, "pay-mana")?;
        let events = pay(&mut driver)?;
        assert_eq!(
            announced(&events).way,
            None,
            "the printed cost is no alternative cost"
        );
        resolve(&mut driver)?;
        assert!(
            names(driver.state(), Zone::Battlefield).contains(&"Bear".to_owned()),
            "it stays, not exiled"
        );
        assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Ox"]);
    }
    Ok(())
}

#[test]
fn a_permission_names_the_cards_it_covers_and_only_the_graveyard_of_its_player()
-> Result<(), ScriptError> {
    let mut mine = lands(4, mountain);
    mine.push(ready(key("Graveyard", r#"[{"Type":"Creature"}]"#)));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard(0, [creature("Bear", 2, &[], 2, 2)])
            .graveyard_named(0, ["Faithless Looting"])
            .graveyard(1, [creature("Foe", 1, &[], 1, 1)])
            .battlefield(0, mine),
    )?;
    assert!(
        driver.begin_cast("Foe").is_err(),
        "another player's graveyard"
    );
    // The sorcery is no creature: it is cast for its flashback cost alone.
    driver.begin_cast("Faithless Looting")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{2}{R}");
    Ok(())
}

#[test]
fn a_permission_to_cast_from_a_library_offers_the_top_card_alone() -> Result<(), ScriptError> {
    let mut mine = lands(2, mountain);
    mine.push(ready(key("Library", "[]")));
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .library(
                0,
                [
                    creature("Top", 2, &[], 2, 2),
                    creature("Under", 2, &[], 2, 2),
                ],
            )
            .battlefield(0, mine),
    )?;
    assert!(driver.begin_cast("Under").is_err());
    driver.begin_cast("Top")?;
    pay(&mut driver)?;
    resolve(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Top".to_owned()));
    Ok(())
}

#[test]
fn backing_out_of_a_free_cast_leaves_the_card_where_it_was() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(omniscience(0, "Omniscience"))?;
    let before = driver.engine().events().len();
    driver.begin_cast("Mulldrifter")?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Cancel))?;
    assert_eq!(driver.engine().events().len(), before);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Mulldrifter"]);
    Ok(())
}
