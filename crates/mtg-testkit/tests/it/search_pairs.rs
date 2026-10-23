//! Goal: Myriad Landscape, Krosan Verge and Path to Exile play whole, read from their Oracle wordings, at tables of two, three and four seats: a search for two basic lands that share a land type asks the first land of any basic type and the second only among the lands of that type (never the first), a search for a Forest card and a Plains card asks for each in turn among the cards of its type, both lands enter tapped under the searcher's control and the library is shuffled once, finding no first land ends the search without a second question, the controller of an exiled creature alone is asked whether to search and then which land, the land enters tapped under their control, and no other library is touched.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Each seat's library from the top down.
const LIBRARY: [&str; 7] = [
    "Swamp", "Forest", "Island", "Forest", "Plains", "Forest", "Plains",
];

fn land(name: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    match name {
        "Forest" => forest(),
        "Plains" => plains(),
        "Island" => island(),
        _ => swamp(),
    }
}

/// Seat 0 controls the land with the search and two Forests to pay for it; every seat has the library.
fn table(seats: usize, source: &str) -> Scenario {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(card(source).unwrap()),
            ready(forest()),
            ready(forest()),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, LIBRARY.iter().map(|name| land(name)));
    }
    scenario
}

/// Activates the land, pays with the other lands and passes until a question about cards is asked.
fn activate(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    pass_until_asked(driver)
}

fn pass_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(
            driver.decision()?.kind,
            DecisionKind::ChooseCards(_) | DecisionKind::YesNo(_)
        ) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The cards offered by the pending question, as names, and who is asked.
fn offered(driver: &Driver) -> (usize, Vec<ObjectId>, Vec<String>) {
    let decision = driver.decision().unwrap();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("a choice of cards is asked, not {}", decision.kind.name());
    };
    let names = options
        .from
        .iter()
        .map(|id| common::name_of(driver.state(), *id))
        .collect();
    (decision.player.index(), options.from.clone(), names)
}

fn pick(driver: &mut Driver, id: ObjectId) -> Result<(), ScriptError> {
    driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
    pass_until_asked(driver)
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn owned(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

fn battlefield(driver: &Driver, seat: usize) -> Vec<String> {
    sorted(
        names(driver.state(), Zone::Battlefield)
            .into_iter()
            .zip(driver.state().zone(Zone::Battlefield))
            .filter(|(_, id)| driver.state().object(**id).unwrap().controller() == pid(seat))
            .map(|(name, _)| name)
            .collect(),
    )
}

fn tapped(driver: &Driver, id: ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn shuffles(driver: &Driver) -> Vec<usize> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::LibraryShuffled { player, .. } => Some(player.index()),
            _ => None,
        })
        .collect()
}

fn library(driver: &Driver, seat: usize) -> Vec<String> {
    sorted(names(driver.state(), Zone::Library(pid(seat))))
}

#[rstest]
fn two_basic_lands_that_share_a_type_are_asked_one_after_the_other(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] which: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Myriad Landscape"))?;
    activate(&mut driver, "Myriad Landscape")?;
    let (seat, first, offered_first) = offered(&driver);
    assert_eq!(seat, 0);
    assert_eq!(sorted(offered_first), sorted(owned(&LIBRARY)));
    // The first land: a Forest or a Plains, by position among the cards offered.
    let kinds = ["Forest", "Plains"];
    let kind = kinds[which];
    let chosen = *first
        .iter()
        .find(|id| common::name_of(driver.state(), **id) == kind)
        .unwrap();
    pick(&mut driver, chosen)?;
    let (seat, second, offered_second) = offered(&driver);
    assert_eq!(seat, 0);
    let like: usize = LIBRARY.iter().filter(|name| **name == kind).count() - 1;
    assert_eq!(
        offered_second,
        vec![kind.to_owned(); like],
        "only lands of the type of the first, and not the first itself"
    );
    assert!(!second.contains(&chosen));
    pick(&mut driver, second[0])?;
    assert!(tapped(&driver, chosen) && tapped(&driver, second[0]));
    assert_eq!(
        [chosen, second[0]].map(|id| driver.state().object(id).unwrap().zone()),
        [Zone::Battlefield; 2]
    );
    assert_eq!(
        shuffles(&driver),
        [0],
        "the searcher's library is shuffled once"
    );
    assert_eq!(library(&driver, 0).len(), LIBRARY.len() - 2);
    for seat in 1..seats {
        assert_eq!(
            library(&driver, seat),
            sorted(owned(&LIBRARY)),
            "seat {seat} is untouched"
        );
    }
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Myriad Landscape"],
        "the land was sacrificed"
    );
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn a_land_type_with_one_card_left_gives_only_that_land(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Myriad Landscape"))?;
    activate(&mut driver, "Myriad Landscape")?;
    let (_, first, _) = offered(&driver);
    let swamp = *first
        .iter()
        .find(|id| common::name_of(driver.state(), **id) == "Swamp")
        .unwrap();
    pick(&mut driver, swamp)?;
    assert!(
        !matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)),
        "no other Swamp, so no second question"
    );
    assert!(tapped(&driver, swamp));
    assert_eq!(shuffles(&driver), [0]);
    assert_eq!(library(&driver, 0).len(), LIBRARY.len() - 1);
    Ok(())
}

