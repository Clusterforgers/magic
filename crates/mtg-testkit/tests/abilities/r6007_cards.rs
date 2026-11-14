//! Goal: the r60-07 cards play as printed at two to four seats: Breathstealer's Crypt reveals each card drawn and a
//! creature card is discarded unless its drawer pays 3 life; Zur's Weirding reveals the card instead and the first other
//! player in turn order who pays 2 life mills it, or it is drawn; Elvish Refueler lets its controller activate an exhaust
//! ability again on their turn until they activate one; Dragon's Disciple enters with a counter for a Dragon revealed
//! from the hand or controlled. Each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn r6007_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Answers every pending question, `yes` to each yes-or-no, until the stack and the waiting triggers are empty.
fn r6007_settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let state = driver.state();
        let busy = !state.stack().is_empty() || !state.pending_triggers().is_empty();
        match &driver.decision()?.kind {
            DecisionKind::YesNo(options) => {
                let yes = yes && !options.only_no;
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) | DecisionKind::ChooseTargets(_) => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// An artifact sacrificed for these steps (JSON), at no mana.
fn r6007_gadget(name: &str, effects: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["SacrificeSelf"],"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

fn r6007_use(driver: &mut Driver, name: &str, yes: bool) -> Result<(), ScriptError> {
    driver.begin_ability(name, 0)?;
    r6007_settle(driver, yes)
}

/// A library of named cards, top first.
fn r6007_pages(seat: usize) -> Vec<Arc<CardDefinition>> {
    ["Top", "Middle", "Bottom"]
        .iter()
        .map(|place| free_creature(&format!("{place} {seat}"), 1, 1))
        .collect()
}

fn r6007_hand(driver: &Driver, seat: usize) -> Vec<String> {
    common::names(driver.state(), Zone::Hand(pid(seat)))
}

fn r6007_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn r6007_breathstealers_crypt_discards_a_drawn_creature_unless_life_is_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] creature: bool,
    #[values(false, true)] pay: bool,
) -> Result<(), ScriptError> {
    let top = if creature {
        free_creature("Drawn", 2, 2)
    } else {
        plains()
    };
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Breathstealer's Crypt").unwrap()),
                ready(r6007_gadget("Study", r#"[{"Draw":{}}]"#)),
            ],
        )
        .library(0, [top, plains()]);
    let mut driver = Driver::from_scenario(scenario)?;
    r6007_use(&mut driver, "Study", pay)?;
    let kept = !creature || pay;
    assert_eq!(r6007_hand(&driver, 0).len(), usize::from(kept));
    assert_eq!(
        common::names(driver.state(), Zone::Graveyard(pid(0))).len(),
        // The sacrificed gadget, and a creature card discarded.
        1 + usize::from(!kept)
    );
    assert_eq!(
        r6007_life(&driver, 0),
        if creature && pay { 17 } else { 20 }
    );
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_zurs_weirding_mills_the_card_when_another_player_pays(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] pay: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Zur's Weirding").unwrap()),
                ready(r6007_gadget("Study", r#"[{"Draw":{}}]"#)),
            ],
        )
        .library(0, r6007_pages(0));
    let mut driver = Driver::from_scenario(scenario)?;
    r6007_use(&mut driver, "Study", pay)?;
    let graveyard = common::names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(graveyard.contains(&"Top 0".to_owned()), pay);
    assert_eq!(
        r6007_hand(&driver, 0),
        if pay {
            Vec::new()
        } else {
            vec!["Top 0".to_owned()]
        }
    );
    // The drawing player is never asked; the first other player in turn order pays.
    assert_eq!(r6007_life(&driver, 0), 20);
    assert_eq!(r6007_life(&driver, 1), if pay { 18 } else { 20 });
    for seat in 2..seats {
        assert_eq!(r6007_life(&driver, seat), 20);
    }
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_elvish_refueler_exhausts_again_until_one_is_activated_this_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(3, 0, Step::Main1).battlefield(
        0,
        std::iter::once(ready(card("Elvish Refueler").unwrap()))
            .chain([(); 6].map(|()| ready(mtg_testkit::cards::forest()))),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let grown = |driver: &Driver| {
        let id = driver.id_of("Elvish Refueler").unwrap();
        driver
            .state()
            .object(id)
            .and_then(|object| object.permanent())
            .map_or(0, |permanent| {
                permanent.counters_of(&CounterKind::PlusOnePlusOne)
            })
    };
    driver.begin_ability("Elvish Refueler", 1)?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    r6007_settle(&mut driver, true)?;
    if std::env::var("R6007_DEBUG").is_ok() {
        for event in driver.engine().events() {
            println!("{event:?}");
        }
    }
    assert_eq!(grown(&driver), 1);
    // Activated an exhaust ability this turn: not again.
    assert!(driver.begin_ability("Elvish Refueler", 1).is_err());
    // On the next turn of its controller it may be activated once more, though it was activated before.
    let next = 3 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Main1)?;
    driver.begin_ability("Elvish Refueler", 1)?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    r6007_settle(&mut driver, true)?;
    assert_eq!(grown(&driver), 2);
    r6007_replays(&driver);
    Ok(())
}

#[rstest]
fn r6007_dragons_disciple_grows_for_a_dragon_revealed_or_controlled(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] dragon: u8,
    #[values(false, true)] reveal: bool,
) -> Result<(), ScriptError> {
    let dragon_card = || {
        Arc::new(
            CardDefinition::new("Wyrm")
                .with_type(CardType::Creature)
                .with_subtype("Dragon")
                .with_power_toughness(4, 4),
        )
    };
    let mut lands: Vec<_> = [(); 3].map(|()| ready(plains())).into();
    let mut hand = vec![card("Dragon's Disciple").unwrap()];
    // 0: no Dragon; 1: a Dragon card in hand; 2: a Dragon permanent.
    match dragon {
        1 => hand.push(dragon_card()),
        2 => lands.push(ready(dragon_card())),
        _ => {}
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, lands)
        .hand(0, hand);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Dragon's Disciple")?;
    r6007_settle(&mut driver, reveal)?;
    let id = driver.id_of("Dragon's Disciple")?;
    let counters = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    let grows = dragon == 2 || (dragon == 1 && reveal);
    assert_eq!(counters, u32::from(grows));
    r6007_replays(&driver);
    Ok(())
}

#[test]
fn r6007_support_probe() {
    if std::env::var("R6007_SUPPORT").is_err() {
        return;
    }
    for name in [
        "Breathstealer's Crypt",
        "Zur's Weirding",
        "Dragon's Disciple",
        "Elvish Refueler",
    ] {
        println!(
            "{name}: {:?}",
            mtg_testkit::corpus::corpus()
                .unwrap()
                .support(name)
                .unwrap()
        );
    }
}
