//! Goal: whole bot games with decks of abilities that pay life, remove counters, sacrifice, tap or return permanents, discard cards and exile cards from a graveyard, and of cards that are cycled, finish and keep every invariant (zones, life, mana, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really pay each kind of part: the bots are asked which permanents and cards to take, pay with them and cycle; an engine rebuilt from the log at each decision asks the same question, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_engine::abilities::AbilitySource;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, parts_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}

/// What the events before each activation paid, by kind of part.
#[derive(Default)]
struct Seen {
    kinds: BTreeSet<&'static str>,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for (at, event) in events.iter().enumerate() {
            let Event::AbilityActivated { origin, .. } = event else {
                continue;
            };
            if matches!(origin, AbilitySource::PrintedKeyword(_)) {
                self.kinds.insert("cycling");
            }
            // The events of the cost are the ones right before the activation.
            for earlier in events[..at].iter().rev() {
                match earlier {
                    Event::LifePaid { .. } => self.kinds.insert("life"),
                    Event::CardDiscarded { .. } => self.kinds.insert("discard"),
                    Event::CountersChanged(changed) if changed.delta < 0 => {
                        self.kinds.insert("counters")
                    }
                    Event::ZoneChanged {
                        cause, from, to, ..
                    } => match (cause, from, to) {
                        (ZoneCause::Sacrificed, _, _) => self.kinds.insert("sacrifice"),
                        (ZoneCause::Exiled, Zone::Graveyard(_), Zone::Exile) => {
                            self.kinds.insert("exile")
                        }
                        (ZoneCause::Moved, Zone::Battlefield, Zone::Hand(_)) => {
                            self.kinds.insert("return")
                        }
                        _ => break,
                    },
                    Event::PermanentTapped { .. } => self.kinds.insert("tap"),
                    _ => break,
                };
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_part_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = parts_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    for kind in [
        "life",
        "counters",
        "sacrifice",
        "discard",
        "exile",
        "tap",
        "return",
        "cycling",
    ] {
        assert!(
            seen.kinds.contains(kind),
            "no {kind} was paid in {} games of {players} seats: {:?}",
            games(),
            seen.kinds
        );
    }
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_including_every_pick(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), EngineError> {
    let mut picks = 0;
    for seed in 0..8 {
        let setup = parts_fuzz_setup(players, seed).unwrap();
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
            // A cast or activation that is not paid for yet lives outside the state, whatever it asks.
            let announcing = engine.state().resolving().is_none()
                && match &decision.kind {
                    DecisionKind::ChooseCards(options) => {
                        picks += u64::from(options.can_cancel);
                        options.can_cancel
                    }
                    DecisionKind::PayMana(_)
                    | DecisionKind::ChooseTargets(_)
                    | DecisionKind::ChooseMode(_)
                    | DecisionKind::ChooseNumber(_) => true,
                    _ => false,
                };
            if !announcing && !engine.replacing() {
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
    assert!(picks > 0, "no permanent or card was picked for a cost");
    Ok(())
}

#[test]
fn a_seeded_part_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(parts_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(parts_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
