//! Goal: each predicate atom of the cast, library, combat, target, greatest and suspend family holds exactly for the objects the game's facts make it true of, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::{CastWay, CounterKind, StatKind};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;
use std::sync::Arc;

use super::apply_event;
use crate::choose::{Choices, Target};
use crate::combat::{AttackTarget, Attacker};
use crate::counters::CountersChanged;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// Seat 0 holds Rat and Bolt in hand and Bear, Owl and a Guest on the battlefield; every other seat a Guest.
fn table(seats: usize) -> GameState {
    let hand = |seat: usize| {
        if seat == 0 {
            vec![creature("Rat", 1, 1), creature("Bolt", 1, 1)]
        } else {
            Vec::new()
        }
    };
    let field = |seat: usize| {
        let mut cards = vec![creature(&format!("Guest {seat}"), 2, 2)];
        if seat == 0 {
            cards.extend([creature("Bear", 3, 3), creature("Owl", 1, 1)]);
        }
        cards
    };
    state(
        (0..seats).map(hand).collect(),
        (0..seats).map(field).collect(),
    )
}

fn id(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn fold(state: &mut GameState, events: &[Event]) {
    for event in events {
        apply_event(state, event).unwrap();
    }
}

fn holds(state: &GameState, pred: Pred, name: &str) -> bool {
    let object = state.object(id(state, name)).unwrap().reference();
    let source = state.object(id(state, "Guest 0")).unwrap().reference();
    Scope::announcing(state, ME, source)
        .selects(&Selector(vec![pred]), object)
        .unwrap()
}

/// Seat 0 casts the card from its hand.
fn cast(game: &mut GameState, name: &str, choices: Choices) {
    let object = id(game, name);
    let event = Event::SpellCast {
        player: ME,
        object,
        choices,
    };
    fold(game, &[event]);
}

fn moved(game: &mut GameState, name: &str, from: Zone, to: Zone) {
    let event = Event::ZoneChanged {
        object: id(game, name),
        from,
        to,
        cause: ZoneCause::Moved,
    };
    fold(game, &[event]);
}

fn named(name: &str) -> Ref {
    Ref::Each(Selector(vec![Pred::Named(name.to_owned())]))
}

#[rstest]
#[case::anyone_anywhere(None, None, true)]
#[case::by_me(Some(Ref::You), None, true)]
#[case::by_opponents(Some(Ref::Opponents), None, false)]
#[case::from_hand(None, Some(ZoneKind::Hand), true)]
#[case::from_graveyard(None, Some(ZoneKind::Graveyard), false)]
#[case::from_library(None, Some(ZoneKind::Library), false)]
#[case::by_me_from_hand(Some(Ref::You), Some(ZoneKind::Hand), true)]
#[case::by_opponents_from_hand(Some(Ref::Opponents), Some(ZoneKind::Hand), false)]
fn a_spell_and_its_permanent_remember_who_cast_them_from_where(
    #[values(2, 3, 4)] seats: usize,
    #[case] by: Option<Ref>,
    #[case] from: Option<ZoneKind>,
    #[case] expected: bool,
) {
    let mut game = table(seats);
    let pred = Pred::WasCast { by, from };
    assert!(!holds(&game, pred.clone(), "Rat"));
    cast(&mut game, "Rat", Choices::default());
    assert_eq!(holds(&game, pred.clone(), "Rat"), expected);
    moved(&mut game, "Rat", Zone::Stack, Zone::Battlefield);
    assert_eq!(holds(&game, pred.clone(), "Rat"), expected);
    assert!(!holds(&game, pred.clone(), "Bear"));
    moved(&mut game, "Rat", Zone::Battlefield, Zone::Graveyard(ME));
    assert!(!holds(&game, pred, "Rat"));
}

#[rstest]
#[case::flashback(CastWay::Flashback, CastWay::Flashback, true)]
#[case::other_way(CastWay::Flashback, CastWay::Evoke, false)]
#[case::overload(CastWay::Overload, CastWay::Overload, true)]
fn cast_with_reads_the_alternative_cost_of_the_spell_and_the_permanent(
    #[values(2, 3, 4)] seats: usize,
    #[case] paid: CastWay,
    #[case] asked: CastWay,
    #[case] expected: bool,
) {
    let mut game = table(seats);
    let choices = Choices {
        way: Some(paid),
        ..Choices::default()
    };
    cast(&mut game, "Rat", choices);
    assert_eq!(holds(&game, Pred::CastWith(asked), "Rat"), expected);
    moved(&mut game, "Rat", Zone::Stack, Zone::Battlefield);
    assert_eq!(holds(&game, Pred::CastWith(asked), "Rat"), expected);
    assert!(!holds(&game, Pred::CastWith(asked), "Bear"));
}

#[rstest]
fn only_the_ends_of_a_library_are_its_top_and_bottom(#[values(2, 3, 4)] seats: usize) {
    let mut game = table(seats);
    let library = game.zone(Zone::Library(ME)).to_vec();
    let name_at =
        |game: &GameState, at: usize| game.object(library[at]).unwrap().card().name.clone();
    let last = library.len() - 1;
    let (top, bottom) = (name_at(&game, last), name_at(&game, 0));
    for (pred, end) in [(Pred::TopOfLibrary, &top), (Pred::BottomOfLibrary, &bottom)] {
        assert!(holds(&game, pred.clone(), end));
        assert!(!holds(&game, pred.clone(), "Bear"));
        assert!(!holds(&game, pred, "Rat"));
    }
    // Names repeat in a library, so the middle is checked by object.
    let source = game.object(id(&game, "Guest 0")).unwrap().reference();
    let scope = Scope::announcing(&game, ME, source);
    let middle = game.object(library[1]).unwrap().reference();
    assert!(
        !scope
            .selects(&Selector(vec![Pred::TopOfLibrary]), middle)
            .unwrap()
    );
    assert!(
        !scope
            .selects(&Selector(vec![Pred::BottomOfLibrary]), middle)
            .unwrap()
    );
    let drawn = Event::ZoneChanged {
        object: library[last],
        from: Zone::Library(ME),
        to: Zone::Hand(ME),
        cause: ZoneCause::Moved,
    };
    fold(&mut game, &[drawn]);
    let source = game.object(id(&game, "Guest 0")).unwrap().reference();
    let scope = Scope::announcing(&game, ME, source);
    let next = game.object(library[last - 1]).unwrap().reference();
    assert!(
        scope
            .selects(&Selector(vec![Pred::TopOfLibrary]), next)
            .unwrap()
    );
}

#[rstest]
#[case::blocked(true)]
#[case::unblocked(false)]
fn blocked_by_and_blocks_pair_an_attacker_with_its_blockers(
    #[values(2, 3, 4)] seats: usize,
    #[case] blocked: bool,
) {
    let mut game = table(seats);
    let (bear, guest) = (id(&game, "Bear"), id(&game, "Guest 1"));
    game.combat.attackers.push(Attacker {
        object: bear,
        target: AttackTarget::Player(PlayerId(1)),
        blocked,
        blockers: if blocked { vec![guest] } else { Vec::new() },
        assignment: None,
        target_left: false,
    });
    assert_eq!(
        holds(&game, Pred::BlockedBy(named("Guest 1")), "Bear"),
        blocked
    );
    assert_eq!(
        holds(&game, Pred::Blocks(named("Bear")), "Guest 1"),
        blocked
    );
    assert!(!holds(&game, Pred::BlockedBy(named("Owl")), "Bear"));
    assert!(!holds(&game, Pred::Blocks(named("Owl")), "Guest 1"));
    assert!(!holds(&game, Pred::BlockedBy(named("Guest 1")), "Owl"));
    assert!(!holds(&game, Pred::Blocks(named("Bear")), "Owl"));
}

#[rstest]
#[case::the_target(true, "Bear", true)]
#[case::another_object(true, "Owl", false)]
#[case::a_spell_with_no_target(false, "Bear", false)]
fn targeted_by_finds_the_targets_of_a_spell_on_the_stack(
    #[values(2, 3, 4)] seats: usize,
    #[case] has_target: bool,
    #[case] object: &str,
    #[case] expected: bool,
) {
    let mut game = table(seats);
    let bear = game.object(id(&game, "Bear")).unwrap().reference();
    let choices = Choices {
        targets: if has_target {
            vec![vec![Target::Object(bear)]]
        } else {
            Vec::new()
        },
        ..Choices::default()
    };
    cast(&mut game, "Bolt", choices);
    let bolt = Ref::Each(Selector(vec![Pred::Spell, Pred::Named("Bolt".to_owned())]));
    assert_eq!(holds(&game, Pred::TargetedBy(bolt), object), expected);
}

#[rstest]
#[case::power(StatKind::Power, &["Bear"])]
#[case::toughness(StatKind::Toughness, &["Bear"])]
#[case::mana_value(StatKind::ManaValue, &["Bear", "Owl", "Guest 0"])]
fn greatest_among_takes_every_object_tied_for_the_most(
    #[values(2, 3, 4)] seats: usize,
    #[case] stat: StatKind,
    #[case] winners: &[&str],
) {
    let game = table(seats);
    let group = Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::You),
    ]);
    let pred = Pred::GreatestAmong { stat, group };
    for name in ["Bear", "Owl", "Guest 0"] {
        assert_eq!(
            holds(&game, pred.clone(), name),
            winners.contains(&name),
            "{name}"
        );
    }
}

