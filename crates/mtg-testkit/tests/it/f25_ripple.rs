//! Goal: ripple reads the name of its spell from last-known information, so a ripple whose spell was countered before the trigger resolves still casts the revealed cards of that name, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature, gain_life, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::removal::corpus;
use crate::common::{name_of, names, pid};

/// An instant for `{R}` that gains 2 life, with ripple 4.
fn surge() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Surge")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Keyword(Keyword::Ripple(4)))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(2)],
            }),
    )
}

/// Seat 0 holds a Surge and a Counterspell over a Mountain and two Islands; its library, top first: Surge, Forest, Surge,
/// Bear.
fn table(seats: usize) -> Scenario {
    let mut table = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 1..seats {
        table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    table
        .hand(0, [surge(), corpus("Counterspell")])
        .battlefield(0, [ready(mountain()), ready(island()), ready(island())])
        .library(
            0,
            vec![surge(), forest(), surge(), free_creature("Bear", 2, 2)],
        )
}

/// Answers the first yes-or-no questions by `answers` and the later ones no, everything else passively, until the stack
/// is empty.
fn settle(driver: &mut Driver, answers: &[bool]) -> Result<(), ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::YesNo(question) => {
                let yes = answers.get(asked).copied().unwrap_or(false) && !question.only_no;
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("an empty stack".into()))
}

#[rstest]
#[case::both_cast(&[true, true, true], 24)]
#[case::one_cast(&[true, true, false], 22)]
#[case::not_revealed(&[false], 20)]
fn ripple_of_a_countered_spell_casts_cards_of_its_name(
    #[values(2, 3, 4)] seats: usize,
    #[case] answers: &[bool],
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.cast("Surge")?;
    let spell = (driver.state().stack().iter().copied())
        .find(|&id| name_of(driver.state(), id) == "Surge")
        .expect("Surge is on the stack");
    assert_eq!(
        driver.state().stack().len(),
        2,
        "the ripple trigger waits above Surge"
    );
    let target = Target::Object(driver.state().object(spell).unwrap().reference());
    driver.cast_at("Counterspell", &[target])?;
    assert_eq!(
        name_of(driver.state(), *driver.state().stack().last().unwrap()),
        "Counterspell"
    );
    settle(&mut driver, answers)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life);
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    let surges = graveyard.iter().filter(|name| *name == "Surge").count();
    assert_eq!(
        surges,
        1 + usize::try_from((life - 20) / 2).unwrap(),
        "{graveyard:?}"
    );
    Ok(())
}
