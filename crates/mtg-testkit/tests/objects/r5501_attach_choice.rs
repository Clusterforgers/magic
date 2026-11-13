//! Goal: Sanctuary Blade's "As this Equipment becomes attached to a creature, choose a color." and "Equipped creature gets +2/+0 and has protection from the last chosen color." play at tables of two, three and four seats: equipping asks its controller for a color as it becomes attached, the equipped creature gets +2/+0 and protection from that color, a move to another creature asks again and the newer color replaces the older, an equip to the creature it is already attached to asks nothing (the ruling), and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::Engine;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 controls Sanctuary Blade, Grizzly Bears, Llanowar Elves and nine Forests; the other seats a Bear each.
fn table(seats: usize) -> Scenario {
    let mut mine = vec![
        ready(card("Sanctuary Blade").unwrap()),
        ready(card("Grizzly Bears").unwrap()),
        ready(card("Llanowar Elves").unwrap()),
    ];
    mine.extend(vec![ready(forest()); 9]);
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(card("Grizzly Bears").unwrap())]);
    }
    scenario
}

/// Equips the Blade to `host` and answers a choice of a color with `color`; whether a color was asked.
fn equip(driver: &mut Driver, host: &str, color: Color) -> bool {
    driver.begin_activation("Sanctuary Blade").unwrap();
    let target = driver.target_named(host).unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let mut asked = false;
    while !driver.state().stack().is_empty() {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Color(color))
                    .unwrap();
                assert_eq!(decision.player.index(), 0, "the Blade's controller chooses");
                asked = true;
                driver
                    .answer(Answer::ChooseOption(OptionChoice(at)))
                    .unwrap();
            }
            _ => {
                driver.pass().unwrap();
            }
        }
    }
    asked
}

fn chosen(driver: &Driver) -> Option<Chosen> {
    let blade = driver.id_of("Sanctuary Blade").unwrap();
    driver
        .state()
        .object(blade)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned()
}

fn power(driver: &Driver, id: mtg_core::ids::ObjectId) -> i32 {
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

#[rstest]
fn the_blade_chooses_a_color_each_time_it_becomes_attached(#[values(2, 3, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    assert_eq!(
        chosen(&driver),
        None,
        "nothing is chosen before it is attached"
    );
    let bears = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            crate::common::name_of(driver.state(), *id) == "Grizzly Bears"
                && driver.state().object(*id).unwrap().controller().index() == 0
        })
        .unwrap();
    let elves = driver.id_of("Llanowar Elves").unwrap();
    assert!(equip(&mut driver, "Llanowar Elves", Color::Red));
    assert_eq!(chosen(&driver), Some(Chosen::Color(Color::Red)));
    assert_eq!(power(&driver, elves), 3);
    assert!(
        mtg_engine::keywords::keywords_of(driver.state(), elves)
            .iter()
            .any(
                |keyword| matches!(keyword, mtg_cards::keyword::Keyword::Protection(_))
                    && format!("{keyword:?}").contains("Color(Red)")
            ),
        "the Elves have protection from red"
    );
    // Equipped to the creature it is on already, it does not become attached again and asks nothing.
    assert!(!equip(&mut driver, "Llanowar Elves", Color::Blue));
    assert_eq!(chosen(&driver), Some(Chosen::Color(Color::Red)));
    // Moved to the Bears, it asks again and the newer color is the last chosen.
    driver.begin_activation("Sanctuary Blade").unwrap();
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(bears).unwrap().reference());
    driver.choose_targets(&[target]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let mut asked = false;
    while !driver.state().stack().is_empty() {
        if let DecisionKind::ChooseOption(options) = &driver.decision().unwrap().kind {
            let at = options
                .options
                .iter()
                .position(|option| *option == ChoiceOption::Color(Color::Green))
                .unwrap();
            asked = true;
            driver
                .answer(Answer::ChooseOption(OptionChoice(at)))
                .unwrap();
        } else {
            driver.pass().unwrap();
        }
    }
    assert!(asked);
    assert_eq!(chosen(&driver), Some(Chosen::Color(Color::Green)));
    assert_eq!(power(&driver, bears), 4);
    assert_eq!(power(&driver, elves), 1);
    let setup = table(seats).setup().unwrap();
    let log = driver.engine().events().to_vec();
    assert_eq!(
        replay(&setup, &log).unwrap().state_hash(),
        driver.state().state_hash()
    );
    let resumed = Engine::resume(setup, log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}
