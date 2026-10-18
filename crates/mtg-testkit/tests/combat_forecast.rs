//! Goal: the combat forecast names the damage, the deaths and the players at lethal for every attacker/blocker shape, and exists only while damage is still ahead.

#![allow(clippy::expect_used)]

mod common;

use common::combat::to_blockers;
use common::forecast::disagreement;
use common::name_of;
use mtg_engine::combat::DamageTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::forecast::{CombatForecast, pending_combat_forecast};
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, damaged};
use rstest::rstest;

/// A creature of the table: name, power, toughness and damage already marked.
type Fighter = (&'static str, i32, i32, u32);
/// One packet of expected damage: source, target (a card name or `P1`) and amount.
type Hit = (&'static str, &'static str, u32);

fn creatures(list: &[Fighter]) -> Vec<mtg_engine::PresetPermanent> {
    list.iter()
        .map(|&(name, power, toughness, marked)| {
            damaged(free_creature(name, power, toughness), marked)
        })
        .collect()
}

/// Player 0 attacks player 1 with every attacker; blocks are declared; priority is back with player 0.
fn blocked_combat(
    attackers: &[Fighter],
    blockers: &[Fighter],
    blocks: &[(&str, &str)],
    life: i32,
) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, creatures(attackers))
            .battlefield(1, creatures(blockers))
            .life(1, life),
    )?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|f| (f.0, 1)).collect();
    driver.attack(&attacks)?;
    to_blockers(&mut driver, 1)?;
    driver.block(blocks)?;
    Ok(driver)
}

fn target_name(driver: &Driver, target: DamageTarget) -> String {
    match target {
        DamageTarget::Player(p) => format!("P{}", p.index()),
        DamageTarget::Object(o) => name_of(driver.state(), o),
    }
}

fn forecast_of(driver: &Driver) -> CombatForecast {
    pending_combat_forecast(driver.state()).expect("a forecast while damage is ahead")
}

fn hits_of(driver: &Driver, forecast: &CombatForecast) -> Vec<(String, String, u32)> {
    let mut hits: Vec<_> = forecast
        .attackers
        .iter()
        .flat_map(|a| a.deals.iter().chain(&a.receives))
        .map(|h| {
            (
                name_of(driver.state(), h.source),
                target_name(driver, h.target),
                h.amount,
            )
        })
        .collect();
    hits.sort();
    hits
}

