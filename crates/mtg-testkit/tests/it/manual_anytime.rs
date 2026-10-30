//! Goal: a seat in the game takes every kind of manual action out of band at every kind of moment (its own main phase with an empty stack, the opponent's priority, a spell on the stack, another seat's prompt, combat), each logged first by an event that names the seat, with the pending decision recomputed and never lost and a rebuilt engine in the same state; state-based actions follow manual damage and life; a decision whose options the change alters is asked again with the options of the moment; refusals leave the engine as it was; and bot games with out-of-band actions at random points keep every invariant and replay to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::combat::DamageTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::manual::{
    CommonToken, ManualAction, ManualRefusal, ManualRoll, ManualToken, ManualZone,
};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, characteristics};
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{
    Driver, PlayOptions, Scenario, ScriptError, manual_fuzz_setup, play_game_with, ready,
};
use rstest::rstest;

use crate::common::seeds::fuzz_games;
use crate::common::{name_of, pid};

/// The moments at which a manual action is taken.
#[derive(Debug, Clone, Copy)]
enum Moment {
    /// Seat 0 in its own main phase with an empty stack; seat 0 acts.
    OwnMain,
    /// Seat 0 holds priority in its turn; seat 1 acts.
    OpponentsPriority,
    /// Seat 0's Bolt is on the stack; seat 1 acts.
    SpellOnStack,
    /// Seat 0 is choosing the target of its Bolt; seat 1 acts.
    OthersPrompt,
    /// Seat 0 is declaring attackers; seat 1 acts.
    Combat,
}

fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

/// Seat 0 holds priority in its first main phase; each seat has a creature, a card in hand and a library.
fn table() -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3), free_creature("Spare", 1, 1)])
            .hand(1, [free_creature("Foe card", 1, 1)])
            .library(0, vec![free_creature("Deep", 1, 1); 4])
            .library(1, vec![free_creature("Far", 1, 1); 4])
            .battlefield(
                0,
                [
                    ready(free_creature("Mine", 2, 2)),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
            .battlefield(1, [ready(free_creature("Theirs", 3, 3))]),
    )
    .unwrap()
}

/// The table at `moment` and the seat that acts.
fn at(moment: Moment) -> (Driver, PlayerId) {
    let mut driver = table();
    let actor = match moment {
        Moment::OwnMain => pid(0),
        Moment::OpponentsPriority => pid(1),
        Moment::SpellOnStack => {
            let face = driver.target_player(1);
            driver.cast_at("Bolt", &[face]).unwrap();
            assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
            pid(1)
        }
        Moment::OthersPrompt => {
            driver.begin_cast("Bolt").unwrap();
            assert!(matches!(
                driver.decision().unwrap().kind,
                DecisionKind::ChooseTargets(_)
            ));
            pid(1)
        }
        Moment::Combat => {
            driver.pass_until(Step::BeginCombat).unwrap();
            while matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)) {
                driver.pass().unwrap();
            }
            assert!(matches!(
                driver.decision().unwrap().kind,
                DecisionKind::DeclareAttackers(_)
            ));
            pid(1)
        }
    };
    (driver, actor)
}

/// One action of each kind for `actor`, named for the table.
fn actions(driver: &Driver, actor: PlayerId) -> Vec<(&'static str, ManualAction)> {
    let mut all = rolls();
    let (own_card, own_creature) = if actor == pid(0) {
        ("Spare", "Mine")
    } else {
        ("Foe card", "Theirs")
    };
    let hand = find(driver, Zone::Hand(actor), own_card);
    let creature = find(driver, Zone::Battlefield, own_creature);
    let theirs = find(driver, Zone::Battlefield, "Theirs");
    let other = PlayerId(1 - actor.0);
    all.extend([
        ("draw", ManualAction::Draw { count: 1 }),
        (
            "mill",
            ManualAction::MoveTop {
                count: 2,
                to: ManualZone::Graveyard,
            },
        ),
        ("discard", ManualAction::Discard { object: hand }),
        (
            "move",
            ManualAction::Move {
                object: hand,
                to: ManualZone::Exile,
            },
        ),
        (
            "lose life",
            ManualAction::Life {
                player: actor,
                delta: -2,
            },
        ),
        (
            "gain life",
            ManualAction::Life {
                player: actor,
                delta: 3,
            },
        ),
        (
            "poison",
            ManualAction::PlayerCounters {
                player: actor,
                counter: CounterKind::Poison,
                delta: 1,
            },
        ),
        (
            "counters",
            ManualAction::Counters {
                object: theirs,
                counter: CounterKind::PlusOnePlusOne,
                delta: 1,
            },
        ),
        ("tap", ManualAction::Tap { object: theirs }),
        (
            "token",
            ManualAction::CreateToken {
                token: ManualToken::Common(CommonToken::ALL[0]),
                count: 2,
            },
        ),
        (
            "damage a player",
            ManualAction::Damage {
                source: creature,
                target: DamageTarget::Player(other),
                amount: 2,
            },
        ),
        (
            "damage a creature",
            ManualAction::Damage {
                source: creature,
                target: DamageTarget::Object(theirs),
                amount: 1,
            },
        ),
        (
            "mana",
            ManualAction::AddMana {
                mana: ManaType::Red,
                count: 2,
            },
        ),
        (
            "pump",
            ManualAction::Pump {
                object: theirs,
                power: 2,
                toughness: 1,
            },
        ),
    ]);
    all
}

