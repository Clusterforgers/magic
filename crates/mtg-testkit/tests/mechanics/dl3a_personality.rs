//! Goal: a boss's personality decides whom it eyes among the heroes the cap leaves it: a Spiteful boss the hero who
//! gained the most life, a Brutish one the biggest board of creatures, a Cunning one the most noncreature permanents and
//! cards in hand, a Watchful one the plain presence; at every party size, while a capped hero stays spared and an
//! overwhelming threat reads the plain presence.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::house::HouseRules;
use mtg_core::personality::Personality;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::GameState;
use mtg_engine::events::Event;
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::spotlight;
use mtg_testkit::cards::{bear, free_creature, walker};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::spotlight::emblem;

/// What a hero brings to the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hero {
    /// A 2/2 and a 6/6.
    Board,
    /// A 2/2, three planeswalkers and three cards in hand.
    Combo,
    /// A 2/2 and a planeswalker; gains 6 life.
    Healer,
    /// A 2/2.
    Plain,
}

use Hero::{Board, Combo, Healer, Plain};

/// A fight with `heroes` in seat order, then the boss with `personality`, its Bear dealing `hits` damage to each named
/// hero.
fn fight(heroes: &[Hero], personality: Personality, hits: &[(usize, u32)]) -> GameState {
    let boss = heroes.len();
    let mut seats = vec![0; heroes.len()];
    seats.push(1);
    let house = HouseRules {
        teams: Some(Teams {
            mode: TeamMode::Teams,
            seats,
        }),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(boss + 1).house_rules(house);
    for (seat, hero) in heroes.iter().enumerate() {
        let mut board = vec![ready(free_creature(&format!("Bear {seat}"), 2, 2))];
        let walkers = match hero {
            Combo => 3,
            Healer => 1,
            Board | Plain => 0,
        };
        board.extend((0..walkers).map(|at| ready(walker(&format!("Walker {seat}.{at}"), 1))));
        if *hero == Board {
            board.push(ready(free_creature(&format!("Giant {seat}"), 6, 6)));
        }
        scenario = scenario
            .battlefield(seat, board)
            .life(seat, 30)
            .spirit(seat, emblem("Haunt", &[]));
        if *hero == Combo {
            scenario = scenario.hand(seat, (0..3).map(|_| bear()));
        }
    }
    let phase = BossPhase {
        begins: PhaseStart::AtLife(20),
        emblem: emblem("Wrath", &[]),
    };
    scenario = scenario
        .battlefield(boss, [ready(free_creature(&format!("Bear {boss}"), 2, 2))])
        .life(boss, 20)
        .phases(boss, [phase])
        .personality(boss, personality);
    let driver = Driver::from_scenario(scenario).unwrap();
    let source = driver.id_of(&format!("Bear {boss}")).unwrap();
    let mut state = driver.state().clone();
    for (seat, hero) in heroes.iter().enumerate() {
        if *hero == Healer {
            let gain = Event::LifeGained {
                player: pid(seat),
                amount: 6,
            };
            mtg_engine::apply_event(&mut state, &gain).unwrap();
        }
    }
    for (seat, amount) in hits {
        let hit = Event::DamageDealt {
            source,
            target: mtg_engine::combat::DamageTarget::Player(pid(*seat)),
            amount: *amount,
            deathtouch: false,
            form: mtg_engine::combat::DamageForm::default(),
            toxic: 0,
            unpreventable: false,
        };
        mtg_engine::apply_event(&mut state, &hit).unwrap();
    }
    state
}

/// The kind of hero the boss eyes.
fn eyed(heroes: &[Hero], state: &GameState) -> Hero {
    let boss = heroes.len();
    heroes[spotlight::focus(state, pid(boss)).unwrap().index()]
}

#[rstest]
#[case::two_heroes(&[Board, Healer], [Board, Healer, Board, Board])]
#[case::three_heroes(&[Board, Combo, Healer], [Combo, Healer, Board, Combo])]
#[case::four_heroes(&[Healer, Plain, Combo, Board], [Combo, Healer, Board, Combo])]
#[case::six_heroes(&[Plain, Board, Plain, Healer, Combo, Plain], [Combo, Healer, Board, Combo])]
fn each_personality_eyes_its_own_threat(#[case] heroes: &[Hero], #[case] expected: [Hero; 4]) {
    for (personality, want) in Personality::ALL.into_iter().zip(expected) {
        let state = fight(heroes, personality, &[]);
        assert_eq!(spotlight::personality(&state), personality);
        assert_eq!(
            eyed(heroes, &state),
            want,
            "{personality:?} with {heroes:?}"
        );
    }
}

#[rstest]
#[case::spiteful_spares_the_capped_healer(Personality::Spiteful, Healer, Combo)]
#[case::brutish_spares_the_capped_board(Personality::Brutish, Board, Combo)]
#[case::cunning_spares_the_capped_combo(Personality::Cunning, Combo, Board)]
fn the_cap_holds_whatever_the_personality(
    #[case] personality: Personality,
    #[case] capped: Hero,
    #[case] next: Hero,
) {
    let heroes = [Board, Combo, Healer];
    let at = heroes.iter().position(|h| *h == capped).unwrap();
    let state = fight(&heroes, personality, &[(at, 6)]);
    assert!(spotlight::capped(&state, pid(at)));
    assert_eq!(eyed(&heroes, &state), next, "{personality:?}");
    let ranking = spotlight::ranking(&state);
    assert_eq!(ranking.last(), Some(&pid(at)), "the capped hero comes last");
}

#[rstest]
#[case::watchful(Personality::Watchful)]
#[case::spiteful(Personality::Spiteful)]
#[case::brutish(Personality::Brutish)]
#[case::cunning(Personality::Cunning)]
fn the_threat_weighs_the_table_and_the_presence_stays_plain(#[case] personality: Personality) {
    let heroes = [Board, Combo, Healer];
    let state = fight(&heroes, personality, &[]);
    let w = personality.weights();
    // Board: 8 power; Combo: 2 power, 3 walkers, 3 cards; Healer: 2 power, 1 walker, 6 life gained.
    let tables = [(8, 0, 0, 0), (2, 3, 3, 0), (2, 1, 0, 6)];
    for (seat, (power, permanents, hand, gained)) in tables.into_iter().enumerate() {
        let want = w.power * power + w.permanent * permanents + w.hand * hand + w.gained * gained;
        assert_eq!(
            spotlight::threat(&state, pid(seat)),
            want,
            "{personality:?} seat {seat}"
        );
        assert_eq!(
            spotlight::presence(&state, pid(seat)),
            power + 2 * permanents + hand
        );
        assert_eq!(
            spotlight::gained(&state, pid(seat)),
            u32::try_from(gained).unwrap()
        );
    }
}
