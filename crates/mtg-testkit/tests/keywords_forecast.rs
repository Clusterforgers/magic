//! Goal: in bot-driven games whose creatures hold the keywords of combat, the combat forecast taken right before the first combat damage equals all the damage, deaths and life totals that follow it, across the first-strike step, the state-based actions between and the regular step, for every seed and table size; and the forecast of staged fights names what each keyword changes.

#![allow(clippy::expect_used)]

mod common;

use common::combat::to_blockers;
use common::forecast::{damage_was_dealt_in, disagreement};
use common::keywords::{Unit, card, duel, ready_units};
use common::name_of;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::combat::DamageTarget;
use mtg_engine::decision::{Answer, DamageOptions, DecisionKind};
use mtg_engine::events::DeathCause;
use mtg_engine::forecast::{CombatForecast, pending_combat_forecast};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, Event};
use mtg_testkit::{Driver, RandomBot, Scenario, ScriptError, keyword_fuzz_setup, ready, sick};
use proptest::prelude::*;
use rstest::rstest;

/// The default division, written out as the rule: lethal damage to each blocker in order, the rest to the
/// player attacked with trample and else on the last blocker. The forecast assumes it for a controller who
/// has not divided yet, so the games answer with it.
fn lethal_first(options: &DamageOptions) -> Vec<(ObjectId, u32)> {
    let mut left = options.power;
    let mut shares = Vec::new();
    for (at, blocker) in options.blockers.iter().enumerate() {
        let last = at + 1 == options.blockers.len();
        let share = if last && options.excess.is_none() {
            left
        } else {
            left.min(options.lethal[at])
        };
        left -= share;
        shares.push((*blocker, share));
    }
    shares
}

/// What one game exercised, so a passing property cannot hide an empty one.
#[derive(Debug, Default, Clone, Copy)]
struct Coverage {
    comparisons: u32,
    first_strike_steps: u32,
    deaths: u32,
    deathtouch_deaths: u32,
    lifelink_gains: u32,
    trample_through: u32,
    double_hits: u32,
    survivors_of_lethal_damage: u32,
}

impl Coverage {
    fn add(&mut self, other: Coverage) {
        self.comparisons += other.comparisons;
        self.first_strike_steps += other.first_strike_steps;
        self.deaths += other.deaths;
        self.deathtouch_deaths += other.deathtouch_deaths;
        self.lifelink_gains += other.lifelink_gains;
        self.trample_through += other.trample_through;
        self.double_hits += other.double_hits;
        self.survivors_of_lethal_damage += other.survivors_of_lethal_damage;
    }
}

/// The forecast taken before the first damage of a combat and everything that has happened since.
struct Held {
    forecast: CombatForecast,
    events: Vec<Event>,
    dealing: bool,
}

/// Compares the held forecast with the damage that followed and notes what the combat exercised.
fn settle(held: &Held, engine: &Engine, coverage: &mut Coverage) -> Result<(), String> {
    if let Some(difference) = disagreement(&held.forecast, &held.events, engine.state()) {
        return Err(difference);
    }
    coverage.comparisons += 1;
    coverage.first_strike_steps += u32::from(held.events.iter().any(|e| {
        matches!(
            e,
            Event::StepBegan {
                step: Step::FirstStrikeDamage
            }
        )
    }));
    coverage.deaths += u32::try_from(held.forecast.dying().count()).unwrap_or(0);
    coverage.deathtouch_deaths += u32::try_from(
        held.events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::CreatureDied {
                        cause: DeathCause::Deathtouch,
                        ..
                    }
                )
            })
            .count(),
    )
    .unwrap_or(0);
    coverage.lifelink_gains += u32::try_from(
        held.events
            .iter()
            .filter(|e| matches!(e, Event::LifeGained { .. }))
            .count(),
    )
    .unwrap_or(0);
    // A blocked attacker that also hits a player: the damage went past its blockers.
    coverage.trample_through += u32::try_from(
        held.forecast
            .attackers
            .iter()
            .filter(|a| {
                a.blocked
                    && a.deals
                        .iter()
                        .any(|hit| matches!(hit.target, DamageTarget::Player(_)) && hit.amount > 0)
            })
            .count(),
    )
    .unwrap_or(0);
    // A double striker that dealt damage: its packet holds both steps.
    coverage.double_hits += u32::try_from(
        held.forecast
            .attackers
            .iter()
            .filter(|a| {
                has_keyword(engine.state(), a.attacker, &Keyword::DoubleStrike)
                    && a.deals.iter().any(|hit| hit.amount > 0)
            })
            .count(),
    )
    .unwrap_or(0);
    coverage.survivors_of_lethal_damage += u32::try_from(
        held.forecast
            .creatures
            .iter()
            .filter(|c| {
                let toughness = mtg_engine::characteristics(engine.state(), c.object)
                    .and_then(|chars| chars.toughness);
                !c.dies && toughness.is_some_and(|t| i64::from(c.damage) >= i64::from(t))
            })
            .count(),
    )
    .unwrap_or(0);
    Ok(())
}

