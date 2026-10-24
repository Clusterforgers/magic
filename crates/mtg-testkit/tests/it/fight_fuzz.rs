//! Goal: whole bot games with decks of fights (Warbriar Blessing, a spell that makes two creatures fight), decayed creatures and permanents that stay tapped (Basalt Monolith, Goblin Sharpshooter, Fall from Favor, Well Rested) finish and keep every invariant for many seeds at tables of two, three and four seats, and the mechanics occur in a campaign (a fight resolves and damages creatures outside combat, a decayed creature attacks and is sacrificed at end of combat and never blocks), and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use mtg_engine::combat::DamageTarget;
use mtg_engine::events::ZoneCause;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, replay};
use mtg_testkit::fight_cards::{DECAYED_CARDS, FIGHT_SPELLS};
use mtg_testkit::{fight_fuzz_setup, play_game};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(10)
}

#[derive(Default)]
struct Seen {
    cast: BTreeMap<String, u64>,
    /// Damage dealt to a creature by a creature while no combat damage step was running.
    creature_damage_outside_combat: u64,
    decayed_attacks: u64,
    decayed_blocks: u64,
    decayed_sacrificed: u64,
}

impl Seen {
    /// What happened in the events, judged against the state at the end of the game.
    fn add(&mut self, game: &GameState, events: &[Event]) {
        let name = |id| game.object(id).map(|object| object.card().name.clone());
        let decayed = |id| {
            game.object(id)
                .is_some_and(|object| DECAYED_CARDS.contains(&object.card().name.as_str()))
        };
        let mut combat_damage = false;
        for event in events {
            match event {
                Event::SpellResolved { object } => {
                    if let Some(name) = name(*object) {
                        *self.cast.entry(name).or_default() += 1;
                    }
                }
                Event::AttackersDeclared { attackers, .. } => {
                    self.decayed_attacks += attackers
                        .iter()
                        .filter(|(attacker, _)| decayed(*attacker))
                        .count() as u64;
                }
                Event::BlockersDeclared { blocks, .. } => {
                    self.decayed_blocks += blocks
                        .iter()
                        .filter(|(blocker, _)| decayed(*blocker))
                        .count() as u64;
                }
                Event::DamageDealt {
                    source,
                    target: DamageTarget::Object(to),
                    ..
                } if !combat_damage
                    && game.object(*source).is_some_and(|object| {
                        object
                            .card()
                            .types
                            .contains(&mtg_core::card::CardType::Creature)
                    })
                    && game.object(*to).is_some() =>
                {
                    self.creature_damage_outside_combat += 1;
                }
                Event::ZoneChanged {
                    object,
                    from: Zone::Battlefield,
                    cause: ZoneCause::Sacrificed,
                    ..
                } if decayed(*object) => {
                    self.decayed_sacrificed += 1;
                }
                Event::StepBegan { step } => {
                    combat_damage = matches!(step, Step::FirstStrikeDamage | Step::CombatDamage);
                }
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_fight_and_decayed_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    for_each_seed(0..games(), |seed| {
        let setup = fight_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.lock().unwrap().add(&end, &outcome.events);
    });
    let seen = seen.into_inner().unwrap();
    let campaign = format!("{} games of {players} seats", games());
    for name in FIGHT_SPELLS {
        assert!(
            seen.cast.get(*name).copied().unwrap_or(0) > 0,
            "{name} never resolved in {campaign}"
        );
    }
    assert!(
        seen.creature_damage_outside_combat > 0,
        "no creature dealt damage to a creature outside combat in {campaign}"
    );
    assert!(
        seen.decayed_blocks == 0,
        "a creature with decayed blocked in {campaign}"
    );
    assert!(
        seen.decayed_attacks == 0 || seen.decayed_sacrificed > 0,
        "decayed creatures attacked but none was sacrificed in {campaign}"
    );
}

#[test]
fn a_seeded_fight_game_is_the_same_game_every_time() {
    let seed = 3;
    let setup = fight_fuzz_setup(2, seed).unwrap();
    let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
    assert_eq!(first.events, second.events, "seed {seed}");
}
