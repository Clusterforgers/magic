//! Goal: a card name chosen as a permanent enters takes only the names its words allow (a creature is no
//! "noncreature, nonland" name, a basic land no "nonbasic land" name), and the name of a card revealed from the opponents' hands is chosen among the nonland names revealed (asked only when
//! there are two or more), at tables of two, three and four seats, with the log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{NameChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, creature, forest, instant};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::pid;

/// A free artifact that chooses a name of this kind as it enters.
fn namer(choice: &str) -> Arc<CardDefinition> {
    let enters = format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{{"Choose":"{choice}"}}}}}}"#
    );
    Arc::new(
        CardDefinition::new("Namer")
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(&enters).unwrap()),
    )
}

/// A land that is not basic.
fn dune() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Dune").with_type(CardType::Land))
}

/// The cards of every seat but the first: an Elf, a Shock, a Forest and a Dune.
fn opponents_hand() -> Vec<Arc<CardDefinition>> {
    vec![
        creature("Elf", 0, &[Color::Green], 1, 1),
        instant("Shock", 0, &[Color::Red]),
        forest(),
        dune(),
    ]
}

fn table(seats: usize, choice: &str) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats).hand(0, [namer(choice)]);
    for seat in 1..seats {
        scenario = scenario.hand(seat, opponents_hand());
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Namer")?;
    Ok(driver)
}

/// Passes priority until a question other than priority is asked.
fn until_asked(driver: &mut Driver) -> Result<DecisionKind, ScriptError> {
    for _ in 0..50 {
        let decision = driver.decision()?.clone();
        if !matches!(decision.kind, DecisionKind::Priority(_)) {
            return Ok(decision.kind);
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("no question".into()))
}

fn chosen(driver: &Driver) -> Option<Chosen> {
    let namer = driver.id_of("Namer").unwrap();
    driver
        .state()
        .object(namer)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned()
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn named(name: &str) -> Answer {
    Answer::ChooseName(NameChoice(name.to_owned()))
}

#[rstest]
#[case::an_instant_is_noncreature_nonland("NoncreatureNonlandCardName", "Shock", true)]
#[case::a_creature_is_not("NoncreatureNonlandCardName", "Elf", false)]
#[case::a_land_is_not("NoncreatureNonlandCardName", "Dune", false)]
#[case::a_name_not_in_the_game("NoncreatureNonlandCardName", "Lightning Bolt", true)]
#[case::a_nonbasic_land("NonbasicLandCardName", "Dune", true)]
#[case::a_basic_land_is_not("NonbasicLandCardName", "Forest", false)]
#[case::an_instant_is_no_land("NonbasicLandCardName", "Shock", false)]
fn a_name_is_taken_only_when_its_kind_allows(
    #[values(2, 3, 4)] seats: usize,
    #[case] choice: &str,
    #[case] name: &str,
    #[case] taken: bool,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, choice)?;
    let DecisionKind::ChooseName(options) = until_asked(&mut driver)? else {
        panic!("a name is asked");
    };
    assert_eq!(options.noncreature, choice == "NoncreatureNonlandCardName");
    assert_eq!(options.nonbasic_land, choice == "NonbasicLandCardName");
    let answered = driver.answer(named(name));
    assert_eq!(answered.is_ok(), taken, "{name} for {choice}");
    if taken {
        until_asked(&mut driver).ok();
        assert_eq!(chosen(&driver), Some(Chosen::Name(name.to_owned())));
        replays(&driver);
    }
    Ok(())
}

#[rstest]
fn the_name_of_a_revealed_nonland_card_is_chosen(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "RevealedNonlandName")?;
    // The nonland names revealed are Elf and Shock, sorted; Shock is the second.
    let DecisionKind::ChooseOption(options) = until_asked(&mut driver)? else {
        panic!("a revealed name is asked");
    };
    assert_eq!(options.options.len(), 2);
    let events = driver.answer(Answer::ChooseOption(OptionChoice(1)))?;
    let revealed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Revealed(shown) => Some(shown.player),
            _ => None,
        })
        .collect();
    assert_eq!(revealed, (1..seats).map(pid).collect::<Vec<_>>());
    assert_eq!(chosen(&driver), Some(Chosen::Name("Shock".to_owned())));
    replays(&driver);
    Ok(())
}

#[rstest]
fn one_revealed_name_is_taken_and_none_names_nothing(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] one: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).hand(0, [namer("RevealedNonlandName")]);
    for seat in 1..seats {
        let mut hand = vec![forest()];
        if one {
            hand.push(creature("Elf", 0, &[Color::Green], 1, 1));
        }
        scenario = scenario.hand(seat, hand);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Namer")?;
    for _ in 0..10 {
        if driver.id_of("Namer").is_ok_and(|namer| {
            driver
                .state()
                .object(namer)
                .is_some_and(|object| object.permanent().is_some())
        }) {
            break;
        }
        assert!(
            matches!(driver.decision()?.kind, DecisionKind::Priority(_)),
            "nothing is asked"
        );
        driver.pass()?;
    }
    let want = one.then(|| Chosen::Name("Elf".to_owned()));
    assert_eq!(chosen(&driver), want);
    replays(&driver);
    Ok(())
}
