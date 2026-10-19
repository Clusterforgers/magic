//! Goal: every event of the effect pipeline folds as the rules say: abilities become objects of their own on top of the stack and cease to exist when they resolve, the journal grows and clears at the steps, spells that fizzle go to the graveyard, a zone change makes a new object, and an event that cannot follow from the state is refused. The state hash sees each change and nothing else.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Bounds, Ref, TargetSpec, Targetable, ZoneKind};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::abilities::{Cause, PutOnStack};
use crate::choose::{Choice, Choices, Target};
use crate::counters::CountersChanged;
use crate::events::{Event, ResolutionOutcome, ZoneCause};
use crate::fixtures::{creature, state};
use crate::library::{CardsFound, LibraryLooked, Revealed, Scried};
use crate::object::{ObjectKind, ObjectRef};
use crate::snapshot::snapshot;
use crate::state::GameState;
use crate::tokens::TokenCreated;
use crate::triggers::Triggered;
use crate::zones::Zone;

fn zap() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Zap")
            .with_type(CardType::Instant)
            .with_rules(Ability::Spell {
                targets: vec![TargetSpec {
                    what: Targetable::AnyTarget,
                    bounds: Bounds::default(),
                    divide: None,
                }],
                effects: vec![Effect::DealDamage {
                    source: None,
                    to: Ref::Target(0),
                    amount: Amount::Fixed(1),
                }],
            }),
    )
}

fn totem() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Totem")
            .with_type(CardType::Artifact)
            .with_rules(Ability::Activated {
                cost: Cost(Vec::new()),
                timing: Timing::Instant,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![Effect::Draw {
                    who: Ref::You,
                    count: Amount::one(),
                }],
            }),
    )
}

/// Seat 0 holds a Zap and controls a Totem and a Bear; seat 1 controls an Ogre.
fn game() -> GameState {
    state(
        vec![vec![zap()], vec![]],
        vec![
            vec![totem(), creature("Bear", 2, 2)],
            vec![creature("Ogre", 3, 4)],
        ],
    )
}

fn fold(game: &mut GameState, events: &[Event]) {
    for event in events {
        apply_event(game, event).unwrap_or_else(|error| panic!("{event:?}: {error}"));
    }
}

fn totem_id(game: &GameState) -> ObjectId {
    game.zone(Zone::Battlefield)[0]
}

fn put_totem_ability(game: &GameState) -> Event {
    let source = game.object(totem_id(game)).unwrap();
    Event::AbilityPutOnStack(Box::new(PutOnStack {
        object: game.next_object(),
        source: source.reference(),
        snapshot: snapshot(game, source.id()).unwrap(),
        ability: 0,
        controller: PlayerId(0),
        choices: Choices::default(),
        cause: None,
    }))
}

fn cast_zap(game: &GameState, at: Target) -> Event {
    Event::SpellCast {
        player: PlayerId(0),
        object: game.zone(Zone::Hand(PlayerId(0)))[0],
        choices: Choices {
            targets: vec![vec![at]],
            ..Choices::default()
        },
    }
}

#[test]
fn an_ability_is_an_object_of_its_own_on_top_of_the_stack() {
    let mut game = game();
    let before = game.next_object();
    let put = put_totem_ability(&game);
    fold(&mut game, &[put]);
    assert_eq!(game.next_object(), ObjectId(before.get() + 1));
    let id = *game.stack().last().unwrap();
    assert_eq!(id, before);
    let object = game.object(id).unwrap();
    assert_eq!(object.kind(), ObjectKind::Ability);
    assert_eq!(
        (object.owner(), object.controller()),
        (PlayerId(0), PlayerId(0))
    );
    assert_eq!(object.card().name, "Totem", "it is named after its source");
    let ability = object.ability().unwrap();
    assert_eq!(
        ability.source(),
        game.object(totem_id(&game)).unwrap().reference()
    );
    assert_eq!(ability.index(), 0);
    assert_eq!(ability.snapshot().id, totem_id(&game));
    assert!(matches!(ability.rules(), Ability::Activated { .. }));
    assert!(object.permanent().is_none());
}

