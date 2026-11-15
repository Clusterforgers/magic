//! Goal: at tables of two and four seats any seat carries out the effect of a card by hand on any seat's cards and on any seat in the game (it moves another seat's permanent, graveyard and exile cards to every zone of their owner, mills a library, changes any seat's life and counters, deals damage from, attaches, turns over and takes control of another seat's permanents), each logged under the acting seat; a card in another seat's hand or library and a foretold card in exile are never named, and a refused action changes nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::characteristics::controller_of;
use mtg_engine::combat::DamageTarget;
use mtg_engine::decision::{Answer, PriorityAction};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::manual::{ManualAction, ManualRefusal, ManualReveal, ManualZone};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::manual_cards::manual_equipment;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

const DELVER: &str = "Delver of Secrets // Insectile Aberration";

/// Seat 0 holds priority in its first main phase; every seat has a permanent, an Equipment, a card in hand, one in the
/// graveyard and a library of four.
fn table(seats: usize) -> Driver {
    Driver::from_scenario(scenario(seats)).unwrap()
}

fn scenario(seats: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario
            .hand(seat, [free_creature(&format!("Held {seat}"), 1, 1)])
            .library(seat, vec![free_creature(&format!("Deep {seat}"), 1, 1); 4])
            .graveyard(seat, [free_creature(&format!("Dead {seat}"), 1, 1)])
            .battlefield(
                seat,
                [
                    ready(free_creature(&format!("Perm {seat}"), 2, 2)),
                    ready(manual_equipment(&format!("Blade {seat}"))),
                ],
            )
    })
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

fn zone(driver: &Driver, id: ObjectId) -> Zone {
    driver.state().object(id).unwrap().zone()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn poison(driver: &Driver, seat: usize) -> u32 {
    driver
        .state()
        .player(pid(seat))
        .unwrap()
        .counters_of(&CounterKind::Poison)
}

/// Takes the action as seat 0 and checks that the log names the acting seat first and that a rebuilt engine agrees.
fn act(driver: &mut Driver, action: ManualAction) -> Vec<Event> {
    let expected = action.clone();
    let events = driver.manual(action).unwrap();
    assert!(
        matches!(
            events.first(),
            Some(Event::ManualAction { player, action: logged })
                if *player == pid(0) && **logged == expected
        ),
        "{events:?}"
    );
    let rebuilt = Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
    events
}

/// Takes the action as seat 0 and returns why it was refused, after checking that nothing changed.
fn refused(driver: &mut Driver, action: ManualAction) -> ManualRefusal {
    let (log, hash) = (driver.engine().events().len(), driver.state().state_hash());
    let error = driver.manual(action).unwrap_err();
    let ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(refusal))) = error else {
        panic!("not a manual refusal: {error:?}");
    };
    assert_eq!(driver.engine().events().len(), log);
    assert_eq!(driver.state().state_hash(), hash);
    refusal
}

const fn mv(object: ObjectId, to: ManualZone) -> ManualAction {
    ManualAction::Move { object, to }
}

#[rstest]
fn a_seat_moves_another_seats_permanent_to_every_zone(
    #[values(2, 4)] seats: usize,
    #[values(
        ManualZone::Hand,
        ManualZone::Graveyard,
        ManualZone::Exile,
        ManualZone::LibraryTop,
        ManualZone::LibraryBottom
    )]
    to: ManualZone,
) {
    let mut driver = table(seats);
    for foe in 1..seats {
        let object = find(&driver, Zone::Battlefield, &format!("Perm {foe}"));
        let events = act(&mut driver, mv(object, to));
        assert!(events.iter().any(|e| matches!(
            e,
            Event::ZoneChanged { object: moved, from: Zone::Battlefield, .. } if *moved == object
        )));
        let library = driver.state().zone(Zone::Library(pid(foe)));
        match to {
            ManualZone::Hand => assert_eq!(zone(&driver, object), Zone::Hand(pid(foe))),
            ManualZone::Graveyard => {
                assert_eq!(zone(&driver, object), Zone::Graveyard(pid(foe)));
            }
            ManualZone::Exile => assert_eq!(zone(&driver, object), Zone::Exile),
            ManualZone::LibraryTop => assert_eq!(library.last(), Some(&object)),
            _ => assert_eq!(library.first(), Some(&object)),
        }
        assert_eq!(driver.state().object(object).unwrap().owner(), pid(foe));
    }
}

