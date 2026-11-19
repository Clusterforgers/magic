//! Goal: the Delves modifier rules work from an emblem on the boss's seat for every player: Mirror Hall sacrifices
//! the newest creatures beyond five of one name; No Rest, Stone Sleep and No Mercy flip a logged coin from the game's
//! seed and apply only on a loss; Living Stone's granted mana ability, Overflowing Mana's extra mana and Prismatic
//! Chaos naming only the heroes all run from an emblem; Free Command spares only the heroes the commander tax; and
//! Echoing Spells copies only the first instant or sorcery of each player each turn, whatever they cast before it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, forest, gain_life, mountain, plains};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

const MIRROR_HALL: &str = r#"{"Rule":{"rule":{"SameNameLimit":{"who":"EachPlayer","most":5}}}}"#;
const NO_REST: &str = r#"{"Rule":{"rule":{"FlipOrExiled":"EachPlayer"}}}"#;
const NO_MERCY: &str = r#"{"Rule":{"rule":{"FlipOrNoLifeGain":"EachPlayer"}}}"#;
const STONE_SLEEP: &str = r#"{"Rule":{"rule":{"FlipOrStayTapped":{"who":"EachPlayer","what":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":3}}]}}}}"#;
const LIVING_STONE: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Artifact"}]},"mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}}]}}"#;
const OVERFLOWING_MANA: &str = r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":"OfTapped"}}]}}"#;
const PRISMATIC_CHAOS: &str = r#"{"Rule":{"rule":{"CostColorsDrawn":"Opponents"}}}"#;
const FREE_COMMAND: &str = r#"{"Rule":{"rule":{"NoCommanderTax":"Opponents"}}}"#;
const ECHOING_SPELLS: &str = r#"{"Triggered":{"when":{"Nth":{"event":{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"EachPlayer"}},"nth":1}},"effects":[{"CopySpell":{"what":"Triggering","who":"TriggeringPlayer","new_targets":true}}]}}"#;

/// An instant for {W}: its controller gains 3 life.
fn mercy_salve() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Salve")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::White]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(3)],
            }),
    )
}

/// The boss's modifier emblem with this ability.
fn modifier(ability: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(ability).unwrap();
    Arc::new(emblem_definition("Modifier", &[ability]))
}

/// A table of `seats` with the modifier emblem on the last seat, the boss.
fn table(seats: usize, ability: &str) -> Scenario {
    Scenario::new(seats).emblems(seats - 1, [modifier(ability)])
}

fn named(driver: &Driver, seat: usize, name: &str) -> Vec<mtg_core::ids::ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            let object = state.object(*id).unwrap();
            object.controller() == pid(seat) && object.card().name == name
        })
        .collect()
}

fn flips(events: &[Event], player: PlayerId) -> Vec<bool> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped {
                player: who, won, ..
            } if *who == player => Some(*won),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::five_stay(5, 0, 5)]
#[case::a_sixth_goes(6, 0, 5)]
#[case::eight_keep_five(8, 0, 5)]
#[case::other_names_do_not_count(5, 5, 5)]
fn mirror_hall_keeps_five_of_a_name(
    #[case] bears: usize,
    #[case] elves: usize,
    #[case] kept: usize,
    #[values(0, 1)] seat: usize,
) -> Result<(), ScriptError> {
    let board: Vec<_> = (0..bears)
        .map(|_| ready(creature("Bear", 2, &[Color::Green], 2, 2)))
        .chain((0..elves).map(|_| ready(creature("Elf", 1, &[Color::Green], 1, 1))))
        .collect();
    let mut driver = Driver::from_scenario(table(2, MIRROR_HALL).battlefield(seat, board))?;
    driver.pass()?;
    let first: Vec<_> = named(&driver, seat, "Bear");
    assert_eq!(first.len(), kept);
    assert_eq!(named(&driver, seat, "Elf").len(), elves);
    Ok(())
}

