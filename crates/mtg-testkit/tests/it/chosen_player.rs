//! Goal: a permanent that chooses a player as it enters (True-Name Nemesis, Sewer Nemesis, Stuffy Doll) asks its controller for one of the players in turn order, keeps the choice as a public `Chosen::Player`, and plays on at tables of two, three and four seats: Sewer Nemesis counts the chosen player's graveyard and mills them when they cast a spell, Stuffy Doll hurts the chosen player for the damage it takes, and the game goes on once the chosen player has left it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::Event;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, instant, island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const NAME: &str = "True-Name Nemesis";

/// Seat 0 holds `name` and the lands `lands`; seat 1 holds a Cantrip and an Island; every seat has a library.
fn table(seats: usize, name: &str, lands: &[&str], buried: usize) -> Driver {
    let land = |kind: &&str| match *kind {
        "Island" => ready(island()),
        "Swamp" => ready(swamp()),
        _ => ready(plains()),
    };
    let cantrip: Arc<CardDefinition> = instant("Cantrip", 1, &[]);
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(name).unwrap()])
        .battlefield(0, lands.iter().map(land).collect::<Vec<_>>())
        .hand(1, [cantrip])
        .battlefield(1, [ready(island()), ready(free_creature("Wall", 0, 5))])
        .graveyard(1, (0..buried).map(|_| plains()));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Casts `name` with seat 0's lands and answers the choice of a player with the `pick`th option offered.
fn entered(seats: usize, name: &str, lands: &[&str], pick: usize) -> (Driver, Vec<ChoiceOption>) {
    let buried = usize::from(name == "Sewer Nemesis");
    let mut driver = table(seats, name, lands, buried);
    driver.cast_tapping(name, lands).unwrap();
    // Every seat passes with the spell on the stack, then it resolves and the choice is asked.
    for _ in 0..seats {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseOption(_)
        ) {
            break;
        }
        driver.pass().unwrap();
    }
    let decision = driver.decision().unwrap().clone();
    let DecisionKind::ChooseOption(options) = &decision.kind else {
        panic!(
            "a player is chosen as {name} enters, not {:?}",
            decision.kind
        );
    };
    assert_eq!(decision.player, pid(0));
    driver
        .answer(Answer::ChooseOption(OptionChoice(pick)))
        .unwrap();
    (driver, options.options.clone())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn the_players_in_turn_order_are_offered_and_the_pick_is_kept(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] pick: usize,
) {
    let (driver, offered) = entered(
        seats,
        NAME,
        &["Island", "Island", "Swamp"],
        pick.min(seats - 1),
    );
    let everyone: Vec<ChoiceOption> = (0..seats)
        .map(|seat| ChoiceOption::Player(pid(seat)))
        .collect();
    assert_eq!(offered, everyone);
    let id = driver.id_of(NAME).unwrap();
    let kept = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned();
    assert_eq!(kept, Some(Chosen::Player(pid(pick.min(seats - 1)))));
}

#[rstest]
fn a_chosen_player_who_leaves_the_game_leaves_the_card_choosing_nobody(
    #[values(2, 3, 4)] seats: usize,
) {
    let (mut driver, _) = entered(seats, NAME, &["Island", "Island", "Swamp"], 1);
    driver.concede(pid(1)).unwrap();
    assert!(!driver.state().is_alive(pid(1)));
    // The game goes on: the card is still there and its controller still has priority or the game is over.
    assert!(driver.state().object(driver.id_of(NAME).unwrap()).is_some());
}

#[rstest]
fn sewer_nemesis_counts_the_chosen_players_graveyard_and_mills_them_when_they_cast_a_spell(
    #[values(2, 3, 4)] seats: usize,
) {
    let (mut driver, _) = entered(seats, "Sewer Nemesis", &["Swamp"; 4], 1);
    let id = driver.id_of("Sewer Nemesis").unwrap();
    let power = |driver: &Driver| characteristics(driver.state(), id).unwrap().power;
    let library = |driver: &Driver| driver.state().zone(Zone::Library(pid(1))).len();
    let graveyard = |driver: &Driver| driver.state().zone(Zone::Graveyard(pid(1))).len();
    assert_eq!((power(&driver), graveyard(&driver)), (Some(1), 1));
    let before = library(&driver);
    // Seat 0 passes with an empty stack and the chosen player casts a spell.
    driver.pass().unwrap();
    driver.cast_tapping("Cantrip", &["Island"]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(
        library(&driver),
        before - 1,
        "the chosen player mills a card"
    );
    assert_eq!(
        graveyard(&driver),
        3,
        "the buried card, the milled card and the Cantrip"
    );
    assert_eq!(
        power(&driver),
        Some(3),
        "one for each card in the graveyard"
    );
    // A spell of the controller's own is nobody's concern: the trigger watches the chosen player only.
    assert_eq!(life(&driver, 0), 20);
}

#[rstest]
fn stuffy_doll_deals_the_damage_it_takes_to_the_chosen_player(#[values(2, 3, 4)] seats: usize) {
    let (mut driver, _) = entered(seats, "Stuffy Doll", &["Plains"; 5], 1);
    // The Doll is no longer summoning sick on the next turn of its controller.
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Main1).unwrap();
    let before = life(&driver, 1);
    driver.begin_activation("Stuffy Doll").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(
        life(&driver, 1),
        before - 1,
        "1 damage to itself is 1 to the chosen player"
    );
    let doll = driver.id_of("Stuffy Doll").unwrap();
    assert_eq!(
        driver.state().object(doll).unwrap().zone(),
        Zone::Battlefield
    );
    for other in (0..seats).filter(|seat| *seat != 1) {
        assert_eq!(
            life(&driver, other),
            20,
            "seat {other} is not the chosen player"
        );
    }
}

#[rstest]
fn a_creature_of_the_chosen_player_cannot_block_the_creature_with_protection_from_them(
    #[values(2, 3, 4)] seats: usize,
) {
    let (mut driver, _) = entered(seats, NAME, &["Island", "Island", "Swamp"], 1);
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::DeclareAttackers).unwrap();
    driver.attack(&[(NAME, 1)]).unwrap();
    let from = driver.engine().events().len();
    // The Wall could block a creature without the protection, but here nobody is asked to block at all.
    driver.idle_until(turn, Step::Main2).unwrap();
    let events = driver.engine().events()[from..].to_vec();
    assert!(
        !events.iter().any(
            |event| matches!(event, Event::BlockersDeclared { blocks, .. } if !blocks.is_empty())
        ),
        "the chosen player has no creature that can block it"
    );
    let id = driver.id_of(NAME).unwrap();
    let power = characteristics(driver.state(), id).unwrap().power.unwrap();
    assert_eq!(life(&driver, 1), 20 - power, "it was unblocked");
}
