//! Goal: manifest and cloak play whole at tables of two, three and four seats: the top card of the named library comes onto the battlefield face down as a 2/2 creature under the control of the player who acts (an opponent's card included), only that player looks at it, a creature card is turned face up for its mana cost as a special action (a card of another type never is) and triggers what watches for a permanent turned face up, and a cloaked card has ward {2}.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::ModeChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::FaceDown;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{free_creature, island, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

use crate::common::casting::{lands, pay};
use crate::common::pid;

/// A library of the card called `top` over plain creatures.
fn library(top: &str) -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
    let mut cards = vec![corpus::card(top).unwrap()];
    cards.extend((0..6).map(|i| free_creature(&format!("Pad {i}"), 1, 1)));
    cards
}

/// Seat 0 controls `card` and `plains` Plains on turn 1; every seat's library has `top` over it.
fn table(seats: usize, card: &str, lands_wanted: usize, top: &str) -> Scenario {
    let mut battlefield: Vec<PresetPermanent> = vec![ready(corpus::card(card).unwrap())];
    battlefield.extend(lands(lands_wanted, plains));
    (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, battlefield),
        |scenario, seat| scenario.library(seat, library(top)),
    )
}

/// Lets the stack resolve: priority passes until it is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// The face-down permanents of every kind on the battlefield.
fn face_down(driver: &Driver) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .collect()
}

fn library_size(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Library(pid(seat))).len()
}

fn turnable(driver: &Driver) -> Vec<ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.turnable.clone(),
        _ => Vec::new(),
    }
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
#[case::a_creature_card("Serra Angel", true)]
#[case::an_instant("Lightning Bolt", false)]
#[case::an_artifact("Sol Ring", false)]
fn the_activation_manifests_the_top_card_and_a_creature_card_can_be_turned_face_up(
    #[case] top: &str,
    #[case] creature: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    // {3}{W} for the ability and {3}{W}{W} for Serra Angel.
    let mut driver = Driver::from_scenario(table(seats, "Mastery of the Unseen", 9, top)).unwrap();
    let sizes: Vec<usize> = (0..seats).map(|seat| library_size(&driver, seat)).collect();
    let card = driver.state().zone(Zone::Library(pid(0))).last().copied();
    driver.begin_activation("Mastery of the Unseen").unwrap();
    pay(&mut driver).unwrap();
    settle(&mut driver).unwrap();
    let made = face_down(&driver);
    assert_eq!(made.len(), 1);
    let object = driver.state().object(made[0]).unwrap();
    assert_eq!(object.face_down(), Some(FaceDown::Manifest));
    assert_eq!(object.zone(), Zone::Battlefield);
    assert_eq!(object.whole_card().name, top);
    assert_eq!(object.card().name, "");
    assert_eq!(object.controller(), pid(0));
    assert_eq!(Some(object.id()), card, "the top card, now a new object");
    assert_eq!(library_size(&driver, 0), sizes[0] - 1);
    for (seat, size) in sizes.iter().enumerate().skip(1) {
        assert_eq!(library_size(&driver, seat), *size);
    }
    let chars = characteristics(driver.state(), made[0]).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
    assert!(chars.types.contains(CardType::Creature));
    assert_eq!(
        turnable(&driver),
        if creature { made.clone() } else { vec![] },
        "only a creature card is turned face up for its mana cost"
    );
}

#[rstest]
fn a_manifested_creature_card_is_turned_face_up_for_its_mana_cost_and_the_enchantment_sees_it(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver =
        Driver::from_scenario(table(seats, "Mastery of the Unseen", 9, "Serra Angel")).unwrap();
    driver.begin_activation("Mastery of the Unseen").unwrap();
    pay(&mut driver).unwrap();
    settle(&mut driver).unwrap();
    let id = face_down(&driver)[0];
    assert_eq!(life(&driver, 0), 20);
    let logged = driver.engine().events().len();
    driver
        .answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))
        .unwrap();
    pay(&mut driver).unwrap();
    assert!(
        driver.state().stack().is_empty() || driver.state().stack().len() == 1,
        "the turn is a special action; only the trigger it causes waits"
    );
    settle(&mut driver).unwrap();
    let events = &driver.engine().events()[logged..];
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::TurnedFaceUp { object, .. } if *object == id))
    );
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.face_down(), None);
    assert_eq!(object.card().name, "Serra Angel");
    let chars = characteristics(driver.state(), id).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(4), Some(4)));
    assert!(keywords_of(driver.state(), id).contains(&&Keyword::Flying));
    assert_eq!(
        life(&driver, 0),
        21,
        "one life for the creature it controls"
    );
}