#[rstest]
#[case::power(StatKind::Power)]
#[case::toughness(StatKind::Toughness)]
fn least_among_takes_only_the_smallest_and_not_the_greatest(
    #[values(2, 3, 4)] seats: usize,
    #[case] stat: StatKind,
) {
    let game = table(seats);
    let group = Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::You),
    ]);
    let pred = Pred::LeastAmong { stat, group };
    assert!(holds(&game, pred.clone(), "Owl"));
    assert!(!holds(&game, pred.clone(), "Guest 0"));
    assert!(!holds(&game, pred, "Bear"));
}

#[test]
fn greatest_among_follows_a_growing_creature() {
    let mut game = table(2);
    let owl = game.object(id(&game, "Owl")).unwrap().reference();
    let group = Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::You),
    ]);
    let pred = Pred::GreatestAmong {
        stat: StatKind::Power,
        group,
    };
    let grown = Event::CountersChanged(CountersChanged {
        on: Target::Object(owl),
        counter: CounterKind::PlusOnePlusOne,
        delta: 5,
    });
    fold(&mut game, &[grown]);
    assert!(holds(&game, pred.clone(), "Owl"));
    assert!(!holds(&game, pred, "Bear"));
}

#[rstest]
#[case::any_value_of_the_keyword("Toxic", &["Toxic 1", "Toxic 3"])]
#[case::another_keyword("Flying", &[])]
#[case::unspoken("Cycling", &[])]
fn has_keyword_named_holds_at_any_value_of_the_parameter(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] holders: &[&str],
) {
    let toxic = |label: &str, n: u32| {
        Arc::new(
            CardDefinition::new(label)
                .with_type(CardType::Creature)
                .with_power_toughness(1, 1)
                .with_rules(Ability::Keyword(Keyword::Toxic(n))),
        )
    };
    let game = state(
        (0..seats).map(|_| Vec::new()).collect(),
        (0..seats)
            .map(|s| {
                if s == 0 {
                    vec![
                        toxic("Toxic 1", 1),
                        toxic("Toxic 3", 3),
                        creature("Plain", 1, 1),
                        creature("Guest 0", 2, 2),
                    ]
                } else {
                    vec![creature(&format!("Guest {s}"), 2, 2)]
                }
            })
            .collect(),
    );
    for label in ["Toxic 1", "Toxic 3", "Plain"] {
        assert_eq!(
            holds(&game, Pred::HasKeywordNamed(name.to_owned()), label),
            holders.contains(&label),
            "{label}"
        );
    }
}

