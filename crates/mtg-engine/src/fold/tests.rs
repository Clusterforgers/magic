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
use crate::combat::AttackTarget;
use crate::copies::SpellCopy;
use crate::counters::CountersChanged;
use crate::events::{Event, ResolutionOutcome, ZoneCause};
use crate::fixtures::{creature, state};
use crate::library::{CardsFound, LibraryLooked, Revealed, Scried, Surveilled};
use crate::object::{ObjectKind, ObjectRef};
use crate::snapshot::snapshot;
use crate::state::GameState;
use crate::tokens::TokenCreated;
use crate::triggers::{DelayedId, Triggered};
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
                condition: None,
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
        origin: crate::abilities::AbilitySource::default(),
        object: game.next_object(),
        source: source.reference(),
        snapshot: snapshot(game, source.id()).unwrap(),
        ability: 0,
        controller: PlayerId(0),
        choices: Choices::default(),
        cause: None,
        delayed: None,
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

#[rstest]
#[case::fizzled_while_resolving(ResolutionOutcome::Fizzled, true)]
#[case::countered_from_below(ResolutionOutcome::Countered, false)]
fn the_end_of_a_spell_copy_that_a_replacement_effect_sent_to_exile_finds_it_ceased_to_exist(
    #[case] outcome: ResolutionOutcome,
    #[case] resolving: bool,
) {
    let mut game = game();
    let at = Target::Player(PlayerId(1));
    let cast = cast_zap(&game, at);
    fold(&mut game, &[cast]);
    let original = *game.stack().last().unwrap();
    let copy = game.next_object();
    let copied = Event::SpellCopied(Box::new(SpellCopy {
        object: copy,
        of: game.object(original).unwrap().reference(),
        controller: PlayerId(0),
        definition: zap(),
        choices: Choices {
            targets: vec![vec![at]],
            ..Choices::default()
        },
    }));
    fold(&mut game, &[copied]);
    if resolving {
        fold(&mut game, &[Event::ResolutionBegan { object: copy }]);
    }
    // Rest in Peace: the move to the graveyard is made to exile, where a token cannot be (CR 111.7).
    let moved = Event::ZoneChanged {
        object: copy,
        from: Zone::Stack,
        to: Zone::Exile,
        cause: ZoneCause::Countered,
    };
    fold(&mut game, &[moved]);
    assert!(game.has_ceased(copy) && game.object(copy).is_none());
    fold(
        &mut game,
        &[Event::ResolutionEnded {
            object: copy,
            outcome,
        }],
    );
    assert_eq!(game.stack(), [original], "the original is left alone");
    assert!(game.resolving().is_none());
    assert!(game.priority().is_some());
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

/// A token of the given id for `controller`, as the event says it.
fn token(id: u64, controller: PlayerId, attacking: Option<AttackTarget>) -> TokenCreated {
    TokenCreated {
        object: ObjectId(id),
        copy_of: None,
        controller,
        name: "Treasure".into(),
        tapped: false,
        attacking,
        definition: Arc::new(CardDefinition::new("Treasure").with_type(CardType::Artifact)),
    }
}

fn cause() -> Cause {
    Cause {
        subjects: vec![Target::Player(PlayerId(1))],
        snapshots: Vec::new(),
        amount: 0,
        source: None,
    }
}

/// The event that says ability `ability` of the totem triggered, whether or not it is a triggered ability.
fn trigger_of(game: &GameState, ability: usize) -> Event {
    let source = game.object(totem_id(game)).unwrap();
    Event::AbilityTriggered(Box::new(Triggered {
        origin: crate::abilities::AbilitySource::default(),
        source: source.reference(),
        ability,
        controller: PlayerId(0),
        cause: cause(),
        snapshot: snapshot(game, source.id()).unwrap(),
        delayed: None,
    }))
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
    |g: &GameState| Event::AbilityActivated { player: PlayerId(0), source: g.object(totem_id(g)).unwrap().reference(), ability: 3, origin: crate::abilities::AbilitySource::default() }
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
#[case::a_trigger_of_an_ability_that_is_not_triggered(
    |g: &GameState| trigger_of(g, 0)
)]
#[case::a_trigger_of_an_ability_that_is_not_there(
    |g: &GameState| trigger_of(g, 5)
)]
#[case::a_trigger_removed_that_never_waited(
    |g: &GameState| Event::TriggerRemoved { source: g.object(totem_id(g)).unwrap().reference(), ability: 0, origin: crate::abilities::AbilitySource::default(), delayed: None, cause: Box::new(cause()) }
)]
#[case::a_trigger_put_on_the_stack_that_never_triggered(
    |g: &GameState| { let Event::AbilityPutOnStack(mut put) = put_totem_ability(g) else { unreachable!() }; put.cause = Some(cause()); Event::AbilityPutOnStack(put) }
)]
#[case::an_order_when_nothing_waits(
    |_: &GameState| Event::TriggersOrdered { player: PlayerId(0), order: vec![0] }
)]
#[case::a_delayed_trigger_that_is_not_waiting_expiring(
    |_: &GameState| Event::DelayedTriggerExpired { id: DelayedId(3) }
)]
#[case::a_token_that_reuses_an_id(
    |g: &GameState| Event::TokenCreated(token(totem_id(g).get(), PlayerId(0), None))
)]
#[case::a_token_for_nobody(
    |_: &GameState| Event::TokenCreated(token(900, PlayerId(7), None))
)]
#[case::a_token_attacking_before_attackers_are_declared(
    |_: &GameState| Event::TokenCreated(token(900, PlayerId(0), Some(AttackTarget::Player(PlayerId(1)))))
)]
#[case::counters_taken_off_a_player_who_has_none(
    |_: &GameState| Event::CountersChanged(CountersChanged { on: Target::Player(PlayerId(0)), counter: CounterKind::Poison, delta: -1 })
)]
#[case::counters_for_nobody(
    |_: &GameState| Event::CountersChanged(CountersChanged { on: Target::Player(PlayerId(7)), counter: CounterKind::Poison, delta: 1 })
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
            Event::Revealed(Revealed {
                player: PlayerId(0),
                cards: vec![card],
            }),
        ],
    );
    assert_eq!(game.state_hash(), before);
}

