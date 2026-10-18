//! Goal: in bot-driven games without keywords, the combat forecast taken right before combat damage equals the damage, deaths and life totals that follow, for every seed and table size.

mod common;

use common::forecast::{damage_was_dealt, disagreement};
use mtg_engine::combat::DamageTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::forecast::pending_combat_forecast;
use mtg_engine::{Answer, Engine};
use mtg_testkit::{RandomBot, fuzz_setup};
use proptest::prelude::*;
use rstest::rstest;

/// What one game exercised, so a passing property cannot hide an empty one.
#[derive(Debug, Default)]
struct Coverage {
    resolutions: u32,
    multi_blocks: u32,
    deaths: u32,
    lethal_players: u32,
    unblocked_hits: u32,
}

/// Plays a bot game, comparing the forecast with the real damage at every combat damage step.
///
/// Damage divisions among several blockers are answered with the forecast's own division, which
/// checks that it is legal; every other choice is the bot's.
fn play(players: usize, seed: u64) -> Result<Coverage, String> {
    let mut engine = Engine::new(fuzz_setup(players, seed)).map_err(|e| e.to_string())?;
    let mut bots: Vec<RandomBot> = (0..players as u64)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut coverage = Coverage::default();
    for _ in 0..200_000 {
        let Some(decision) = engine.decision().cloned() else {
            return Ok(coverage);
        };
        let forecast = pending_combat_forecast(engine.state());
        let answer = match (&decision.kind, &forecast) {
            (DecisionKind::AssignDamage(options), Some(forecast)) => {
                let division = forecast
                    .attackers
                    .iter()
                    .find(|a| a.attacker == options.attacker)
                    .map(|a| {
                        a.deals
                            .iter()
                            .filter_map(|h| match h.target {
                                DamageTarget::Object(o) => Some((o, h.amount)),
                                DamageTarget::Player(_) => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Answer::AssignDamage(division)
            }
            _ => bots[decision.player.index()].choose(&decision),
        };
        let step = engine.state().turn().step();
        let had_attackers = !engine.state().combat().attackers().is_empty();
        let events = engine
            .apply(answer)
            .map_err(|e| format!("seed {seed}: engine refused an answer: {e}"))?;
        if !(had_attackers && damage_was_dealt(step, &events)) {
            continue;
        }
        let forecast =
            forecast.ok_or_else(|| format!("seed {seed}: no forecast before combat damage"))?;
        if let Some(difference) = disagreement(&forecast, &events, engine.state()) {
            return Err(format!("seed {seed}, {players} players: {difference}"));
        }
        coverage.resolutions += 1;
        coverage.multi_blocks += u32::from(forecast.attackers.iter().any(|a| a.blockers.len() > 1));
        coverage.deaths += u32::try_from(forecast.dying().count()).unwrap_or(0);
        coverage.lethal_players +=
            u32::try_from(forecast.players.iter().filter(|p| p.lethal).count()).unwrap_or(0);
        coverage.unblocked_hits += u32::try_from(
            forecast
                .attackers
                .iter()
                .filter(|a| !a.blocked && a.deals.iter().any(|h| h.amount > 0))
                .count(),
        )
        .unwrap_or(0);
    }
    Err(format!("seed {seed}: game did not finish"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn the_forecast_agrees_with_real_combat(players in 2usize..=4, seed in any::<u64>()) {
        if let Err(difference) = play(players, seed) {
            prop_assert!(false, "{}", difference);
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn the_games_exercise_every_kind_of_outcome(#[case] players: usize) {
    let mut total = Coverage::default();
    for seed in 0..40 {
        let coverage = play(players, seed).unwrap_or_else(|difference| panic!("{difference}"));
        total.resolutions += coverage.resolutions;
        total.multi_blocks += coverage.multi_blocks;
        total.deaths += coverage.deaths;
        total.lethal_players += coverage.lethal_players;
        total.unblocked_hits += coverage.unblocked_hits;
    }
    assert!(total.resolutions > 0, "{total:?}");
    assert!(total.deaths > 0, "{total:?}");
    assert!(total.unblocked_hits > 0, "{total:?}");
    assert!(total.multi_blocks > 0, "{total:?}");
    assert!(total.lethal_players > 0, "{total:?}");
}
