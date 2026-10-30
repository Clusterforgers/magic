//! Goal: Pippin, Guard of the Citadel taps to give another creature its controller has protection from the card type chosen as the ability resolves, for every card type a player may name, at tables of two and four seats; the protection ends with the turn, Pippin cannot aim it at himself and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::CardType;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, ScriptError};
use rstest::rstest;

use crate::dk1f_cards::{assert_replays, first_main, settle, with_lands};

const PIPPIN: &str = "Pippin, Guard of the Citadel";

/// Answers the option with this label when the table asks for one.
fn choose_label(driver: &mut Driver, label: &str) -> Result<(), ScriptError> {
    for _ in 0..50 {
        if let DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
            let index = options
                .options
                .iter()
                .position(|option| matches!(option, ChoiceOption::Label(text) if text == label))
                .unwrap();
            driver.answer(Answer::ChooseOption(OptionChoice(index)))?;
            return Ok(());
        }
        driver.answer_passively()?;
    }
    panic!("no choice was asked for");
}

/// The card types protection can be chosen from.
const CARD_TYPES: [(&str, CardType); 9] = [
    ("Artifact", CardType::Artifact),
    ("Battle", CardType::Battle),
    ("Creature", CardType::Creature),
    ("Enchantment", CardType::Enchantment),
    ("Instant", CardType::Instant),
    ("Kindred", CardType::Kindred),
    ("Land", CardType::Land),
    ("Planeswalker", CardType::Planeswalker),
    ("Sorcery", CardType::Sorcery),
];

fn protection_from(driver: &Driver, name: &str) -> Vec<CardType> {
    let id = driver.id_of(name).unwrap();
    mtg_engine::keywords::keywords_of(driver.state(), id)
        .into_iter()
        .filter_map(|keyword| match keyword {
            Keyword::Protection(Selector(preds)) => match preds.as_slice() {
                [Pred::Type(card_type)] => Some(*card_type),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[rstest]
fn pippin_gives_another_creature_protection_from_the_card_type_chosen_as_it_resolves(
    #[values(2, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4, 5, 6, 7, 8)] picked: usize,
) -> Result<(), ScriptError> {
    let (label, card_type) = CARD_TYPES[picked];
    let build = || first_main(seats).battlefield(0, with_lands(0, [PIPPIN, "Grizzly Bears"]));
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.begin_activation(PIPPIN)?;
    let itself = driver.target_named(PIPPIN)?;
    assert!(
        driver.choose_targets(&[itself]).is_err(),
        "another creature is the target, not Pippin"
    );
    let bears = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bears])?;
    assert!(
        !matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)),
        "the type is chosen as the ability resolves"
    );
    choose_label(&mut driver, label)?;
    settle(&mut driver)?;
    assert_eq!(protection_from(&driver, "Grizzly Bears"), [card_type]);
    assert_eq!(protection_from(&driver, PIPPIN), []);
    assert_replays(&driver, &setup)
}

#[test]
fn the_protection_of_pippin_ends_with_the_turn() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        first_main(2).battlefield(0, with_lands(0, [PIPPIN, "Grizzly Bears"])),
    )?;
    driver.begin_activation(PIPPIN)?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bears])?;
    choose_label(&mut driver, "Instant")?;
    settle(&mut driver)?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(protection_from(&driver, "Grizzly Bears"), []);
    Ok(())
}