#[rstest]
#[case::the_same_object_after_a_move(vec![(7, 0)], vec![(7, 1)], true)]
#[case::the_same_binding_twice(vec![(7, 0)], vec![(7, 0)], false)]
#[case::an_empty_binding_twice(vec![], vec![], false)]
#[case::two_objects_each_moved_on(vec![(7, 0), (8, 2)], vec![(7, 1), (8, 3)], true)]
#[case::another_object(vec![(7, 0)], vec![(8, 1)], false)]
#[case::an_older_incarnation(vec![(7, 2)], vec![(7, 1)], false)]
#[case::another_count(vec![(7, 0)], vec![(7, 1), (8, 0)], false)]
#[case::nothing_for_something(vec![(7, 0)], vec![], false)]
fn a_name_is_bound_again_only_to_the_objects_it_named_after_they_moved_on(
    #[case] first: Vec<(u64, u32)>,
    #[case] again: Vec<(u64, u32)>,
    #[case] allowed: bool,
) {
    let mut game = game();
    game.resolving = Some(crate::resolve::Resolving::begin(totem_id(&game)));
    let targets = |list: &[(u64, u32)]| -> Vec<Target> {
        list.iter()
            .map(|(id, counter)| {
                Target::Object(ObjectRef {
                    id: ObjectId(*id),
                    counter: *counter,
                })
            })
            .collect()
    };
    let bind = |bound: Vec<Target>| Event::StepDone {
        position: vec![1],
        bound: vec![("cards".to_owned(), bound)],
        snapshots: Vec::new(),
    };
    fold(&mut game, &[bind(targets(&first))]);
    assert_eq!(
        apply_event(&mut game, &bind(targets(&again))).is_ok(),
        allowed
    );
}

