//! Goal: a required additional cost whose X the text counts from the spell's targets (Urgent Necropsy: "collect evidence X, where X is the total mana value of the permanents this spell targets", CR 601.2b, 601.2c, 601.2h) plays at tables of two to four seats: no X is asked, the evidence is picked after the targets, for exactly their total mana value, no target asks for no evidence, a graveyard short of the total takes the cast back, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds Urgent Necropsy with two Swamps and two Forests and graveyard creature cards of mana values `dead`;
/// the last seat controls a 2-mana and a 3-mana creature.
fn table(seats: usize, dead: &[u32]) -> Scenario {
    let graveyard: Vec<_> = dead
        .iter()
        .enumerate()
        .map(|(at, value)| creature(&format!("Dead {at}"), *value, &[], 1, 1))
        .collect();
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(swamp()),
                ready(swamp()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .battlefield(
            seats - 1,
            [
                ready(creature("Two", 1, &[Color::Red], 2, 2)),
                ready(creature("Three", 2, &[Color::Red], 3, 3)),
            ],
        )
        .hand(0, [card("Urgent Necropsy").unwrap()])
        .graveyard(0, graveyard)
}

/// Casts the spell aiming at the creature named `aim` (none for `None`), picks the graveyard cards named `evidence`
/// when asked, and lets the spell resolve; the names of the decisions asked come back.
fn cast(
    driver: &mut Driver,
    aim: Option<&str>,
    evidence: &[&str],
) -> Result<Vec<&'static str>, ScriptError> {
    driver.begin_cast("Urgent Necropsy")?;
    let mut asked = Vec::new();
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::ChooseTargets(options) => {
                asked.push("targets");
                let chosen: Vec<_> = aim
                    .map(|name| driver.target_named(name))
                    .transpose()?
                    .filter(|target| options.legal.contains(target))
                    .into_iter()
                    .collect();
                driver.choose_targets(&chosen)?;
            }
            DecisionKind::ChooseCards(_) => {
                asked.push("evidence");
                let ids: Vec<ObjectId> = evidence
                    .iter()
                    .map(|name| {
                        driver
                            .state()
                            .objects()
                            .find(|found| found.card().name == *name)
                            .map(mtg_engine::object::GameObject::id)
                            .unwrap()
                    })
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(ids)))?;
            }
            DecisionKind::ChooseNumber(_) => panic!("no X is asked"),
            DecisionKind::PayMana(_) => {
                asked.push("pay");
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the spell did not resolve: {asked:?}");
}

fn graveyard(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Graveyard(PlayerId::from_index(0)))
        .len()
}

#[rstest]
#[case::two_from_one_card(Some("Two"), &[2, 1], &["Dead 0"], 1)]
#[case::three_from_two_cards(Some("Three"), &[2, 1, 4], &["Dead 0", "Dead 1"], 1)]
fn the_evidence_is_picked_after_the_targets(
    #[values(2, 3, 4)] seats: usize,
    #[case] aim: Option<&str>,
    #[case] dead: &[u32],
    #[case] evidence: &[&str],
    #[case] left: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, dead);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let asked = cast(&mut driver, aim, evidence)?;
    let first_evidence = asked.iter().position(|name| *name == "evidence").unwrap();
    let last_target = asked.iter().rposition(|name| *name == "targets").unwrap();
    assert!(last_target < first_evidence, "{asked:?}");
    assert!(
        driver.id_of(aim.unwrap()).is_err(),
        "the target was destroyed"
    );
    // The graveyard keeps the cards not picked, plus the spell and the destroyed creature's card goes to its owner.
    assert_eq!(graveyard(&driver), left + 1);
    assert_eq!(driver.state().zone(Zone::Exile).len(), evidence.len());
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn no_target_asks_for_no_evidence(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[2, 3]))?;
    let asked = cast(&mut driver, None, &[])?;
    assert!(!asked.contains(&"evidence"), "{asked:?}");
    assert_eq!(
        graveyard(&driver),
        3,
        "both cards stay and the spell joins them"
    );
    Ok(())
}

#[rstest]
fn a_graveyard_short_of_the_total_takes_the_cast_back(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[1, 1]))?;
    driver.begin_cast("Urgent Necropsy")?;
    let three = driver.target_named("Three")?;
    for _ in 0..8 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(options) => {
                let chosen: Vec<_> = options
                    .legal
                    .iter()
                    .copied()
                    .filter(|t| *t == three)
                    .collect();
                driver.choose_targets(&chosen)?;
            }
            _ => break,
        }
    }
    assert!(
        !matches!(
            driver.decision()?.kind,
            DecisionKind::PayMana(_) | DecisionKind::ChooseCards(_)
        ),
        "the cast is taken back"
    );
    assert_eq!(
        driver
            .state()
            .zone(Zone::Hand(PlayerId::from_index(0)))
            .len(),
        1
    );
    assert!(driver.id_of("Three").is_ok());
    Ok(())
}