#[rstest]
fn a_seat_moves_another_seats_graveyard_and_exile_cards(
    #[values(2, 4)] seats: usize,
    #[values(ManualZone::Battlefield, ManualZone::Hand, ManualZone::Exile)] to: ManualZone,
) {
    let mut driver = table(seats);
    for foe in 1..seats {
        let dead = find(&driver, Zone::Graveyard(pid(foe)), &format!("Dead {foe}"));
        act(&mut driver, mv(dead, to));
        let expected = match to {
            ManualZone::Battlefield => Zone::Battlefield,
            ManualZone::Hand => Zone::Hand(pid(foe)),
            _ => Zone::Exile,
        };
        assert_eq!(zone(&driver, dead), expected);
        // What is in exile or on the battlefield goes on to the owner's graveyard.
        if to != ManualZone::Hand {
            act(&mut driver, mv(dead, ManualZone::Graveyard));
            assert_eq!(zone(&driver, dead), Zone::Graveyard(pid(foe)));
        }
    }
}

#[rstest]
fn a_seat_removes_another_seats_token(#[values(2, 4)] seats: usize) {
    use mtg_engine::manual::{CommonToken, ManualToken};
    let mut driver = table(seats);
    let before = driver.state().zone(Zone::Battlefield).len();
    let token = ManualAction::CreateToken {
        token: ManualToken::Common(CommonToken::Soldier),
        count: 1,
    };
    for foe in 1..seats {
        driver.manual_by(pid(foe), &token).unwrap();
    }
    let made: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(driver.state(), *id) == "Soldier")
        .collect();
    assert_eq!(made.len(), seats - 1);
    for object in made {
        act(&mut driver, mv(object, ManualZone::Graveyard));
        assert!(
            driver
                .state()
                .object(object)
                .is_none_or(|o| o.zone() != Zone::Battlefield)
        );
    }
    assert_eq!(driver.state().zone(Zone::Battlefield).len(), before);
}

#[rstest]
fn a_seat_changes_the_life_and_counters_of_any_seat(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats);
    for seat in 0..seats {
        let events = act(
            &mut driver,
            ManualAction::Life {
                player: pid(seat),
                delta: -3,
            },
        );
        assert!(events.iter().any(|e| matches!(
            e,
            Event::LifeLost { player, amount: 3 } if *player == pid(seat)
        )));
        assert_eq!(life(&driver, seat), 17);
        act(
            &mut driver,
            ManualAction::Life {
                player: pid(seat),
                delta: 5,
            },
        );
        assert_eq!(life(&driver, seat), 22);
        for (delta, now) in [(2, 2), (-1, 1)] {
            act(
                &mut driver,
                ManualAction::PlayerCounters {
                    player: pid(seat),
                    counter: CounterKind::Poison,
                    delta,
                },
            );
            assert_eq!(poison(&driver, seat), now);
        }
        assert_eq!(
            refused(
                &mut driver,
                ManualAction::PlayerCounters {
                    player: pid(seat),
                    counter: CounterKind::Poison,
                    delta: -2,
                }
            ),
            ManualRefusal::NotEnough("counters there")
        );
    }
}

#[rstest]
fn a_seat_that_left_the_game_has_no_life_or_counters_to_change(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats);
    let gone = PlayerId(9);
    for action in [
        ManualAction::Life {
            player: gone,
            delta: 1,
        },
        ManualAction::PlayerCounters {
            player: gone,
            counter: CounterKind::Poison,
            delta: 1,
        },
        ManualAction::MoveTop {
            count: 1,
            to: ManualZone::Graveyard,
            library: Some(gone),
        },
    ] {
        assert_eq!(refused(&mut driver, action), ManualRefusal::NoSuchSeat);
    }
}