#[test]
fn a_cleared_name_is_bound_again_and_one_that_is_not_cleared_is_not() {
    let mut game = game();
    game.resolving = Some(crate::resolve::Resolving::begin(totem_id(&game)));
    let player = |n: u64| vec![Target::Player(PlayerId(n))];
    let step = |bound: Vec<(String, Vec<Target>)>| Event::StepDone {
        position: vec![1],
        bound,
        snapshots: Vec::new(),
    };
    fold(&mut game, &[step(vec![("x".to_owned(), player(0))])]);
    let freed = step(vec![
        ("~clear:x".to_owned(), Vec::new()),
        ("x".to_owned(), player(1)),
    ]);
    fold(&mut game, &[freed]);
    assert_eq!(game.resolving().unwrap().bindings()["x"], player(1));
    // Clearing a name nothing bound is nothing.
    fold(
        &mut game,
        &[step(vec![("~clear:y".to_owned(), Vec::new())])],
    );
    assert!(!game.resolving().unwrap().bindings().contains_key("y"));
    assert!(apply_event(&mut game, &step(vec![("x".to_owned(), player(2))])).is_err());
}

#[rstest]
#[case::untapped_under_its_owner(false, 0)]
#[case::tapped_under_its_owner(true, 0)]
#[case::untapped_under_another(false, 1)]
#[case::tapped_under_another(true, 1)]
fn a_card_put_onto_the_battlefield_enters_tapped_or_under_another_player_and_keeps_its_owner(
    #[case] tapped: bool,
    #[case] controller: usize,
) {
    let mut game = game();
    let card = game.zone(Zone::Library(PlayerId(0)))[0];
    let controller = PlayerId::from_index(controller);
    fold(
        &mut game,
        &[
            Event::ZoneChanged {
                object: card,
                from: Zone::Library(PlayerId(0)),
                to: Zone::Battlefield,
                cause: ZoneCause::Moved,
            },
            Event::Entered {
                object: card,
                tapped,
                controller,
                chosen: None,
            },
        ],
    );
    let object = game.object(card).unwrap();
    assert_eq!(
        (object.owner(), object.controller()),
        (PlayerId(0), controller)
    );
    assert_eq!(object.permanent().unwrap().tapped(), tapped);
    // Leaving the battlefield hands the card back to its owner (CR 108.3).
    fold(
        &mut game,
        &[Event::ZoneChanged {
            object: card,
            from: Zone::Battlefield,
            to: Zone::Hand(PlayerId(0)),
            cause: ZoneCause::Moved,
        }],
    );
    assert_eq!(game.object(card).unwrap().controller(), PlayerId(0));
}

#[test]
fn entering_what_is_not_on_the_battlefield_or_under_nobody_is_refused() {
    let mut game = game();
    let card = game.zone(Zone::Library(PlayerId(0)))[0];
    let in_library = Event::Entered {
        object: card,
        tapped: true,
        controller: PlayerId(0),
        chosen: None,
    };
    assert!(apply_event(&mut game, &in_library).is_err());
    let moved = Event::ZoneChanged {
        object: card,
        from: Zone::Library(PlayerId(0)),
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    };
    fold(&mut game, &[moved]);
    let under_nobody = Event::Entered {
        object: card,
        tapped: false,
        controller: PlayerId(9),
        chosen: None,
    };
    assert!(apply_event(&mut game, &under_nobody).is_err());
}

#[test]
fn what_a_resolution_showed_is_part_of_the_state_until_the_resolution_ends() {
    let mut game = game();
    let source = totem_id(&game);
    let card = game.zone(Zone::Library(PlayerId(1)))[0];
    let before = game.state_hash();
    game.resolving = Some(crate::resolve::Resolving::begin(source));
    let began = game.state_hash();
    fold(
        &mut game,
        &[Event::Revealed(Revealed {
            player: PlayerId(0),
            cards: vec![card],
        })],
    );
    assert_ne!(
        game.state_hash(),
        began,
        "the state hash sees what was shown"
    );
    assert!(game.is_shown_to(PlayerId(1), card));
    game.resolving = None;
    assert!(!game.is_shown_to(PlayerId(1), card));
    assert_ne!(before, began);
}

/// The library of seat 0 as five names from the bottom up.
fn library_names(game: &GameState) -> Vec<String> {
    game.zone(Zone::Library(PlayerId(0)))
        .iter()
        .map(|id| game.object(*id).unwrap().card().name.clone())
        .collect()
}

/// The five cards of the library of seat 0 from the top down: "Library 0" is the top card.
fn from_the_top(game: &GameState, numbers: &[usize]) -> Vec<ObjectId> {
    let library = game.zone(Zone::Library(PlayerId(0)));
    numbers
        .iter()
        .map(|n| library[library.len() - 1 - n])
        .collect()
}

