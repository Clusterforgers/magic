//! Goal: whole bot games at two to four seats with improvise, affinity and typecycling cards (the corpus cards and synthetic ones, artifact creatures and mana rocks beside them) finish with every invariant held, the bots paying in part and by hand with artifacts tapped, and the log of each game replays to its final state; across the games artifacts were tapped for improvise, spells with affinity were cast with something to count, and cards were typecycled and found a card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::{Event, GameState, replay};
use mtg_testkit::{PlayOptions, improvise_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// What the games showed: artifacts tapped for improvise, affinity spells cast with permanents to count, cards
/// typecycled, cards found by a typecycling, and the spells cast in all.
#[derive(Default)]
struct Seen {
    improvised: u64,
    affine: u64,
    typecycled: u64,
    found: u64,
    casts: u64,
}

/// Whether the spell `id` of the state has affinity and its caster controls a permanent that it counts.
fn counts_something(state: &GameState, id: mtg_core::ids::ObjectId) -> bool {
    let Some(spell) = state.object(id) else {
        return false;
    };
    let caster = spell.controller();
    let Some(Keyword::Affinity(kind)) =
        spell.card().rules.iter().find_map(|rules| match &**rules {
            mtg_cards::ability::Ability::Keyword(keyword @ Keyword::Affinity(_)) => {
                Some(keyword.clone())
            }
            _ => None,
        })
    else {
        return false;
    };
    state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|permanent| state.object(*permanent))
        .filter(|permanent| permanent.controller() == caster)
        .any(|permanent| {
            kind.0.iter().all(|pred| match pred {
                mtg_cards::refs::Pred::Type(card_type) => {
                    permanent.card().types.contains(card_type)
                }
                mtg_cards::refs::Pred::Subtype(subtype) => {
                    permanent.card().subtypes.contains(subtype)
                }
                _ => false,
            })
        })
}

#[test]
fn bot_games_with_improvise_affinity_and_typecycling_keep_every_invariant_and_replay() {
    let seen = Mutex::new(Seen::default());
    let games = fuzz_games(40);
    for_each_seed(0..games, |seed| {
        for (players, options) in [
            (2, PlayOptions::default()),
            (
                3,
                PlayOptions {
                    partial_payments: true,
                    ..PlayOptions::default()
                },
            ),
            (
                4,
                PlayOptions {
                    partial_payments: true,
                    symbol_choices: true,
                    ..PlayOptions::default()
                },
            ),
        ] {
            let setup = improvise_fuzz_setup(players, seed);
            let outcome =
                play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
            let setup = improvise_fuzz_setup(players, seed);
            let replayed = replay(&setup, &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            let mut last_cast_at = None;
            let mut cycled = false;
            let mut affine_checked = false;
            for (at, event) in outcome.events.iter().enumerate() {
                match event {
                    Event::SpellCast { object, .. } => {
                        seen.casts += 1;
                        last_cast_at = Some(at);
                        // Only the first spell with affinity of a game is looked at: it takes a replay to the cast.
                        if !affine_checked {
                            let state = replay(&setup, &outcome.events[..=at]).unwrap();
                            if state.object(*object).is_some_and(|o| {
                                o.card().rules.iter().any(|rules| {
                                    matches!(
                                        &**rules,
                                        mtg_cards::ability::Ability::Keyword(Keyword::Affinity(_))
                                    )
                                })
                            }) {
                                affine_checked = true;
                                if counts_something(&state, *object) {
                                    seen.affine += 1;
                                }
                            }
                        }
                    }
                    Event::PermanentTapped { object } => {
                        // An artifact tapped right after a cast and not for mana is an improvise.
                        let by_mana = outcome.events[at + 1..].iter().take(1).any(|e| {
                            matches!(e, Event::ManaAdded { source: Some(s), .. } if s == object)
                        });
                        if last_cast_at.is_some_and(|cast| cast + 8 > at)
                            && !by_mana
                            && replayed
                                .object(*object)
                                .is_some_and(|o| o.card().types.contains(&CardType::Artifact))
                        {
                            seen.improvised += 1;
                        }
                    }
                    Event::Cycled { .. } => {
                        seen.typecycled += 1;
                        cycled = true;
                    }
                    Event::Revealed(_) if cycled => {
                        seen.found += 1;
                        cycled = false;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen.casts > 0, "spells were cast");
    assert!(
        seen.improvised > 0,
        "some artifact was tapped for improvise"
    );
    assert!(
        seen.affine > 0,
        "some spell with affinity had permanents to count"
    );
    assert!(seen.typecycled > 0, "some card was typecycled");
    assert!(seen.found > 0, "some typecycling found a card");
}
