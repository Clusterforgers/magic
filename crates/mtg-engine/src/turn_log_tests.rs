//! Goal: the turn's log records damage, attacks, blocks, activations and counters in event order, `Pred::DidThisTurn` and `Amount::Tally` read it by player, target and combat, a spell's counters count for its controller and an unattributed one for nobody, a new turn clears it, and an object that is not public leaves a record that counts but that no filter on its characteristics selects.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::{Amount, CounterKind, TallyEvent};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use crate::abilities::PutOnStack;
use crate::choose::{Choices, Target};
use crate::combat::{AttackTarget, DamageForm, DamageTarget};
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::object::ObjectRef;
use crate::resolve::{Resolving, Scope};
use crate::snapshot::snapshot;
use crate::state::GameState;
use crate::turn::Step;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

/// Seat 0 holds a Card in hand and controls a Bear; seat 1 controls an Ogre.
fn table() -> GameState {
    state(
        vec![vec![creature("Card", 1, 1)], Vec::new()],
        vec![vec![creature("Bear", 2, 2)], vec![creature("Ogre", 3, 4)]],
    )
}

fn id(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn named(state: &GameState, name: &str) -> ObjectRef {
    state.object(id(state, name)).unwrap().reference()
}

fn fold(state: &mut GameState, events: &[Event]) {
    for event in events {
        apply_event(state, event).unwrap();
    }
}

fn hit(state: &GameState, source: &str, to: DamageTarget, amount: u32) -> Event {
    Event::DamageDealt {
        source: id(state, source),
        target: to,
        amount,
        deathtouch: false,
        form: DamageForm::Normal,
        toxic: 0,
    }
}

/// Whether `name` did what `pred` says, read by the Bear.
fn did(state: &GameState, name: &str, pred: Pred) -> bool {
    Scope::announcing(state, ME, named(state, "Bear"))
        .selects(&Selector(vec![pred]), named(state, name))
        .unwrap()
}

fn pred(event: TallyEvent, by: Option<Ref>, to: Option<Ref>, combat: bool) -> Pred {
    Pred::DidThisTurn {
        event,
        by,
        to,
        combat,
    }
}

fn total(state: &GameState, event: TallyEvent, who: Ref, filter: Vec<Pred>) -> i64 {
    Scope::announcing(state, ME, named(state, "Bear"))
        .amount(&Amount::Tally {
            event,
            who,
            filter: Selector(filter),
        })
        .unwrap()
}

#[rstest]
#[case::the_dealer(None, None, "Bear", true)]
#[case::not_the_victim(None, None, "Ogre", false)]
#[case::to_the_opponents(None, Some(Ref::Opponents), "Bear", true)]
#[case::not_to_me(None, Some(Ref::You), "Bear", false)]
#[case::for_my_creatures(Some(Ref::You), None, "Bear", true)]
#[case::not_for_theirs(Some(Ref::Opponents), None, "Bear", false)]
fn a_dealer_of_damage_is_found_by_whom_it_hit_and_whose_it_is(
    #[case] by: Option<Ref>,
    #[case] to: Option<Ref>,
    #[case] who: &str,
    #[case] expected: bool,
) {
    let mut game = table();
    let event = hit(&game, "Bear", DamageTarget::Player(FOE), 2);
    fold(&mut game, &[event]);
    let found = did(&game, who, pred(TallyEvent::Damaged, by, to, false));
    assert_eq!(found, expected);
}

#[test]
fn damage_adds_up_by_amount_for_the_controller_of_the_dealer() {
    let mut game = table();
    let (one, two) = (
        hit(&game, "Bear", DamageTarget::Player(FOE), 2),
        hit(&game, "Ogre", DamageTarget::Object(id(&game, "Bear")), 3),
    );
    fold(&mut game, &[one, two]);
    assert_eq!(total(&game, TallyEvent::Damaged, Ref::You, vec![]), 2);
    assert_eq!(total(&game, TallyEvent::Damaged, Ref::Opponents, vec![]), 3);
    let creatures = vec![Pred::Type(CardType::Creature)];
    assert_eq!(
        total(&game, TallyEvent::Damaged, Ref::EachPlayer, creatures),
        5
    );
}

#[test]
fn an_attack_a_block_and_the_combat_they_belong_to_are_told_apart() {
    let mut game = table();
    let begin = Event::StepBegan {
        step: Step::BeginCombat,
    };
    fold(&mut game, std::slice::from_ref(&begin));
    let attack = Event::AttackersDeclared {
        player: ME,
        attackers: vec![(id(&game, "Bear"), AttackTarget::Player(FOE))],
    };
    let block = Event::BlockersDeclared {
        player: FOE,
        blocks: vec![(id(&game, "Ogre"), id(&game, "Bear"))],
    };
    fold(&mut game, &[attack, block]);
    let attacked = pred(TallyEvent::Attacked, None, Some(Ref::Opponents), true);
    let blocked = pred(
        TallyEvent::Blocked,
        Some(Ref::Opponents),
        Some(Ref::This),
        false,
    );
    assert!(did(&game, "Bear", attacked.clone()));
    assert!(!did(&game, "Ogre", attacked));
    assert!(did(&game, "Ogre", blocked.clone()));
    assert!(!did(&game, "Bear", blocked));
    // The next combat phase starts "this combat" afresh; the turn still remembers.
    fold(&mut game, &[begin]);
    let this_combat = pred(TallyEvent::Attacked, None, None, true);
    let this_turn = pred(TallyEvent::Attacked, None, None, false);
    assert!(!did(&game, "Bear", this_combat));
    assert!(did(&game, "Bear", this_turn));
    assert_eq!(total(&game, TallyEvent::Blocked, Ref::Opponents, vec![]), 1);
}

fn watcher() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Triggered {
                when: serde_json::from_str(r#"{"Attacks":{"what":[]}}"#).unwrap(),
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: Vec::new(),
            }),
    )
}

