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

#[rstest]
#[case::target_player("Defined$ Targeted | ValidTgts$ Player", 0, 2)]
#[case::default_you("Defined$ You", 2, 0)]
fn the_mana_lands_in_the_named_pool(#[case] who: &str, #[case] mine: u32, #[case] theirs: u32) {
    let script = format!(
        "Name:Gift\nManaCost:G\nTypes:Sorcery\nA:SP$ Mana | {who} | Produced$ C | Amount$ 2\n"
    );
    let spell = translated(&script, "Gift", "Sorcery", "{G}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, [ready(forest())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    if who.contains("Targeted") {
        let target = driver.target_player(1);
        driver.cast_at("Gift", &[target]).unwrap();
    } else {
        driver.cast("Gift").unwrap();
    }
    settle(&mut driver);
    let pool = |seat| {
        driver
            .state()
            .player(PlayerId::from_index(seat))
            .unwrap()
            .mana()
            .total()
    };
    assert_eq!((pool(0), pool(1)), (mine, theirs));
}

#[rstest]
#[case::default("TokenTapped$ False", 1, 1)]
#[case::to_the_controller("Controller$ TargetedController", 0, 2)]
fn the_copy_goes_to_the_named_player(
    #[case] extra: &str,
    #[case] mine: usize,
    #[case] theirs: usize,
) {
    let script = format!(
        "Name:Mirror\nManaCost:G\nTypes:Sorcery\nA:SP$ CopyPermanent | ValidTgts$ Creature | {extra}\n"
    );
    let spell = translated(&script, "Mirror", "Sorcery", "{G}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, [ready(forest())])
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at("Mirror", &[target]).unwrap();
    settle(&mut driver);
    assert_eq!(count(&driver, 0, "Grizzly Bears"), mine);
    assert_eq!(count(&driver, 1, "Grizzly Bears"), theirs);
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

#[rstest]
#[case::caster("TriggeredCardController", 20, 26)]
#[case::me("You", 23, 23)]
fn the_copy_of_the_opponents_spell_is_controlled_as_the_card_says(
    #[case] controller: &str,
    #[case] mine: i32,
    #[case] theirs: i32,
) {
    let watcher = format!(
        "Name:Watcher\nManaCost:G\nTypes:Creature Wizard\nPT:1/1\nT:Mode$ SpellCast | ValidCard$ Instant | Execute$ Copy | TriggerZones$ Battlefield | TriggerDescription$ x\nSVar:Copy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility | Controller$ {controller}\n"
    );
    let watcher = translated(&watcher, "Watcher", "Creature — Wizard", "{G}");
    let boon = translated(
        "Name:Boon\nManaCost:G\nTypes:Instant\nA:SP$ GainLife | LifeAmount$ 3\n",
        "Boon",
        "Instant",
        "{G}",
    );
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(watcher)])
        .battlefield(1, [ready(forest())])
        .hand(1, [boon]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for _ in 0..8 {
        if driver.decision().unwrap().player == PlayerId::from_index(1) {
            break;
        }
        driver.answer_passively().unwrap();
    }
    driver.cast("Boon").unwrap();
    settle(&mut driver);
    assert_eq!((life(&driver, 0), life(&driver, 1)), (mine, theirs));
}