#[rstest]
#[case::all_stay_in_the_same_order(&[0, 1, 2], &[], &["Library 4", "Library 3", "Library 2", "Library 1", "Library 0"])]
#[case::two_stay_reversed(&[1, 0], &[], &["Library 4", "Library 3", "Library 2", "Library 0", "Library 1"])]
#[case::the_top_card_goes_to_the_bottom(&[], &[0], &["Library 0", "Library 4", "Library 3", "Library 2", "Library 1"])]
#[case::some_stay_and_some_go(&[1], &[0, 2], &["Library 2", "Library 0", "Library 4", "Library 3", "Library 1"])]
#[case::the_last_of_the_bottom_is_the_bottom_card(&[], &[2, 0, 1], &["Library 1", "Library 0", "Library 2", "Library 4", "Library 3"])]
#[case::all_go_to_the_bottom(&[], &[0, 1, 2, 3, 4], &["Library 4", "Library 3", "Library 2", "Library 1", "Library 0"])]
#[case::a_scry_of_nothing(&[], &[], &["Library 4", "Library 3", "Library 2", "Library 1", "Library 0"])]
fn a_scry_puts_the_cards_looked_at_back_on_top_and_on_the_bottom_each_from_the_top_down(
    #[case] top: &[usize],
    #[case] bottom: &[usize],
    #[case] expected: &[&str],
) {
    let mut game = game();
    assert_eq!(library_names(&game)[4], "Library 0");
    let event = Event::Scried(Scried {
        player: PlayerId(0),
        top: from_the_top(&game, top),
        bottom: from_the_top(&game, bottom),
    });
    fold(&mut game, &[event]);
    assert_eq!(library_names(&game), expected);
}

#[rstest]
#[case::more_than_the_library_holds(&[0, 1, 2, 3, 4, 5], &[])]
#[case::cards_that_are_not_the_top_ones(&[1, 2], &[])]
#[case::the_same_card_twice(&[0], &[0])]
#[case::a_card_from_below(&[0], &[4])]
fn a_scry_of_cards_that_are_not_the_top_of_the_library_is_refused(
    #[case] top: &[usize],
    #[case] bottom: &[usize],
) {
    let mut game = game();
    let before = game.state_hash();
    let pick = |numbers: &[usize]| {
        let library = game.zone(Zone::Library(PlayerId(0)));
        numbers
            .iter()
            .map(|n| {
                library
                    .len()
                    .checked_sub(1 + n)
                    .map_or(ObjectId(900), |at| library[at])
            })
            .collect::<Vec<_>>()
    };
    let event = Event::Scried(Scried {
        player: PlayerId(0),
        top: pick(top),
        bottom: pick(bottom),
    });
    assert!(apply_event(&mut game, &event).is_err());
    assert_eq!(game.state_hash(), before);
}

#[rstest]
#[case::all_stay_in_the_same_order(&[0, 1, 2], &[], &["Library 4", "Library 3", "Library 2", "Library 1", "Library 0"])]
#[case::two_stay_reversed(&[1, 0], &[], &["Library 4", "Library 3", "Library 2", "Library 0", "Library 1"])]
#[case::the_others_lie_just_beneath(&[2], &[0, 1], &["Library 4", "Library 3", "Library 1", "Library 0", "Library 2"])]
#[case::all_wait_for_the_graveyard(&[], &[0, 1, 2], &["Library 4", "Library 3", "Library 2", "Library 1", "Library 0"])]
#[case::a_surveil_of_nothing(&[], &[], &["Library 4", "Library 3", "Library 2", "Library 1", "Library 0"])]
fn a_surveil_arranges_the_cards_looked_at_with_those_that_stay_on_top(
    #[case] top: &[usize],
    #[case] graveyard: &[usize],
    #[case] expected: &[&str],
) {
    let mut game = game();
    let event = Event::Surveilled(Surveilled {
        player: PlayerId(0),
        top: from_the_top(&game, top),
        graveyard: from_the_top(&game, graveyard),
    });
    fold(&mut game, &[event]);
    assert_eq!(library_names(&game), expected);
}

