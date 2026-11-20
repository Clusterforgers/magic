//! Goal: foretell beyond the printed keyword (CR 702.143a) at 2 to 4 seats: a nonland card without foretell in a hand
//! a static ability gives foretell is foretold for {2} and cast on a later turn for the cost it was given, even once the
//! giver is gone, while a land card and a card with its own foretell keep theirs; "whenever you foretell a card"
//! triggers on the special action; foretelling costs {1} less per Cosmos Charger and opens on any player's turn; and the
//! first card foretold each turn under Ranar costs {0}, the second {2} again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::object::{FaceDown, PermanentState};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{forest, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The id of the card called `name` in a zone.
fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// The options of the priority decision now, when it is one.
fn priority_names(driver: &Driver, cast: bool) -> Vec<String> {
    let Ok(decision) = driver.decision() else {
        return Vec::new();
    };
    let DecisionKind::Priority(options) = &decision.kind else {
        return Vec::new();
    };
    let ids = if cast {
        &options.castable
    } else {
        &options.foretellable
    };
    let mut names: Vec<String> = ids.iter().map(|id| name_of(driver.state(), *id)).collect();
    names.sort();
    names
}

/// The number of tapped permanents seat 0 controls.
fn tapped(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.controller() == pid(0) && o.permanent().is_some_and(PermanentState::tapped))
        .count()
}

/// Foretells the card called `name` from seat 0's hand, paying with the named lands when a payment is asked.
fn foretell(driver: &mut Driver, name: &str, lands: &[&str]) -> Result<(), ScriptError> {
    let id = find(driver, Zone::Hand(pid(0)), name);
    driver.answer(Answer::Priority(PriorityAction::Foretell(id)))?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.pay_tapping(lands, 0)?;
    }
    Ok(())
}

/// Passes until the stack is empty and seat 0 holds priority again.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().zone(Zone::Stack).is_empty() || driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    Ok(())
}

/// The power of the permanent called `name`.
fn power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    mtg_engine::characteristics::characteristics(driver.state(), id)
        .unwrap()
        .power
        .unwrap()
}

/// The lands and the corpus permanents called `own`, untapped.
fn with_corpus(mut lands: Vec<PresetPermanent>, own: &[&str]) -> Vec<PresetPermanent> {
    lands.extend(own.iter().map(|name| ready(card(name).unwrap())));
    lands
}

/// Seat 0 holds `hand`, controls `own` and `lands`, at `seats` seats, in its first main phase of turn 1.
fn game(
    seats: usize,
    hand: &[&str],
    own: &[&str],
    lands: Vec<PresetPermanent>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, with_corpus(lands, own));
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    Driver::from_scenario(scenario)
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_given_foretell_cost_stays_with_the_foretold_card_after_the_giver_leaves(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let lands = vec![
        ready(island()),
        ready(island()),
        ready(plains()),
        ready(plains()),
        ready(plains()),
    ];
    let mut driver = game(
        seats,
        &["Serra Angel", "Forest", "Augury Raven"],
        &["Dream Devourer"],
        lands,
    )?;
    assert_eq!(
        priority_names(&driver, false),
        ["Augury Raven", "Serra Angel"],
        "a land card is not given foretell"
    );
    assert_eq!(power(&driver, "Dream Devourer"), 0);
    foretell(&mut driver, "Serra Angel", &["Island", "Island"])?;
    assert_eq!(tapped(&driver), 2, "foretelling costs {{2}}");
    settle(&mut driver)?;
    assert_eq!(
        power(&driver, "Dream Devourer"),
        2,
        "whenever you foretell a card, it gets +2/+0"
    );
    let angel = find(&driver, Zone::Exile, "Serra Angel");
    assert_eq!(
        driver.state().object(angel).unwrap().face_down(),
        Some(FaceDown::Foretold { turn: 1 })
    );
    let devourer = driver.id_of("Dream Devourer")?;
    driver.manual(ManualAction::Move {
        object: devourer,
        to: ManualZone::Graveyard,
    })?;
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    assert!(priority_names(&driver, true).contains(&"Serra Angel".to_owned()));
    let before = tapped(&driver);
    driver.answer(Answer::Priority(PriorityAction::CastSpell(angel)))?;
    driver.pay_tapping(&["Plains", "Plains", "Plains"], 0)?;
    assert_eq!(
        tapped(&driver) - before,
        3,
        "{{1}}{{W}}{{W}}, its mana cost reduced by {{2}}"
    );
    let way = driver
        .engine()
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.way),
            _ => None,
        });
    assert_eq!(way, Some(Some(CastWay::Foretold)));
    Ok(())
}

#[rstest]
#[case::one_charger_two_seats(2, 1, 1)]
#[case::one_charger_four_seats(4, 1, 1)]
#[case::two_chargers_three_seats(3, 2, 0)]
fn foretelling_costs_less_per_charger_and_opens_on_any_turn(
    #[case] seats: usize,
    #[case] chargers: usize,
    #[case] paid: usize,
) -> Result<(), ScriptError> {
    let lands = vec![ready(island()), ready(island())];
    let own = vec!["Cosmos Charger"; chargers];
    let mut driver = Driver::from_scenario({
        let mut scenario = Scenario::new(seats)
            .at(2, 1, Step::Main1)
            .hand_named(0, ["Behold the Multiverse"])
            .battlefield(0, with_corpus(lands, &own));
        for seat in 0..seats {
            scenario = scenario.library(seat, vec![forest(); 12]);
        }
        scenario
    })?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    assert_eq!(driver.state().turn().active(), pid(1));
    assert_eq!(priority_names(&driver, false), ["Behold the Multiverse"]);
    foretell(&mut driver, "Behold the Multiverse", &vec!["Island"; paid])?;
    assert_eq!(tapped(&driver), paid);
    let angel = find(&driver, Zone::Exile, "Behold the Multiverse");
    assert!(driver.state().object(angel).unwrap().face_down().is_some());
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn the_first_card_foretold_each_turn_costs_nothing(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let lands = vec![ready(island()), ready(island())];
    let mut driver = game(
        seats,
        &["Behold the Multiverse", "Saw It Coming"],
        &["Ranar the Ever-Watchful"],
        lands,
    )?;
    foretell(&mut driver, "Behold the Multiverse", &[])?;
    assert_eq!(tapped(&driver), 0, "the first costs {{0}}");
    settle(&mut driver)?;
    foretell(&mut driver, "Saw It Coming", &["Island", "Island"])?;
    assert_eq!(tapped(&driver), 2, "the second costs {{2}}");
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))), []);
    Ok(())
}

#[test]
fn without_a_charger_the_turn_of_another_player_stays_closed() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 1, Step::Main1)
            .library(0, vec![forest(); 6])
            .library(1, vec![forest(); 6])
            .hand_named(0, ["Behold the Multiverse"])
            .battlefield(
                0,
                with_corpus(vec![ready(island()), ready(island())], &["Dream Devourer"]),
            ),
    )?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    assert_eq!(priority_names(&driver, false), Vec::<String>::new());
    Ok(())
}
