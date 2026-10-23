//! Goal: whole bot games with decks of the four Umbra Auras (Eel, Snake, Bear and Octopus Umbra) against destroy effects, Wrath of God, burn, deathtouch and regeneration finish and keep every invariant for many seeds at tables of two, three and four seats, and the armor really works in them: a creature never dies by destruction (a destroy effect, a destruction that can't be regenerated, lethal damage, deathtouch) while an Umbra Aura is attached to it, every removal of damage by the armor is followed in its batch by the destruction of one of the Auras attached to the creature, a creature that was saved is on the battlefield afterwards without damage, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::events::{DeathCause, ZoneCause};
use mtg_engine::keywords::has_keyword;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, GameState, apply_event, replay};
use mtg_testkit::{play_game, umbra_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(24)
}

/// What a campaign saw the armor do.
#[derive(Default)]
struct Seen {
    saved: u64,
    deaths: u64,
    regenerated: u64,
}

/// The Auras with umbra armor attached to the object.
fn armor_on(state: &GameState, object: ObjectId) -> Vec<ObjectId> {
    state
        .attachments_of(object)
        .into_iter()
        .filter(|aura| has_keyword(state, *aura, &Keyword::UmbraArmor))
        .collect()
}

/// Replays the log event by event and checks each against the state it happened in.
fn ledger(setup: &GameSetup, events: &[Event], seen: &mut Seen) {
    let mut state = GameState::from_setup(setup).unwrap();
    for (at, event) in events.iter().enumerate() {
        match event {
            Event::DamageRemoved { object } => {
                let armor = armor_on(&state, *object);
                assert!(!armor.is_empty(), "damage removed at {at} with no armor");
                // The Aura that is destroyed in its place follows in the batch.
                let destroyed = match events.get(at + 1) {
                    Some(Event::ZoneChanged {
                        object: aura,
                        from: Zone::Battlefield,
                        cause: ZoneCause::Destroyed,
                        ..
                    }) => Some(*aura),
                    _ => None,
                };
                assert!(
                    destroyed.is_some_and(|aura| armor.contains(&aura)),
                    "the armor of {object:?} at {at} destroyed {destroyed:?}, not one of {armor:?}"
                );
                seen.saved += 1;
            }
            Event::Regenerated { .. } => seen.regenerated += 1,
            Event::ZoneChanged {
                object,
                from: Zone::Battlefield,
                cause: ZoneCause::Destroyed | ZoneCause::Buried,
                ..
            }
            | Event::CreatureDied {
                object,
                cause: DeathCause::LethalDamage | DeathCause::Deathtouch,
            } if state
                .object(*object)
                .is_some_and(|o| o.card().power.is_some()) =>
            {
                assert!(
                    armor_on(&state, *object).is_empty(),
                    "{object:?} was destroyed at {at} with umbra armor on it"
                );
                seen.deaths += 1;
            }
            _ => {}
        }
        apply_event(&mut state, event).unwrap();
        // A creature that was saved has no damage.
        if let Event::DamageRemoved { object } = event {
            let damage = state
                .object(*object)
                .and_then(|o| o.permanent())
                .map(mtg_engine::object::PermanentState::damage);
            assert_eq!(damage, Some(0), "{object:?} kept damage after the armor");
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_umbra_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = umbra_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        assert_eq!(end.state_hash(), outcome.final_hash);
        ledger(&setup, &outcome.events, &mut seen);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.saved > 0, "no creature was saved in {campaign}");
    assert!(seen.deaths > 0, "no creature was destroyed in {campaign}");
    assert!(
        seen.regenerated > 0,
        "nothing regenerated in {campaign}, so the armor never met a shield"
    );
}

#[test]
fn a_seeded_umbra_game_is_the_same_game_every_time() {
    let setup = umbra_fuzz_setup(3, 11).unwrap();
    let first = play_game(setup.clone(), 11).unwrap();
    let second = play_game(setup, 11).unwrap();
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