#[test]
fn an_ability_that_resolved_has_ceased_to_exist_and_the_active_player_gets_priority() {
    let mut game = game();
    let put = put_totem_ability(&game);
    fold(&mut game, &[put]);
    let id = *game.stack().last().unwrap();
    fold(
        &mut game,
        &[
            Event::ResolutionBegan { object: id },
            Event::StepDone {
                position: vec![1],
                bound: Vec::new(),
                snapshots: Vec::new(),
            },
        ],
    );
    assert_eq!(game.resolving().unwrap().object(), id);
    assert_eq!(game.resolving().unwrap().position(), [1]);
    fold(
        &mut game,
        &[Event::ResolutionEnded {
            object: id,
            outcome: ResolutionOutcome::Resolved,
        }],
    );
    assert!(game.object(id).is_none());
    assert_eq!(game.stack(), []);
    assert!(game.resolving().is_none());
    let priority = game.priority().unwrap();
    assert_eq!(priority.holder(), PlayerId(0));
    assert!(priority.passed().is_empty());
    // The id is not used again.
    assert!(game.next_object() > id);
}

#[test]
fn a_spell_that_resolved_stays_on_the_stack_until_it_is_resolved() {
    let mut game = game();
    let at = Target::Player(PlayerId(1));
    let cast = cast_zap(&game, at);
    fold(&mut game, &[cast]);
    let spell = *game.stack().last().unwrap();
    assert_eq!(game.object(spell).unwrap().choices().targets, [vec![at]]);
    fold(
        &mut game,
        &[
            Event::ResolutionBegan { object: spell },
            Event::ResolutionEnded {
                object: spell,
                outcome: ResolutionOutcome::Resolved,
            },
        ],
    );
    assert_eq!(game.object(spell).unwrap().zone(), Zone::Stack);
    assert!(game.resolving().is_none());
    fold(&mut game, &[Event::SpellResolved { object: spell }]);
    assert_eq!(
        game.object(spell).unwrap().zone(),
        Zone::Graveyard(PlayerId(0))
    );
    assert!(
        game.object(spell).unwrap().choices().is_empty(),
        "what was announced goes with the stack"
    );
}

#[rstest]
#[case::fizzled(ResolutionOutcome::Fizzled)]
#[case::countered(ResolutionOutcome::Countered)]
fn a_spell_that_did_not_resolve_goes_to_its_owners_graveyard(#[case] outcome: ResolutionOutcome) {
    let mut game = game();
    let at = Target::Player(PlayerId(1));
    let cast = cast_zap(&game, at);
    fold(&mut game, &[cast]);
    let spell = *game.stack().last().unwrap();
    fold(
        &mut game,
        &[
            Event::ResolutionBegan { object: spell },
            Event::ResolutionEnded {
                object: spell,
                outcome,
            },
        ],
    );
    let object = game.object(spell).unwrap();
    assert_eq!(object.zone(), Zone::Graveyard(PlayerId(0)));
    assert_eq!(object.counter(), 2, "cast, then left the stack");
    assert!(game.resolving().is_none() && game.priority().is_some());
}

#[test]
fn an_object_below_the_resolving_one_can_be_countered_without_ending_the_resolution() {
    let mut game = game();
    let at = Target::Player(PlayerId(1));
    let cast = cast_zap(&game, at);
    fold(&mut game, &[cast]);
    let spell = *game.stack().last().unwrap();
    let put = put_totem_ability(&game);
    fold(&mut game, &[put]);
    let ability = *game.stack().last().unwrap();
    fold(&mut game, &[Event::ResolutionBegan { object: ability }]);
    fold(
        &mut game,
        &[Event::ResolutionEnded {
            object: spell,
            outcome: ResolutionOutcome::Countered,
        }],
    );
    assert_eq!(game.resolving().unwrap().object(), ability);
    assert_eq!(
        game.object(spell).unwrap().zone(),
        Zone::Graveyard(PlayerId(0))
    );
}

#[test]
fn an_object_that_leaves_the_game_while_it_resolves_ends_the_resolution() {
    let mut game = game();
    let put = put_totem_ability(&game);
    fold(&mut game, &[put]);
    let id = *game.stack().last().unwrap();
    fold(&mut game, &[Event::ResolutionBegan { object: id }]);
    fold(&mut game, &[Event::ObjectLeftGame { object: id }]);
    assert!(game.resolving().is_none());
    assert_eq!(game.stack(), []);
}