#[rstest]
fn finding_no_first_land_ends_the_search_without_a_second_question(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Myriad Landscape"))?;
    activate(&mut driver, "Myriad Landscape")?;
    driver.answer(Answer::ChooseCards(CardsChoice(Vec::new())))?;
    pass_until_asked(&mut driver)?;
    assert!(
        !matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)),
        "no second question"
    );
    assert_eq!(shuffles(&driver), [0]);
    assert_eq!(library(&driver, 0), sorted(owned(&LIBRARY)));
    Ok(())
}

#[rstest]
fn a_forest_and_a_plains_are_asked_each_among_the_cards_of_its_type(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Krosan Verge"))?;
    activate(&mut driver, "Krosan Verge")?;
    let (seat, first, names_first) = offered(&driver);
    assert_eq!(seat, 0);
    assert_eq!(names_first, vec!["Forest".to_owned(); 3]);
    pick(&mut driver, first[0])?;
    let (_, second, names_second) = offered(&driver);
    assert_eq!(names_second, vec!["Plains".to_owned(); 2]);
    pick(&mut driver, second[1])?;
    assert!(tapped(&driver, first[0]) && tapped(&driver, second[1]));
    assert_eq!(shuffles(&driver), [0]);
    let lands = battlefield(&driver, 0);
    assert_eq!(lands.iter().filter(|name| *name == "Plains").count(), 1);
    assert_eq!(lands.iter().filter(|name| *name == "Forest").count(), 3);
    assert_eq!(library(&driver, 0).len(), LIBRARY.len() - 2);
    Ok(())
}

/// Seat 0 casts Path to Exile on the Ogre of seat 1; every seat has the library.
fn path_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Path to Exile").unwrap()])
        .battlefield(0, [ready(plains())])
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, LIBRARY.iter().map(|name| land(name)));
    }
    scenario
}

#[rstest]
fn the_controller_of_the_exiled_creature_may_search_and_the_land_enters_tapped_under_their_control(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(path_table(seats))?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Path to Exile", &[ogre])?;
    pass_until_asked(&mut driver)?;
    for _ in 0..4 {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            break;
        }
        driver.pass()?;
    }
    let decision = driver.decision()?;
    assert!(matches!(decision.kind, DecisionKind::YesNo(_)));
    assert_eq!(decision.player, pid(1), "the creature's controller decides");
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    pass_until_asked(&mut driver)?;
    let (seat, from, offered_names) = offered(&driver);
    assert_eq!(seat, 1, "the creature's controller searches");
    assert_eq!(offered_names.len(), LIBRARY.len());
    let land = from[0];
    pick(&mut driver, land)?;
    let found = driver.state().object(land).unwrap();
    assert_eq!(found.zone(), Zone::Battlefield);
    assert_eq!(found.controller(), pid(1));
    assert!(tapped(&driver, land));
    assert_eq!(shuffles(&driver), [1], "only their library is shuffled");
    assert_eq!(library(&driver, 0), sorted(owned(&LIBRARY)));
    assert!(
        names(driver.state(), Zone::Exile).contains(&"Ogre".to_owned()),
        "the creature was exiled"
    );
    assert!(driver.engine().events().iter().any(|event| matches!(
        event,
        Event::ResolutionEnded {
            outcome: ResolutionOutcome::Resolved,
            ..
        }
    )));
    Ok(())
}

#[rstest]
fn declining_the_search_leaves_the_library_alone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(path_table(seats))?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Path to Exile", &[ogre])?;
    pass_until_asked(&mut driver)?;
    for _ in 0..4 {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            break;
        }
        driver.pass()?;
    }
    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
    pass_until_asked(&mut driver)?;
    assert_eq!(shuffles(&driver), Vec::<usize>::new());
    assert_eq!(library(&driver, 1), sorted(owned(&LIBRARY)));
    assert_eq!(battlefield(&driver, 1), Vec::<String>::new());
    Ok(())
}