/// A coin and a die, which any seat takes at any moment.
fn rolls() -> Vec<(&'static str, ManualAction)> {
    let roll = |kind| ManualAction::Roll { kind };
    vec![
        ("flip a coin", roll(ManualRoll::Coin {})),
        ("roll a die", roll(ManualRoll::Die { sides: 20 })),
    ]
}

/// The engine rebuilt from the setup and the log is in the live engine's state.
fn replays(driver: &Driver) {
    let engine = driver.engine();
    let rebuilt = Engine::resume(engine.setup().clone(), engine.events().to_vec()).unwrap();
    assert_eq!(rebuilt.state_hash(), engine.state_hash());
}

#[rstest]
fn every_action_is_taken_at_every_moment_and_replays(
    #[values(
        Moment::OwnMain,
        Moment::OpponentsPriority,
        Moment::SpellOnStack,
        Moment::OthersPrompt,
        Moment::Combat
    )]
    moment: Moment,
) {
    let (probe, actor) = at(moment);
    for (name, action) in actions(&probe, actor) {
        let (mut driver, _) = at(moment);
        let before = driver.decision().unwrap().clone();
        let events = driver
            .manual_by(actor, &action)
            .unwrap_or_else(|error| panic!("{name} at {moment:?}: {error}"));
        assert_eq!(
            events.first(),
            Some(&Event::ManualAction {
                player: actor,
                action: Box::new(action.clone())
            }),
            "{name} at {moment:?}"
        );
        let after = driver.decision().unwrap();
        assert_eq!(after.player, before.player, "{name} at {moment:?}");
        assert_eq!(
            std::mem::discriminant(&after.kind),
            std::mem::discriminant(&before.kind),
            "{name} at {moment:?}"
        );
        replays(&driver);
    }
}

#[test]
fn a_cast_in_progress_keeps_its_answers_while_another_seat_acts() {
    let (mut driver, actor) = at(Moment::OthersPrompt);
    driver
        .manual_by(actor, &ManualAction::Draw { count: 1 })
        .unwrap();
    let face = driver.target_player(1);
    driver.choose_targets(&[face]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    assert_eq!(driver.state().zone(Zone::Stack).len(), 1);
}

#[test]
fn lethal_damage_by_hand_kills_a_creature_and_zero_life_loses_the_game() {
    let mut driver = table();
    let mine = find(&driver, Zone::Battlefield, "Mine");
    let theirs = find(&driver, Zone::Battlefield, "Theirs");
    let hit = ManualAction::Damage {
        source: mine,
        target: DamageTarget::Object(theirs),
        amount: 3,
    };
    driver
        .manual_by(pid(1), &ManualAction::Draw { count: 1 })
        .unwrap();
    driver.manual_by(pid(0), &hit).unwrap();
    assert!(!driver.state().zone(Zone::Battlefield).contains(&theirs));
    let face = ManualAction::Damage {
        source: mine,
        target: DamageTarget::Player(pid(1)),
        amount: 20,
    };
    driver.manual_by(pid(0), &face).unwrap();
    assert!(!driver.state().is_alive(pid(1)));
    assert!(driver.engine().result().is_some());
    replays(&driver);
}

#[test]
fn losing_all_life_by_hand_on_the_opponents_turn_loses_the_game() {
    let mut driver = table();
    driver
        .manual_by(
            pid(1),
            &ManualAction::Life {
                player: pid(1),
                delta: -20,
            },
        )
        .unwrap();
    assert!(!driver.state().is_alive(pid(1)));
    assert!(driver.engine().result().is_some());
}

#[test]
fn a_pump_lasts_until_end_of_turn() {
    let mut driver = table();
    let theirs = find(&driver, Zone::Battlefield, "Theirs");
    driver
        .manual_by(
            pid(1),
            &ManualAction::Pump {
                object: theirs,
                power: 2,
                toughness: 2,
            },
        )
        .unwrap();
    let pumped = characteristics(driver.state(), theirs).unwrap();
    assert_eq!((pumped.power, pumped.toughness), (Some(5), Some(5)));
}

#[test]
fn a_payment_whose_source_was_tapped_by_hand_is_asked_again_without_it() {
    let mut driver = table();
    let land = find(&driver, Zone::Battlefield, "Mountain");
    driver.begin_cast("Bolt").unwrap();
    let face = driver.target_player(1);
    driver.choose_targets(&[face]).unwrap();
    let offered = |driver: &Driver| {
        driver
            .pay_options()
            .unwrap()
            .sources
            .iter()
            .any(|s| s.source.object == land)
    };
    assert!(offered(&driver));
    driver
        .manual_by(pid(0), &ManualAction::Tap { object: land })
        .unwrap();
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::PayMana(_)
    ));
    assert!(!offered(&driver));
}

