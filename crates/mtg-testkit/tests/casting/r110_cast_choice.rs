//! Goal: a creature type chosen as an additional cost (`CostItem::R110Choose`, Caller of the Hunt) is asked as the spell is
//! cast, kept on the spell and remembered by the permanent it becomes, so its power and toughness count the creatures of
//! that type on the battlefield, at tables of two to four seats and for types that do and do not count the creature itself.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::Chosen;
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 holds Caller of the Hunt and four Forests and controls Llanowar Elves; the last seat controls Elvish Mystic.
fn r110_table(seats: usize) -> Driver {
    let mut own: Vec<_> = (0..4).map(|_| ready(forest())).collect();
    own.push(ready(card("Llanowar Elves").unwrap()));
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, own)
        .battlefield(seats - 1, [ready(card("Elvish Mystic").unwrap())])
        .hand(0, [card("Caller of the Hunt").unwrap()]);
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
fn r110_the_type_chosen_as_it_is_cast_sets_its_size(
    #[values(2, 3, 4)] seats: usize,
    #[values(("Elf", 2), ("Human", 1), ("Goblin", 0))] case: (&str, i32),
) {
    let (kind, size) = case;
    let mut driver = r110_table(seats);
    driver.begin_cast("Caller of the Hunt").unwrap();
    let DecisionKind::ChooseOption(options) = driver.decision().unwrap().kind.clone() else {
        panic!("the creature type is asked as the spell is cast");
    };
    assert_eq!(options.text, "Choose a creature type.");
    let at = options
        .options
        .iter()
        .position(|option| *option == ChoiceOption::Label(kind.to_owned()))
        .unwrap();
    driver
        .answer(Answer::ChooseOption(OptionChoice(at)))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let spell = *driver.state().stack().last().unwrap();
    assert_eq!(
        driver
            .state()
            .object(spell)
            .unwrap()
            .choices()
            .r110_chosen(),
        Some(&Chosen::CreatureType(mtg_core::card::Subtype(
            kind.to_owned()
        )))
    );
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    if size == 0 {
        // A 0/0 is put into its owner's graveyard (CR 704.5f).
        assert!(
            crate::common::names(
                driver.state(),
                mtg_engine::zones::Zone::Graveyard(crate::common::pid(0))
            )
            .contains(&"Caller of the Hunt".to_owned())
        );
        return;
    }
    let caller = driver.id_of("Caller of the Hunt").unwrap();
    let chars = mtg_engine::characteristics(driver.state(), caller).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(size), Some(size)));
}