#[rstest]
fn a_seat_mills_and_exiles_the_top_of_another_seats_library(
    #[values(2, 4)] seats: usize,
    #[values(ManualZone::Graveyard, ManualZone::Exile)] to: ManualZone,
) {
    let mut driver = table(seats);
    for foe in 1..seats {
        let top: Vec<ObjectId> = driver
            .state()
            .zone(Zone::Library(pid(foe)))
            .iter()
            .rev()
            .take(2)
            .copied()
            .collect();
        act(
            &mut driver,
            ManualAction::MoveTop {
                count: 2,
                to,
                library: Some(pid(foe)),
            },
        );
        assert_eq!(driver.state().zone(Zone::Library(pid(foe))).len(), 2);
        let expected = if to == ManualZone::Exile {
            Zone::Exile
        } else {
            Zone::Graveyard(pid(foe))
        };
        assert!(top.iter().all(|id| zone(&driver, *id) == expected));
        // The seat's own library is untouched.
        assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 4);
    }
}

#[rstest]
fn the_top_cards_of_another_seats_library_put_into_its_hand_stay_unseen(
    #[values(2, 4)] seats: usize,
) {
    let mut driver = table(seats);
    for foe in 1..seats {
        let top = *driver.state().zone(Zone::Library(pid(foe))).last().unwrap();
        act(
            &mut driver,
            ManualAction::MoveTop {
                count: 1,
                to: ManualZone::Hand,
                library: Some(pid(foe)),
            },
        );
        assert_eq!(zone(&driver, top), Zone::Hand(pid(foe)));
        assert!(!driver.state().is_shown_to(pid(0), top));
        assert_eq!(
            refused(&mut driver, mv(top, ManualZone::Graveyard)),
            ManualRefusal::Hidden(top)
        );
    }
}

#[rstest]
fn the_acting_seats_own_library_is_named_by_either_form(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats);
    act(
        &mut driver,
        ManualAction::MoveTop {
            count: 1,
            to: ManualZone::Graveyard,
            library: Some(pid(0)),
        },
    );
    act(
        &mut driver,
        ManualAction::MoveTop {
            count: 1,
            to: ManualZone::Graveyard,
            library: None,
        },
    );
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 2);
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), 3);
}

#[rstest]
fn another_seats_hand_and_library_are_never_named(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats);
    for foe in 1..seats {
        let held = find(&driver, Zone::Hand(pid(foe)), &format!("Held {foe}"));
        let deep = *driver.state().zone(Zone::Library(pid(foe))).last().unwrap();
        for object in [held, deep] {
            let actions = [
                mv(object, ManualZone::Graveyard),
                mv(object, ManualZone::Exile),
                mv(object, ManualZone::Battlefield),
                ManualAction::Damage {
                    source: object,
                    target: DamageTarget::Player(pid(0)),
                    amount: 1,
                },
                ManualAction::GiveControl { object, to: pid(0) },
                ManualAction::MoveBack { object },
                ManualAction::Attach {
                    object: find(&driver, Zone::Battlefield, "Blade 0"),
                    to: object,
                },
            ];
            for action in actions {
                let what = refused(&mut driver, action.clone());
                assert!(
                    matches!(
                        what,
                        ManualRefusal::Hidden(_) | ManualRefusal::WrongZone { .. }
                    ),
                    "{action:?} gave {what:?}"
                );
                if !matches!(action, ManualAction::Attach { .. }) {
                    assert_eq!(what, ManualRefusal::Hidden(object), "{action:?}");
                }
            }
        }
        // What is the owner's alone stays its own: no reveal, discard, note or tutor.
        for action in [
            ManualAction::Reveal {
                what: ManualReveal::Hand { object: held },
            },
            ManualAction::Discard { object: held },
            ManualAction::Note {
                object: held,
                text: "x".to_owned(),
            },
        ] {
            assert_eq!(refused(&mut driver, action), ManualRefusal::NotYours);
        }
        assert_eq!(
            refused(&mut driver, ManualAction::Tutor { object: deep }),
            ManualRefusal::NotFound
        );
        assert!(!driver.state().is_shown_to(pid(0), held));
        assert!(!driver.state().is_shown_to(pid(0), deep));
    }
}

