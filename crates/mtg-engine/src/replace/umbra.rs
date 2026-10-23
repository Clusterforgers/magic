//! Goal: umbra armor replaces every destruction of the creature its Aura enchants (a destroy effect, one that can't be regenerated, lethal damage, deathtouch) with the removal of the creature's damage and the destruction of the Aura, and nothing else (not a sacrifice, an exile, zero toughness or the destruction of another creature); the Aura's own destruction is an event other effects replace, so an indestructible Aura stays and an Aura the batch already moves is moved once; two Auras on one creature, or an Aura and a regeneration shield, ask the controller of the creature which applies; at tables of two, three and four seats.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Duration;
use mtg_core::card::CardType;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use rstest::rstest;

use super::{Processed, process};
use crate::choose::Choice;
use crate::decision::DecisionKind;
use crate::events::{DeathCause, Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::rules::{Rule, RuleEffect};
use crate::state::GameState;
use crate::zones::Zone;

fn seat(n: usize) -> PlayerId {
    PlayerId(u64::try_from(n).unwrap())
}

/// An Aura with umbra armor and these other keywords.
fn umbra(name: &str, keywords: &[Keyword]) -> Arc<CardDefinition> {
    let aura = CardDefinition::new(name)
        .with_type(CardType::Enchantment)
        .with_subtype("Aura")
        .with_rules(mtg_cards::ability::Ability::Keyword(Keyword::UmbraArmor));
    Arc::new(keywords.iter().cloned().fold(aura, |card, keyword| {
        card.with_rules(mtg_cards::ability::Ability::Keyword(keyword))
    }))
}

fn id(game: &GameState, name: &str) -> ObjectId {
    game.objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

/// Every seat controls a Bear of its number; seat 1 also controls these Auras, each attached to the Bear it is paired with.
fn game_on(seats: usize, auras: Vec<(Arc<CardDefinition>, &str)>) -> GameState {
    let hands = vec![Vec::new(); seats];
    let mut fields: Vec<Vec<Arc<CardDefinition>>> = (0..seats)
        .map(|n| vec![creature(&format!("Bear{n}"), 2, 2)])
        .collect();
    fields[1].extend(auras.iter().map(|(aura, _)| Arc::clone(aura)));
    let mut table = state(hands, fields);
    for (aura, host) in &auras {
        let (object, to) = (id(&table, &aura.name), id(&table, host));
        apply_event(&mut table, &Event::Attached { object, to }).unwrap();
    }
    table
}

/// The Auras are attached to `Bear0`.
fn game(seats: usize, auras: Vec<Arc<CardDefinition>>) -> GameState {
    game_on(seats, auras.into_iter().map(|aura| (aura, "Bear0")).collect())
}

fn move_of(game: &GameState, name: &str, cause: ZoneCause) -> Event {
    Event::ZoneChanged {
        object: id(game, name),
        from: Zone::Battlefield,
        to: Zone::Graveyard(game.object(id(game, name)).unwrap().owner()),
        cause,
    }
}

fn run(game: &GameState, batch: &[Event]) -> Vec<Event> {
    match process(game, batch.to_vec(), &[]).unwrap() {
        Processed::Events(events) => events,
        other @ Processed::Ask { .. } => panic!("events were expected, got {other:?}"),
    }
}

/// The question the batch needs answered: who asked and the objects whose abilities or shields are the candidates.
fn ask(game: &GameState, batch: &[Event]) -> (PlayerId, Vec<ObjectId>) {
    match process(game, batch.to_vec(), &[]).unwrap() {
        Processed::Ask { decision, .. } => match decision.kind {
            DecisionKind::ChooseReplacement(options) => (
                decision.player,
                options.candidates.iter().map(|c| c.source).collect(),
            ),
            other => panic!("the order of replacements was asked, got {other:?}"),
        },
        other @ Processed::Events(_) => panic!("a question was expected, got {other:?}"),
    }
}

fn answered(game: &GameState, batch: &[Event], pick: usize) -> Vec<Event> {
    match process(
        game,
        batch.to_vec(),
        &[(seat(0), Choice::Replacement(pick))],
    )
    .unwrap()
    {
        Processed::Events(events) => events,
        other @ Processed::Ask { .. } => panic!("events were expected, got {other:?}"),
    }
}

/// The events of a saved creature: its damage removed and its Aura destroyed.
fn saved(game: &GameState, aura: &str) -> Vec<Event> {
    vec![
        Event::DamageRemoved {
            object: id(game, "Bear0"),
        },
        move_of(game, aura, ZoneCause::Destroyed),
    ]
}

#[rstest]
#[case::destroy_effect(ZoneCause::Destroyed)]
#[case::destroy_that_cant_be_regenerated(ZoneCause::Buried)]
fn a_destruction_by_an_effect_is_replaced_by_removing_the_damage_and_destroying_the_aura(
    #[values(2, 3, 4)] seats: usize,
    #[case] cause: ZoneCause,
) {
    let table = game(seats, vec![umbra("Umbra", &[])]);
    let result = run(&table, &[move_of(&table, "Bear0", cause)]);
    assert_eq!(result, saved(&table, "Umbra"));
}

#[rstest]
#[case::lethal_damage(DeathCause::LethalDamage)]
#[case::deathtouch(DeathCause::Deathtouch)]
fn a_death_by_damage_is_replaced_the_same_way(
    #[values(2, 3, 4)] seats: usize,
    #[case] cause: DeathCause,
) {
    let table = game(seats, vec![umbra("Umbra", &[])]);
    let died = Event::CreatureDied {
        object: id(&table, "Bear0"),
        cause,
    };
    assert_eq!(run(&table, &[died]), saved(&table, "Umbra"));
}

#[rstest]
#[case::sacrificed(ZoneCause::Sacrificed)]
#[case::exiled(ZoneCause::Exiled)]
#[case::state_based(ZoneCause::StateBased)]
#[case::bounced(ZoneCause::Moved)]
fn a_move_that_is_no_destruction_is_not_replaced(
    #[values(2, 3, 4)] seats: usize,
    #[case] cause: ZoneCause,
) {
    let table = game(seats, vec![umbra("Umbra", &[])]);
    let moved = move_of(&table, "Bear0", cause);
    assert_eq!(run(&table, std::slice::from_ref(&moved)), [moved]);
}

#[test]
fn zero_toughness_is_no_destruction() {
    let table = game(2, vec![umbra("Umbra", &[])]);
    let died = Event::CreatureDied {
        object: id(&table, "Bear0"),
        cause: DeathCause::ZeroToughness,
    };
    assert_eq!(run(&table, std::slice::from_ref(&died)), [died]);
}

#[rstest]
fn the_destruction_of_another_creature_is_not_replaced(#[values(2, 3, 4)] seats: usize) {
    let table = game(seats, vec![umbra("Umbra", &[])]);
    let moved = move_of(&table, "Bear1", ZoneCause::Destroyed);
    assert_eq!(run(&table, std::slice::from_ref(&moved)), [moved]);
}

#[rstest]
fn an_indestructible_aura_stays_while_the_creature_is_saved(#[values(2, 3, 4)] seats: usize) {
    let table = game(seats, vec![umbra("Umbra", &[Keyword::Indestructible])]);
    let result = run(&table, &[move_of(&table, "Bear0", ZoneCause::Destroyed)]);
    assert_eq!(
        result,
        [Event::DamageRemoved {
            object: id(&table, "Bear0")
        }]
    );
}

#[rstest]
fn an_aura_the_batch_destroys_anyway_is_destroyed_once(#[values(2, 3, 4)] seats: usize) {
    let table = game(seats, vec![umbra("Umbra", &[])]);
    let batch = [
        move_of(&table, "Bear0", ZoneCause::Destroyed),
        move_of(&table, "Umbra", ZoneCause::Destroyed),
    ];
    assert_eq!(run(&table, &batch), saved(&table, "Umbra"));
}

#[rstest]
fn a_batch_saves_each_creature_with_its_own_aura(#[values(3, 4)] seats: usize) {
    let table = game_on(
        seats,
        vec![(umbra("Umbra", &[]), "Bear0"), (umbra("Second", &[]), "Bear2")],
    );
    let batch = [
        move_of(&table, "Bear0", ZoneCause::Destroyed),
        move_of(&table, "Bear2", ZoneCause::Buried),
        move_of(&table, "Bear1", ZoneCause::Destroyed),
    ];
    let result = run(&table, &batch);
    assert_eq!(
        result,
        [
            Event::DamageRemoved {
                object: id(&table, "Bear0")
            },
            move_of(&table, "Umbra", ZoneCause::Destroyed),
            Event::DamageRemoved {
                object: id(&table, "Bear2")
            },
            move_of(&table, "Second", ZoneCause::Destroyed),
            move_of(&table, "Bear1", ZoneCause::Destroyed),
        ]
    );
}

#[rstest]
#[case::first(0, "Alpha")]
#[case::second(1, "Beta")]
fn two_auras_on_one_creature_ask_which_applies_and_the_other_stays(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: usize,
    #[case] spent: &str,
) {
    let table = game(seats, vec![umbra("Alpha", &[]), umbra("Beta", &[])]);
    let batch = [move_of(&table, "Bear0", ZoneCause::Destroyed)];
    let (asked, sources) = ask(&table, &batch);
    assert_eq!(asked, seat(0), "the controller of the creature chooses");
    assert_eq!(sources, [id(&table, "Alpha"), id(&table, "Beta")]);
    assert_eq!(answered(&table, &batch, pick), saved(&table, spent));
}

fn regeneration(game: &mut GameState) -> EffectId {
    let bear = game.object(id(game, "Bear0")).unwrap();
    let shield = EffectId(game.next_effect);
    let effect = RuleEffect {
        id: shield,
        rule: Rule::Regenerate(bear.reference()),
        duration: Duration::UntilEndOfTurn,
        source: Some(bear.reference()),
        controller: bear.controller(),
        turn: 1,
    };
    apply_event(game, &Event::RuleCreated { effect }).unwrap();
    shield
}

#[rstest]
#[case::armor(0, true)]
#[case::regeneration(1, false)]
fn a_regeneration_shield_and_umbra_armor_are_both_offered(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: usize,
    #[case] armor: bool,
) {
    let mut table = game(seats, vec![umbra("Umbra", &[])]);
    let shield = regeneration(&mut table);
    let batch = [move_of(&table, "Bear0", ZoneCause::Destroyed)];
    let (asked, sources) = ask(&table, &batch);
    assert_eq!(asked, seat(0));
    assert_eq!(sources.len(), 2);
    let result = answered(&table, &batch, pick);
    if armor {
        assert_eq!(result, saved(&table, "Umbra"));
    } else {
        assert_eq!(
            result,
            [
                Event::Regenerated {
                    object: id(&table, "Bear0")
                },
                Event::RuleExpired { id: shield }
            ]
        );
    }
}

#[rstest]
fn a_regeneration_shield_on_the_aura_replaces_the_destruction_the_armor_causes(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut table = game(seats, vec![umbra("Umbra", &[])]);
    let aura = table.object(id(&table, "Umbra")).unwrap();
    let shield = EffectId(table.next_effect);
    let effect = RuleEffect {
        id: shield,
        rule: Rule::Regenerate(aura.reference()),
        duration: Duration::UntilEndOfTurn,
        source: Some(aura.reference()),
        controller: aura.controller(),
        turn: 1,
    };
    apply_event(&mut table, &Event::RuleCreated { effect }).unwrap();
    let result = run(&table, &[move_of(&table, "Bear0", ZoneCause::Destroyed)]);
    assert_eq!(
        result,
        [
            Event::DamageRemoved {
                object: id(&table, "Bear0")
            },
            Event::Regenerated {
                object: id(&table, "Umbra")
            },
            Event::RuleExpired { id: shield },
        ]
    );
}
