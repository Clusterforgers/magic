//! Goal: an effect that lasts "for as long as you control ~" applies while its player controls the source as the layers leave it, ends for good when another player controls the source or the source leaves, does nothing when it begins under another player's control, never reads the control it is itself deriving, and settles a loop of such effects by timestamp the same way every time.
use mtg_cards::modify::Duration;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use super::tests::{fold, named};
use crate::characteristics::controller_of;
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::sba::housekeeping;
use crate::state::GameState;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

/// Seat 0 has the Flayer and a Mate, seat 1 has an Ogre.
fn game() -> GameState {
    state(
        vec![vec![], vec![]],
        vec![
            vec![creature("Flayer", 3, 3), creature("Mate", 1, 1)],
            vec![creature("Ogre", 4, 4)],
        ],
    )
}

fn controls(game: &GameState, name: &str) -> PlayerId {
    controller_of(game, named(game, name).id).unwrap()
}

/// `who` takes the object `taken` for as long as they control `source`, as an effect with the id and the timestamp.
fn takes(game: &mut GameState, id: u64, who: PlayerId, source: &str, taken: &str, at: u64) {
    let source = named(game, source);
    let effect = ContinuousEffect::new(
        EffectId(id),
        Selector::Objects(vec![named(game, taken)]),
        Modification::SetController(who),
        Duration::WhileYouControlSource,
        at,
    )
    .from_source(source)
    .begun(who, 1);
    fold(game, &[Event::EffectCreated { effect }]);
}

/// A plain taking of `name` for good, which comes after the effects made so far.
fn steals(game: &mut GameState, id: u64, who: PlayerId, name: &str, at: u64) {
    let effect = ContinuousEffect::new(
        EffectId(id),
        Selector::Objects(vec![named(game, name)]),
        Modification::SetController(who),
        Duration::Forever,
        at,
    )
    .begun(who, 1);
    fold(game, &[Event::EffectCreated { effect }]);
}

fn expired(game: &GameState) -> Vec<EffectId> {
    housekeeping(game)
        .into_iter()
        .filter_map(|event| match event {
            Event::EffectExpired { id } => Some(id),
            _ => None,
        })
        .collect()
}

#[test]
fn the_effect_applies_while_its_player_controls_the_source() {
    let mut game = game();
    takes(&mut game, 1, ME, "Flayer", "Ogre", 5);
    assert_eq!(controls(&game, "Ogre"), ME);
    assert_eq!(expired(&game), Vec::<EffectId>::new());
}

#[test]
fn another_players_control_of_the_source_ends_it_for_good() {
    let mut game = game();
    takes(&mut game, 1, ME, "Flayer", "Ogre", 5);
    steals(&mut game, 2, FOE, "Flayer", 6);
    assert_eq!(controls(&game, "Flayer"), FOE);
    assert_eq!(controls(&game, "Ogre"), FOE, "the Ogre is home again");
    assert_eq!(expired(&game), vec![EffectId(1)]);
    fold(&mut game, &[Event::EffectExpired { id: EffectId(1) }]);
    // The taking of the Flayer is itself undone by a later effect: the Ogre does not follow the Flayer back.
    steals(&mut game, 3, ME, "Flayer", 7);
    assert_eq!(controls(&game, "Flayer"), ME);
    assert_eq!(controls(&game, "Ogre"), FOE);
}

#[test]
fn an_effect_that_begins_when_the_player_does_not_control_the_source_does_nothing_and_ends() {
    let mut game = game();
    takes(&mut game, 1, FOE, "Flayer", "Mate", 5);
    assert_eq!(controls(&game, "Mate"), ME, "the Flayer is not the Foe's");
    assert_eq!(expired(&game), vec![EffectId(1)]);
}

#[test]
fn a_source_that_left_ends_the_effect() {
    let mut game = game();
    takes(&mut game, 1, ME, "Flayer", "Ogre", 5);
    let flayer: ObjectId = named(&game, "Flayer").id;
    let owner = game.object(flayer).unwrap().owner();
    fold(
        &mut game,
        &[Event::ZoneChanged {
            object: flayer,
            from: crate::zones::Zone::Battlefield,
            to: crate::zones::Zone::Graveyard(owner),
            cause: crate::events::ZoneCause::Destroyed,
        }],
    );
    assert_eq!(controls(&game, "Ogre"), FOE);
    assert_eq!(expired(&game), vec![EffectId(1)]);
}

#[test]
fn two_sources_that_take_each_other_are_settled_by_timestamp() {
    // The Flayer's player takes the Ogre while they control the Flayer, the Ogre's player takes the Flayer while they
    // control the Ogre: the later effect wins and the earlier one ends, the same way in every run.
    let outcome = || {
        let mut game = game();
        takes(&mut game, 1, ME, "Flayer", "Ogre", 5);
        takes(&mut game, 2, FOE, "Ogre", "Flayer", 6);
        (
            controls(&game, "Flayer"),
            controls(&game, "Ogre"),
            expired(&game),
        )
    };
    let (flayer, ogre, ended) = outcome();
    assert_eq!((flayer, ogre), (FOE, FOE));
    assert_eq!(ended, vec![EffectId(1)]);
    assert_eq!(outcome(), (flayer, ogre, ended));
}
