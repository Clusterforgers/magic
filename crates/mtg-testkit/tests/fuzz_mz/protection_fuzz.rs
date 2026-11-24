//! Goal: whole bot games with decks of Black Knight, Kor Firewalker and Mother of Runes against burn, Auras, Equipment, removal and creatures that attack and block finish and keep every invariant for many seeds at tables of two, three and four seats, and the games use protection: an independent ledger replays each log and finds that no damage is dealt to a creature by a source it has protection from, that no creature blocks one it has protection from, that nothing is attached to a permanent that has protection from it, and that Mother of Runes gave protection in some game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::ids::ObjectId;
use mtg_engine::combat::DamageTarget;
use mtg_engine::effects::Modification;
use mtg_engine::keywords::keywords_of;
use mtg_engine::{Event, GameSetup, GameState, apply_event, characteristics, replay};
use mtg_testkit::{play_game, protect_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

/// What a campaign saw protection do.
#[derive(Default)]
struct Seen {
    damage_checked: u64,
    holders_in_play_when_damage_was_dealt: u64,
    blocks_checked: u64,
    attachments_checked: u64,
    protection_given: u64,
}

/// Whether the source `source` has the quality `quality` now: the colours and types the deck's qualities name.
fn has_quality(state: &GameState, source: ObjectId, quality: &Selector) -> bool {
    let Some(chars) = characteristics(state, source) else {
        return false;
    };
    quality.0.iter().all(|pred| match pred {
        Pred::Color(color) => chars.colors.contains(*color),
        Pred::Type(card_type) => chars.types.contains(*card_type),
        Pred::Colorless => chars.colors.is_empty(),
        other => panic!("the ledger does not read {other:?}"),
    })
}

/// Whether `holder` has protection from `source` in `state`.
fn protected(state: &GameState, holder: ObjectId, source: ObjectId) -> bool {
    keywords_of(state, holder).into_iter().any(|keyword| {
        matches!(keyword, Keyword::Protection(quality) if has_quality(state, source, quality))
    })
}

fn holds_protection(state: &GameState, holder: ObjectId) -> bool {
    keywords_of(state, holder)
        .into_iter()
        .any(|keyword| matches!(keyword, Keyword::Protection(_)))
}

fn ledger(setup: &GameSetup, events: &[Event], seen: &mut Seen) {
    let mut state = GameState::from_setup(setup).unwrap();
    for event in events {
        match event {
            Event::DamageDealt {
                source,
                target: DamageTarget::Object(hit),
                ..
            } => {
                seen.damage_checked += 1;
                seen.holders_in_play_when_damage_was_dealt +=
                    u64::from(holds_protection(&state, *hit));
                assert!(
                    !protected(&state, *hit, *source),
                    "{source:?} damaged {hit:?}, which has protection from it"
                );
            }
            Event::BlockersDeclared { blocks, .. } => {
                for (blocker, attacker) in blocks {
                    seen.blocks_checked += 1;
                    assert!(
                        !protected(&state, *attacker, *blocker),
                        "{blocker:?} blocked {attacker:?}, which has protection from it"
                    );
                }
            }
            Event::Attached { object, to } => {
                seen.attachments_checked += 1;
                assert!(
                    !protected(&state, *to, *object),
                    "{object:?} was attached to {to:?}, which has protection from it"
                );
            }
            Event::EffectCreated { effect }
                if matches!(
                    &effect.modification,
                    Modification::AddKeyword(Keyword::Protection(_))
                ) =>
            {
                seen.protection_given += 1;
            }
            _ => {}
        }
        apply_event(&mut state, event).unwrap();
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_protection_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = protect_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        assert_eq!(end.state_hash(), outcome.final_hash);
        ledger(&setup, &outcome.events, &mut seen);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.damage_checked > 0, "no damage in {campaign}");
    assert!(seen.blocks_checked > 0, "no block in {campaign}");
    assert!(
        seen.protection_given > 0,
        "Mother of Runes gave no protection in {campaign}"
    );
    assert!(
        seen.holders_in_play_when_damage_was_dealt > 0,
        "no creature with protection was in play when damage was dealt in {campaign}"
    );
}

#[test]
fn a_seeded_protection_game_is_the_same_game_every_time() {
    let setup = protect_fuzz_setup(3, 11).unwrap();
    let first = play_game(setup.clone(), 11).unwrap();
    let second = play_game(setup, 11).unwrap();
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
