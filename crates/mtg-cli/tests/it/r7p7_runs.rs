//! Goal: tokens made with `PumpKeywords$` have the keyword only for the stated time, and a group line aimed at a player ("creatures target player controls") hits that player's creatures and spares everyone else's.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cli::forge::script::Script;
use mtg_cli::translate::ctx::World;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::controller_of;
use mtg_engine::keywords::keywords_of;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::{settle, translated, translated_in};

fn soldier_world() -> World {
    let soldier =
        Script::parse("Name:Soldier Token\nColors:white\nTypes:Creature Soldier\nPT:1/1\n");
    let mut world = World::default();
    world.tokens.insert(
        "w_1_1_soldier".to_owned(),
        soldier.faces.into_iter().next().unwrap(),
    );
    world
}

fn count(driver: &Driver, seat: usize, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state.object(**id).is_some_and(|o| o.card().name == name)
                && controller_of(state, **id) == Some(PlayerId::from_index(seat))
        })
        .count()
}

#[rstest]
#[case::pumped("PumpKeywords$ Haste | PumpDuration$ EOT", true)]
#[case::plain("TokenOwner$ You", false)]
fn a_pumped_token_has_the_keyword(#[case] extra: &str, #[case] hasty: bool) {
    let script = format!(
        "Name:Muster\nManaCost:G\nTypes:Sorcery\nA:SP$ Token | TokenAmount$ 2 | TokenScript$ w_1_1_soldier | {extra}\n"
    );
    let spell = translated_in(&soldier_world(), &script, "Muster", "Sorcery", "{G}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, [ready(forest())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Muster").unwrap();
    settle(&mut driver);
    let state = driver.state();
    let tokens: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.card().name == "Soldier")
        })
        .collect();
    assert_eq!(tokens.len(), 2);
    for id in tokens {
        let haste = keywords_of(state, id)
            .iter()
            .any(|k| format!("{k:?}") == "Haste");
        assert_eq!(haste, hasty);
    }
}

#[test]
fn the_curse_hits_only_the_targeted_players_creatures() {
    let script = "Name:Hex\nManaCost:G\nTypes:Sorcery\nA:SP$ PumpAll | ValidTgts$ Player | ValidCards$ Creature | NumAtt$ -2 | NumDef$ -2 | IsCurse$ True\n";
    let spell = translated(script, "Hex", "Sorcery", "{G}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, [ready(forest()), ready(bear())])
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.cast_at("Hex", &[target]).unwrap();
    settle(&mut driver);
    assert_eq!(count(&driver, 0, "Grizzly Bears"), 1, "mine are spared");
    assert_eq!(count(&driver, 1, "Grizzly Bears"), 0, "theirs die");
}

#[test]
fn the_ultimate_takes_only_the_targeted_players_creatures() {
    let script = "Name:Take\nManaCost:G\nTypes:Sorcery\nA:SP$ GainControl | ValidTgts$ Opponent | AllValid$ Creature.TargetedPlayerCtrl | NewController$ You\n";
    let spell = translated(script, "Take", "Sorcery", "{G}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, [ready(forest())])
        .battlefield(1, [ready(bear()), ready(bear())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.cast_at("Take", &[target]).unwrap();
    settle(&mut driver);
    assert_eq!(count(&driver, 0, "Grizzly Bears"), 2);
    assert_eq!(count(&driver, 1, "Grizzly Bears"), 0);
}
