//! Goal: the searches that another replaces under a condition (Far Wanderings with seven cards in the graveyard, Primal Growth kicked), the search that keeps one card and puts the rest in the hand (Kodama's Reach), the searches for basic lands and/or a land type (Map the Frontier, Circuitous Route) and the exile whose card kind decides what follows (Scavenging Ooze) play whole from their Oracle wordings at tables of two, three and four seats: the branch the condition picks is the one that happens, a library with no card, one card or many gives none, one or the most, the lands the filter names are offered and no other, and the replay of the log gives the state the game was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, CostsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const LANDS: [&str; 5] = ["Forest", "Plains", "Island", "Swamp", "Mountain"];

/// Seat 0 holds `spell` and four Forests, a Bear and `graveyard` filler cards; every seat's library is `library`.
fn table(seats: usize, spell: &str, graveyard: usize, library: &[&str]) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(spell).unwrap()])
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(forest()))
                .chain([ready(free_creature("Bear", 2, 2))]),
        )
        .graveyard(
            0,
            (0..graveyard).map(|n| free_creature(&format!("Filler {n}"), 1, 1)),
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, defs(library));
    }
    scenario
}

/// The cards with these names: the basic lands are built, the others are corpus cards.
fn defs(cards: &[&str]) -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
    cards
        .iter()
        .map(|name| match *name {
            "Forest" => forest(),
            "Plains" => plains(),
            "Island" => island(),
            "Swamp" => swamp(),
            "Mountain" => mountain(),
            other => card(other).unwrap(),
        })
        .collect()
}

/// Answers every question until the stack is empty: the most cards offered are chosen, mana is paid automatically.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) => {
                let take = usize::try_from(options.max)
                    .unwrap()
                    .min(options.from.len());
                driver.answer(Answer::ChooseCards(CardsChoice(
                    options.from[..take].to_vec(),
                )))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Answers what a cast or an activation asks before it is on the stack (a sacrifice for a kicker, the payment).
fn announce(driver: &mut Driver) -> Result<(), ScriptError> {
    while driver.state().stack().is_empty() {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                let take = usize::try_from(options.max)
                    .unwrap()
                    .min(options.from.len());
                let chosen = options.from[..take].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            other => panic!("unexpected question {}", other.name()),
        }
    }
    Ok(())
}

/// The number of lands on the battlefield beyond the four Forests dealt to seat 0.
fn lands(driver: &Driver) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| {
            LANDS.contains(&name.as_str()) || name.contains("Gate") || name.contains("Verge")
        })
        .count()
        - 4
}

fn hand_lands(driver: &Driver) -> usize {
    names(driver.state(), Zone::Hand(pid(0)))
        .iter()
        .filter(|name| LANDS.contains(&name.as_str()))
        .count()
}

/// The replay of the log gives the state the driver ends in.
fn replays(scenario: &Scenario, driver: &Driver) {
    let replayed = replay(&scenario.clone().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

const MANY: [&str; 6] = ["Forest", "Plains", "Forest", "Island", "Plains", "Swamp"];

#[rstest]
#[case::six_cards_is_one_land(6, &MANY, 1)]
#[case::seven_cards_is_three_lands(7, &MANY, 3)]
#[case::seven_cards_and_two_in_the_library(7, &["Forest", "Plains"], 2)]
#[case::seven_cards_and_one_in_the_library(7, &["Plains"], 1)]
#[case::seven_cards_and_none_in_the_library(7, &[], 0)]
#[case::six_cards_and_none_in_the_library(6, &[], 0)]
#[case::many_cards_is_three_lands(12, &MANY, 3)]
fn the_graveyard_decides_how_many_lands_far_wanderings_finds(
    #[values(2, 3, 4)] seats: usize,
    #[case] graveyard: usize,
    #[case] library: &[&str],
    #[case] found: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Far Wanderings", graveyard, library);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Far Wanderings")?;
    settle(&mut driver)?;
    assert_eq!(lands(&driver), found);
    let library_left = names(driver.state(), Zone::Library(pid(0))).len();
    assert_eq!(library_left, library.len() - found);
    replays(&scenario, &driver);
    Ok(())
}

#[rstest]
#[case::unkicked(false, &MANY, 1)]
#[case::kicked(true, &MANY, 2)]
#[case::kicked_with_one_card(true, &["Plains"], 1)]
#[case::kicked_with_none(true, &[], 0)]
#[case::unkicked_with_none(false, &[], 0)]
fn kicking_primal_growth_finds_two_lands_instead_of_one(
    #[values(2, 3, 4)] seats: usize,
    #[case] kicked: bool,
    #[case] library: &[&str],
    #[case] found: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Primal Growth", 0, library);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.begin_cast("Primal Growth")?;
    driver.expect_decision(0, "choose-costs")?;
    let costs = if kicked { vec![0] } else { vec![] };
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs)))?;
    announce(&mut driver)?;
    settle(&mut driver)?;
    assert_eq!(lands(&driver), found);
    // The kicker sacrificed the Bear, and the unkicked spell did not.
    let bears = names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Bear")
        .count();
    assert_eq!(bears, usize::from(!kicked));
    replays(&scenario, &driver);
    Ok(())
}