#[rstest]
#[case::an_ability_you_activated(ME, 1, 0)]
#[case::an_ability_they_activated(FOE, 0, 1)]
fn an_activation_counts_for_the_player_who_activated_it(
    #[case] who: PlayerId,
    #[case] mine: i64,
    #[case] theirs: i64,
) {
    let mut game = state(
        vec![Vec::new(), Vec::new()],
        vec![vec![watcher(), creature("Bear", 2, 2)], Vec::new()],
    );
    let source = named(&game, "Watcher");
    let put = Event::AbilityPutOnStack(Box::new(PutOnStack {
        origin: crate::abilities::AbilitySource::default(),
        object: game.next_object(),
        source,
        snapshot: snapshot(&game, source.id).unwrap(),
        ability: 0,
        controller: who,
        choices: Choices::default(),
        cause: None,
        delayed: None,
    }));
    fold(&mut game, &[put]);
    assert_eq!(total(&game, TallyEvent::Activated, Ref::You, vec![]), mine);
    assert_eq!(
        total(&game, TallyEvent::Activated, Ref::Opponents, vec![]),
        theirs
    );
}

#[test]
fn counters_count_for_the_controller_of_the_spell_that_put_them_and_for_nobody_otherwise() {
    let mut game = table();
    let put = |game: &GameState, delta: i32| {
        Event::CountersChanged(CountersChanged {
            on: Target::Object(named(game, "Ogre")),
            counter: CounterKind::PlusOnePlusOne,
            delta,
        })
    };
    // Put by the turn itself, with no spell: counted for no one.
    let unattributed = put(&game, 1);
    fold(&mut game, &[unattributed]);
    assert_eq!(
        total(&game, TallyEvent::CountersPut, Ref::EachPlayer, vec![]),
        0
    );
    let counted = pred(TallyEvent::CountersPut, None, None, false);
    assert!(did(&game, "Ogre", counted));
    // Put by my spell: three counters for me; a removal is not a put.
    game.resolving = Some(Resolving::begin(id(&game, "Card")));
    let (three, removed) = (put(&game, 3), put(&game, -1));
    fold(&mut game, &[three, removed]);
    assert_eq!(total(&game, TallyEvent::CountersPut, Ref::You, vec![]), 3);
    assert_eq!(
        total(&game, TallyEvent::CountersPut, Ref::Opponents, vec![]),
        0
    );
    let mine = pred(TallyEvent::CountersPut, Some(Ref::You), None, false);
    assert!(did(&game, "Ogre", mine));
}

#[test]
fn a_new_turn_clears_the_log() {
    let mut game = table();
    let event = hit(&game, "Bear", DamageTarget::Player(FOE), 2);
    fold(&mut game, &[event]);
    game.clear_turn_uses();
    assert_eq!(
        total(&game, TallyEvent::Damaged, Ref::EachPlayer, vec![]),
        0
    );
    assert!(!did(
        &game,
        "Bear",
        pred(TallyEvent::Damaged, None, None, false)
    ));
}

#[test]
fn a_hidden_object_counts_but_no_filter_on_it_selects_and_nothing_names_it() {
    let mut game = table();
    let card = id(&game, "Card");
    game.record(TallyEvent::CountersPut, Some(ME), (card, None), 1);
    assert_eq!(total(&game, TallyEvent::CountersPut, Ref::You, vec![]), 1);
    // A negative filter would select an absent subject: it must not.
    let not_token = vec![Pred::Not(Box::new(Pred::Token))];
    assert_eq!(
        total(&game, TallyEvent::CountersPut, Ref::You, not_token),
        0
    );
    assert_eq!(game.tallies.records[0].subject, None);
    assert!(!did(
        &game,
        "Card",
        pred(TallyEvent::CountersPut, None, None, false)
    ));
}