#[rstest]
#[case::more_than_the_library_holds(&[0, 1, 2, 3, 4, 5], &[])]
#[case::cards_that_are_not_the_top_ones(&[1, 2], &[])]
#[case::the_same_card_twice(&[0], &[0])]
#[case::a_card_from_below(&[0], &[4])]
fn a_surveil_of_cards_that_are_not_the_top_of_the_library_is_refused(
    #[case] top: &[usize],
    #[case] graveyard: &[usize],
) {
    let mut game = game();
    let before = game.state_hash();
    let pick = |numbers: &[usize]| {
        let library = game.zone(Zone::Library(PlayerId(0)));
        numbers
            .iter()
            .map(|n| {
                library
                    .len()
                    .checked_sub(1 + n)
                    .map_or(ObjectId(900), |at| library[at])
            })
            .collect::<Vec<_>>()
    };
    let event = Event::Surveilled(Surveilled {
        player: PlayerId(0),
        top: pick(top),
        graveyard: pick(graveyard),
    });
    assert!(apply_event(&mut game, &event).is_err());
    assert_eq!(game.state_hash(), before);
}

#[test]
fn a_surveil_shows_the_cards_to_the_player_who_looked_while_the_resolution_lasts() {
    let mut game = game();
    let looked = from_the_top(&game, &[0, 1]);
    game.resolving = Some(crate::resolve::Resolving::begin(totem_id(&game)));
    let event = Event::Surveilled(Surveilled {
        player: PlayerId(0),
        top: vec![looked[0]],
        graveyard: vec![looked[1]],
    });
    apply_event(&mut game, &event).unwrap();
    for card in &looked {
        assert!(game.is_shown_to(PlayerId(0), *card));
        assert!(!game.is_shown_to(PlayerId(1), *card));
    }
}

#[rstest]
#[case::one_card(&[2], &["Library 2", "Library 4", "Library 3", "Library 1", "Library 0"])]
#[case::two_cards_the_last_is_the_bottom(&[0, 3], &["Library 3", "Library 0", "Library 4", "Library 2", "Library 1"])]
#[case::the_bottom_card_stays(&[4], &["Library 4", "Library 3", "Library 2", "Library 1", "Library 0"])]
fn cards_put_on_the_bottom_of_a_library_go_under_the_others_with_the_last_of_them_lowest(
    #[case] cards: &[usize],
    #[case] expected: &[&str],
) {
    let mut game = game();
    let event = Event::LibraryBottomed {
        player: PlayerId(0),
        cards: from_the_top(&game, cards),
        random: false,
    };
    fold(&mut game, &[event]);
    assert_eq!(library_names(&game), expected);
}

#[test]
fn cards_that_are_not_in_the_library_cannot_be_put_on_its_bottom() {
    let mut game = game();
    let before = game.state_hash();
    let in_hand = game.zone(Zone::Hand(PlayerId(0)))[0];
    let top = from_the_top(&game, &[0])[0];
    for cards in [vec![in_hand], vec![top, top]] {
        let event = Event::LibraryBottomed {
            player: PlayerId(0),
            cards,
            random: false,
        };
        assert!(apply_event(&mut game, &event).is_err());
        assert_eq!(game.state_hash(), before);
    }
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

#[test]
fn a_planeswalker_that_leaves_the_battlefield_leaves_the_attackers_aimed_at_it_attacking_nothing() {
    let walker = Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(3),
    );
    let mut game = state(
        vec![vec![], vec![]],
        vec![
            vec![walker],
            vec![creature("Raider", 2, 2), creature("Bear", 2, 2)],
        ],
    );
    let battlefield = game.zone(Zone::Battlefield).to_vec();
    let (walker, raider, bear) = (battlefield[0], battlefield[1], battlefield[2]);
    fold(
        &mut game,
        &[Event::AttackersDeclared {
            player: PlayerId(1),
            attackers: vec![
                (raider, AttackTarget::Planeswalker(walker)),
                (bear, AttackTarget::Player(PlayerId(0))),
            ],
        }],
    );
    fold(
        &mut game,
        &[Event::ZoneChanged {
            object: walker,
            from: Zone::Battlefield,
            to: Zone::Graveyard(PlayerId(0)),
            cause: ZoneCause::Sacrificed,
        }],
    );
    let attackers: Vec<(ObjectId, bool)> = game
        .combat()
        .attackers()
        .iter()
        .map(|a| (a.object(), a.attacks_nothing()))
        .collect();
    assert_eq!(
        attackers,
        [(raider, true), (bear, false)],
        "both go on attacking and only the one aimed at the walker is aimed at nothing"
    );
    let defenders: Vec<Option<PlayerId>> = game
        .combat()
        .attackers()
        .iter()
        .map(|a| a.defender(&game))
        .collect();
    assert_eq!(
        defenders,
        [None, Some(PlayerId(0))],
        "nobody may block an attacker aimed at nothing"
    );
}