/// Plays a bot game, comparing the forecast with the real damage of every combat.
///
/// The forecast kept is the last one taken before any damage of that combat; the real damage is everything
/// the combat dealt, in the first-strike step and the regular one.
fn play(players: usize, seed: u64) -> Result<Coverage, String> {
    let mut engine = Engine::new(keyword_fuzz_setup(players, seed)).map_err(|e| e.to_string())?;
    let mut bots: Vec<RandomBot> = (0..players as u64)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut coverage = Coverage::default();
    let mut held: Option<Held> = None;
    for _ in 0..200_000 {
        let Some(decision) = engine.decision().cloned() else {
            // The game ended mid-combat (lethal first-strike damage): the damage so far is all there is.
            if let Some(held) = held.filter(|h| h.dealing) {
                settle(&held, &engine, &mut coverage)
                    .map_err(|e| format!("seed {seed}, {players} players, at the end: {e}"))?;
            }
            return Ok(coverage);
        };
        if held.as_ref().is_none_or(|h| !h.dealing) {
            held = pending_combat_forecast(engine.state()).map(|forecast| Held {
                forecast,
                events: Vec::new(),
                dealing: false,
            });
        }
        let answer = match &decision.kind {
            DecisionKind::AssignDamage(options) => Answer::AssignDamage(lethal_first(options)),
            _ => bots[decision.player.index()].choose(&decision),
        };
        let step = engine.state().turn().step();
        let events = engine
            .apply(answer)
            .map_err(|e| format!("seed {seed}: engine refused an answer: {e}"))?;
        let Some(open) = held.as_mut() else {
            continue;
        };
        let dealt_first = damage_was_dealt_in(Step::FirstStrikeDamage, step, &events);
        let dealt_regular = damage_was_dealt_in(Step::CombatDamage, step, &events);
        if dealt_first || dealt_regular {
            open.dealing = true;
        }
        if open.dealing {
            open.events.extend(events);
        }
        if dealt_regular && let Some(done) = held.take() {
            settle(&done, &engine, &mut coverage)
                .map_err(|e| format!("seed {seed}, {players} players: {e}"))?;
        }
    }
    Err(format!("seed {seed}: game did not finish"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn the_forecast_agrees_with_real_combat_between_keyword_creatures(players in 2usize..=4, seed in any::<u64>()) {
        if let Err(difference) = play(players, seed) {
            prop_assert!(false, "{}", difference);
        }
    }
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn the_games_exercise_every_keyword_of_damage(#[case] players: usize) {
    let mut total = Coverage::default();
    for seed in 0..60 {
        total.add(play(players, seed).unwrap_or_else(|difference| panic!("{difference}")));
    }
    assert!(total.comparisons > 0, "{total:?}");
    assert!(total.first_strike_steps > 0, "{total:?}");
    assert!(total.deaths > 0, "{total:?}");
    assert!(total.lifelink_gains > 0, "{total:?}");
    assert!(total.trample_through > 0, "{total:?}");
    assert!(total.deathtouch_deaths > 0, "{total:?}");
    assert!(total.survivors_of_lethal_damage > 0, "{total:?}");
    assert!(total.double_hits > 0, "{total:?}");
}

/// The forecast of a fight with the blocks declared, read at the first moment damage is ahead.
fn forecast_of_fight(
    attackers: &[Unit],
    blockers: &[Unit],
    blocks: &[(&str, &str)],
) -> Result<(Driver, CombatForecast), ScriptError> {
    let mut driver = duel(attackers, blockers)?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|unit| (unit.0, 1)).collect();
    driver.attack(&attacks)?;
    to_blockers(&mut driver, 1)?;
    driver.block(blocks)?;
    let forecast = pending_combat_forecast(driver.state()).expect("damage is ahead");
    Ok((driver, forecast))
}

/// (source, target, amount) of every packet with damage in it, with cards and players by name.
fn packets(driver: &Driver, forecast: &CombatForecast) -> Vec<(String, String, u32)> {
    let mut hits: Vec<_> = forecast
        .attackers
        .iter()
        .flat_map(|a| a.deals.iter().chain(&a.receives))
        .filter(|hit| hit.amount > 0)
        .map(|hit| {
            let target = match hit.target {
                DamageTarget::Player(p) => format!("P{}", p.index()),
                DamageTarget::Object(o) => name_of(driver.state(), o),
            };
            (name_of(driver.state(), hit.source), target, hit.amount)
        })
        .collect();
    hits.sort();
    hits
}

fn hit(source: &str, target: &str, amount: u32) -> (String, String, u32) {
    (source.to_owned(), target.to_owned(), amount)
}

fn dying(driver: &Driver, forecast: &CombatForecast) -> Vec<String> {
    let mut names: Vec<String> = forecast
        .dying()
        .map(|id| name_of(driver.state(), id))
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::first_strike_kills_before_it_is_hit(("A", 2, 2, "first-strike"), ("B", 2, 2, ""), vec![hit("A", "B", 2)], vec!["B"], 20)]
#[case::double_strike_is_one_packet_of_both_steps(("A", 3, 3, "double-strike"), ("B", 4, 9, ""), vec![hit("A", "B", 6), hit("B", "A", 4)], vec!["A"], 20)]
#[case::double_strike_stops_when_the_blocker_died(("A", 3, 3, "double-strike"), ("B", 1, 1, ""), vec![hit("A", "B", 3)], vec!["B"], 20)]
#[case::deathtouch_kills_with_one_damage(("A", 1, 1, "deathtouch"), ("B", 1, 9, ""), vec![hit("A", "B", 1), hit("B", "A", 1)], vec!["A", "B"], 20)]
#[case::indestructible_does_not_die(("A", 5, 5, ""), ("B", 1, 2, "indestructible"), vec![hit("A", "B", 5), hit("B", "A", 1)], vec![], 20)]
#[case::indestructible_does_not_die_to_deathtouch(("A", 1, 1, "deathtouch"), ("B", 1, 9, "indestructible"), vec![hit("A", "B", 1), hit("B", "A", 1)], vec!["A"], 20)]
#[case::trample_goes_past_the_blocker(("A", 5, 5, "trample"), ("B", 1, 2, ""), vec![hit("A", "B", 2), hit("A", "P1", 3), hit("B", "A", 1)], vec!["B"], 17)]
#[case::trample_and_double_strike_trample_over_the_dead(("A", 3, 3, "trample double-strike"), ("B", 1, 1, ""), vec![hit("A", "B", 1), hit("A", "P1", 5)], vec!["B"], 15)]
#[case::deathtouch_trample(("A", 3, 3, "trample deathtouch"), ("B", 0, 5, ""), vec![hit("A", "B", 1), hit("A", "P1", 2)], vec!["B"], 18)]
fn the_forecast_of_a_fight_names_what_each_keyword_changes(
    #[case] attacker: Unit,
    #[case] blocker: Unit,
    #[case] hits: Vec<(String, String, u32)>,
    #[case] dies: Vec<&str>,
    #[case] defender_life_after: i32,
) -> Result<(), ScriptError> {
    let (driver, forecast) =
        forecast_of_fight(&[attacker], &[blocker], &[(blocker.0, attacker.0)])?;
    assert_eq!(packets(&driver, &forecast), hits);
    assert_eq!(dying(&driver, &forecast), dies);
    let defender = forecast
        .players
        .iter()
        .find(|p| p.player.index() == 1)
        .expect("the defender is in the forecast");
    assert_eq!(defender.life_after, defender_life_after);
    Ok(())
}

#[test]
fn the_forecast_carries_lifelink_to_the_life_of_players_nobody_attacks() -> Result<(), ScriptError>
{
    let (_, forecast) = forecast_of_fight(&[("A", 3, 3, "lifelink")], &[("B", 1, 1, "")], &[])?;
    let lives: Vec<(usize, i32, i32, bool)> = forecast
        .players
        .iter()
        .map(|p| (p.player.index(), p.life, p.life_after, p.lethal))
        .collect();
    // The player attacked takes 3; the attacker's controller gains 3 and is listed for it.
    assert_eq!(lives, [(0, 20, 23, false), (1, 20, 17, false)]);
    Ok(())
}

#[test]
fn a_lifelinking_blocker_can_keep_its_controller_from_a_lethal_hit() -> Result<(), ScriptError> {
    // 5 life, hit for 3 by A1 while the lifelinker blocks A2 and deals 3: life goes 5 - 3 + 3 and the hit is not lethal.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, ready_units(&[("A1", 5, 1, ""), ("A2", 1, 9, "")]))
            .battlefield(1, ready_units(&[("L", 3, 9, "lifelink")]))
            .life(1, 5),
    )?;
    driver.attack(&[("A1", 1), ("A2", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("L", "A2")])?;
    let forecast = pending_combat_forecast(driver.state()).expect("damage is ahead");
    let defender = forecast
        .players
        .iter()
        .find(|p| p.player.index() == 1)
        .expect("the defender is in the forecast");
    assert_eq!(
        (defender.damage, defender.life_after, defender.lethal),
        (5, 3, false)
    );
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 3)
}

#[test]
fn between_the_steps_the_forecast_describes_the_regular_damage_that_is_left()
-> Result<(), ScriptError> {
    let attackers: [Unit; 1] = [("A", 2, 6, "double-strike")];
    let (mut driver, before) = forecast_of_fight(&attackers, &[("B", 1, 9, "")], &[("B", "A")])?;
    assert_eq!(
        packets(&driver, &before),
        [hit("A", "B", 4), hit("B", "A", 1)]
    );
    driver.pass()?;
    driver.pass()?;
    // The first-strike damage is done: 2 is marked on B and the second strike and B's one are still to come.
    driver.expect_step(Step::FirstStrikeDamage)?;
    let during = pending_combat_forecast(driver.state()).expect("regular damage is ahead");
    assert_eq!(
        packets(&driver, &during),
        [hit("A", "B", 2), hit("B", "A", 1)]
    );
    driver.pass()?;
    driver.pass()?;
    driver.expect_step(Step::CombatDamage)?;
    assert!(
        pending_combat_forecast(driver.state()).is_none(),
        "all the damage has been dealt"
    );
    Ok(())
}

#[test]
fn a_sick_haste_attacker_and_a_ready_one_forecast_alike() -> Result<(), ScriptError> {
    let hasty: Unit = ("A", 2, 2, "haste first-strike");
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [sick(card(hasty))])
            .battlefield(1, [ready(card(("B", 1, 1, "")))]),
    )?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    let forecast = pending_combat_forecast(driver.state()).expect("damage is ahead");
    assert_eq!(packets(&driver, &forecast), [hit("A", "B", 2)]);
    Ok(())
}
