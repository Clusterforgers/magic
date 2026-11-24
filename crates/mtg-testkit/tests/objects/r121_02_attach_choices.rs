//! Goal: choices made as an Equipment becomes attached play at two to four seats: Psychic Paper asks a creature card
//! name and a creature type, and the equipped creature has ward {1}, that name and only that creature type, a move to
//! another creature asks again and the first creature gets its own name back; Dinosaur Headdress, crafted from
//! Paleontologist's Pick-Axe, asks for one of the exiled creature cards used to craft it, the equipped creature is a copy
//! of the last chosen card and a move to another creature makes that one a copy of the newer choice; a non-creature name
//! is refused, and each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::Subtype;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, ChoiceOption, NameChoice, NumberChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 controls `equipment`, Grizzly Bears, Llanowar Elves and nine Forests, with `graveyard` in its graveyard; the
/// other seats have libraries only.
fn table(seats: usize, equipment: &str, graveyard: &[&str]) -> Scenario {
    let mut mine = vec![
        ready(card(equipment).unwrap()),
        ready(card("Grizzly Bears").unwrap()),
        ready(card("Llanowar Elves").unwrap()),
    ];
    mine.extend(vec![ready(forest()); 9]);
    (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, mine)
            .graveyard(0, graveyard.iter().map(|name| card(name).unwrap())),
        |scenario, seat| scenario.library(seat, vec![mountain(); 8]),
    )
}

fn mine(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            let object = driver.state().object(*id).unwrap();
            object.card().name == name && object.controller().index() == 0
        })
        .unwrap()
}

fn name(driver: &Driver, id: ObjectId) -> String {
    let object = driver.state().object(id).unwrap();
    characteristics(driver.state(), id)
        .unwrap()
        .name_for(&object.card().name)
}

fn has_type(driver: &Driver, id: ObjectId, subtype: &str) -> bool {
    let object = driver.state().object(id).unwrap();
    characteristics(driver.state(), id)
        .unwrap()
        .has_subtype(&object.card().subtypes, &Subtype(subtype.to_owned()))
}

/// Resolves the stack, answering a name choice with `named`, a creature type with `kind` and a card choice with the
/// card named `picked`; how many of the three were asked.
fn resolve(driver: &mut Driver, named: &str, kind: &str, picked: &str) -> usize {
    let mut asked = 0;
    for _ in 0..80 {
        if driver.state().stack().is_empty()
            && matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseName(options) => {
                assert!(options.r121_creature, "only a creature card name is asked");
                asked += 1;
                driver
                    .answer(Answer::ChooseName(NameChoice(named.to_owned())))
                    .unwrap();
            }
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Label(kind.to_owned()))
                    .unwrap();
                asked += 1;
                driver
                    .answer(Answer::ChooseOption(OptionChoice(at)))
                    .unwrap();
            }
            DecisionKind::ChooseCards(options) => {
                let at = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| driver.state().object(*id).unwrap().card().name == picked)
                    .unwrap();
                asked += 1;
                driver
                    .answer(Answer::ChooseCards(CardsChoice(vec![at])))
                    .unwrap();
            }
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_named("Grizzly Bears").unwrap();
                driver.choose_targets(&[target]).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    asked
}

/// Equips `equipment` to the creature `host` names and pays automatically.
fn equip(driver: &mut Driver, equipment: &str, host: ObjectId) {
    driver.begin_activation(equipment).unwrap();
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(host).unwrap().reference());
    driver.choose_targets(&[target]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
}

#[rstest]
fn psychic_paper_names_and_types_the_equipped_creature(#[values(2, 3, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(table(seats, "Psychic Paper", &[])).unwrap();
    let (bears, elves) = (
        mine(&driver, "Grizzly Bears"),
        mine(&driver, "Llanowar Elves"),
    );
    equip(&mut driver, "Psychic Paper", bears);
    assert_eq!(resolve(&mut driver, "Serra Angel", "Wizard", ""), 2);
    assert_eq!(name(&driver, bears), "Serra Angel");
    assert!(has_type(&driver, bears, "Wizard"));
    assert!(
        !has_type(&driver, bears, "Bear"),
        "the chosen type is its only creature type"
    );
    assert!(
        mtg_engine::keywords::keywords_of(driver.state(), bears)
            .iter()
            .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Ward(_))),
        "the equipped creature has ward"
    );
    // Moved to the Elves, it asks again; the Bears are themselves once more.
    equip(&mut driver, "Psychic Paper", elves);
    assert_eq!(resolve(&mut driver, "Grizzly Bears", "Goblin", ""), 2);
    assert_eq!(name(&driver, elves), "Grizzly Bears");
    assert!(has_type(&driver, elves, "Goblin"));
    assert!(!has_type(&driver, elves, "Elf"));
    assert_eq!(name(&driver, bears), "Grizzly Bears");
    assert!(has_type(&driver, bears, "Bear"));
    replays(&driver);
}