/// A Walker holding `loyalty` loyalty counters (and 2/2 as a creature too when `creature`), a Raider that is another
/// seat's and a Bear, on seat 0's side.
fn walker_game(loyalty: u32, also_a_creature: bool) -> (GameState, ObjectId) {
    let mut card = CardDefinition::new("Walker")
        .with_type(CardType::Planeswalker)
        .with_loyalty(loyalty);
    if also_a_creature {
        card = card
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2);
    }
    let game = state(
        vec![vec![], vec![]],
        vec![
            vec![Arc::new(card), creature("Bear", 2, 2)],
            vec![creature("Raider", 4, 4)],
        ],
    );
    let walker = game.zone(Zone::Battlefield)[0];
    (game, walker)
}

fn damage(source: ObjectId, to: ObjectId, amount: u32, deathtouch: bool) -> Event {
    Event::DamageDealt {
        source,
        target: crate::combat::DamageTarget::Object(to),
        amount,
        deathtouch,
        form: crate::combat::DamageForm::Normal,
        toxic: 0,
    }
}

fn loyalty_of(game: &GameState, id: ObjectId) -> u32 {
    game.object(id)
        .and_then(|o| o.permanent())
        .map_or(0, |p| p.counters_of(&CounterKind::Loyalty))
}

#[rstest]
#[case::less_than_it_holds(5, 2, 3)]
#[case::exactly_what_it_holds(5, 5, 0)]
#[case::more_than_it_holds(3, 7, 0)]
fn damage_to_a_planeswalker_removes_that_many_loyalty_counters_and_marks_nothing(
    #[case] loyalty: u32,
    #[case] dealt: u32,
    #[case] left: u32,
) {
    let (mut game, walker) = walker_game(loyalty, false);
    let raider = game.zone(Zone::Battlefield)[2];
    fold(&mut game, &[damage(raider, walker, dealt, false)]);
    assert_eq!(loyalty_of(&game, walker), left);
    let marked = game
        .object(walker)
        .and_then(|o| o.permanent())
        .map(crate::object::PermanentState::damage);
    assert_eq!(
        marked,
        Some(0),
        "damage to a planeswalker is not marked (CR 120.3c)"
    );
}

#[test]
fn damage_to_a_planeswalker_that_is_a_creature_too_marks_damage_and_removes_loyalty() {
    let (mut game, walker) = walker_game(4, true);
    let raider = game.zone(Zone::Battlefield)[2];
    fold(&mut game, &[damage(raider, walker, 3, true)]);
    let permanent = game.object(walker).and_then(|o| o.permanent()).unwrap();
    assert_eq!(
        (
            permanent.counters_of(&CounterKind::Loyalty),
            permanent.damage(),
            permanent.deathtouched()
        ),
        (1, 3, true)
    );
}

#[test]
fn damage_to_a_creature_marks_damage_and_leaves_its_counters_alone() {
    let (mut game, _) = walker_game(4, false);
    let (bear, raider) = (
        game.zone(Zone::Battlefield)[1],
        game.zone(Zone::Battlefield)[2],
    );
    fold(&mut game, &[damage(raider, bear, 3, false)]);
    let permanent = game.object(bear).and_then(|o| o.permanent()).unwrap();
    assert_eq!((permanent.damage(), permanent.counters().count()), (3, 0));
}

