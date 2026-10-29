//! Goal: the cards that look at or reveal the top of the library and take cards from among them play whole at tables of two, three and four seats: Genesis Wave puts the permanents of mana value X or less onto the battlefield and the rest into the graveyard, Bounty of Skemfar takes a land tapped and an Elf in two takes, Wakanda Forever! takes one permanent with an indestructible counter and one for the hand, Boromir takes a Human or artifact card when he enters and Loot takes a creature by the lands its controller has; every log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::GameObject;
use mtg_engine::replay;
use mtg_testkit::cards::{forest, land, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;

/// Two untapped Islands.
fn lands_of(count: usize) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land(Color::Blue))).collect()
}

/// The picks of each question from "A,B;C": the cards of each question separated by commas.
fn w20a_picks(text: &str) -> Vec<Vec<&str>> {
    text.split(';')
        .map(|pick| pick.split(',').filter(|name| !name.is_empty()).collect())
        .collect()
}

/// The first object named `name`, wherever it is.
fn w20a_object<'a>(driver: &'a Driver, name: &str) -> &'a GameObject {
    driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .min_by_key(|object| object.id())
        .unwrap()
}

/// Where the first card named `name` is: "battlefield", "hand", "graveyard" or "library".
fn w20a_where(driver: &Driver, name: &str) -> String {
    let zone = format!("{:?}", w20a_object(driver, name).zone());
    zone.split(['(', ' ']).next().unwrap().to_lowercase()
}

fn w20a_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes priority until the stack is empty; each choice of cards is answered with the cards of `picks`, one list for
/// each question in order, that the question offers.
fn w20a_resolve(driver: &mut Driver, picks: &[Vec<&str>]) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..60 {
        let decision = driver.decision()?.kind.clone();
        match decision {
            DecisionKind::ChooseCards(options) => {
                let names = picks.get(asked).cloned().unwrap_or_default();
                asked += 1;
                let chosen = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| {
                        let name = &driver.state().object(*id).unwrap().card().name;
                        names.contains(&name.as_str())
                    })
                    .take(options.max as usize)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A table of `seats` where seat 0 holds `hand`, controls `forests` untapped Forests and `board`, and has `top` over a
/// library of Forests.
fn w20a_table(
    seats: usize,
    hand: &[&str],
    board: Vec<mtg_engine::PresetPermanent>,
    top: &[&str],
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, board);
    for seat in 0..seats {
        let mut names: Vec<&str> = if seat == 0 { top.to_vec() } else { Vec::new() };
        names.extend(std::iter::repeat_n("Forest", 8));
        scenario = scenario.library_named(seat, names);
    }
    scenario
}

#[rstest]
#[case::wave_takes_the_cheap_permanents(
    "Genesis Wave", Some(2), 5,
    &["Grizzly Bears", "Serra Angel", "Lightning Bolt"],
    "Grizzly Bears",
    &[("Grizzly Bears", "battlefield"), ("Serra Angel", "graveyard"), ("Lightning Bolt", "library")]
)]
#[case::wave_with_x_one_finds_nothing_cheap(
    "Genesis Wave", Some(1), 4,
    &["Grizzly Bears", "Serra Angel"],
    "",
    &[("Grizzly Bears", "graveyard"), ("Serra Angel", "library")]
)]
#[case::bounty_takes_a_land_and_an_elf(
    "Bounty of Skemfar", None, 3,
    &["Llanowar Elves", "Lightning Bolt", "Grizzly Bears", "Serra Angel", "Forest", "Forest"],
    "Forest;Llanowar Elves",
    &[("Llanowar Elves", "hand"), ("Lightning Bolt", "library"), ("Grizzly Bears", "library")]
)]
#[case::wakanda_takes_one_for_the_battlefield_and_one_for_the_hand(
    "Wakanda Forever!", None, 6,
    &["Grizzly Bears", "Llanowar Elves", "Lightning Bolt", "Serra Angel"],
    "Grizzly Bears;Llanowar Elves",
    &[("Grizzly Bears", "battlefield"), ("Llanowar Elves", "hand"), ("Lightning Bolt", "graveyard"), ("Serra Angel", "graveyard")]
)]
fn w20a_spells_take_cards_from_among_the_top(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] x: Option<u32>,
    #[case] forests: usize,
    #[case] top: &[&str],
    #[case] picks: &str,
    #[case] expected: &[(&str, &str)],
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(w20a_table(seats, &[spell], lands(forests, forest), top))?;
    if let Some(x) = x {
        driver.begin_cast(spell)?;
        driver.answer(Answer::ChooseNumber(
            mtg_engine::choose::NumberChoice::Number(x),
        ))?;
        driver.pay_tapping(&vec!["Forest"; forests], 0)?;
    } else {
        driver.cast_tapping(spell, &vec!["Forest"; forests])?;
    }
    w20a_resolve(&mut driver, &w20a_picks(picks))?;
    for (name, place) in expected {
        assert_eq!(w20a_where(&driver, name), *place, "{spell}: {name}");
    }
    if spell == "Wakanda Forever!" {
        let bears = w20a_object(&driver, "Grizzly Bears");
        let indestructible = CounterKind::Other("indestructible".into());
        assert_eq!(bears.permanent().unwrap().counters_of(&indestructible), 1);
    }
    w20a_replays(&driver);
    Ok(())
}

#[rstest]
fn w20a_boromir_takes_a_human_or_an_artifact_when_he_enters(
    #[values(2, 3, 4)] seats: usize,
    #[values("Akrasan Squire", "Arcane Signet")] wanted: &str,
) -> Result<(), ScriptError> {
    let top = [
        "Lightning Bolt",
        "Grizzly Bears",
        "Akrasan Squire",
        "Arcane Signet",
        "Serra Angel",
        "Llanowar Elves",
    ];
    let mut board = lands(2, plains);
    board.extend(lands_of(2));
    let scenario = w20a_table(seats, &["Boromir, Gondor's Hope"], board, &top);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping(
        "Boromir, Gondor's Hope",
        &["Plains", "Plains", "Island", "Island"],
    )?;
    let asked = w20a_resolve(&mut driver, &[vec![wanted]])?;
    assert_eq!(asked, 1, "one question for the take");
    assert_eq!(w20a_where(&driver, wanted), "hand");
    let other = if wanted == "Arcane Signet" {
        "Akrasan Squire"
    } else {
        "Arcane Signet"
    };
    assert_eq!(w20a_where(&driver, other), "library");
    assert_eq!(w20a_where(&driver, "Lightning Bolt"), "library");
    w20a_replays(&driver);
    Ok(())
}

#[rstest]
fn w20a_loot_puts_a_creature_of_a_mana_value_up_to_its_lands_onto_the_battlefield(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let top = [
        "Lightning Bolt",
        "Serra Angel",
        "Grizzly Bears",
        "Llanowar Elves",
    ];
    let mut board = lands(6, forest);
    board.push(ready(card("Loot, Exuberant Explorer").unwrap()));
    let scenario = w20a_table(seats, &[], board, &top);
    let mut driver = Driver::from_scenario(scenario)?;
    let ability = *driver
        .offered_abilities("Loot, Exuberant Explorer")?
        .last()
        .unwrap();
    driver.begin_ability("Loot, Exuberant Explorer", ability)?;
    driver.pay_tapping(&["Forest"; 6], 0)?;
    w20a_resolve(&mut driver, &[vec!["Serra Angel"]])?;
    assert_eq!(w20a_where(&driver, "Serra Angel"), "battlefield");
    assert_eq!(w20a_where(&driver, "Grizzly Bears"), "library");
    w20a_replays(&driver);
    Ok(())
}