#[rstest]
#[case::with_time_counters(3, true)]
#[case::without_counters(0, false)]
fn suspended_is_a_card_in_exile_with_time_counters_and_suspend(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] expected: bool,
) {
    let sus = Arc::new(
        CardDefinition::new("Sus")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Keyword(Keyword::Suspend {
                time: 3,
                cost: Cost::mana("{R}".parse().unwrap()),
            })),
    );
    let hand = |seat: usize| {
        if seat == 0 {
            vec![sus.clone(), creature("Plain", 1, 1)]
        } else {
            Vec::new()
        }
    };
    let mut game = state(
        (0..seats).map(hand).collect(),
        (0..seats)
            .map(|s| vec![creature(&format!("Guest {s}"), 2, 2)])
            .collect(),
    );
    for name in ["Sus", "Plain"] {
        moved(&mut game, name, Zone::Hand(ME), Zone::Exile);
        let object = id(&game, name);
        game.objects
            .get_mut(&object)
            .unwrap()
            .card_counters
            .insert(crate::keywords::suspend::time_counter(), counters);
    }
    assert_eq!(holds(&game, Pred::Suspended, "Sus"), expected);
    assert!(!holds(&game, Pred::Suspended, "Plain"));
    assert!(!holds(&game, Pred::Suspended, "Guest 0"));
}