#[rstest]
#[case::unblocked(vec![("A", 3, 3, 0)], vec![("B", 1, 1, 0)], vec![], 20, vec![("A", "P1", 3)], (vec![], 3, false))]
#[case::unblocked_exactly_lethal(vec![("A", 5, 5, 0)], vec![("B", 1, 1, 0)], vec![], 5, vec![("A", "P1", 5)], (vec![], 5, true))]
#[case::unblocked_overkill(vec![("A", 6, 1, 0)], vec![("B", 1, 1, 0)], vec![], 4, vec![("A", "P1", 6)], (vec![], 6, true))]
#[case::unblocked_zero_power(vec![("A", 0, 4, 0)], vec![("B", 1, 1, 0)], vec![], 20, vec![("A", "P1", 0)], (vec![], 0, false))]
#[case::one_short_of_lethal(vec![("A1", 3, 3, 0), ("A2", 2, 2, 0)], vec![("B", 1, 1, 0)], vec![], 6, vec![("A1", "P1", 3), ("A2", "P1", 2)], (vec![], 5, false))]
#[case::two_attackers_lethal_together(vec![("A1", 3, 3, 0), ("A2", 2, 2, 0)], vec![("B", 1, 1, 0)], vec![], 5, vec![("A1", "P1", 3), ("A2", "P1", 2)], (vec![], 5, true))]
#[case::trade(vec![("A", 2, 2, 0)], vec![("B", 2, 2, 0)], vec![("B", "A")], 20, vec![("A", "B", 2), ("B", "A", 2)], (vec!["A", "B"], 0, false))]
#[case::attacker_wins(vec![("A", 3, 3, 0)], vec![("B", 1, 1, 0)], vec![("B", "A")], 20, vec![("A", "B", 3), ("B", "A", 1)], (vec!["B"], 0, false))]
#[case::blocker_wins(vec![("A", 1, 1, 0)], vec![("B", 3, 3, 0)], vec![("B", "A")], 20, vec![("A", "B", 1), ("B", "A", 3)], (vec!["A"], 0, false))]
#[case::wall(vec![("A", 1, 5, 0)], vec![("B", 1, 5, 0)], vec![("B", "A")], 20, vec![("A", "B", 1), ("B", "A", 1)], (vec![], 0, false))]
#[case::chump_block_stops_all_damage(vec![("A", 10, 10, 0)], vec![("B", 1, 1, 0)], vec![("B", "A")], 3, vec![("A", "B", 10), ("B", "A", 1)], (vec!["B"], 0, false))]
#[case::zero_power_blocker(vec![("A", 5, 1, 0)], vec![("B", 0, 5, 0)], vec![("B", "A")], 20, vec![("A", "B", 5), ("B", "A", 0)], (vec!["B"], 0, false))]
#[case::zero_power_attacker_blocked(vec![("A", 0, 1, 0)], vec![("B", 2, 2, 0)], vec![("B", "A")], 20, vec![("A", "B", 0), ("B", "A", 2)], (vec!["A"], 0, false))]
#[case::marked_damage_kills_attacker(vec![("A", 2, 4, 2)], vec![("B", 2, 2, 0)], vec![("B", "A")], 20, vec![("A", "B", 2), ("B", "A", 2)], (vec!["A", "B"], 0, false))]
#[case::marked_damage_one_short(vec![("A", 2, 4, 1)], vec![("B", 2, 2, 0)], vec![("B", "A")], 20, vec![("A", "B", 2), ("B", "A", 2)], (vec!["B"], 0, false))]
#[case::marked_damage_kills_blocker(vec![("A", 2, 9, 0)], vec![("B", 1, 3, 1)], vec![("B", "A")], 20, vec![("A", "B", 2), ("B", "A", 1)], (vec!["B"], 0, false))]
#[case::double_block_rest_on_last(vec![("A", 5, 5, 0)], vec![("B1", 2, 2, 0), ("B2", 2, 2, 0)], vec![("B1", "A"), ("B2", "A")], 20, vec![("A", "B1", 2), ("A", "B2", 3), ("B1", "A", 2), ("B2", "A", 2)], (vec!["B1", "B2"], 0, false))]
#[case::double_block_power_runs_out(vec![("A", 3, 9, 0)], vec![("B1", 2, 2, 0), ("B2", 2, 2, 0)], vec![("B1", "A"), ("B2", "A")], 20, vec![("A", "B1", 2), ("A", "B2", 1), ("B1", "A", 2), ("B2", "A", 2)], (vec!["B1"], 0, false))]
#[case::double_block_kills_attacker(vec![("A", 4, 4, 0)], vec![("B1", 2, 3, 0), ("B2", 2, 3, 0)], vec![("B1", "A"), ("B2", "A")], 20, vec![("A", "B1", 3), ("A", "B2", 1), ("B1", "A", 2), ("B2", "A", 2)], (vec!["A", "B1"], 0, false))]
#[case::double_block_first_already_hurt(vec![("A", 3, 9, 0)], vec![("B1", 2, 3, 2), ("B2", 2, 3, 0)], vec![("B1", "A"), ("B2", "A")], 20, vec![("A", "B1", 1), ("A", "B2", 2), ("B1", "A", 2), ("B2", "A", 2)], (vec!["B1"], 0, false))]
#[case::triple_block(vec![("A", 6, 6, 0)], vec![("B1", 1, 1, 0), ("B2", 1, 1, 0), ("B3", 1, 1, 0)], vec![("B1", "A"), ("B2", "A"), ("B3", "A")], 20, vec![("A", "B1", 1), ("A", "B2", 1), ("A", "B3", 4), ("B1", "A", 1), ("B2", "A", 1), ("B3", "A", 1)], (vec!["B1", "B2", "B3"], 0, false))]
#[case::double_block_zero_power_attacker(vec![("A", 0, 3, 0)], vec![("B1", 1, 1, 0), ("B2", 1, 1, 0)], vec![("B1", "A"), ("B2", "A")], 20, vec![("A", "B1", 0), ("A", "B2", 0), ("B1", "A", 1), ("B2", "A", 1)], (vec![], 0, false))]
#[case::one_blocked_one_through(vec![("A1", 3, 3, 0), ("A2", 2, 2, 0)], vec![("B", 1, 1, 0)], vec![("B", "A2")], 20, vec![("A1", "P1", 3), ("A2", "B", 2), ("B", "A2", 1)], (vec!["B"], 3, false))]
#[case::two_pairs(vec![("A1", 2, 2, 0), ("A2", 4, 4, 0)], vec![("B1", 2, 2, 0), ("B2", 1, 1, 0)], vec![("B1", "A1"), ("B2", "A2")], 20, vec![("A1", "B1", 2), ("B1", "A1", 2), ("A2", "B2", 4), ("B2", "A2", 1)], (vec!["A1", "B1", "B2"], 0, false))]
#[case::chump_one_lethal_through_the_other(vec![("A1", 4, 4, 0), ("A2", 5, 1, 0)], vec![("B", 1, 1, 0)], vec![("B", "A1")], 5, vec![("A1", "B", 4), ("B", "A1", 1), ("A2", "P1", 5)], (vec!["B"], 5, true))]
fn forecast_shapes(
    #[case] attackers: Vec<Fighter>,
    #[case] blockers: Vec<Fighter>,
    #[case] blocks: Vec<(&str, &str)>,
    #[case] life: i32,
    #[case] hits: Vec<Hit>,
    #[case] outcome: (Vec<&str>, u32, bool),
) -> Result<(), ScriptError> {
    let (dies, player_damage, lethal) = outcome;
    let driver = blocked_combat(&attackers, &blockers, &blocks, life)?;
    let forecast = forecast_of(&driver);

    let mut want: Vec<_> = hits
        .iter()
        .map(|&(s, t, n)| (s.to_owned(), t.to_owned(), n))
        .collect();
    want.sort();
    assert_eq!(hits_of(&driver, &forecast), want, "damage packets");

    let mut doomed: Vec<String> = forecast
        .dying()
        .map(|o| name_of(driver.state(), o))
        .collect();
    doomed.sort();
    assert_eq!(doomed, dies, "deaths");

    let [player] = forecast.players.as_slice() else {
        panic!("one defending player expected: {:?}", forecast.players);
    };
    assert_eq!(player.player.index(), 1);
    assert_eq!(player.damage, player_damage);
    assert_eq!(player.life, life);
    assert_eq!(
        i64::from(player.life_after),
        i64::from(life) - i64::from(player_damage)
    );
    assert_eq!(player.lethal, lethal);

    for attacker in &forecast.attackers {
        assert!(attacker.deals.iter().all(|h| h.source == attacker.attacker));
        assert!(
            attacker
                .receives
                .iter()
                .all(|h| h.target == DamageTarget::Object(attacker.attacker))
        );
        assert_eq!(attacker.blocked, !attacker.blockers.is_empty());
    }
    Ok(())
}