#[rstest]
fn mirror_hall_sacrifices_the_newest(#[values(2, 4)] seats: usize) -> Result<(), ScriptError> {
    let bear = || creature("Bear", 1, &[Color::Green], 2, 2);
    let mut driver = Driver::from_scenario(
        table(seats, MIRROR_HALL)
            .battlefield(
                0,
                (0..5)
                    .map(|_| ready(bear()))
                    .chain([ready(forest()), ready(forest())]),
            )
            .hand(0, [bear()]),
    )?;
    let old = named(&driver, 0, "Bear");
    driver.cast("Bear")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(named(&driver, 0, "Bear"), old);
    assert_eq!(
        driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .filter(|id| driver.state().object(**id).unwrap().card().name == "Bear")
            .count(),
        1
    );
    Ok(())
}

#[rstest]
fn no_rest_exiles_on_a_lost_flip(
    #[values(1, 2, 3, 4, 5, 6, 7, 8)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table(2, NO_REST)
            .seed(seed)
            .battlefield(0, [ready(mountain())])
            .hand(0, [card("Lightning Bolt").unwrap()]),
    )?;
    let bolt = driver.state().zone(Zone::Hand(pid(0)))[0];
    let target = driver.target_player(1);
    let mut events = driver.cast_at("Lightning Bolt", &[target])?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    let flipped = flips(&events, pid(0));
    assert_eq!(flipped.len(), 1, "one flip for the bolt");
    let zone = driver.state().object(bolt).unwrap().zone();
    assert_eq!(
        zone,
        if flipped[0] {
            Zone::Graveyard(pid(0))
        } else {
            Zone::Exile
        }
    );
    Ok(())
}

#[rstest]
fn no_mercy_gains_nothing_on_a_lost_flip(
    #[values(1, 2, 3, 4, 5, 6, 7, 8)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table(2, NO_MERCY)
            .seed(seed)
            .battlefield(0, [ready(plains())])
            .hand(0, [mercy_salve()]),
    )?;
    let before = driver.state().player(pid(0)).unwrap().life();
    let mut events = driver.cast("Salve")?;
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    let flipped = flips(&events, pid(0));
    assert_eq!(flipped.len(), 1, "one flip for the gain");
    let gained = driver.state().player(pid(0)).unwrap().life() - before;
    assert_eq!(gained, if flipped[0] { 3 } else { 0 });
    Ok(())
}

#[rstest]
fn stone_sleep_holds_big_creatures_on_a_lost_flip(
    #[values(1, 2, 3, 4, 5, 6, 7, 8)] seed: u64,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        table(2, STONE_SLEEP)
            .seed(seed)
            .at(1, 0, Step::Untap)
            .battlefield(
                0,
                [
                    tapped(creature("Giant", 3, &[Color::Red], 3, 3)),
                    tapped(creature("Bear", 2, &[Color::Green], 2, 2)),
                ],
            ),
    )?;
    let flipped = flips(driver.engine().events(), pid(0));
    assert_eq!(flipped.len(), 1, "one flip for the untap step");
    let is_tapped = |name: &str| {
        let id = named(&driver, 0, name)[0];
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    };
    assert_eq!(is_tapped("Giant"), !flipped[0]);
    assert!(!is_tapped("Bear"));
    Ok(())
}

#[test]
fn the_flips_go_both_ways_over_seeds() {
    let lost = (1..=16u64).filter(|seed| {
        let mut driver = Driver::from_scenario(
            table(2, NO_MERCY)
                .seed(*seed)
                .battlefield(0, [ready(plains())])
                .hand(0, [mercy_salve()]),
        )
        .unwrap();
        let mut events = driver.cast("Salve").unwrap();
        while !driver.state().stack().is_empty() {
            events.extend(driver.pass().unwrap());
        }
        flips(&events, pid(0)) == [false]
    });
    let lost = lost.count();
    assert!((1..16).contains(&lost), "{lost} of 16 lost");
}

