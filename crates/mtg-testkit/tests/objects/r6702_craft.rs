//! Goal: the exiled cards used to craft a permanent are linked to it (CR 702.167c) at two to four seats: Wretched
//! Bonemass, crafted from Altar of the Wretched, has power and toughness equal to their total power and each listed
//! keyword one of them has (protection with its quality) and none that they lack.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 crafts Altar of the Wretched with the named cards of its graveyard: the Bonemass, once the stack is empty.
fn r6702_crafted(seats: usize, materials: &[&str]) -> Result<(Driver, ObjectId), ScriptError> {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(
                0,
                std::iter::once(ready(
                    card("Altar of the Wretched // Wretched Bonemass").unwrap(),
                ))
                .chain((0..4).map(|_| ready(swamp())))
                .collect::<Vec<_>>(),
            )
            .graveyard(
                0,
                [
                    card("Serra Angel").unwrap(),
                    card("Black Knight").unwrap(),
                    card("Grizzly Bears").unwrap(),
                ],
            )
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![mountain(); 8]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let altar = "Altar of the Wretched";
    let abilities = driver.offered_abilities(altar)?;
    assert!(!abilities.is_empty(), "the craft ability is offered");
    driver.begin_activation(altar)?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            // Craft with one or more creatures: X is the number of materials.
            DecisionKind::ChooseNumber(_) => {
                let x = u32::try_from(materials.len()).unwrap();
                driver.answer(Answer::ChooseNumber(
                    mtg_engine::choose::NumberChoice::Number(x),
                ))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
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
                driver.answer(Answer::ChooseCards(CardsChoice(taken)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    let bonemass = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            let object = driver.state().object(*id).unwrap();
            object.card().name.contains("Bonemass")
                || object.whole_card().name.contains("Altar of the Wretched")
        })
        .expect("the Bonemass is on the battlefield");
    Ok((driver, bonemass))
}

#[rstest]
#[case::angel_and_knight_two_seats(2, &["Serra Angel", "Black Knight"], (6, 6))]
#[case::the_angel_alone_three_seats(3, &["Serra Angel"], (4, 4))]
#[case::the_bears_four_seats(4, &["Grizzly Bears"], (2, 2))]
fn r6702_bonemass_is_the_total_of_its_materials(
    #[case] seats: usize,
    #[case] materials: &[&str],
    #[case] size: (i32, i32),
) -> Result<(), ScriptError> {
    let (driver, bonemass) = r6702_crafted(seats, materials)?;
    let chars = mtg_engine::characteristics(driver.state(), bonemass).unwrap();
    assert_eq!(chars.power.zip(chars.toughness), Some(size));
    let held = mtg_engine::keywords::keywords_of(driver.state(), bonemass);
    let has = |wanted: &Keyword| held.contains(&wanted);
    let angel = materials.contains(&"Serra Angel");
    let knight = materials.contains(&"Black Knight");
    assert_eq!(has(&Keyword::Flying), angel);
    assert_eq!(has(&Keyword::Vigilance), angel);
    assert_eq!(has(&Keyword::FirstStrike), knight);
    assert_eq!(
        held.iter().any(|k| matches!(k, Keyword::Protection(_))),
        knight
    );
    assert!(!has(&Keyword::Trample));
    Ok(())
}