#[rstest]
fn psychic_paper_refuses_a_name_that_is_no_creature(#[values(2, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(table(seats, "Psychic Paper", &[])).unwrap();
    let bears = mine(&driver, "Grizzly Bears");
    equip(&mut driver, "Psychic Paper", bears);
    while !matches!(driver.decision().unwrap().kind, DecisionKind::ChooseName(_)) {
        driver.answer_passively().unwrap();
    }
    // A Forest is in the game and is no creature card.
    assert!(
        driver
            .answer(Answer::ChooseName(NameChoice("Forest".to_owned())))
            .is_err()
    );
}

/// Crafts the Pick-Axe from the named creature cards of the graveyard and answers what Dinosaur Headdress asks as it
/// enters and becomes attached to the Bears, choosing `first`; the driver and the ids of the Bears and the Elves.
fn crafted(seats: usize, materials: &[&str], first: &str) -> (Driver, (ObjectId, ObjectId)) {
    let pick = "Paleontologist's Pick-Axe // Dinosaur Headdress";
    let mut driver = Driver::from_scenario(table(seats, pick, materials)).unwrap();
    let front = "Paleontologist's Pick-Axe";
    let (bears, elves) = (
        mine(&driver, "Grizzly Bears"),
        mine(&driver, "Llanowar Elves"),
    );
    // The craft ability comes after Equip among the activated abilities of the Pick-Axe.
    let pick_axe = driver.id_of(front).unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    let craft = *options
        .abilities
        .iter()
        .rfind(|ability| ability.object == pick_axe)
        .unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::Activate(craft)))
        .unwrap();
    for _ in 0..40 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(_) => {
                let x = u32::try_from(materials.len()).unwrap();
                driver
                    .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::ChooseCards(options) => {
                let taken: Vec<ObjectId> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| {
                        let name = driver.state().object(*id).unwrap().card().name.clone();
                        materials.contains(&name.as_str())
                    })
                    .collect();
                driver
                    .answer(Answer::ChooseCards(CardsChoice(taken)))
                    .unwrap();
            }
            // The craft ability is on the stack.
            _ => break,
        }
    }
    resolve(&mut driver, "", "", first);
    (driver, (bears, elves))
}

fn headdress(driver: &Driver) -> ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .unwrap()
                .whole_card()
                .name
                .contains("Pick-Axe")
        })
        .unwrap()
}

#[rstest]
fn dinosaur_headdress_copies_the_last_chosen_crafted_card(#[values(2, 3, 4)] seats: usize) {
    let (mut driver, (bears, elves)) =
        crafted(seats, &["Serra Angel", "Black Knight"], "Serra Angel");
    let equipment = headdress(&driver);
    assert_eq!(
        driver
            .state()
            .object(equipment)
            .unwrap()
            .permanent()
            .unwrap()
            .attached_to()
            .map(|held| held.id),
        Some(bears)
    );
    let chars = characteristics(driver.state(), bears).unwrap();
    assert_eq!(name(&driver, bears), "Serra Angel");
    assert_eq!(chars.power.zip(chars.toughness), Some((4, 4)));
    // Equipped to the Elves, it asks again and they become a copy of the Knight; the Bears are Bears again.
    equip(&mut driver, "Dinosaur Headdress", elves);
    assert_eq!(resolve(&mut driver, "", "", "Black Knight"), 1);
    assert_eq!(name(&driver, elves), "Black Knight");
    let chars = characteristics(driver.state(), elves).unwrap();
    assert_eq!(chars.power.zip(chars.toughness), Some((2, 2)));
    assert_eq!(name(&driver, bears), "Grizzly Bears");
    replays(&driver);
}
