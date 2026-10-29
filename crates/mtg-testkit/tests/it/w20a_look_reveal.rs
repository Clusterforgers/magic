//! Goal: the cards that look at or reveal the top of the library and take cards from among them play whole at tables of two, three and four seats: Genesis Wave puts the permanents of mana value X or less onto the battlefield and the rest into the graveyard (X of 0 reveals nothing, an X past the library reveals all of it), Bounty of Skemfar takes a land tapped and an Elf in two takes and puts the rest on the bottom in a random order without asking, Wakanda Forever! takes one permanent with an indestructible counter and one for the hand, Boromir takes a Human or artifact card when he enters and Loot takes a creature by the lands its controller has; a reveal shows the cards to every other seat, a look to none of them, and the card taken after a look is revealed; every log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::GameObject;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, land, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

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

/// What the choices of a resolution showed: how many were asked, and for each that offered cards whether they were
/// shown to every other seat (a reveal) or to none of them (a look).
struct W20aAsked {
    asked: usize,
    public: Vec<bool>,
}

/// Whether the offered cards are shown to every seat but the chooser's, or to none; a mix fails the test.
fn w20a_public(driver: &Driver, offered: &[ObjectId]) -> Option<bool> {
    let seats = driver.state().players().len();
    let seen: Vec<bool> = (1..seats)
        .flat_map(|seat| offered.iter().map(move |id| (seat, *id)))
        .map(|(seat, id)| driver.state().is_shown_to(pid(seat), id))
        .collect();
    let all = seen.iter().all(|shown| *shown);
    assert!(
        all || !seen.iter().any(|shown| *shown),
        "shown to some seats only"
    );
    (!seen.is_empty()).then_some(all)
}

