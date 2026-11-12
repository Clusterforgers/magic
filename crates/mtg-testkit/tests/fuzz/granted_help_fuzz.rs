//! Goal: whole bot games at two to four seats with Inspiring Statuary and Chief Engineer beside the spells and helpers they act on (artifacts, artifact creatures, mana rocks, cards that count historic permanents and Equipment) finish with every invariant held, the bots paying in part and by hand, and the log of each game replays to its final state; across the games artifacts were tapped for a spell that has no improvise of its own, creatures for an artifact spell that has no convoke, and spells with affinity for historic permanents and Equipment were cast with something to count.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::{Event, replay};
use mtg_testkit::{PlayOptions, granted_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// What the games showed.
#[derive(Default)]
struct Seen {
    /// Artifacts tapped, not for mana, right after a spell without improvise of its own was cast.
    given_improvise: u64,
    /// Creatures tapped, not for mana, right after an artifact spell without convoke of its own was cast.
    given_convoke: u64,
    /// Spells with affinity cast, and spells cast in all.
    affine: u64,
    casts: u64,
}

#[test]
fn bot_games_with_keywords_given_to_spells_keep_every_invariant_and_replay() {
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
            let setup = granted_fuzz_setup(players, seed);
            let outcome =
                play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
            let setup = granted_fuzz_setup(players, seed);
            let replayed = replay(&setup, &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let has = |id, keyword: &Keyword| {
                replayed.object(id).is_some_and(|object| {
                    object
                        .card()
                        .rules
                        .iter()
                        .any(|rules| matches!(&**rules, Ability::Keyword(k) if k == keyword))
                })
            };
            let mut seen = seen.lock().unwrap();
            let mut last_cast = None;
            for (at, event) in outcome.events.iter().enumerate() {
                match event {
                    Event::SpellCast { object, .. } => {
                        seen.casts += 1;
                        last_cast = Some((at, *object));
                        if replayed.object(*object).is_some_and(|spell| {
                            spell.card().rules.iter().any(|rules| {
                                matches!(&**rules, Ability::Keyword(Keyword::Affinity(_)))
                            })
                        }) {
                            seen.affine += 1;
                        }
                    }
                    Event::PermanentTapped { object } => {
                        let by_mana = outcome.events[at + 1..].iter().take(1).any(|e| {
                            matches!(e, Event::ManaAdded { source: Some(s), .. } if s == object)
                        });
                        let Some((cast, spell)) = last_cast else {
                            continue;
                        };
                        if cast + 8 <= at || by_mana {
                            continue;
                        }
                        let Some(tapped) = replayed.object(*object) else {
                            continue;
                        };
                        let spell_is_artifact = replayed
                            .object(spell)
                            .is_some_and(|o| o.card().types.contains(&CardType::Artifact));
                        if tapped.card().types.contains(&CardType::Artifact)
                            && !has(spell, &Keyword::Improvise)
                            && !spell_is_artifact
                        {
                            seen.given_improvise += 1;
                        }
                        if tapped.card().types.contains(&CardType::Creature)
                            && !tapped.card().types.contains(&CardType::Artifact)
                            && !has(spell, &Keyword::Convoke)
                            && spell_is_artifact
                        {
                            seen.given_convoke += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    assert!(seen.casts > 0, "spells were cast");
    assert!(
        seen.given_improvise > 0,
        "some nonartifact spell was paid with artifacts it was given improvise for"
    );
    assert!(
        seen.given_convoke > 0,
        "some artifact spell was paid with creatures it was given convoke for"
    );
    assert!(seen.affine > 0, "some spell with affinity was cast");
}
