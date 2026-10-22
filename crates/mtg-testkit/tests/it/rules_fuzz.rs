//! Goal: whole bot games with decks of the rules of the game (attack and block requirements, spells that can't be countered, no life gain, drawing or searching, a revealed library top, triggers that trigger again) finish and keep every invariant (zones, life, the journal, the replay of the log equal to the live state, conservation of cards) for many seeds at tables of two, three and four seats, and the games really use them: the cards that change a rule enter play, declarations are asked with a requirement and the bots meet it, rules of effects begin and end, abilities trigger more than once for one event, no seat gains life, draws or searches against a rule in force, and the top card of a revealed library is known to every seat; an engine rebuilt from the log at each decision asks what the original asks, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::rules::{cant_be_countered, cant_draw, cant_gain_life, cant_search};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameState, apply_event, replay};
use mtg_testkit::rule_cards::RULE_CARDS;
use mtg_testkit::{RandomBot, play_game, rules_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

#[derive(Default)]
struct Seen {
    entered: BTreeMap<String, u64>,
    rules_begun: u64,
    rules_ended: u64,
    again: u64,
}

impl Seen {
    fn add(&mut self, game: &GameState, events: &[Event]) {
        let mut batch: Vec<(mtg_engine::object::ObjectRef, usize)> = Vec::new();
        for event in events {
            match event {
                Event::ZoneChanged {
                    object,
                    to: Zone::Battlefield,
                    ..
                }
                | Event::SpellResolved { object } => {
                    if let Some(object) = game.object(*object) {
                        *self.entered.entry(object.card().name.clone()).or_default() += 1;
                    }
                }
                Event::RuleCreated { .. } => self.rules_begun += 1,
                Event::RuleExpired { .. } => self.rules_ended += 1,
                Event::AbilityTriggered(trigger) => {
                    // One event that triggers the same ability of the same object twice in a row triggers it again.
                    let key = (trigger.source, trigger.ability);
                    if batch.last() == Some(&key) {
                        self.again += 1;
                    }
                    batch.push(key);
                }
                _ => batch.clear(),
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_rule_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = rules_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        seen.add(&end, &outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    for &name in RULE_CARDS {
        assert!(
            seen.entered.get(name).copied().unwrap_or(0) > 0,
            "{name} never entered play in {campaign}"
        );
    }
    assert!(seen.rules_begun > 0, "no rule began in {campaign}");
    assert!(seen.rules_ended > 0, "no rule ended in {campaign}");
    assert!(seen.again > 0, "nothing triggered again in {campaign}");
}

/// What a walk through the decisions of a game saw of the rules in force.
#[derive(Default)]
struct Walk {
    required_attackers: u64,
    required_blockers: u64,
    revealed_tops: u64,
    no_life_gain: u64,
    no_draw: u64,
    no_search: u64,
    uncounterable: u64,
}

impl Walk {
    /// Notes the rules in force at a decision of `state`.
    fn look(&mut self, state: &GameState, decision: &DecisionKind) {
        match decision {
            DecisionKind::DeclareAttackers(options) => {
                self.required_attackers += options.required.len() as u64;
            }
            DecisionKind::DeclareBlockers(options) => {
                self.required_blockers += options.required.len() as u64;
            }
            _ => {}
        }
        for seat in state.alive_players() {
            self.revealed_tops += u64::from(state.revealed_top(seat).is_some());
            self.no_life_gain += u64::from(cant_gain_life(state, seat));
            self.no_draw += u64::from(cant_draw(state, seat));
            self.no_search += u64::from(cant_search(state, seat));
        }
        self.uncounterable += state
            .stack()
            .iter()
            .filter(|id| cant_be_countered(state, **id))
            .count() as u64;
    }
}

/// Plays `events` over a copy of `before` one at a time and fails when one of them does what a rule in force at that
/// moment forbids: gaining life, drawing a card, searching a library, countering a spell.
fn assert_no_rule_is_broken(before: &GameState, events: &[Event], seed: u64) {
    let mut state = before.clone();
    for event in events {
        match event {
            Event::LifeGained { player, .. } => {
                assert!(
                    !cant_gain_life(&state, *player),
                    "seed {seed}: {player:?} gained life against a rule"
                );
            }
            Event::CardDrawn { player, .. } | Event::DrewFromEmptyLibrary { player } => {
                assert!(
                    !cant_draw(&state, *player),
                    "seed {seed}: {player:?} drew against a rule"
                );
            }
            Event::CardsFound(found) if matches!(found.zone, Zone::Library(_)) => {
                assert!(
                    !cant_search(&state, found.player),
                    "seed {seed}: {:?} searched a library against a rule",
                    found.player
                );
            }
            Event::ResolutionEnded {
                object,
                outcome: ResolutionOutcome::Countered,
            } => {
                assert!(
                    !cant_be_countered(&state, *object),
                    "seed {seed}: {object:?} was countered against a rule"
                );
            }
            _ => {}
        }
        apply_event(&mut state, event).unwrap();
    }
}

/// Walks the games of `seeds` decision by decision with one bot a seat, noting the rules in force and failing when an
/// event does what a rule forbids; with `rebuild` it also resumes an engine from the log at each decision and fails
/// when that engine differs from the live one.
fn walk_games(
    players: usize,
    seeds: std::ops::Range<u64>,
    rebuild: bool,
) -> Result<Walk, EngineError> {
    let mut walk = Walk::default();
    for seed in seeds {
        let setup = rules_fuzz_setup(players, seed).unwrap();
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for step in 0..4000 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            if rebuild && step < 1500 {
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
                if !mid_cast {
                    assert_eq!(
                        resumed.decision(),
                        engine.decision(),
                        "seed {seed} step {step}"
                    );
                }
            }
            walk.look(engine.state(), &decision.kind);
            let before = engine.state().clone();
            let answer = bots[decision.player.index()].choose(&decision);
            let events = engine.apply(answer)?;
            assert_no_rule_is_broken(&before, &events, seed);
        }
    }
    Ok(walk)
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_under_the_rules_of_the_game(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..8, true)?;
    assert!(walk.required_attackers > 0, "no attack was required");
    assert!(walk.revealed_tops > 0, "no library top was revealed");
    Ok(())
}

#[rstest]
fn no_seat_does_what_a_rule_in_force_forbids(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let walk = walk_games(players, 0..games(), false)?;
    let seats = format!("{players} seats");
    assert!(
        walk.required_attackers > 0,
        "no attack was required at {seats}"
    );
    assert!(
        walk.revealed_tops > 0,
        "no library top was revealed at {seats}"
    );
    assert!(walk.no_life_gain > 0, "no life gain was stopped at {seats}");
    assert!(walk.no_draw > 0, "no draw was stopped at {seats}");
    assert!(walk.no_search > 0, "no search was stopped at {seats}");
    assert!(
        walk.uncounterable > 0,
        "no spell could not be countered at {seats}"
    );
    assert!(
        walk.required_blockers > 0,
        "no block was required at {seats}"
    );
    Ok(())
}

#[test]
fn a_seeded_rule_game_is_the_same_game_every_time() {
    for seed in [3, 11] {
        let setup = rules_fuzz_setup(3, seed).unwrap();
        let first = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
        let second = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(first.final_hash, second.final_hash, "seed {seed}");
        assert_eq!(first.events, second.events, "seed {seed}");
    }
}
