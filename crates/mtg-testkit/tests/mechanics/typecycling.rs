//! Goal: typecycling plays as the rules say at two to four seats. It is cycling that searches: the card is cycled from a hand whenever its owner has priority when the cost can be paid (one ability for each typecycling the card prints), the discard is the cost and is logged as a cycling, and on resolution its owner searches the library for a card of the type, which is revealed and put into the hand, and shuffles. Basic landcycling finds basic lands of any type and no nonbasic land, a land type finds that type only, a library with no such card or none at all finds nothing and is shuffled all the same, and the owner may find nothing when cards match. Backing out of the payment (Undo or Cancel) keeps the card in the hand with nothing tapped or discarded; the payment names the ability it pays for and proposes the game's payment, which pays it when confirmed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, names, pid};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, island, mountain, wastes};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A card of a library by name: a basic land by its name, anything else from the corpus.
fn card(name: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    match name {
        "Forest" => forest(),
        "Island" => island(),
        "Mountain" => mountain(),
        "Wastes" => wastes(),
        other => corpus_card(other).unwrap(),
    }
}

/// Seat 0 holds `hand` with `lands` in play and `library` to search, and the other seats just exist.
fn table(seats: usize, hand: &[&str], lands: usize, library: &[&str]) -> Driver {
    let scenario = Scenario::new(seats)
        .hand(0, hand.iter().map(|name| card(name)))
        .battlefield(0, (0..lands).map(|_| ready(mountain())))
        .library(0, library.iter().map(|name| card(name)));
    Driver::from_scenario(scenario).unwrap()
}

/// How many abilities of the cards called `name` are offered.
fn offered(driver: &Driver, name: &str) -> usize {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    options
        .abilities
        .iter()
        .filter(|ability| name_of(driver.state(), ability.object) == name)
        .count()
}

/// Activates the `nth` ability offered of the card called `name`.
fn activate(driver: &mut Driver, name: &str, nth: usize) -> Vec<Event> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    let ability = options
        .abilities
        .iter()
        .filter(|ability| name_of(driver.state(), ability.object) == name)
        .nth(nth)
        .copied()
        .unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::Activate(ability)))
        .unwrap()
}

/// Passes priority until the stack is empty and answers the search with the first card offered that is called `pick` (no
/// card for `None`). Returns the names offered, empty when nothing was asked, and the events from the first pass.
fn resolve(driver: &mut Driver, pick: Option<&str>) -> (Vec<String>, Vec<Event>) {
    let (mut offered, mut events) = (Vec::new(), Vec::new());
    for _ in 0..50 {
        let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
            if driver.state().stack().is_empty() {
                break;
            }
            events.extend(driver.pass().unwrap());
            continue;
        };
        assert_eq!(
            driver.decision().unwrap().player,
            pid(0),
            "the owner searches"
        );
        assert_eq!(options.min, 0, "a library need not give up a card");
        offered = options
            .from
            .iter()
            .map(|id| name_of(driver.state(), *id))
            .collect();
        let picked: Vec<ObjectId> = options
            .from
            .iter()
            .copied()
            .filter(|id| Some(name_of(driver.state(), *id).as_str()) == pick)
            .take(1)
            .collect();
        events.extend(
            driver
                .answer(Answer::ChooseCards(CardsChoice(picked)))
                .unwrap(),
        );
    }
    (offered, events)
}

#[rstest]
#[case::basic_landcycling_with_its_cost("Ash Barrens", 1, 1)]
#[case::basic_landcycling_without_it("Ash Barrens", 0, 0)]
#[case::a_land_type_with_its_cost("Chartooth Cougar", 2, 1)]
#[case::a_land_type_one_short("Chartooth Cougar", 1, 0)]
#[case::two_typecyclings_are_two_abilities("Valley Rannet", 2, 2)]
#[case::two_typecyclings_one_short("Valley Rannet", 1, 0)]
fn a_typecycling_is_offered_from_the_hand_when_its_cost_can_be_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] hand: &str,
    #[case] lands: usize,
    #[case] abilities: usize,
) {
    let driver = table(seats, &[hand], lands, &["Forest"]);
    assert_eq!(offered(&driver, hand), abilities);
}

#[rstest]
#[case::not_from_the_battlefield(Scenario::new(2).battlefield(0, [ready(card("Ash Barrens")), ready(mountain())]))]
#[case::not_from_the_graveyard(Scenario::new(2).graveyard(0, [card("Ash Barrens")]).battlefield(0, [ready(mountain())]))]
fn a_typecycling_works_from_a_hand_and_nowhere_else(#[case] scenario: Scenario) {
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(offered(&driver, "Ash Barrens"), 0);
}

