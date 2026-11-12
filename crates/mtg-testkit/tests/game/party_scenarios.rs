//! Goal: the party is counted in a game from the creature types the creatures have now: none for no roles, one for any number of creatures of one role, each distinct role once, a changeling in any role it is needed for, a creature of two roles in either, and only the creatures its player controls; "a full party" holds with four and no fewer, and a spell that costs {1} less for each creature in the party is cast for exactly that much less, with one mana fewer not enough; at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready, typed_creature};
use rstest::rstest;

/// An enchantment for {1} that gains its controller life equal to the creatures in their party when it enters.
fn gauge() -> Arc<CardDefinition> {
    when_entering(
        "Party Gauge",
        r#"{"GainLife":{"amount":{"Party":"You"}}}"#,
        "",
    )
}

/// An enchantment for {1} that gains its controller 10 life when it enters if they have a full party.
fn gate() -> Arc<CardDefinition> {
    when_entering(
        "Party Gate",
        r#"{"GainLife":{"amount":10}}"#,
        r#","if":{"Compare":{"left":{"Party":"You"},"op":"Ge","right":4}}"#,
    )
}

fn when_entering(name: &str, effect: &str, condition: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}}{condition},"effects":[{effect}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(1, &[]))
            .with_rules(ability),
    )
}

/// The creatures for a party: each is the list of its creature types, `Changeling` meaning the keyword.
fn members(kinds: &[&[&str]]) -> Vec<mtg_engine::setup::PresetPermanent> {
    kinds
        .iter()
        .enumerate()
        .map(|(at, kinds)| {
            let name = format!("Member {at}");
            let card = if *kinds == ["Changeling"] {
                typed_creature(&name, 0, &[], &[], &[Keyword::Changeling])
            } else {
                typed_creature(&name, 0, &[], kinds, &[])
            };
            ready(card)
        })
        .collect()
}

const CLERIC: &[&str] = &["Cleric"];
const ROGUE: &[&str] = &["Rogue"];
const WARRIOR: &[&str] = &["Warrior"];
const WIZARD: &[&str] = &["Wizard"];
const CHANGELING: &[&str] = &["Changeling"];
const ROGUE_WARRIOR: &[&str] = &["Rogue", "Warrior"];
const ELF: &[&str] = &["Elf"];

/// Creatures of seat 0 and the size of their party.
const PARTIES: [(&[&[&str]], i32); 13] = [
    (&[], 0),
    (&[ELF, ELF], 0),
    (&[CLERIC], 1),
    (&[CLERIC, CLERIC, CLERIC], 1),
    (&[CLERIC, ROGUE], 2),
    (&[CLERIC, ROGUE, WARRIOR], 3),
    (&[CLERIC, ROGUE, WARRIOR, WIZARD], 4),
    (&[CHANGELING], 1),
    (&[CLERIC, ROGUE, WARRIOR, CHANGELING], 4),
    (&[ROGUE_WARRIOR, ROGUE], 2),
    (&[ROGUE_WARRIOR, CLERIC, WIZARD], 3),
    (&[ROGUE_WARRIOR, ROGUE, WARRIOR, CLERIC, WIZARD], 4),
    (&[CLERIC, ROGUE, WARRIOR, ELF], 3),
];

/// A table where seat 0 holds `hand` and the creatures `mine` and the lands, and seat 1 holds a full party of its own.
fn table(seats: usize, hand: &[Arc<CardDefinition>], mine: &[&[&str]], lands: usize) -> Driver {
    let mut battlefield = members(mine);
    battlefield.extend((0..lands).map(|_| ready(island())));
    let theirs = members(&[CLERIC, ROGUE, WARRIOR, WIZARD]);
    let mut scenario = Scenario::new(seats)
        .hand(0, hand.iter().cloned())
        .battlefield(0, battlefield)
        .battlefield(1, theirs);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Casts the enchantment `name` for its {1} and lets it and its trigger resolve.
fn enters(driver: &mut Driver, name: &str) {
    driver.cast_tapping(name, &["Island"]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    driver.idle_until(1, Step::End).unwrap();
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

#[rstest]
fn the_party_is_the_roles_the_creatures_of_its_player_fill(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12)] which: usize,
) {
    let (creatures, size) = PARTIES[which];
    let mut driver = table(seats, &[gauge()], creatures, 1);
    enters(&mut driver, "Party Gauge");
    assert_eq!(life(&driver), 20 + size, "{creatures:?}");
}

#[rstest]
fn a_full_party_is_four_roles_and_no_fewer(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12)] which: usize,
) {
    let (creatures, size) = PARTIES[which];
    let mut driver = table(seats, &[gate()], creatures, 1);
    enters(&mut driver, "Party Gate");
    assert_eq!(
        life(&driver),
        if size == 4 { 30 } else { 20 },
        "{creatures:?}"
    );
}

/// The lands that pay for Spoils of Adventure ({4}{W}{U}) when `size` creatures are in the party, by name.
fn spoils_lands(size: i32) -> Vec<&'static str> {
    let mut lands = vec!["Plains", "Island"];
    lands.extend((0..4 - size).map(|_| "Forest"));
    lands
}

fn spoils_table(seats: usize, creatures: &[&[&str]], lands: &[&str]) -> Driver {
    let mut battlefield = members(creatures);
    for land in lands {
        battlefield.push(match *land {
            "Plains" => ready(plains()),
            "Island" => ready(island()),
            _ => ready(forest()),
        });
    }
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Spoils of Adventure").unwrap()])
        .battlefield(0, battlefield)
        .battlefield(1, members(&[CLERIC, ROGUE, WARRIOR, WIZARD]));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
fn a_spell_costs_one_less_for_each_creature_in_the_party(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12)] which: usize,
) {
    let (creatures, size) = PARTIES[which];
    let lands = spoils_lands(size);
    let mut driver = spoils_table(seats, creatures, &lands);
    driver.cast_tapping("Spoils of Adventure", &lands).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(life(&driver), 23, "the spell resolved: {creatures:?}");
    // One mana fewer is not enough.
    let mut short = spoils_table(seats, creatures, &lands[..lands.len() - 1]);
    assert!(
        short
            .cast_tapping("Spoils of Adventure", &lands[..lands.len() - 1])
            .is_err(),
        "{creatures:?} with one land fewer"
    );
}