#[test]
fn a_target_moved_by_hand_is_no_longer_offered() {
    let (mut driver, _) = at(Moment::OthersPrompt);
    let theirs = find(&driver, Zone::Battlefield, "Theirs");
    driver
        .manual_by(
            pid(1),
            &ManualAction::Move {
                object: theirs,
                to: ManualZone::Hand,
            },
        )
        .unwrap();
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("the target prompt is gone");
    };
    assert!(!format!("{options:?}").contains(&format!("{theirs:?}")));
}

fn refused(result: Result<Vec<Event>, ScriptError>) -> ManualRefusal {
    match result {
        Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(refusal)))) => refusal,
        other => panic!("not refused by the manual check: {other:?}"),
    }
}

#[test]
fn refusals_leave_the_engine_as_it_was() {
    let mut driver = table();
    let mine = find(&driver, Zone::Battlefield, "Mine");
    let theirs = find(&driver, Zone::Battlefield, "Theirs");
    let hidden = find(&driver, Zone::Hand(pid(1)), "Foe card");
    let land = find(&driver, Zone::Battlefield, "Mountain");
    let hash = driver.engine().state_hash();
    let length = driver.engine().events().len();
    let cases = [
        (
            ManualAction::Life {
                player: pid(0),
                delta: -1,
            },
            ManualRefusal::NotYours,
        ),
        (
            ManualAction::Damage {
                source: hidden,
                target: DamageTarget::Player(pid(0)),
                amount: 1,
            },
            ManualRefusal::WrongZone {
                object: hidden,
                zone: Zone::Hand(pid(1)),
            },
        ),
        (
            ManualAction::Damage {
                source: mine,
                target: DamageTarget::Player(pid(0)),
                amount: 1,
            },
            ManualRefusal::NotYours,
        ),
        (
            ManualAction::Damage {
                source: theirs,
                target: DamageTarget::Object(land),
                amount: 1,
            },
            ManualRefusal::NotDamageable,
        ),
        (
            ManualAction::Damage {
                source: theirs,
                target: DamageTarget::Object(mine),
                amount: 0,
            },
            ManualRefusal::OutOfRange("the damage"),
        ),
        (
            ManualAction::AddMana {
                mana: ManaType::Green,
                count: 21,
            },
            ManualRefusal::OutOfRange("the amount of mana"),
        ),
        (
            ManualAction::Pump {
                object: land,
                power: 1,
                toughness: 1,
            },
            ManualRefusal::NotACreature(land),
        ),
        (
            ManualAction::Pump {
                object: mine,
                power: 0,
                toughness: 0,
            },
            ManualRefusal::NoChange,
        ),
    ];
    for (action, refusal) in cases {
        assert_eq!(
            refused(driver.manual_by(pid(1), &action)),
            refusal,
            "{action:?}"
        );
        assert_eq!(driver.engine().state_hash(), hash);
        assert_eq!(driver.engine().events().len(), length);
    }
}

#[test]
fn a_seat_out_of_the_game_and_a_finished_game_refuse() {
    let mut driver = Driver::from_scenario(Scenario::new(3)).unwrap();
    driver.concede(pid(2)).unwrap();
    let draw = ManualAction::Draw { count: 1 };
    assert!(matches!(
        driver.manual_by(pid(2), &draw),
        Err(ScriptError::Engine(EngineError::NotInGame(_)))
    ));
    driver.concede(pid(1)).unwrap();
    assert!(matches!(
        driver.manual_by(pid(0), &draw),
        Err(ScriptError::Engine(EngineError::NoDecision))
    ));
}

#[rstest]
fn bot_games_with_out_of_band_actions_keep_every_invariant_and_replay(
    #[values(2, 3, 4)] players: usize,
) {
    let mut taken = 0;
    for seed in 0..fuzz_games(12) {
        let setup = manual_fuzz_setup(players, seed).unwrap();
        let options = PlayOptions {
            manual_anytime_every: 6,
            ..PlayOptions::default()
        };
        let outcome = play_game_with(setup.clone(), seed, options)
            .unwrap_or_else(|failure| panic!("{failure}"));
        taken += outcome.manual_actions;
        let rebuilt = Engine::resume(setup, outcome.events.clone()).unwrap();
        assert_eq!(rebuilt.state_hash(), outcome.final_hash, "seed {seed}");
    }
    assert!(taken > 0, "no out-of-band action was taken");
}
