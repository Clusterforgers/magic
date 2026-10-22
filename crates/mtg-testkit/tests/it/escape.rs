//! Goal: escape is a cast from the graveyard for its mana and the exile of other cards of the graveyard: it is offered only with enough other cards of its owner's graveyard and the mana of its colours (a cast that cannot be paid is refused and leaves no trace), never counts the card itself, asks the exile of its caster alone and refuses a card it did not offer, logs nothing of the picks until the spell is paid for, leaves the card where it goes after it resolves (no exile), gives "escapes with" counters only to the permanent that escaped and not to a cast from the hand or a copy, and a graveyard that is emptied by Rest in Peace sends the spell on to exile.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, on_battlefield, pid};
use mtg_cards::value::{CastWay, CounterKind};
use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::GameState;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The object called `name` in `zone`.
fn find(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

fn count(state: &GameState, zone: Zone) -> usize {
    state.zone(zone).len()
}

/// The cards the pending question offers, by name.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("a pick was expected");
    };
    let mut from: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    from.sort();
    from
}

/// Seat 0's graveyard holds `escaping` and `others` fodder cards, and `lands` Swamps are in play.
fn game(escaping: &str, others: &[&str], lands: usize) -> Result<Driver, ScriptError> {
    let mut names = vec![escaping];
    names.extend(others);
    Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(0, names)
            .battlefield(0, (0..lands).map(|_| ready(swamp()))),
    )
}

/// The cast of the card is offered from the graveyard.
fn castable(driver: &Driver, name: &str) -> bool {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return false;
    };
    options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

const FODDER: [&str; 5] = [
    "Dregscape Zombie",
    "Kathari Screecher",
    "Reconstructed Thopter",
    "Staggershock",
    "Artful Maneuver",
];

#[rstest]
#[case::enough_cards_and_mana(3, 4, true)]
#[case::more_cards_than_needed(5, 4, true)]
#[case::one_card_short(2, 4, false)]
#[case::one_land_short(3, 3, false)]
#[case::no_other_card(0, 4, false)]
fn escape_is_offered_with_enough_other_cards_and_the_mana(
    #[case] others: usize,
    #[case] lands: usize,
    #[case] offered_now: bool,
) -> Result<(), ScriptError> {
    let driver = game("Fruit of Tizerus", &FODDER[..others], lands)?;
    assert_eq!(castable(&driver, "Fruit of Tizerus"), offered_now);
    Ok(())
}

#[rstest]
#[case::too_few_other_cards_with_the_mana(2, 0, 4, 0, false)]
#[case::the_cards_of_another_graveyard_do_not_count(2, 3, 4, 0, false)]
#[case::too_little_mana_with_the_cards(3, 0, 3, 0, false)]
#[case::mana_of_the_wrong_colour_with_the_cards(3, 0, 0, 4, false)]
#[case::the_cards_and_the_mana(3, 0, 4, 0, true)]
#[case::the_cards_and_mana_of_two_colours(3, 0, 2, 2, true)]
fn a_cost_that_cannot_be_paid_is_not_offered_and_a_cast_is_refused_without_a_trace(
    #[case] own: usize,
    #[case] foe: usize,
    #[case] swamps: usize,
    #[case] islands: usize,
    #[case] payable: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let lands = (0..swamps)
        .map(|_| ready(swamp()))
        .chain((0..islands).map(|_| ready(island())));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .graveyard_named(
                0,
                std::iter::once("Fruit of Tizerus").chain(FODDER[..own].iter().copied()),
            )
            .graveyard_named(1, FODDER[..foe].iter().copied())
            .battlefield(0, lands),
    )?;
    assert_eq!(castable(&driver, "Fruit of Tizerus"), payable);
    let events = driver.engine().events().len();
    let hash = driver.state().state_hash();
    if payable {
        driver.begin_cast("Fruit of Tizerus")?;
    } else {
        assert!(driver.begin_cast("Fruit of Tizerus").is_err());
        assert_eq!(driver.engine().events().len(), events, "nothing is logged");
        assert_eq!(driver.state().state_hash(), hash, "nothing changed");
        assert_eq!(count(driver.state(), Zone::Exile), 0);
    }
    Ok(())
}

#[test]
fn the_pick_is_asked_of_its_caster_alone_refuses_a_card_not_offered_and_logs_nothing_until_paid()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(
                0,
                [
                    "Fruit of Tizerus",
                    FODDER[0],
                    FODDER[1],
                    FODDER[2],
                    FODDER[3],
                ],
            )
            .graveyard_named(1, ["Artful Maneuver"])
            .hand_named(0, ["Artful Maneuver"])
            .battlefield(0, (0..4).map(|_| ready(swamp()))),
    )?;
    driver.begin_cast("Fruit of Tizerus")?;
    driver.expect_decision(0, "choose-cards")?;
    assert_eq!(
        offered(&driver),
        [
            "Dregscape Zombie",
            "Kathari Screecher",
            "Reconstructed Thopter",
            "Staggershock"
        ],
        "the other cards of the caster's graveyard, not the card itself or a card of another zone"
    );
    let logged = driver.engine().events().len();
    for name_zone in [
        ("Fruit of Tizerus", Zone::Graveyard(pid(0))),
        ("Artful Maneuver", Zone::Graveyard(pid(1))),
        ("Artful Maneuver", Zone::Hand(pid(0))),
    ] {
        let id = find(driver.state(), name_zone.1, name_zone.0);
        let refused = driver.answer(Answer::ChooseCards(CardsChoice(vec![id])));
        assert!(refused.is_err(), "{refused:?}");
        driver.expect_decision(0, "choose-cards")?;
    }
    let too_few = find(driver.state(), Zone::Graveyard(pid(0)), FODDER[0]);
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(vec![too_few])))
            .is_err()
    );
    assert_eq!(
        driver.engine().events().len(),
        logged,
        "nothing is logged yet"
    );
    let picks: Vec<ObjectId> = FODDER[..3]
        .iter()
        .map(|name| find(driver.state(), Zone::Graveyard(pid(0)), name))
        .collect();
    driver.answer(Answer::ChooseCards(CardsChoice(picks.clone())))?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    assert_eq!(
        driver.engine().events().len(),
        logged,
        "the picks wait for the payment"
    );
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let exiled = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    to: Zone::Exile,
                    ..
                }
            )
        })
        .count();
    assert_eq!(exiled, 3, "the exile is public once the spell is paid for");
    assert_eq!(count(driver.state(), Zone::Exile), 3);
    Ok(())
}