#[rstest]
#[case::none(&[], 0, 0)]
#[case::one(&["Plains"], 1, 0)]
#[case::two(&["Plains", "Forest"], 1, 1)]
#[case::many(&MANY, 1, 1)]
fn kodamas_reach_puts_one_land_onto_the_battlefield_and_the_other_into_the_hand(
    #[values(2, 3, 4)] seats: usize,
    #[case] library: &[&str],
    #[case] on_battlefield: usize,
    #[case] in_hand: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Kodama's Reach", 0, library);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Kodama's Reach")?;
    settle(&mut driver)?;
    assert_eq!(lands(&driver), on_battlefield);
    assert_eq!(hand_lands(&driver), in_hand);
    let state = driver.state();
    // The land that entered is tapped.
    let tapped = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id)?.permanent())
        .filter(|permanent| permanent.tapped())
        .count();
    assert_eq!(
        tapped,
        3 + on_battlefield,
        "three Forests paid, the new land is tapped"
    );
    replays(&scenario, &driver);
    Ok(())
}

/// The names the pending search offers.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a choice of cards is asked");
    };
    let mut found: Vec<String> = options
        .from
        .iter()
        .map(|id| common::name_of(driver.state(), *id))
        .collect();
    found.sort();
    found
}

#[rstest]
#[case::map_the_frontier("Map the Frontier", &["Forest", "Plains", "Krosan Verge", "Citadel Gate"], &["Forest", "Plains"])]
#[case::circuitous_route("Circuitous Route", &["Forest", "Plains", "Krosan Verge", "Citadel Gate"], &["Citadel Gate", "Forest", "Plains"])]
fn the_lands_a_list_of_basic_lands_and_a_land_type_names_are_offered(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] library: &[&str],
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let scenario = table(seats, spell, 0, library);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast(spell)?;
    while !matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        driver.answer_passively()?;
    }
    let expected: Vec<String> = expected.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(offered(&driver), expected);
    settle(&mut driver)?;
    assert_eq!(lands(&driver), expected.len().min(2));
    replays(&scenario, &driver);
    Ok(())
}

/// Seat 0 controls Scavenging Ooze and a Forest, seat 1 has `graveyard` named cards; the Ooze exiles the first.
fn ooze_table(seats: usize, graveyard: &[&str]) -> Scenario {
    Scenario::new(seats)
        .battlefield(
            0,
            [ready(card("Scavenging Ooze").unwrap()), ready(forest())],
        )
        .graveyard(1, defs(graveyard))
}

#[rstest]
#[case::a_creature_card(&["Grizzly Bears"], 1, 1)]
#[case::a_land_card(&["Forest"], 0, 0)]
#[case::an_instant_card(&["Lightning Bolt"], 0, 0)]
fn the_ooze_grows_and_gains_life_for_a_creature_card_only(
    #[values(2, 3, 4)] seats: usize,
    #[case] graveyard: &[&str],
    #[case] counters: usize,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let scenario = ooze_table(seats, graveyard).life(0, 20);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.begin_activation("Scavenging Ooze")?;
    let victim: ObjectId = driver.state().zone(Zone::Graveyard(pid(1)))[0];
    let target = Target::Object(driver.state().object(victim).unwrap().reference());
    driver.choose_targets(&[target])?;
    announce(&mut driver)?;
    settle(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(1))).len(), 0);
    assert_eq!(names(driver.state(), Zone::Exile).len(), 1);
    let ooze = driver.id_of("Scavenging Ooze")?;
    let marks = driver
        .state()
        .object(ooze)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .map(|(_, count)| count)
        .sum::<u32>();
    assert_eq!(marks as usize, counters);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20 + life);
    replays(&scenario, &driver);
    Ok(())
}

#[test]
fn the_ooze_has_nothing_to_exile_when_no_graveyard_holds_a_card() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(ooze_table(2, &[]))?;
    assert!(
        driver.begin_activation("Scavenging Ooze").is_err(),
        "the ability needs a card to target"
    );
    Ok(())
}
