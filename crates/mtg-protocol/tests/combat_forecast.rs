//! Goal: every seat's view carries the same combat forecast while combat damage is ahead and none otherwise,
//! it names only objects every seat can see, and it survives the batch fold and JSON like the rest of the table.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::table::{Table, two_player_board};
use common::{Step, run_game, unique_setup, visible_ids};
use mtg_core::ids::PlayerId;
use mtg_engine::combat::DamageTarget;
use mtg_engine::forecast::{CombatForecast, pending_combat_forecast};
use mtg_engine::turn::Step as TurnStep;
use mtg_protocol::{StateView, TableView, project_events};
use mtg_testkit::cards::{elf, free_creature, giant};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn forecasts(table: &Table) -> Vec<Option<CombatForecast>> {
    table
        .views
        .iter()
        .map(|v| v.table.combat_forecast.clone())
        .collect()
}

/// Every id the forecast names.
fn named_objects(forecast: &CombatForecast) -> Vec<mtg_core::ids::ObjectId> {
    let mut ids = Vec::new();
    for attacker in &forecast.attackers {
        ids.push(attacker.attacker);
        ids.extend(&attacker.blockers);
        for hit in attacker.deals.iter().chain(&attacker.receives) {
            ids.push(hit.source);
            if let DamageTarget::Object(o) = hit.target {
                ids.push(o);
            }
        }
    }
    ids.extend(forecast.creatures.iter().map(|c| c.object));
    ids
}

#[test]
fn a_scripted_attack_shows_the_same_forecast_to_both_seats_until_damage() {
    let mut table = Table::new(
        Scenario::new(2)
            .at(2, 0, TurnStep::DeclareAttackers)
            .battlefield(0, [ready(free_creature("Bear", 2, 2)), ready(elf())])
            .battlefield(1, [ready(giant())]),
    );
    assert_eq!(forecasts(&table), [None, None], "before the attack");

    table.run(|d| d.attack(&[("Bear", 1), ("Llanowar Elf", 1)]));
    let all = forecasts(&table);
    let seen = all[0]
        .clone()
        .expect("a forecast once attackers are declared");
    assert_eq!(all, [Some(seen.clone()), Some(seen.clone())]);
    assert_eq!(seen.players.len(), 1);
    assert_eq!(seen.players[0].damage, 3, "nothing is blocked yet");

    while !matches!(
        table.driver.decision().unwrap().kind,
        mtg_engine::DecisionKind::DeclareBlockers(_)
    ) {
        table.run(Driver::pass);
    }
    table.run(|d| d.block(&[("Craw Wurm", "Bear")]));
    let all = forecasts(&table);
    let seen = all[0].clone().expect("a forecast after blocks");
    assert_eq!(all[1].as_ref(), Some(&seen));
    assert_eq!(seen.players[0].damage, 1, "only the elf gets through");
    let bear = table.id("Bear");
    assert_eq!(seen.dying().collect::<Vec<_>>(), [bear]);

    table.run(Driver::pass);
    table.run(Driver::pass);
    assert_eq!(forecasts(&table), [None, None], "damage has been dealt");
    assert_eq!(table.views[0].table.players[1].life, 19);
}

#[test]
fn the_forecast_travels_in_the_json_table() {
    let mut table = Table::new(two_player_board().at(2, 0, TurnStep::DeclareAttackers));
    let quiet = serde_json::to_value(&table.views[0].table).unwrap();
    assert_eq!(quiet["combat_forecast"], serde_json::Value::Null);

    table.run(|d| d.attack(&[("Grizzly Bears", 1)]));
    let text = serde_json::to_string(&table.views[1]).unwrap();
    let back: StateView = serde_json::from_str(&text).unwrap();
    assert_eq!(back, table.views[1]);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let forecast = &json["combat_forecast"];
    assert_eq!(forecast["players"][0]["damage"], 2);
    assert_eq!(forecast["players"][0]["lethal"], false);
    assert!(forecast["attackers"][0]["deals"][0]["target"]["Player"].is_number());
}

#[rstest]
#[case(2, 71)]
#[case(2, 72)]
#[case(3, 73)]
#[case(4, 74)]
fn seats_agree_on_the_forecast_in_every_batch(#[case] players: usize, #[case] seed: u64) {
    let viewers: Vec<PlayerId> = (0..players).map(PlayerId::from_index).collect();
    let mut with_forecast = 0usize;
    let mut views: Vec<Option<StateView>> = vec![None; players];
    run_game(unique_setup(players, seed), seed, |step: &Step| {
        let batches = project_events(step.before, step.events, &viewers, step.from_seq).unwrap();
        let truth = pending_combat_forecast(step.after);
        let pending_step = matches!(
            step.after.turn().step(),
            TurnStep::DeclareAttackers
                | TurnStep::DeclareBlockers
                | TurnStep::FirstStrikeDamage
                | TurnStep::CombatDamage
        );
        assert!(
            truth.is_none() || pending_step,
            "answer {}: forecast outside combat",
            step.answers
        );
        with_forecast += usize::from(truth.is_some());
        for (viewer, batch) in viewers.iter().zip(&batches) {
            let context = format!("seed {seed} answer {} viewer {viewer:?}", step.answers);
            assert_eq!(batch.table.combat_forecast, truth, "{context}: batch");
            let snapshot = StateView::for_player(step.after, *viewer);
            assert_eq!(snapshot.table.combat_forecast, truth, "{context}: snapshot");
            if let Some(forecast) = &truth {
                let visible = visible_ids(step.after, *viewer);
                for id in named_objects(forecast) {
                    assert!(visible.contains(&id), "{context}: {id:?} is not visible");
                }
            }
            let wire: TableView =
                serde_json::from_str(&serde_json::to_string(&batch.table).unwrap()).unwrap();
            assert_eq!(wire.combat_forecast, truth, "{context}: json");
            match &mut views[viewer.index()] {
                slot @ None => *slot = StateView::from_first_batch(batch),
                Some(view) => view.apply(batch),
            }
            let view = views[viewer.index()].as_ref().unwrap();
            assert_eq!(view.table.combat_forecast, truth, "{context}: folded view");
        }
    });
    assert!(
        with_forecast > 5,
        "only {with_forecast} states had a forecast"
    );
}
