//! Goal: whole bot games with decks of Giant Growth, grants of haste and flying, shrinks, surges that change a set of creatures twice and bounces finish and keep every invariant (zones, life, the journal, effects listed in the order of their timestamps and none past its duration, no creature left that state-based actions would destroy, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use them: pumps and keywords are created and expire, a shrink kills by zero toughness, spells fizzle, a set of several creatures is changed at once and a creature that was changed goes back to the hand; an engine rebuilt from the log at each decision asks what the original asks, and a seeded game is the same game every time.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::{Modification, Selector};
use mtg_engine::events::{DeathCause, ResolutionOutcome, ZoneCause};
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, play_game, pump_fuzz_setup};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60)
}

#[derive(Default)]
struct Seen {
    pumps: u64,
    shrinks: u64,
    keywords: u64,
    sets: u64,
    expired: u64,
    zero_toughness: u64,
    fizzled: u64,
    bounced: u64,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::EffectCreated { effect } => {
                    match &effect.modification {
                        Modification::ModifyPowerToughness { toughness, .. } if *toughness < 0 => {
                            self.shrinks += 1;
                        }
                        Modification::ModifyPowerToughness { .. } => self.pumps += 1,
                        Modification::AddKeyword(_) => self.keywords += 1,
                        _ => {}
                    }
                    if matches!(&effect.selector, Selector::Objects(named) if named.len() > 1) {
                        self.sets += 1;
                    }
                }
                Event::EffectExpired { .. } => self.expired += 1,
                Event::CreatureDied {
                    cause: DeathCause::ZeroToughness,
                    ..
                } => self.zero_toughness += 1,
                Event::ResolutionEnded {
                    outcome: ResolutionOutcome::Fizzled,
                    ..
                } => self.fizzled += 1,
                Event::ZoneChanged {
                    cause: ZoneCause::Moved,
                    to: mtg_engine::zones::Zone::Hand(_),
                    from: mtg_engine::zones::Zone::Battlefield,
                    ..
                } => self.bounced += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn seeded_pump_games_finish_and_keep_every_invariant(#[case] players: usize) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = pump_fuzz_setup(players, seed).unwrap();
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        seen.add(&outcome.events);
    }
    let campaign = format!("{} games of {players} seats", games());
    assert!(seen.pumps > 0, "no creature was pumped in {campaign}");
    assert!(seen.shrinks > 0, "no creature was shrunk in {campaign}");
    assert!(seen.keywords > 0, "no keyword was granted in {campaign}");
    assert!(
        seen.sets > 0,
        "no set of creatures was changed in {campaign}"
    );
    assert!(seen.expired > 0, "no effect ran out in {campaign}");
    assert!(
        seen.zero_toughness > 0,
        "no creature died of zero toughness in {campaign}"
    );
    assert!(seen.fizzled > 0, "no spell fizzled in {campaign}");
    assert!(seen.bounced > 0, "no creature was returned in {campaign}");
}

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_through_pumps(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let mut with_effects = 0;
    for seed in 0..8 {
        let setup = pump_fuzz_setup(players, seed).unwrap();
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
            if !mid_cast {
                assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                );
            }
            with_effects += u64::from(!engine.state().effects().is_empty());
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    assert!(
        with_effects > 0,
        "no decision was met with an effect in play"
    );
    Ok(())
}

#[test]
fn a_seeded_pump_game_is_the_same_game_every_time() {
    for seed in [3, 17, 91] {
        let first = play_game(pump_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        let second = play_game(pump_fuzz_setup(3, seed).unwrap(), seed).unwrap();
        assert_eq!(first.events, second.events);
        assert_eq!(first.final_hash, second.final_hash);
    }
}
