//! Goal: "Whenever you activate an ability of a creature" (`Did { ActivatedAny }`) triggers on a mana ability, at priority and while paying, and on any other ability, never on a land's mana or an ability of another kind of permanent; `Activated` still leaves the mana ability out; an opponent's watcher sees the activation; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::settle;
use crate::common::pid;

/// A creature with "{T}: Add {G}.", "{0}: You gain 1 life." and "Discard a card: Add {R}." (a mana ability the payment
/// does not run, announced like any ability)
fn dork() -> Arc<CardDefinition> {
    let rules = [
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        r#"{"Activated":{"cost":[],"effects":[{"GainLife":{"amount":1}}]}}"#,
        r#"{"Activated":{"cost":[{"Discard":{}}],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}}]}}"#,
    ];
    let mut card = CardDefinition::new("Dork")
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1);
    for rule in rules {
        card = card.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(card)
}

/// An enchantment with "Whenever <who> <action> of a creature, you gain 100 life."
fn watcher(action: &str, who: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Triggered":{{"when":{{"Did":{{"action":"{action}","who":"{who}","what":[{{"Type":"Creature"}},{{"In":"Battlefield"}}]}}}},"effects":[{{"GainLife":{{"amount":100}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// What the activator (seat 0) and the watcher's controller gain when seat 0 does `act`.
fn gains(
    seats: usize,
    action: &str,
    watcher_seat: usize,
    act: impl FnOnce(&mut Driver) -> Result<(), ScriptError>,
) -> Result<i32, ScriptError> {
    let who = if watcher_seat == 0 {
        "You"
    } else {
        "Opponents"
    };
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(dork()), ready(forest())])
        .hand(0, [bear()]);
    scenario = scenario.battlefield(
        watcher_seat,
        if watcher_seat == 0 {
            vec![ready(dork()), ready(forest()), ready(watcher(action, who))]
        } else {
            vec![ready(watcher(action, who))]
        },
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let life = |driver: &Driver| driver.state().player(pid(watcher_seat)).unwrap().life();
    let before = life(&driver);
    act(&mut driver)?;
    settle(&mut driver)?;
    Ok(life(&driver) - before)
}

#[rstest]
#[case::any_on_a_mana_ability("ActivatedAny", 0, "mana", 100)]
#[case::any_on_another_ability("ActivatedAny", 0, "life", 101)]
#[case::any_not_on_a_land("ActivatedAny", 0, "land", 0)]
#[case::any_while_paying("ActivatedAny", 0, "pay", 100)]
#[case::any_on_an_announced_mana_ability("ActivatedAny", 0, "announced", 100)]
#[case::not_mana_leaves_an_announced_mana_ability_out("Activated", 0, "announced", 0)]
#[case::any_seen_by_an_opponent("ActivatedAny", 1, "mana", 100)]
#[case::not_mana_leaves_the_mana_out("Activated", 0, "mana", 0)]
#[case::not_mana_on_another_ability("Activated", 0, "life", 101)]
fn a_mana_ability_is_an_activation(
    #[case] action: &str,
    #[case] watcher_seat: usize,
    #[case] act: &str,
    #[case] expected: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let gained = gains(seats, action, watcher_seat, |driver| match act {
        "mana" => driver.tap_for_mana("Dork").map(drop),
        "land" => driver.tap_for_mana("Forest").map(drop),
        "life" => driver.begin_ability("Dork", 1).map(drop),
        "announced" => driver.begin_ability("Dork", 2).map(drop),
        _ => driver
            .cast_tapping("Grizzly Bears", &["Dork", "Forest"])
            .map(drop),
    })?;
    assert_eq!(gained, expected);
    Ok(())
}