#[rstest]
fn living_stone_gives_every_artifact_a_mana_ability(
    #[values(0, 1)] seat: usize,
) -> Result<(), ScriptError> {
    let relic = Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact));
    let mut driver = Driver::from_scenario(
        table(2, LIVING_STONE)
            .at(1, seat, Step::Main1)
            .battlefield(seat, [ready(relic)]),
    )?;
    driver.tap_for_mana("Relic")?;
    assert_eq!(driver.state().player(pid(seat)).unwrap().mana().total(), 1);
    Ok(())
}

#[rstest]
fn overflowing_mana_adds_one_more_for_every_player(
    #[values(0, 1)] seat: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table(2, OVERFLOWING_MANA)
            .at(1, seat, Step::Main1)
            .battlefield(seat, [ready(forest())]),
    )?;
    driver.tap_for_mana("Forest")?;
    assert_eq!(driver.state().player(pid(seat)).unwrap().mana().total(), 2);
    Ok(())
}

#[rstest]
fn prismatic_chaos_names_only_the_heroes(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, PRISMATIC_CHAOS))?;
    for seat in 0..seats {
        let drawn = mtg_engine::rules::cost_colors_drawn(driver.state(), pid(seat));
        assert_eq!(drawn, seat != seats - 1, "seat {seat}");
    }
    Ok(())
}

/// Lets the stack resolve, keeping the targets of any copy.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(false)))?;
        } else {
            driver.pass()?;
        }
    }
    Ok(())
}

#[rstest]
fn free_command_spares_only_the_heroes_the_tax(
    #[values(0, 1, 2)] seat: usize,
    #[values(1, 2)] casts: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table(3, FREE_COMMAND)
            .format(Arc::new(Commander))
            .at(1, seat, Step::Main1)
            .commanders(seat, [legend("Cmd", 0, &[], 2, 2)])
            .battlefield(seat, [ready(mountain()), ready(mountain())]),
    )?;
    let object = driver.state().commanders().next().unwrap();
    for _ in 0..casts {
        driver.cast("Cmd")?;
        settle(&mut driver)?;
        driver.manual(ManualAction::Move {
            object,
            to: ManualZone::Graveyard,
        })?;
        driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    }
    let tax = if seat == 2 { 2 * casts } else { 0 };
    assert_eq!(driver.state().commander_tax(object), tax);
    Ok(())
}

#[rstest]
#[case::bolt_first(&[], 6)]
#[case::after_a_creature(&["Bear"], 6)]
#[case::after_another_bolt(&["Lightning Bolt"], 9)]
fn echoing_spells_copies_the_first_instant_or_sorcery(
    #[case] before: &[&str],
    #[case] dealt: i32,
    #[values(0, 2)] seat: usize,
) -> Result<(), ScriptError> {
    let hand: Vec<_> = before
        .iter()
        .map(|name| match *name {
            "Bear" => creature("Bear", 0, &[Color::Red], 2, 2),
            other => card(other).unwrap(),
        })
        .chain([card("Lightning Bolt").unwrap()])
        .collect();
    let target_seat = 1;
    let mut driver = Driver::from_scenario(
        table(3, ECHOING_SPELLS)
            .at(1, seat, Step::Main1)
            .battlefield(seat, [ready(mountain()), ready(mountain())])
            .hand(seat, hand),
    )?;
    let life = driver.state().player(pid(target_seat)).unwrap().life();
    for name in before.iter().chain(&["Lightning Bolt"]) {
        if *name == "Bear" {
            driver.cast("Bear")?;
        } else {
            let target = driver.target_player(target_seat);
            driver.cast_at(name, &[target])?;
        }
        settle(&mut driver)?;
    }
    let now = driver.state().player(pid(target_seat)).unwrap().life();
    assert_eq!(life - now, dealt);
    Ok(())
}