#[test]
fn the_journal_collects_answers_and_every_step_clears_it_and_binds_names() {
    let mut game = game();
    let put = put_totem_ability(&game);
    fold(&mut game, &[put]);
    let id = *game.stack().last().unwrap();
    let bear = game
        .object(game.zone(Zone::Battlefield)[1])
        .unwrap()
        .reference();
    fold(
        &mut game,
        &[
            Event::ResolutionBegan { object: id },
            Event::ChoiceMade {
                player: PlayerId(1),
                choice: Choice::YesNo(true),
            },
            Event::ChoiceMade {
                player: PlayerId(0),
                choice: Choice::Number(2),
            },
        ],
    );
    assert_eq!(game.resolving().unwrap().journal().len(), 2);
    let shot = snapshot(&game, bear.id).unwrap();
    fold(
        &mut game,
        &[Event::StepDone {
            position: vec![1],
            bound: vec![("bear".to_owned(), vec![Target::Object(bear)])],
            snapshots: vec![shot.clone()],
        }],
    );
    let resolving = game.resolving().unwrap();
    assert_eq!(resolving.journal(), []);
    assert_eq!(resolving.bindings()["bear"], [Target::Object(bear)]);
    assert_eq!(resolving.snapshots(), [shot]);
}

fn cause() -> Cause {
    Cause {
        subjects: vec![Target::Player(PlayerId(1))],
        snapshots: Vec::new(),
    }
}

/// Every event that must be refused in a given state: the events to fold first, then the one that cannot follow.
#[rstest]
#[case::an_ability_with_an_id_in_use(
    |g: &GameState| { let Event::AbilityPutOnStack(mut put) = put_totem_ability(g) else { unreachable!() }; put.object = totem_id(g); Event::AbilityPutOnStack(put) }
)]
#[case::an_ability_whose_source_is_gone(
    |g: &GameState| { let Event::AbilityPutOnStack(mut put) = put_totem_ability(g) else { unreachable!() }; put.source = ObjectRef { id: ObjectId(900), counter: 0 }; Event::AbilityPutOnStack(put) }
)]
#[case::an_ability_the_source_does_not_have(
    |g: &GameState| { let Event::AbilityPutOnStack(mut put) = put_totem_ability(g) else { unreachable!() }; put.ability = 4; Event::AbilityPutOnStack(put) }
)]
#[case::an_activation_of_an_ability_that_is_not_there(
    |g: &GameState| Event::AbilityActivated { player: PlayerId(0), source: g.object(totem_id(g)).unwrap().reference(), ability: 3 }
)]
#[case::a_resolution_with_nothing_on_the_stack(
    |g: &GameState| Event::ResolutionBegan { object: totem_id(g) }
)]
#[case::an_answer_when_nothing_resolves(
    |_: &GameState| Event::ChoiceMade { player: PlayerId(0), choice: Choice::YesNo(true) }
)]
#[case::a_step_when_nothing_resolves(
    |_: &GameState| Event::StepDone { position: vec![1], bound: Vec::new(), snapshots: Vec::new() }
)]
#[case::the_end_of_what_is_not_on_the_stack(
    |g: &GameState| Event::ResolutionEnded { object: totem_id(g), outcome: ResolutionOutcome::Countered }
)]
#[case::the_end_of_what_is_not_there(
    |_: &GameState| Event::ResolutionEnded { object: ObjectId(900), outcome: ResolutionOutcome::Resolved }
)]
#[case::a_move_from_the_wrong_zone(
    |g: &GameState| Event::ZoneChanged { object: totem_id(g), from: Zone::Exile, to: Zone::Hand(PlayerId(0)), cause: ZoneCause::Moved }
)]
#[case::a_gain_of_nothing(
    |_: &GameState| Event::LifeGained { player: PlayerId(0), amount: 0 }
)]
#[case::a_loss_of_nothing(
    |_: &GameState| Event::LifeLost { player: PlayerId(0), amount: 0 }
)]
#[case::a_gain_for_nobody(
    |_: &GameState| Event::LifeGained { player: PlayerId(7), amount: 1 }
)]
#[case::triggers_are_not_in_the_engine_yet(
    |g: &GameState| Event::AbilityTriggered(Triggered { source: g.object(totem_id(g)).unwrap().reference(), ability: 0, cause: cause() })
)]
#[case::tokens_are_not_in_the_engine_yet(
    |_: &GameState| Event::TokenCreated(TokenCreated { object: ObjectId(900), controller: PlayerId(0), name: "Treasure".into() })
)]
#[case::counters_are_not_in_the_engine_yet(
    |_: &GameState| Event::CountersChanged(CountersChanged { on: Target::Player(PlayerId(0)), counter: CounterKind::Poison, delta: 1 })
)]
fn an_event_that_cannot_follow_from_the_state_is_refused(
    #[case] event: impl Fn(&GameState) -> Event,
) {
    let mut game = game();
    let event = event(&game);
    assert!(apply_event(&mut game, &event).is_err(), "{event:?}");
}