#[test]
fn a_loyalty_ability_is_activated_once_for_each_planeswalker_each_turn() {
    let ability = |cost: &str| -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Loyalty":{{"cost":{cost},"effects":[{{"Draw":{{}}}}]}}}}"#
        ))
        .unwrap()
    };
    let card = Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(5)
            .with_rules(ability(r#"{"Plus":1}"#))
            .with_rules(ability(r#"{"Minus":2}"#)),
    );
    let mut game = state(vec![vec![], vec![]], vec![vec![card], vec![]]);
    let walker = game
        .object(game.zone(Zone::Battlefield)[0])
        .unwrap()
        .reference();
    let activate = |ability: usize| Event::AbilityActivated {
        player: PlayerId(0),
        source: walker,
        ability,
        origin: crate::abilities::AbilitySource::Printed,
    };
    assert!(!game.loyalty_spent(walker));
    fold(&mut game, &[activate(0)]);
    assert!(game.loyalty_spent(walker));
    for second in [0, 1] {
        assert!(
            apply_event(&mut game, &activate(second)).is_err(),
            "ability {second} is a second loyalty ability of the turn"
        );
    }
    let hash = game.state_hash();
    fold(
        &mut game,
        &[Event::TurnBegan {
            number: 2,
            active: PlayerId(1),
            extra: false,
        }],
    );
    assert!(!game.loyalty_spent(walker), "a new turn allows one more");
    assert_ne!(game.state_hash(), hash);
    fold(
        &mut game,
        &[
            Event::TurnBegan {
                number: 3,
                active: PlayerId(0),
                extra: false,
            },
            activate(1),
        ],
    );
    assert!(game.loyalty_spent(walker));
}

#[test]
fn a_planeswalker_that_changes_zones_is_a_new_object_that_may_activate_again() {
    let card = Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(5)
            .with_rules(
                serde_json::from_str(r#"{"Loyalty":{"cost":"Zero","effects":[{"Draw":{}}]}}"#)
                    .unwrap(),
            ),
    );
    let mut game = state(vec![vec![], vec![]], vec![vec![card], vec![]]);
    let id = game.zone(Zone::Battlefield)[0];
    let old = game.object(id).unwrap().reference();
    fold(
        &mut game,
        &[
            Event::AbilityActivated {
                player: PlayerId(0),
                source: old,
                ability: 0,
                origin: crate::abilities::AbilitySource::Printed,
            },
            Event::ZoneChanged {
                object: id,
                from: Zone::Battlefield,
                to: Zone::Hand(PlayerId(0)),
                cause: ZoneCause::Moved,
            },
            Event::ZoneChanged {
                object: id,
                from: Zone::Hand(PlayerId(0)),
                to: Zone::Battlefield,
                cause: ZoneCause::Moved,
            },
        ],
    );
    let now = game.object(id).unwrap().reference();
    assert_ne!(now, old);
    assert!(game.loyalty_spent(old));
    assert!(!game.loyalty_spent(now));
}

#[rstest]
#[case::entered_since_the_turn_began(true, None, 0, true, false)]
#[case::held_since_the_turn_began(false, Some(0), 0, false, false)]
#[case::taken_from_another_player(false, Some(1), 0, true, false)]
#[case::taken_and_entered(true, Some(1), 0, true, false)]
#[case::a_turn_of_another_player_leaves_it_as_it_was(true, None, 1, false, true)]
fn a_turn_begins_by_recording_whether_each_permanent_of_its_player_came_under_their_control_since_the_last_one(
    #[case] entered: bool,
    #[case] seen_by: Option<usize>,
    #[case] active: usize,
    #[case] fresh: bool,
    #[case] still_sick: bool,
) {
    let mut game = state(
        vec![vec![], vec![]],
        vec![vec![creature("Holder", 1, 1)], vec![]],
    );
    let holder = game.zone(Zone::Battlefield)[0];
    let permanent = game
        .objects
        .get_mut(&holder)
        .and_then(|object| object.permanent.as_mut())
        .unwrap();
    permanent.summoning_sick = entered;
    permanent.observed = seen_by.map(PlayerId::from_index);
    permanent.new_to = None;
    fold(
        &mut game,
        &[Event::TurnBegan {
            number: 2,
            active: PlayerId::from_index(active),
            extra: false,
        }],
    );
    let permanent = game.object(holder).unwrap().permanent().unwrap();
    assert_eq!(permanent.is_new_to(PlayerId::from_index(active)), fresh);
    assert!(!permanent.is_new_to(PlayerId::from_index(1 - active)));
    assert_eq!(permanent.summoning_sick(), still_sick);
}
