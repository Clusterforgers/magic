//! Goal: the object view of a card with several faces names the card as a whole in a hand (the front face, or both halves of a split card) and the face in use on the stack and the battlefield, for every seat that sees it, at two to four seats; the viewer who does not own the card in hand sees no object for it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::zones::Zone;
use mtg_protocol::ObjectView;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, PayChoice};

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn table(seats: usize, hand: &str) -> Driver {
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 6])
        .hand_named(0, [hand])
        .battlefield(0, (0..5).map(|_| ready(mountain())).collect::<Vec<_>>());
    for other in 1..seats {
        scenario = scenario.library(other, vec![forest(); 6]);
    }
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
#[case::split("Fire // Ice", "Fire // Ice")]
#[case::adventure("Bonecrusher Giant // Stomp", "Bonecrusher Giant")]
#[case::modal("Selfless Glyphweaver // Deadly Vanity", "Selfless Glyphweaver")]
fn a_card_in_a_hand_is_named_as_a_whole_to_its_owner_alone(
    #[values(2, 3, 4)] seats: usize,
    #[case] card: &str,
    #[case] name: &str,
) {
    let driver = table(seats, card);
    let id = driver.state().zone(Zone::Hand(seat(0)))[0];
    let own = ObjectView::of(driver.state(), seat(0), id).unwrap();
    assert_eq!((own.name.as_str(), own.face), (name, None));
    for other in 1..seats {
        assert!(ObjectView::of(driver.state(), seat(other), id).is_none());
    }
}

#[rstest]
fn a_spell_cast_as_a_face_is_that_face_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, "Bonecrusher Giant // Stomp");
    driver.begin_cast("Bonecrusher Giant").unwrap();
    // The faces are asked in order: the creature, then the Adventure.
    driver
        .answer(Answer::ChooseOption(OptionChoice(1)))
        .unwrap();
    let target = driver.target_player(1);
    driver.choose_targets(&[target]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let spell = driver.state().zone(Zone::Stack)[0];
    for viewer in 0..seats {
        let view = ObjectView::of(driver.state(), seat(viewer), spell).unwrap();
        assert_eq!((view.name.as_str(), view.face), ("Stomp", Some(1)));
        assert_eq!(view.mana_cost.unwrap().to_string(), "{1}{R}");
    }
}
