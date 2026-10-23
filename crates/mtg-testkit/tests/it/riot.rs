//! Goal: a creature with riot asks its controller, as it enters, for a +1/+1 counter or haste and gets what they pick, whether the keyword is printed on it or a static ability of another permanent gives it (Rhythm of the Wild), each instance of the keyword asks on its own, a creature of another player or a token is not asked, the granting permanent leaving stops the question, and the haste lasts while the creature stays; at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::keywords_of;
use mtg_engine::setup::PresetPermanent;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

const COUNTER: usize = 0;
const HASTE: usize = 1;

/// "Nontoken creatures you control have riot."
fn rhythm() -> Arc<CardDefinition> {
    let given: Ability = serde_json::from_str(
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Riot"}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Rhythm")
            .with_type(CardType::Enchantment)
            .with_rules(given),
    )
}

/// Seat 0 holds the creatures `hand` and controls `mine`; seat 1 controls `theirs`.
fn table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
) -> Driver {
    let mut scenario = Scenario::new(seats)
        .hand(0, hand)
        .battlefield(0, mine)
        .battlefield(1, theirs);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Casts `name` and passes until the stack is empty or a riot question stands; the question, if any.
fn cast(driver: &mut Driver, name: &str) -> Option<Vec<ChoiceOption>> {
    driver.cast(name).unwrap();
    for _ in 0..12 {
        if let DecisionKind::ChooseOption(options) = &driver.decision().unwrap().kind {
            assert_eq!(driver.decision().unwrap().player, pid(0));
            assert_eq!(options.text, "Choose a +1/+1 counter or haste.");
            return Some(options.options.clone());
        }
        if driver.state().stack().is_empty() {
            return None;
        }
        driver.pass().unwrap();
    }
    panic!("the spell neither resolved nor asked");
}

fn pick(driver: &mut Driver, index: usize) {
    driver
        .answer(Answer::ChooseOption(OptionChoice(index)))
        .unwrap();
}

/// The +1/+1 counters on the named permanent and whether it has haste.
fn state_of(driver: &Driver, name: &str) -> (u32, bool) {
    let id = driver.id_of(name).unwrap();
    let counters = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .filter(|(kind, _)| **kind == CounterKind::PlusOnePlusOne)
        .map(|(_, n)| n)
        .sum();
    let haste = keywords_of(driver.state(), id).contains(&&Keyword::Haste);
    (counters, haste)
}

/// A 2/2 creature that costs nothing and prints these keywords.
fn free_with(name: &str, keywords: &[Keyword]) -> Arc<CardDefinition> {
    let mut card = (*free_creature(name, 2, 2)).clone();
    for keyword in keywords {
        card = card.with_rules(Ability::Keyword(keyword.clone()));
    }
    Arc::new(card)
}

fn riot_bear() -> Arc<CardDefinition> {
    free_with("Riot Bear", &[Keyword::Riot])
}

fn double_riot_bear() -> Arc<CardDefinition> {
    free_with("Double Riot Bear", &[Keyword::Riot, Keyword::Riot])
}

fn bear() -> Arc<CardDefinition> {
    free_with("Bear", &[])
}

#[rstest]
fn a_printed_riot_gives_a_counter_or_haste_as_the_creature_enters(
    #[values(2, 3, 4)] seats: usize,
    #[values(COUNTER, HASTE)] choice: usize,
) {
    let mut driver = table(seats, vec![riot_bear()], Vec::new(), Vec::new());
    let options = cast(&mut driver, "Riot Bear").expect("a question");
    assert_eq!(
        options,
        [
            ChoiceOption::Label("+1/+1 counter".to_owned()),
            ChoiceOption::Label("Haste".to_owned())
        ]
    );
    pick(&mut driver, choice);
    let expected = if choice == COUNTER {
        (1, false)
    } else {
        (0, true)
    };
    assert_eq!(state_of(&driver, "Riot Bear"), expected);
}

#[rstest]
fn a_creature_that_enters_under_a_granted_riot_asks_too(
    #[values(2, 3, 4)] seats: usize,
    #[values(COUNTER, HASTE)] choice: usize,
) {
    let mut driver = table(seats, vec![bear()], vec![ready(rhythm())], Vec::new());
    assert!(cast(&mut driver, "Bear").is_some());
    pick(&mut driver, choice);
    let expected = if choice == COUNTER {
        (1, false)
    } else {
        (0, true)
    };
    assert_eq!(state_of(&driver, "Bear"), expected);
}

#[rstest]
fn the_granting_permanent_leaving_stops_the_question(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, vec![bear()], Vec::new(), Vec::new());
    assert_eq!(cast(&mut driver, "Bear"), None);
    assert_eq!(state_of(&driver, "Bear"), (0, false));
}

#[rstest]
fn a_creature_of_another_player_is_not_given_riot(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, vec![bear()], Vec::new(), vec![ready(rhythm())]);
    assert_eq!(cast(&mut driver, "Bear"), None);
    assert_eq!(state_of(&driver, "Bear"), (0, false));
}

#[rstest]
fn each_instance_of_riot_chooses_on_its_own(
    #[values(2, 3, 4)] seats: usize,
    #[values((COUNTER, COUNTER), (COUNTER, HASTE), (HASTE, COUNTER), (HASTE, HASTE))] picks: (
        usize,
        usize,
    ),
) {
    let mut driver = table(
        seats,
        vec![double_riot_bear()],
        vec![ready(rhythm())],
        Vec::new(),
    );
    // Two printed instances and the granted one.
    let name = "Double Riot Bear";
    assert!(cast(&mut driver, name).is_some());
    pick(&mut driver, picks.0);
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseOption(_)
    ));
    pick(&mut driver, picks.1);
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseOption(_)
    ));
    pick(&mut driver, COUNTER);
    let (counters, haste) = state_of(&driver, name);
    let chosen = [picks.0, picks.1, COUNTER];
    let expected = u32::try_from(chosen.iter().filter(|c| **c == COUNTER).count()).unwrap();
    assert_eq!(counters, expected);
    assert_eq!(haste, chosen.contains(&HASTE));
}

#[rstest]
fn an_answer_that_is_no_option_is_refused(#[values(2, 3)] seats: usize) {
    let mut driver = table(seats, vec![riot_bear()], Vec::new(), Vec::new());
    assert!(cast(&mut driver, "Riot Bear").is_some());
    assert!(
        driver
            .answer(Answer::ChooseOption(OptionChoice(2)))
            .is_err()
    );
    pick(&mut driver, HASTE);
    assert_eq!(state_of(&driver, "Riot Bear"), (0, true));
}
