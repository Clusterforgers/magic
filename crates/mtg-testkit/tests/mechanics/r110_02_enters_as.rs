//! Goal: a card put onto the battlefield as an object with changes enters with counters counted as it moves
//! (`Destination::R6502BattlefieldAs.r110_counters`, Abuelo's Awakening): an artifact or non-Aura enchantment card returns
//! as a 1/1 Spirit creature with flying in addition to its other types, with X +1/+1 counters, at tables of two to four
//! seats; an Aura card is no legal target.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype};
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::plains;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds Abuelo's Awakening and eight Plains, with the named card and Pacifism in its graveyard.
fn r110_table(seats: usize, returned: &str) -> Driver {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, (0..8).map(|_| ready(plains())))
        .graveyard(0, [card(returned).unwrap(), card("Pacifism").unwrap()])
        .hand(0, [card("Abuelo's Awakening").unwrap()]);
    Driver::from_scenario(scenario).unwrap()
}

/// The card of seat 0's graveyard with the name.
fn r110_in_graveyard(driver: &Driver, name: &str) -> mtg_core::ids::ObjectId {
    driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == name)
        .unwrap()
}

#[rstest]
fn r110_the_card_returns_as_a_spirit_with_x_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values("Sol Ring", "Intangible Virtue")] returned: &str,
    #[values(0, 3)] x: u32,
) {
    let mut driver = r110_table(seats, returned);
    driver.begin_cast("Abuelo's Awakening").unwrap();
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
        .unwrap();
    let DecisionKind::ChooseTargets(options) = driver.decision().unwrap().kind.clone() else {
        panic!("the target is asked after X");
    };
    let pacifism = r110_in_graveyard(&driver, "Pacifism");
    assert!(
        !format!("{options:?}").contains(&format!("{pacifism:?}")),
        "an Aura card is no legal target"
    );
    let card = r110_in_graveyard(&driver, returned);
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(card).unwrap().reference());
    driver.choose_targets(&[target]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let id = driver.id_of(returned).unwrap();
    assert_eq!(driver.state().object(id).unwrap().zone(), Zone::Battlefield);
    let chars = mtg_engine::characteristics(driver.state(), id).unwrap();
    assert!(chars.types.contains(CardType::Creature));
    assert!(chars.subtypes.contains(&Subtype("Spirit".to_owned())));
    let printed = if returned == "Sol Ring" {
        CardType::Artifact
    } else {
        CardType::Enchantment
    };
    assert!(chars.types.contains(printed), "it keeps its other types");
    let counters = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    assert_eq!(counters, x);
    let x = i32::try_from(x).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(1 + x), Some(1 + x)));
    assert!(keywords_of(driver.state(), id).contains(&&Keyword::Flying));
    assert_eq!(
        driver.state().object(id).unwrap().controller(),
        pid(0),
        "it returns under its owner's control"
    );
}