#[rstest]
fn a_foretold_card_in_exile_is_named_by_its_owner_alone(#[values(2, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(
        scenario(seats)
            .hand_named(0, ["Behold the Multiverse"])
            .battlefield(0, [ready(forest()), ready(forest())]),
    )
    .unwrap();
    let behold = find(&driver, Zone::Hand(pid(0)), "Behold the Multiverse");
    driver
        .answer(Answer::Priority(PriorityAction::Foretell(behold)))
        .unwrap();
    driver.pay_tapping(&["Forest", "Forest"], 0).unwrap();
    assert_eq!(zone(&driver, behold), Zone::Exile);
    for foe in 1..seats {
        for action in [
            mv(behold, ManualZone::Graveyard),
            ManualAction::GiveControl {
                object: behold,
                to: pid(foe),
            },
        ] {
            let result = driver.manual_by(pid(foe), &action).unwrap_err();
            assert!(
                matches!(
                    result,
                    ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(
                        ManualRefusal::Hidden(_)
                    )))
                ),
                "{result:?}"
            );
        }
    }
    act(&mut driver, mv(behold, ManualZone::Graveyard));
    assert_eq!(zone(&driver, behold), Zone::Graveyard(pid(0)));
}

#[rstest]
fn a_seat_deals_damage_from_another_seats_permanent(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats);
    for foe in 1..seats {
        let source = find(&driver, Zone::Battlefield, &format!("Perm {foe}"));
        let events = act(
            &mut driver,
            ManualAction::Damage {
                source,
                target: DamageTarget::Player(pid(0)),
                amount: 2,
            },
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::DamageDealt { .. }))
        );
        assert_eq!(life(&driver, 0), 20 - 2 * i32::try_from(foe).unwrap());
    }
}

#[rstest]
fn a_seat_attaches_and_unattaches_another_seats_equipment(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats);
    let mine = find(&driver, Zone::Battlefield, "Perm 0");
    for foe in 1..seats {
        let blade = find(&driver, Zone::Battlefield, &format!("Blade {foe}"));
        act(
            &mut driver,
            ManualAction::Attach {
                object: blade,
                to: mine,
            },
        );
        assert_eq!(driver.state().host_of(blade), Some(mine));
        act(&mut driver, ManualAction::Unattach { object: blade });
        assert_eq!(driver.state().host_of(blade), None);
    }
}

#[rstest]
fn a_seat_takes_control_of_another_seats_permanent_and_cards(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats);
    for foe in 1..seats {
        let perm = find(&driver, Zone::Battlefield, &format!("Perm {foe}"));
        act(
            &mut driver,
            ManualAction::GiveControl {
                object: perm,
                to: pid(0),
            },
        );
        let held = driver.state().object(perm).unwrap();
        assert_eq!(held.owner(), pid(foe));
        assert_eq!(controller_of(driver.state(), perm), Some(pid(0)));
        assert_eq!(
            refused(
                &mut driver,
                ManualAction::GiveControl {
                    object: perm,
                    to: pid(0),
                }
            ),
            ManualRefusal::NoChange
        );
        let dead = find(&driver, Zone::Graveyard(pid(foe)), &format!("Dead {foe}"));
        act(
            &mut driver,
            ManualAction::GiveControl {
                object: dead,
                to: pid(0),
            },
        );
        assert_eq!(zone(&driver, dead), Zone::Battlefield);
        assert_eq!(controller_of(driver.state(), dead), Some(pid(0)));
    }
}

#[rstest]
fn a_seat_turns_over_another_seats_double_faced_permanent_and_card(#[values(2, 4)] seats: usize) {
    let mut driver = Driver::from_scenario(
        scenario(seats)
            .battlefield(1, [ready(card(DELVER).unwrap())])
            .graveyard(1, [card(DELVER).unwrap()]),
    )
    .unwrap();
    let on_table = driver.id_of("Delver of Secrets").unwrap();
    let events = act(&mut driver, ManualAction::Transform { object: on_table });
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Transformed { .. }))
    );
    let in_graveyard = find(&driver, Zone::Graveyard(pid(1)), "Delver of Secrets");
    act(
        &mut driver,
        ManualAction::MoveBack {
            object: in_graveyard,
        },
    );
    assert_eq!(zone(&driver, in_graveyard), Zone::Battlefield);
}