/// Passes priority until the stack is empty; each choice of cards is answered with the cards of `picks`, one list for
/// each question in order, that the question offers. Any other question (an order for the rest) fails the test.
fn w20a_resolve(driver: &mut Driver, picks: &[Vec<&str>]) -> Result<W20aAsked, ScriptError> {
    let mut asked = 0;
    let mut public = Vec::new();
    for _ in 0..60 {
        let decision = driver.decision()?.kind.clone();
        match decision {
            DecisionKind::ChooseCards(options) => {
                public.extend(w20a_public(driver, &options.from));
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
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => {
                return Ok(W20aAsked { asked, public });
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The cards of seat 0's library, bottom first.
fn w20a_library(driver: &Driver) -> Vec<String> {
    let state = driver.state();
    state
        .zone(Zone::Library(pid(0)))
        .iter()
        .map(|id| state.object(*id).unwrap().card().name.clone())
        .collect()
}

/// How many `Revealed` events the log holds.
fn w20a_reveals(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::Revealed(_)))
        .count()
}

/// A table of `seats` where seat 0 holds `hand`, controls `forests` untapped Forests and `board`, and has `top` over a
/// library of `below` Forests.
fn w20a_table(
    seats: usize,
    hand: &[&str],
    board: Vec<mtg_engine::PresetPermanent>,
    top: &[&str],
    below: usize,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, board);
    for seat in 0..seats {
        let mut names: Vec<&str> = if seat == 0 { top.to_vec() } else { Vec::new() };
        names.extend(std::iter::repeat_n("Forest", below));
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
#[case::wave_with_x_zero_reveals_nothing(
    "Genesis Wave", Some(0), 5,
    &["Grizzly Bears", "Serra Angel"],
    "",
    &[("Grizzly Bears", "library"), ("Serra Angel", "library")]
)]
#[case::bounty_takes_a_land_and_an_elf(
    "Bounty of Skemfar", None, 3,
    &["Llanowar Elves", "Lightning Bolt", "Grizzly Bears", "Serra Angel", "Animal Sanctuary", "Forest"],
    "Animal Sanctuary;Llanowar Elves",
    &[("Llanowar Elves", "hand"), ("Animal Sanctuary", "battlefield"), ("Lightning Bolt", "library"), ("Grizzly Bears", "library")]
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
        Driver::from_scenario(w20a_table(seats, &[spell], lands(forests, forest), top, 8))?;
    if let Some(x) = x {
        driver.begin_cast(spell)?;
        driver.answer(Answer::ChooseNumber(
            mtg_engine::choose::NumberChoice::Number(x),
        ))?;
        driver.pay_tapping(&vec!["Forest"; 3 + x as usize], 0)?;
    } else {
        driver.cast_tapping(spell, &vec!["Forest"; forests])?;
    }
    let asked = w20a_resolve(&mut driver, &w20a_picks(picks))?;
    assert!(
        asked.public.iter().all(|public| *public),
        "{spell}: a reveal is public"
    );
    for (name, place) in expected {
        assert_eq!(w20a_where(&driver, name), *place, "{spell}: {name}");
    }
    if x == Some(0) {
        assert_eq!(asked.asked, 0, "nothing revealed, nothing to take");
        assert_eq!(w20a_library(&driver).len(), top.len() + 8);
    }
    match spell {
        "Wakanda Forever!" => {
            let bears = w20a_object(&driver, "Grizzly Bears");
            let indestructible = CounterKind::Other("indestructible".into());
            assert_eq!(bears.permanent().unwrap().counters_of(&indestructible), 1);
        }
        "Bounty of Skemfar" => {
            let land = w20a_object(&driver, "Animal Sanctuary")
                .permanent()
                .unwrap();
            assert!(land.tapped(), "the land enters tapped");
            // The four cards left over go under the eight Forests that were never revealed.
            let library = w20a_library(&driver);
            assert_eq!(library.len(), 12);
            let mut bottom = library[..4].to_vec();
            bottom.sort();
            assert_eq!(
                bottom,
                ["Forest", "Grizzly Bears", "Lightning Bolt", "Serra Angel"]
            );
        }
        _ => {}
    }
    w20a_replays(&driver);
    Ok(())
}

#[rstest]
fn w20a_wave_with_x_past_the_library_reveals_all_of_it_and_no_one_loses(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let top = ["Grizzly Bears", "Serra Angel", "Lightning Bolt"];
    let mut driver = Driver::from_scenario(w20a_table(
        seats,
        &["Genesis Wave"],
        lands(13, forest),
        &top,
        2,
    ))?;
    driver.begin_cast("Genesis Wave")?;
    driver.answer(Answer::ChooseNumber(
        mtg_engine::choose::NumberChoice::Number(10),
    ))?;
    driver.pay_tapping(&["Forest"; 13], 0)?;
    w20a_resolve(&mut driver, &[vec!["Grizzly Bears", "Serra Angel"]])?;
    assert!(w20a_library(&driver).is_empty(), "all five cards revealed");
    assert_eq!(w20a_where(&driver, "Grizzly Bears"), "battlefield");
    assert_eq!(w20a_where(&driver, "Serra Angel"), "battlefield");
    assert_eq!(w20a_where(&driver, "Lightning Bolt"), "graveyard");
    assert!(driver.state().is_alive(pid(0)));
    w20a_replays(&driver);
    Ok(())
}

#[rstest]
fn w20a_boromir_takes_a_human_or_an_artifact_when_he_enters(
    #[values(2, 3, 4)] seats: usize,
    #[values("Akrasan Squire", "Arcane Signet", "")] wanted: &str,
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
    let scenario = w20a_table(seats, &["Boromir, Gondor's Hope"], board, &top, 8);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping(
        "Boromir, Gondor's Hope",
        &["Plains", "Plains", "Island", "Island"],
    )?;
    let reveals = w20a_reveals(&driver);
    let asked = w20a_resolve(&mut driver, &[vec![wanted]])?;
    assert_eq!(asked.asked, 1, "one question for the take");
    assert_eq!(
        asked.public,
        [false],
        "a look shows the other seats nothing"
    );
    let revealed = usize::from(!wanted.is_empty());
    assert_eq!(
        w20a_reveals(&driver),
        reveals + revealed,
        "the taken card is revealed"
    );
    for name in ["Akrasan Squire", "Arcane Signet"] {
        let place = if name == wanted { "hand" } else { "library" };
        assert_eq!(w20a_where(&driver, name), place, "{name}");
    }
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
    let scenario = w20a_table(seats, &[], board, &top, 8);
    let mut driver = Driver::from_scenario(scenario)?;
    let ability = *driver
        .offered_abilities("Loot, Exuberant Explorer")?
        .last()
        .unwrap();
    driver.begin_ability("Loot, Exuberant Explorer", ability)?;
    driver.pay_tapping(&["Forest"; 6], 0)?;
    let asked = w20a_resolve(&mut driver, &[vec!["Serra Angel"]])?;
    assert_eq!(
        asked.public,
        [false],
        "a look shows the other seats nothing"
    );
    assert_eq!(w20a_where(&driver, "Serra Angel"), "battlefield");
    assert_eq!(w20a_where(&driver, "Grizzly Bears"), "library");
    w20a_replays(&driver);
    Ok(())
}
