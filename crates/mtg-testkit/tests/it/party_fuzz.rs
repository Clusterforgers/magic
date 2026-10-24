//! Goal: whole bot games with decks of True-Name Nemesis, Sewer Nemesis, Stuffy Doll, Spoils of Adventure, Malakir Blood-Priest and creatures of the four party roles finish and keep every invariant (zones, life, the journal, no trigger waiting while a player has priority, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use the cards: players are chosen as permanents enter, True-Name Nemesis attacks the player it chose while that player blocks other attackers and no creature of that player ever blocks it; an engine rebuilt from the log at each decision asks the same question, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;
use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::ChoiceOption;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::object::Chosen;
use mtg_engine::{Engine, EngineError, Event, GameSetup};
use mtg_testkit::{RandomBot, party_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(60)
}

#[derive(Default)]
struct Seen {
    players_chosen: u64,
    attacks_at_the_chosen_player: u64,
    blocked_elsewhere_by_the_chosen_player: u64,
    blocks_of_the_protected_creature: u64,
}

impl Seen {
    /// Counts what the events of a game show; `protected` are the True-Name Nemeses of the game.
    fn add(&mut self, events: &[Event], protected: &BTreeSet<ObjectId>) {
        let mut controllers: BTreeMap<ObjectId, PlayerId> = BTreeMap::new();
        // Each permanent that chose a player, with the player.
        let mut chose: BTreeMap<ObjectId, PlayerId> = BTreeMap::new();
        // The chosen players attacked in the combat under way, with the attackers they were attacked by.
        let mut attacked: BTreeMap<PlayerId, Vec<ObjectId>> = BTreeMap::new();
        for event in events {
            match event {
                Event::SpellCast { player, object, .. } => {
                    controllers.insert(*object, *player);
                }
                Event::Entered {
                    object,
                    chosen: Some(Chosen::Player(player)),
                    ..
                } => {
                    self.players_chosen += 1;
                    if protected.contains(object) {
                        chose.insert(*object, *player);
                    }
                }
                Event::AttackersDeclared { attackers, .. } => {
                    attacked.clear();
                    for (attacker, target) in attackers {
                        let (Some(chosen), AttackTarget::Player(seat)) =
                            (chose.get(attacker), target)
                        else {
                            continue;
                        };
                        if seat == chosen {
                            self.attacks_at_the_chosen_player += 1;
                            attacked.entry(*chosen).or_default().push(*attacker);
                        }
                    }
                }
                Event::BlockersDeclared { player, blocks } => {
                    let Some(attackers) = attacked.remove(player) else {
                        continue;
                    };
                    let blocked_it = blocks
                        .iter()
                        .any(|(_, blocked)| attackers.contains(blocked));
                    self.blocks_of_the_protected_creature += u64::from(blocked_it);
                    self.blocked_elsewhere_by_the_chosen_player +=
                        u64::from(!blocked_it && !blocks.is_empty());
                }
                _ => {}
            }
        }
    }
}

/// The True-Name Nemeses in the decks of the game, which have protection from the player they choose.
fn nemeses(setup: &GameSetup) -> BTreeSet<ObjectId> {
    let engine = Engine::new(setup.clone()).unwrap();
    engine
        .state()
        .objects()
        .filter(|object| object.card().name == "True-Name Nemesis")
        .map(|object| object.reference().id)
        .collect()
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_party_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = party_fuzz_setup(players, seed).unwrap();
        let protected = nemeses(&setup);
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events, &protected);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(
        seen.players_chosen > 0,
        "no permanent chose a player in {campaign}"
    );
    assert!(
        seen.attacks_at_the_chosen_player > 0,
        "no creature attacked the player it chose in {campaign}"
    );
    assert!(
        seen.blocked_elsewhere_by_the_chosen_player > 0,
        "protection from the chosen player was never exercised in {campaign}: the chosen player never blocked \
         another attacker while True-Name Nemesis attacked them"
    );
    assert_eq!(
        seen.blocks_of_the_protected_creature, 0,
        "a creature of the chosen player blocked a creature with protection from them in {campaign}"
    );
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_about_players(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let mut asked = 0;
    for seed in 0..8 {
        let setup = party_fuzz_setup(players, seed).unwrap();
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for step in 0..1500 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
            assert_eq!(
                resumed.state_hash(),
                engine.state_hash(),
                "seed {seed} step {step}"
            );
            let mid_cast = matches!(
                &decision.kind,
                DecisionKind::PayMana(_) | DecisionKind::ChooseTargets(_)
            ) && !matches!(&decision.kind, DecisionKind::ChooseTargets(o) if o.triggered);
            if matches!(&decision.kind, DecisionKind::ChooseOption(o)
                if o.options.iter().any(|option| matches!(option, ChoiceOption::Player(_))))
            {
                asked += 1;
            }
            if !mid_cast {
                assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                );
            }
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    assert!(asked > 0, "no player was asked to be chosen");
    Ok(())
}

#[test]
fn a_seeded_party_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(party_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(party_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
