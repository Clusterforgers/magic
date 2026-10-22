//! Goal: a creature whose power and toughness are defined by a count (cards in a hand or in graveyards, card types among them, lands, artifacts, the permanents of one colour) is as big as the count at every table size, counts only what its words name (its controller's hand and permanents, the opponents' graveyards, every graveyard), and takes the stat it leaves out from the printed number.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use mtg_engine::characteristics;

/// The power and toughness of the first permanent named `name`.
fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// A table of `seats` where seat 0 controls the creature `name` and every seat has the cards `graveyards` gives it.
fn table(
    seats: usize,
    name: &str,
    graveyards: impl Fn(usize) -> Vec<&'static str>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield_named(0, [name]);
    for seat in 0..seats {
        scenario = scenario.graveyard_named(seat, graveyards(seat));
    }
    Driver::from_scenario(scenario)
}

#[rstest]
fn the_cards_in_your_hand_count_and_the_cards_in_theirs_do_not(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 4)] mine: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield_named(0, ["Psychosis Crawler"]);
    scenario = scenario.hand_named(0, vec!["Grizzly Bears"; mine]);
    for seat in 1..seats {
        scenario = scenario.hand_named(seat, vec!["Grizzly Bears"; 7]);
    }
    let driver = Driver::from_scenario(scenario)?;
    let mine = i32::try_from(mine).unwrap();
    assert_eq!(stats(&driver, "Psychosis Crawler"), (mine, mine));
    Ok(())
}

#[rstest]
fn the_creature_cards_of_every_graveyard_count_for_a_goyf(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat n holds n + 1 Bears and a Bolt, which is no creature card.
    let driver = table(seats, "Lhurgoyf", |seat| {
        let mut cards = vec!["Grizzly Bears"; seat + 1];
        cards.push("Lightning Bolt");
        cards
    })?;
    let bears = i32::try_from(seats * (seats + 1) / 2).unwrap();
    assert_eq!(stats(&driver, "Lhurgoyf"), (bears, bears + 1));
    let driver = table(seats, "Mortivore", |seat| vec!["Grizzly Bears"; seat + 1])?;
    assert_eq!(stats(&driver, "Mortivore"), (bears, bears));
    Ok(())
}

#[rstest]
fn every_card_of_every_graveyard_counts_for_the_lord_of_extinction(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = table(seats, "Lord of Extinction", |seat| {
        let mut cards = vec!["Forest"; seat];
        cards.push("Lightning Bolt");
        cards
    })?;
    // 0 + 1 + .. + (seats - 1) Forests and one Bolt for each seat.
    let cards = i32::try_from(seats * (seats - 1) / 2 + seats).unwrap();
    assert_eq!(stats(&driver, "Lord of Extinction"), (cards, cards));
    Ok(())
}

#[rstest]
fn the_card_types_in_the_graveyards_of_opponents_add_to_the_scavengers_power(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 0 holds a land and a creature that no opponent can see; the opponents hold one more type each, and the
    // second opponent holds a type that is already there.
    let types = ["Grizzly Bears", "Lightning Bolt", "Rampant Growth"];
    let driver = table(seats, "Nighthawk Scavenger", |seat| match seat {
        0 => vec!["Forest", "Giant Growth", "Sol Ring"],
        1 => vec![types[0], types[0]],
        seat => vec![types[(seat - 1) % types.len()]],
    })?;
    // Opponents show a creature, then an instant, then a sorcery: one type for each seat after the first.
    let distinct = i32::try_from(seats - 1).unwrap();
    assert_eq!(stats(&driver, "Nighthawk Scavenger"), (1 + distinct, 3));
    Ok(())
}

#[rstest]
fn an_instant_and_a_sorcery_in_your_graveyard_alone_count_for_the_djinn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = table(seats, "Haughty Djinn", |seat| match seat {
        0 => vec![
            "Lightning Bolt",
            "Rampant Growth",
            "Grizzly Bears",
            "Sol Ring",
        ],
        _ => vec!["Lightning Bolt"; 3],
    })?;
    assert_eq!(stats(&driver, "Haughty Djinn"), (2, 4));
    Ok(())
}

#[rstest]
fn the_nonbasic_lands_of_opponents_are_the_elementals_power(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield_named(
        0,
        ["Wilderness Elemental", "Command Tower", "Command Tower"],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield_named(seat, ["Command Tower", "Forest", "Forest"]);
    }
    let driver = Driver::from_scenario(scenario)?;
    let nonbasic = i32::try_from(seats - 1).unwrap();
    assert_eq!(stats(&driver, "Wilderness Elemental"), (nonbasic, 3));
    Ok(())
}

#[rstest]
#[case::alone(&[], &[], (1, 1))]
#[case::an_artifact_of_mine(&["Sol Ring"], &[], (2, 2))]
#[case::three_artifacts_of_mine(&["Sol Ring", "Sol Ring", "Sol Ring"], &[], (4, 4))]
#[case::an_artifact_of_theirs(&[], &["Sol Ring"], (1, 1))]
#[case::both(&["Sol Ring"], &["Sol Ring", "Sol Ring"], (2, 2))]
#[case::a_creature_of_mine_is_no_artifact(&["Grizzly Bears"], &[], (1, 1))]
fn the_artifacts_you_control_count_for_the_master(
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] expected: (i32, i32),
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut names = vec!["Master of Etherium"];
    names.extend(mine);
    let mut scenario = Scenario::new(seats).battlefield_named(0, names);
    for seat in 1..seats {
        scenario = scenario.battlefield_named(seat, theirs.iter().copied());
    }
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(stats(&driver, "Master of Etherium"), expected);
    Ok(())
}

#[test]
fn the_green_permanents_you_control_count_for_the_drove() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [
                    ready(card("Drove of Elves").unwrap()),
                    ready(card("Grizzly Bears").unwrap()),
                    ready(card("Sol Ring").unwrap()),
                ],
            )
            .battlefield_named(1, ["Grizzly Bears", "Llanowar Elves"]),
    )?;
    // The Drove is green itself, so it counts with the Bears and not with the Sol Ring or the other seat's creatures.
    assert_eq!(stats(&driver, "Drove of Elves"), (2, 2));
    Ok(())
}