#[test]
fn a_second_resolution_cannot_begin_and_a_name_cannot_be_bound_twice() {
    let mut game = game();
    let put = put_totem_ability(&game);
    fold(&mut game, &[put]);
    let id = *game.stack().last().unwrap();
    fold(&mut game, &[Event::ResolutionBegan { object: id }]);
    assert!(apply_event(&mut game, &Event::ResolutionBegan { object: id }).is_err());
    let step = Event::StepDone {
        position: vec![1],
        bound: vec![("x".to_owned(), Vec::new())],
        snapshots: Vec::new(),
    };
    fold(&mut game, std::slice::from_ref(&step));
    assert!(apply_event(&mut game, &step).is_err(), "`x` is bound twice");
}

#[test]
fn an_ability_is_not_moved_to_another_zone() {
    let mut game = game();
    let put = put_totem_ability(&game);
    fold(&mut game, &[put]);
    let id = *game.stack().last().unwrap();
    let event = Event::ZoneChanged {
        object: id,
        from: Zone::Stack,
        to: Zone::Graveyard(PlayerId(0)),
        cause: ZoneCause::Countered,
    };
    assert!(apply_event(&mut game, &event).is_err());
}

#[test]
fn life_gained_and_lost_move_the_total_by_exactly_that_much() {
    let mut game = game();
    let life = |game: &GameState| game.player(PlayerId(1)).unwrap().life();
    fold(
        &mut game,
        &[
            Event::LifeGained {
                player: PlayerId(1),
                amount: 5,
            },
            Event::LifeLost {
                player: PlayerId(1),
                amount: 3,
            },
        ],
    );
    assert_eq!(life(&game), 22);
    fold(
        &mut game,
        &[Event::LifeLost {
            player: PlayerId(1),
            amount: 30,
        }],
    );
    assert_eq!(
        life(&game),
        -8,
        "a total may go below zero; state-based actions judge it"
    );
}

#[test]
fn what_a_player_learned_changes_nothing_in_the_game() {
    let mut game = game();
    let before = game.state_hash();
    let card = game.zone(Zone::Library(PlayerId(0)))[0];
    fold(
        &mut game,
        &[
            Event::LibraryLooked(LibraryLooked {
                player: PlayerId(0),
                cards: vec![card],
            }),
            Event::CardsFound(CardsFound {
                player: PlayerId(0),
                zone: Zone::Library(PlayerId(0)),
                cards: vec![card],
            }),
            Event::Scried(Scried {
                player: PlayerId(0),
                top: vec![card],
                bottom: Vec::new(),
            }),
            Event::Revealed(Revealed {
                player: PlayerId(0),
                cards: vec![card],
            }),
        ],
    );
    assert_eq!(game.state_hash(), before);
}

/// The state hash must tell a game that did something from one that did not, event by event.
#[rstest]
#[case::an_ability_on_the_stack(|g: &GameState| vec![put_totem_ability(g)])]
#[case::a_spell_with_a_target(|g: &GameState| vec![cast_zap(g, Target::Player(PlayerId(1)))])]
#[case::a_spell_with_another_target(|g: &GameState| vec![cast_zap(g, Target::Player(PlayerId(0)))])]
#[case::a_life_gain(|_: &GameState| vec![Event::LifeGained { player: PlayerId(0), amount: 1 }])]
#[case::a_life_loss(|_: &GameState| vec![Event::LifeLost { player: PlayerId(0), amount: 1 }])]
#[case::a_zone_change(|g: &GameState| vec![Event::ZoneChanged { object: totem_id(g), from: Zone::Battlefield, to: Zone::Exile, cause: ZoneCause::Exiled }])]
fn the_state_hash_sees_each_change(#[case] events: impl Fn(&GameState) -> Vec<Event>) {
    let mut game = game();
    let before = game.state_hash();
    let events = events(&game);
    fold(&mut game, &events);
    assert_ne!(game.state_hash(), before);
}

#[test]
fn the_state_hash_sees_the_journal_the_position_and_what_was_announced() {
    let mut game = game();
    let put = put_totem_ability(&game);
    fold(&mut game, &[put]);
    let id = *game.stack().last().unwrap();
    let mut hashes = vec![game.state_hash()];
    for event in [
        Event::ResolutionBegan { object: id },
        Event::ChoiceMade {
            player: PlayerId(0),
            choice: Choice::Number(1),
        },
        Event::ChoiceMade {
            player: PlayerId(0),
            choice: Choice::Number(2),
        },
        Event::StepDone {
            position: vec![1],
            bound: Vec::new(),
            snapshots: Vec::new(),
        },
    ] {
        fold(&mut game, &[event]);
        hashes.push(game.state_hash());
    }
    let mut distinct = hashes.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), hashes.len(), "{hashes:?}");
}