#[rstest]
#[case::unblocked(vec![("A", 3, 3, 0)], vec![("B", 1, 1, 0)], vec![])]
#[case::single_block(vec![("A", 2, 2, 0)], vec![("B", 2, 2, 0)], vec![("B", "A")])]
#[case::double_block(vec![("A", 5, 5, 0)], vec![("B1", 2, 2, 0), ("B2", 2, 2, 0)], vec![("B1", "A"), ("B2", "A")])]
#[case::two_pairs(vec![("A1", 2, 2, 0), ("A2", 4, 4, 1)], vec![("B1", 2, 2, 0), ("B2", 1, 1, 0)], vec![("B1", "A1"), ("B2", "A2")])]
fn the_forecast_equals_the_damage_that_follows(
    #[case] attackers: Vec<Fighter>,
    #[case] blockers: Vec<Fighter>,
    #[case] blocks: Vec<(&str, &str)>,
) -> Result<(), ScriptError> {
    let mut driver = blocked_combat(&attackers, &blockers, &blocks, 20)?;
    loop {
        let forecast = forecast_of(&driver);
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::AssignDamage(options) => {
                let split = forecast
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
                mtg_engine::Answer::AssignDamage(split)
            }
            _ => mtg_engine::Answer::Priority(mtg_engine::decision::PriorityAction::Pass),
        };
        let step = driver.state().turn().step();
        let events = driver.answer(answer)?;
        if common::forecast::damage_was_dealt(step, &events) {
            assert_eq!(disagreement(&forecast, &events, driver.state()), None);
            return Ok(());
        }
    }
}

