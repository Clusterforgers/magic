//! Goal: goading plays as CR 701.15 says at tables of two, three and four seats: a goaded creature must attack each combat and attacks a player other than the goader when it can (the goader only when it is the one player left to attack), a spell goads the creatures it names until the goader's next turn and an Aura for as long as it is attached, a creature that no one goads stays free, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::common::{name_of, pid};
use mtg_core::ids::PlayerId;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

/// One declaration of attackers: the turn, who declared, the creatures a rule required and where each creature may attack.
struct Declaration {
    turn: u32,
    player: PlayerId,
    required: Vec<String>,
    defenders: Vec<(String, Vec<AttackTarget>)>,
}

impl Declaration {
    /// Where the creature called `name` may attack.
    fn defenders_of(&self, name: &str) -> &[AttackTarget] {
        &self
            .defenders
            .iter()
            .find(|(creature, _)| creature == name)
            .unwrap()
            .1
    }
}

/// Plays on through `last` turn, declaring what the rules require and nothing more, and returns every declaration of
/// attackers that was asked for.
fn play_through(driver: &mut Driver, last: u32) -> Result<Vec<Declaration>, ScriptError> {
    let mut seen = Vec::new();
    for _ in 0..2_000 {
        if driver.state().turn().number() > last {
            return Ok(seen);
        }
        let decision = driver.decision()?.clone();
        if let DecisionKind::DeclareAttackers(options) = &decision.kind {
            let state = driver.state();
            seen.push(Declaration {
                turn: state.turn().number(),
                player: decision.player,
                required: options
                    .required
                    .iter()
                    .map(|id| name_of(state, *id))
                    .collect(),
                defenders: options
                    .attackers
                    .iter()
                    .map(|a| (name_of(state, a.object), a.defenders.clone()))
                    .collect(),
            });
            driver.answer(Answer::DeclareAttackers(options.required_attack()))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the game did not reach the turn".into()))
}

/// Every seat but the first has a Bear of its own; seat 0 has `extra` and four Mountains.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature(&format!("Bear{seat}"), 2, 2))]);
    }
    scenario
}

/// Seat 0 casts Disrupt Decorum and lets it resolve.
fn cast_decorum(seats: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        table(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, ["Disrupt Decorum"])
            .battlefield(0, lands(4, mountain)),
    )?;
    driver.cast("Disrupt Decorum")?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(driver)
}

/// The targets in a fixed order.
fn sorted(targets: &[AttackTarget]) -> Vec<AttackTarget> {
    let mut targets = targets.to_vec();
    targets.sort();
    targets
}

fn assert_replays(driver: &Driver) {
    let state: GameState = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Where the goaded Bear of the player in `seat` may attack at a table of `seats`: the players other than its owner and
/// the goader, or the goader alone when no one else is left.
fn expected_defenders(seats: usize, seat: usize) -> Vec<AttackTarget> {
    if seats == 2 {
        return vec![AttackTarget::Player(pid(0))];
    }
    (1..seats)
        .filter(|other| *other != seat)
        .map(|other| AttackTarget::Player(pid(other)))
        .collect()
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_spell_goads_every_creature_of_the_opponents_until_the_goaders_next_turn(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = cast_decorum(seats)?;
    let turns = u32::try_from(seats).unwrap();
    let seen = play_through(&mut driver, 2 * turns)?;
    for declaration in seen.iter().filter(|d| d.player != pid(0)) {
        let seat = declaration.player.index();
        let bear = format!("Bear{seat}");
        let goaded = declaration.turn <= turns;
        assert_eq!(
            declaration.required,
            if goaded {
                vec![bear.clone()]
            } else {
                Vec::new()
            },
            "turn {}: a goaded creature must attack until the goader's next turn only",
            declaration.turn
        );
        let defenders = declaration.defenders_of(&bear);
        if goaded {
            assert_eq!(
                sorted(defenders),
                expected_defenders(seats, seat),
                "turn {}",
                declaration.turn
            );
        } else {
            assert!(
                defenders.contains(&AttackTarget::Player(pid(0))),
                "a creature no one goads may attack the old goader"
            );
        }
    }
    assert_eq!(
        seen.iter().filter(|d| d.player != pid(0)).count(),
        2 * (seats - 1),
        "every opponent declared in both rounds"
    );
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn an_aura_goads_the_enchanted_creature_for_as_long_as_it_is_attached(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let aura = ready(card("Psychic Impetus").unwrap());
    let mut driver = Driver::from_scenario(table(seats).battlefield(0, [attached_to(aura, 1, 0)]))?;
    let turns = u32::try_from(seats).unwrap();
    let seen = play_through(&mut driver, 2 * turns)?;
    for declaration in seen.iter().filter(|d| d.player != pid(0)) {
        let seat = declaration.player.index();
        let bear = format!("Bear{seat}");
        let goaded = seat == 1;
        assert_eq!(
            declaration.required,
            if goaded {
                vec![bear.clone()]
            } else {
                Vec::new()
            },
            "turn {}: only the enchanted creature is goaded, every turn",
            declaration.turn
        );
        if goaded {
            assert_eq!(
                sorted(declaration.defenders_of(&bear)),
                expected_defenders(seats, seat)
            );
        }
    }
    assert_eq!(
        seen.iter().filter(|d| d.player == pid(1)).count(),
        2,
        "the enchanted creature declared in both rounds"
    );
    assert_replays(&driver);
    Ok(())
}