#[test]
fn the_card_goes_to_the_graveyard_after_it_resolves_and_is_not_exiled() -> Result<(), ScriptError> {
    let mut driver = game("Fruit of Tizerus", &FODDER[..4], 4)?;
    driver.begin_cast("Fruit of Tizerus")?;
    let picks: Vec<ObjectId> = FODDER[..3]
        .iter()
        .map(|name| find(driver.state(), Zone::Graveyard(pid(0)), name))
        .collect();
    driver.answer(Answer::ChooseCards(CardsChoice(picks)))?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::SpellCast { choices, .. } if choices.way == Some(CastWay::Escape)
    )));
    driver.pass()?;
    driver.pass()?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 18);
    find(driver.state(), Zone::Graveyard(pid(0)), "Fruit of Tizerus");
    assert_eq!(
        count(driver.state(), Zone::Exile),
        3,
        "only the picks are exiled"
    );
    assert_eq!(
        count(driver.state(), Zone::Graveyard(pid(0))),
        2,
        "the spell and the card left over"
    );
    Ok(())
}

/// Seat 0 casts the Rage-Hound, from the graveyard when `escape` and from the hand otherwise, and it resolves; they hold
/// a Cackling Counterpart and the Islands for it.
fn rage_hound(escape: bool) -> Result<Driver, ScriptError> {
    let lands = (0..4)
        .map(|_| ready(mountain()))
        .chain((0..3).map(|_| ready(island())));
    let scenario = Scenario::new(2).battlefield(0, lands);
    let mut driver = Driver::from_scenario(if escape {
        scenario
            .graveyard_named(
                0,
                [
                    "Underworld Rage-Hound",
                    FODDER[0],
                    FODDER[1],
                    FODDER[2],
                    FODDER[3],
                ],
            )
            .hand_named(0, ["Cackling Counterpart"])
    } else {
        scenario.hand_named(0, ["Underworld Rage-Hound", "Cackling Counterpart"])
    })?;
    driver.begin_cast("Underworld Rage-Hound")?;
    if escape {
        let picks: Vec<ObjectId> = FODDER[..3]
            .iter()
            .map(|name| find(driver.state(), Zone::Graveyard(pid(0)), name))
            .collect();
        driver.answer(Answer::ChooseCards(CardsChoice(picks)))?;
    }
    // The Mountains pay, so the Islands are left for the copy: {1}{R} from the hand, {3}{R} by escape.
    let paid: &[&str] = if escape {
        &["Mountain"; 4]
    } else {
        &["Mountain"; 2]
    };
    driver.pay_tapping(paid, 0)?;
    driver.pass()?;
    driver.pass()?;
    Ok(driver)
}

fn plus_counters(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

#[rstest]
#[case::cast_from_the_hand(false, 0)]
#[case::escaped(true, 1)]
fn only_a_permanent_that_escaped_has_its_counter(
    #[case] escape: bool,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let driver = rage_hound(escape)?;
    let id = on_battlefield(driver.state(), "Underworld Rage-Hound").expect("it resolved");
    assert_eq!(plus_counters(&driver, id), counters);
    Ok(())
}

#[test]
fn a_copy_of_the_escaped_permanent_has_no_counter() -> Result<(), ScriptError> {
    let mut driver = rage_hound(true)?;
    let hound = on_battlefield(driver.state(), "Underworld Rage-Hound").unwrap();
    assert_eq!(plus_counters(&driver, hound), 1);
    let at = driver.target_named("Underworld Rage-Hound")?;
    driver.cast_at("Cackling Counterpart", &[at])?;
    driver.pass()?;
    driver.pass()?;
    let hounds: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(driver.state(), *id) == "Underworld Rage-Hound")
        .collect();
    assert_eq!(hounds.len(), 2, "the original and its copy");
    let counters: Vec<u32> = hounds
        .iter()
        .map(|id| plus_counters(&driver, *id))
        .collect();
    assert_eq!(
        counters.iter().sum::<u32>(),
        1,
        "only the one that escaped: {counters:?}"
    );
    Ok(())
}

#[test]
fn rest_in_peace_sends_the_escaped_spell_on_to_exile() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(
                0,
                [
                    "Fruit of Tizerus",
                    FODDER[0],
                    FODDER[1],
                    FODDER[2],
                    FODDER[3],
                ],
            )
            .battlefield_named(1, ["Rest in Peace"])
            .battlefield(0, (0..4).map(|_| ready(swamp()))),
    )?;
    driver.begin_cast("Fruit of Tizerus")?;
    let picks: Vec<ObjectId> = FODDER[..3]
        .iter()
        .map(|name| find(driver.state(), Zone::Graveyard(pid(0)), name))
        .collect();
    driver.answer(Answer::ChooseCards(CardsChoice(picks)))?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    driver.pass()?;
    driver.pass()?;
    find(driver.state(), Zone::Exile, "Fruit of Tizerus");
    assert_eq!(count(driver.state(), Zone::Exile), 4);
    Ok(())
}
