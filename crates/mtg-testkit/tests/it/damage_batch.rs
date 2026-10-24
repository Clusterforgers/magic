//! Goal: a trigger for "one or more Dragons you control deal combat damage to a player" (Broodcaller Scourge) triggers once for each player hurt however many Dragons dealt the damage, counts only the damage of Dragons, names the damage dealt to that player in all ("that damage"), and so offers from the hand exactly the permanent cards whose mana value is at most that damage (the boundary included); a Dragon that is blocked or never attacks adds nothing, Dragons that hurt different players trigger once each with the damage of their own, at tables of two, three and four seats, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};

use mtg_core::ids::ObjectId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 has Broodcaller Scourge (5 power) and, as asked, Predator Dragon (4 power) and Grizzly Bears (2 power) ready
/// to attack, and a hand of permanent cards with mana values 2, 5 and 6.
fn table(seats: usize, others: &[&str]) -> Scenario {
    let mut creatures = vec![ready(card("Broodcaller Scourge").unwrap())];
    creatures.extend(others.iter().map(|name| ready(card(name).unwrap())));
    Scenario::new(seats)
        .seed(3)
        .at(1, 0, Step::Main1)
        .battlefield(0, creatures)
        .hand_named(0, ["Grizzly Bears", "Serra Angel", "Serra Avatar"])
        .library_named(0, ["Forest"; 6])
}

/// Attacks with `attackers` (each against a seat), then plays on: every "may" is answered yes and the first card
/// offered of each choice from the hand is taken. Returns the cards offered at each choice, by name.
fn play(driver: &mut Driver, attackers: &[(&str, usize)]) -> Result<Vec<Vec<String>>, ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(attackers)?;
    let mut offers: Vec<Vec<String>> = Vec::new();
    for _ in 0..80 {
        if driver.state().turn().step() == Step::End {
            break;
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let mut offered: Vec<String> = options
                    .from
                    .iter()
                    .filter_map(|id| driver.state().object(*id))
                    .map(|object| object.card().name.clone())
                    .collect();
                offered.sort();
                offers.push(offered);
                let first: Vec<ObjectId> = options.from.iter().take(1).copied().collect();
                driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(first)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(offers)
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn triggers(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::AbilityPutOnStack(_)))
        .count()
}

#[rstest]
#[case::one_dragon(&[], &["Grizzly Bears", "Serra Angel"])]
#[case::two_dragons_add_their_damage(&["Predator Dragon"], &["Grizzly Bears", "Serra Angel", "Serra Avatar"])]
fn the_cards_offered_are_those_with_a_mana_value_at_most_the_damage_of_the_dragons(
    #[values(2, 3, 4)] seats: usize,
    #[case] dragons: &[&str],
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, dragons))?;
    let mut attackers = vec![("Broodcaller Scourge", 1)];
    attackers.extend(dragons.iter().map(|name| (*name, 1)));
    let offers = play(&mut driver, &attackers)?;
    assert_eq!(triggers(&driver), 1, "one trigger for all the Dragons");
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0], expected);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 2);
    replays(&driver);
    Ok(())
}

#[rstest]
fn a_creature_that_is_not_a_dragon_adds_no_damage(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Grizzly Bears"]))?;
    let offers = play(
        &mut driver,
        &[("Broodcaller Scourge", 1), ("Grizzly Bears", 1)],
    )?;
    // 5 damage by the Dragon and 2 by the Bears: only the Dragon's counts, so the card of mana value 6 stays out.
    assert_eq!(triggers(&driver), 1);
    assert_eq!(offers, vec![vec!["Grizzly Bears", "Serra Angel"]]);
    Ok(())
}

#[rstest]
fn the_trigger_waits_for_a_dragon_to_hurt_a_player(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Grizzly Bears"]))?;
    let offers = play(&mut driver, &[("Grizzly Bears", 1)])?;
    assert_eq!(triggers(&driver), 0, "the Bears are not a Dragon");
    assert_eq!(offers, Vec::<Vec<String>>::new());
    Ok(())
}

#[rstest]
fn dragons_that_hurt_different_players_trigger_once_each(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Predator Dragon"]).hand_named(
        0,
        [
            "Grizzly Bears",
            "Grizzly Bears",
            "Serra Angel",
            "Serra Avatar",
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let offers = play(
        &mut driver,
        &[("Broodcaller Scourge", 1), ("Predator Dragon", 2)],
    )?;
    assert_eq!(triggers(&driver), 2, "one trigger for each player hurt");
    // 5 damage to one player and 4 to the other: the damage is not added together across players.
    assert_ne!(offers, Vec::<Vec<String>>::new());
    assert!(
        offers
            .iter()
            .any(|offer| offer.contains(&"Serra Angel".to_owned())),
        "the 5 damage offers the card of mana value 5: {offers:?}"
    );
    assert!(
        offers
            .iter()
            .all(|offer| !offer.contains(&"Serra Avatar".to_owned())),
        "no player was hurt for 6: {offers:?}"
    );
    replays(&driver);
    Ok(())
}