#[rstest]
#[case::basic_landcycling_finds_basic_lands_only(
    "Ash Barrens", 0, &["Forest", "Ash Barrens", "Island"], &["Forest", "Island"], Some("Island")
)]
#[case::wastes_is_a_basic_land("Ash Barrens", 0, &["Wastes", "Forest"], &["Wastes", "Forest"], Some("Wastes"))]
#[case::a_land_type_finds_that_type_only(
    "Chartooth Cougar", 0, &["Forest", "Mountain", "Island", "Mountain"], &["Mountain", "Mountain"], Some("Mountain")
)]
#[case::the_second_typecycling_finds_forests(
    "Valley Rannet", 1, &["Mountain", "Forest", "Island"], &["Forest"], Some("Forest")
)]
#[case::the_first_typecycling_finds_mountains(
    "Valley Rannet", 0, &["Mountain", "Forest", "Island"], &["Mountain"], Some("Mountain")
)]
#[case::the_owner_may_find_nothing("Ash Barrens", 0, &["Forest"], &["Forest"], None)]
#[case::no_card_of_the_type("Chartooth Cougar", 0, &["Forest", "Island"], &[], None)]
#[case::an_empty_library("Chartooth Cougar", 0, &[], &[], None)]
fn typecycling_searches_for_a_card_of_the_type_reveals_it_and_shuffles(
    #[values(2, 3, 4)] seats: usize,
    #[case] cycler: &str,
    #[case] nth: usize,
    #[case] library: &[&str],
    #[case] expected_offer: &[&str],
    #[case] found: Option<&str>,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, &[cycler, "Forest"], 2, library);
    let setup = Scenario::new(seats)
        .hand(0, [card(cycler), card("Forest")])
        .battlefield(0, (0..2).map(|_| ready(mountain())))
        .library(0, library.iter().map(|name| card(name)))
        .setup()?;
    let before = driver.state().zone(Zone::Library(pid(0))).len();
    activate(&mut driver, cycler, nth);
    // Basic landcycling costs {1}, the typecyclings of the other cards {2}.
    let lands: &[&str] = if cycler == "Ash Barrens" {
        &["Mountain"]
    } else {
        &["Mountain", "Mountain"]
    };
    let events = driver.pay_tapping(lands, 0)?;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::CardDiscarded { player, .. } if *player == pid(0))),
        "the discard is the cost"
    );
    assert!(
        events.iter().any(|e| matches!(e, Event::Cycled { .. })),
        "a typecycling is a cycling"
    );
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), [cycler]);
    let (offered, events) = resolve(&mut driver, found);
    // The order of the cards offered is not part of the contract.
    let (mut offered, mut expected_offer) = (offered, expected_offer.to_vec());
    offered.sort();
    expected_offer.sort_unstable();
    assert_eq!(offered, expected_offer);
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    if let Some(name) = found {
        assert_eq!(hand, ["Forest", name]);
        assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), before - 1);
        assert!(
            events.iter().any(|e| matches!(e, Event::Revealed(_))),
            "the card found is revealed"
        );
    } else {
        assert_eq!(hand, ["Forest"]);
        assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), before);
        assert!(!events.iter().any(|e| matches!(e, Event::Revealed(_))));
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::LibraryShuffled { .. })),
        "the library is shuffled whether or not a card was found"
    );
    let replayed = replay(&setup, driver.engine().events())?;
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// The ability offered `nth` of the cards called `name`.
fn offered_ability(driver: &Driver, name: &str, nth: usize) -> mtg_engine::abilities::AbilityRef {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    options
        .abilities
        .iter()
        .filter(|ability| name_of(driver.state(), ability.object) == name)
        .nth(nth)
        .copied()
        .unwrap()
}

/// How many permanents are tapped.
fn tapped(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .and_then(|o| o.permanent())
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count()
}

#[rstest]
#[case::basic_landcycling_by_undo("Ash Barrens", Answer::Undo)]
#[case::basic_landcycling_by_cancel("Ash Barrens", Answer::PayMana(PayChoice::Cancel))]
#[case::a_land_type_by_undo("Chartooth Cougar", Answer::Undo)]
#[case::a_land_type_by_cancel("Chartooth Cougar", Answer::PayMana(PayChoice::Cancel))]
fn backing_out_of_the_payment_of_a_typecycling_keeps_the_card_and_pays_nothing(
    #[values(2, 3, 4)] seats: usize,
    #[case] cycler: &str,
    #[case] back: Answer,
) {
    let mut driver = table(seats, &[cycler], 2, &["Forest"]);
    let ability = offered_ability(&driver, cycler, 0);
    driver
        .answer(Answer::Priority(PriorityAction::Activate(ability)))
        .unwrap();
    let DecisionKind::PayMana(options) = &driver.decision().unwrap().kind else {
        panic!("the cost is asked");
    };
    // The payment names the ability it pays for (CR 602.2b), may be backed out of and proposes the game's payment.
    assert_eq!(options.ability.as_deref(), Some(&ability));
    assert!(options.can_cancel);
    assert_eq!(
        options.proposal.as_ref().map(|payment| payment.taps.len()),
        Some(usize::try_from(options.cost.mana_value()).unwrap())
    );
    let events = driver.answer(back).unwrap();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::CardDiscarded { .. } | Event::Cycled { .. })),
        "nothing is paid before the payment (CR 727.1)"
    );
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Priority(_)
    ));
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), [cycler]);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        Vec::<String>::new()
    );
    assert_eq!(tapped(&driver), 0);
    assert_eq!(driver.state().stack().len(), 0);
}

#[rstest]
#[case::basic_landcycling("Ash Barrens")]
#[case::a_land_type("Chartooth Cougar")]
fn the_proposed_payment_confirmed_pays_the_typecycling(
    #[values(2, 3, 4)] seats: usize,
    #[case] cycler: &str,
) {
    let mut driver = table(seats, &[cycler], 2, &["Forest"]);
    activate(&mut driver, cycler, 0);
    let DecisionKind::PayMana(options) = &driver.decision().unwrap().kind else {
        panic!("the cost is asked");
    };
    let proposal = options.proposal.clone().unwrap();
    let events = driver
        .answer(Answer::PayMana(PayChoice::Manual(*proposal)))
        .unwrap();
    assert!(events.iter().any(|e| matches!(e, Event::Cycled { .. })));
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), [cycler]);
}