#[test]
fn before_blockers_every_attacker_is_unblocked() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, creatures(&[("A1", 3, 3, 0), ("A2", 2, 2, 0)]))
            .battlefield(1, creatures(&[("B", 5, 5, 0)])),
    )?;
    driver.attack(&[("A1", 1), ("A2", 1)])?;
    let forecast = forecast_of(&driver);
    assert!(forecast.attackers.iter().all(|a| a.receives.is_empty()));
    assert_eq!(forecast.players.iter().map(|p| p.damage).sum::<u32>(), 5);
    assert_eq!(forecast.dying().count(), 0);
    // The defender sees the same forecast when the blocker decision is open.
    to_blockers(&mut driver, 1)?;
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::DeclareBlockers(_)
    ));
    assert_eq!(forecast_of(&driver), forecast);
    Ok(())
}

#[test]
fn the_forecast_exists_only_while_damage_is_ahead() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::Main1)
            .battlefield(0, creatures(&[("A", 5, 5, 0)]))
            .battlefield(1, creatures(&[("B1", 2, 2, 0), ("B2", 2, 2, 0)])),
    )?;
    let seen = |driver: &Driver| pending_combat_forecast(driver.state()).is_some();
    assert!(!seen(&driver), "main phase");
    driver.pass_until(Step::DeclareAttackers)?;
    assert!(!seen(&driver), "attackers not declared yet");
    driver.attack(&[("A", 1)])?;
    assert!(seen(&driver), "attackers declared");
    to_blockers(&mut driver, 1)?;
    assert!(seen(&driver), "blockers being declared");
    driver.block(&[("B1", "A"), ("B2", "A")])?;
    assert!(seen(&driver), "blockers declared");
    driver.pass()?;
    driver.pass()?;
    driver.expect_decision(0, "assign-damage")?;
    assert!(seen(&driver), "damage waits on an assignment");
    driver.assign_damage(&[("B1", 2), ("B2", 3)])?;
    assert_eq!(driver.state().turn().step(), Step::CombatDamage);
    assert!(!seen(&driver), "damage is dealt, combat state lingers");
    driver.pass_until(Step::EndCombat)?;
    assert!(!seen(&driver), "end of combat");
    driver.pass_until(Step::Main2)?;
    assert!(!seen(&driver), "after combat");
    Ok(())
}

#[test]
fn a_division_already_chosen_replaces_the_default() -> Result<(), ScriptError> {
    let mut driver = blocked_combat(
        &[("A1", 4, 9, 0), ("A2", 4, 9, 0)],
        &[
            ("B1", 2, 2, 0),
            ("B2", 2, 2, 0),
            ("B3", 2, 2, 0),
            ("B4", 2, 2, 0),
        ],
        &[("B1", "A1"), ("B2", "A1"), ("B3", "A2"), ("B4", "A2")],
        20,
    )?;
    driver.pass()?;
    driver.pass()?;
    driver.expect_decision(0, "assign-damage")?;
    let before = forecast_of(&driver);
    let packets = |f: &CombatForecast, d: &Driver, who: &str| -> Vec<u32> {
        f.attackers
            .iter()
            .filter(|a| name_of(d.state(), a.attacker) == who)
            .flat_map(|a| a.deals.iter().map(|h| h.amount))
            .collect()
    };
    assert_eq!(packets(&before, &driver, "A1"), vec![2, 2]);
    // Which attacker is asked first is the engine's business; split the asked one unusually.
    let DecisionKind::AssignDamage(asked) = driver.decision()?.kind.clone() else {
        panic!("assign-damage expected");
    };
    let asked_name = name_of(driver.state(), asked.attacker);
    let lopsided = [(asked.blockers[0], 0), (asked.blockers[1], 4)];
    driver.answer(mtg_engine::Answer::AssignDamage(lopsided.to_vec()))?;
    let after = forecast_of(&driver);
    assert_eq!(packets(&after, &driver, &asked_name), vec![0, 4]);
    let other = if asked_name == "A1" { "A2" } else { "A1" };
    assert_eq!(packets(&after, &driver, other), vec![2, 2]);
    Ok(())
}