#[rstest]
fn a_card_the_opponent_loses_to_reality_shift_is_manifested_from_their_own_library(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, ["Reality Shift"])
            .battlefield(0, lands(2, island))
            .battlefield(1, vec![ready(free_creature("Bear", 2, 2))]),
        |scenario, seat| scenario.library(seat, library("Exalted Angel")),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let sizes: Vec<usize> = (0..seats).map(|seat| library_size(&driver, seat)).collect();
    let bear = driver.target_named("Bear").unwrap();
    driver.cast_at("Reality Shift", &[bear]).unwrap();
    settle(&mut driver).unwrap();
    let state = driver.state();
    assert!(
        state
            .zone(Zone::Exile)
            .iter()
            .any(|id| state.object(*id).unwrap().card().name == "Bear")
    );
    let made = face_down(&driver);
    assert_eq!(made.len(), 1);
    let object = state.object(made[0]).unwrap();
    assert_eq!(object.face_down(), Some(FaceDown::Manifest));
    assert_eq!(object.whole_card().name, "Exalted Angel");
    assert_eq!(object.controller(), pid(1), "its controller manifests");
    assert_eq!(object.owner(), pid(1));
    assert!(state.may_look_at(pid(1), made[0]));
    for seat in (0..seats).filter(|seat| *seat != 1) {
        assert!(!state.may_look_at(pid(seat), made[0]), "seat {seat}");
        assert_eq!(library_size(&driver, seat), sizes[seat]);
    }
    assert_eq!(library_size(&driver, 1), sizes[1] - 1);
}

#[rstest]
fn the_amalgam_manifests_from_the_library_of_the_player_whose_upkeep_it_is(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::End)
            .hand_named(0, ["Doom Blade"])
            .battlefield(
                0,
                [
                    vec![ready(corpus::card("Thieving Amalgam").unwrap())],
                    lands(2, swamp),
                ]
                .concat(),
            ),
        |scenario, seat| scenario.library(seat, library("Serra Angel")),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = library_size(&driver, 1);
    driver.pass_until(Step::Upkeep).unwrap();
    settle(&mut driver).unwrap();
    assert_eq!(driver.state().active_player(), Some(pid(1)));
    let made = face_down(&driver);
    assert_eq!(made.len(), 1);
    let object = driver.state().object(made[0]).unwrap();
    assert_eq!(object.owner(), pid(1));
    assert_eq!(
        object.controller(),
        pid(0),
        "the Amalgam's controller manifests it"
    );
    assert_eq!(library_size(&driver, 1), before - 1);
    assert!(driver.state().may_look_at(pid(0), made[0]));
    for seat in 1..seats {
        assert!(
            !driver.state().may_look_at(pid(seat), made[0]),
            "seat {seat}"
        );
    }
    let target = mtg_engine::choose::Target::Object(object.reference());
    while driver.decision().unwrap().player != pid(0) {
        driver.pass().unwrap();
    }
    // A creature the Amalgam's controller does not own dies: its owner loses 2 life and the controller gains 2.
    driver.cast_at("Doom Blade", &[target]).unwrap();
    settle(&mut driver).unwrap();
    assert_eq!(life(&driver, 1), 18);
    assert_eq!(life(&driver, 0), 22);
}

#[rstest]
fn a_cloaked_card_has_ward_and_the_sacrifice_mode_cloaks(#[values(2, 3, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(table(seats, "Ransom Note", 2, "Serra Angel")).unwrap();
    driver.begin_activation("Ransom Note").unwrap();
    driver
        .answer(Answer::ChooseMode(ModeChoice::Modes(vec![0])))
        .unwrap();
    pay(&mut driver).unwrap();
    settle(&mut driver).unwrap();
    let made = face_down(&driver);
    assert_eq!(made.len(), 1);
    let object = driver.state().object(made[0]).unwrap();
    assert_eq!(object.face_down(), Some(FaceDown::Cloak));
    let keywords = keywords_of(driver.state(), made[0]);
    assert!(
        keywords
            .iter()
            .all(|keyword| matches!(keyword, Keyword::Ward(_)))
    );
    assert_eq!(keywords.len(), 1, "ward {{2}} and nothing else");
    assert!(
        driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Ransom Note"),
        "the artifact was sacrificed"
    );
}
