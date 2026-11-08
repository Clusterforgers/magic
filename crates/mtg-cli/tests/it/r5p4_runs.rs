//! Goal: round 5 piece 4 rules play right in a game: a doubled pump doubles the target and only it, a trigger's activator and card owner are the right player and not the other, and what is not of the chosen type is the only thing a sweep takes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use std::sync::Arc;

use crate::r3p3_runs::{settle, translated};

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let found = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

#[test]
fn a_doubled_pump_doubles_its_target_and_no_other_creature() {
    let twin = translated(
        "Name:Twin\nManaCost:R\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature.YouCtrl | NumAtt$ Double | NumDef$ Double\n",
        "Twin",
        "Sorcery",
        "{R}",
    );
    let scenario = Scenario::new(2)
        .hand(0, [twin])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(free_creature("Big", 3, 5)),
                ready(bear()),
            ],
        )
        .library(0, [forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .cast_at("Twin", &[driver.target_named("Big").unwrap()])
        .unwrap();
    settle(&mut driver);
    assert_eq!(stats(&driver, "Big"), (6, 10));
    assert_eq!(stats(&driver, "Grizzly Bears"), (2, 2));
}

#[test]
fn the_activator_of_a_spell_is_the_player_who_pays_not_the_other() {
    let watcher = translated(
        "Name:Watcher\nManaCost:R\nTypes:Creature Elf\nPT:1/1\nT:Mode$ SpellCast | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ Go | TriggerDescription$ x\nSVar:Go:DB$ LoseLife | Defined$ TriggeredActivator | LifeAmount$ 2\n",
        "Watcher",
        "Creature — Elf",
        "{R}",
    );
    let scenario = Scenario::new(2)
        .hand(0, [bear()])
        .battlefield(0, [ready(forest()), ready(forest()), ready(watcher)])
        .library(0, [forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Grizzly Bears").unwrap();
    settle(&mut driver);
    assert_eq!((life(&driver, 0), life(&driver, 1)), (18, 20));
}

#[test]
fn the_owner_of_a_dying_card_loses_the_life_not_the_one_who_killed_it() {
    let watcher = translated(
        "Name:Watcher\nManaCost:R\nTypes:Creature Elf\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature.Other | TriggerZones$ Battlefield | Execute$ Go | TriggerDescription$ x\nSVar:Go:DB$ LoseLife | Defined$ TriggeredCardOwner | LifeAmount$ 2\n",
        "Watcher",
        "Creature — Elf",
        "{R}",
    );
    let kill = translated(
        "Name:Kill\nManaCost:R\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Creature.Other\n",
        "Kill",
        "Sorcery",
        "{R}",
    );
    let scenario = Scenario::new(2)
        .hand(0, [kill])
        .battlefield(0, [ready(mountain()), ready(watcher)])
        .battlefield(1, [ready(bear())])
        .library(0, [forest()])
        .library(1, [forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .cast_at("Kill", &[driver.target_named("Grizzly Bears").unwrap()])
        .unwrap();
    settle(&mut driver);
    assert_eq!((life(&driver, 0), life(&driver, 1)), (20, 18));
}

fn elf(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_power_toughness(1, 1),
    )
}

fn goblin(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Goblin")
            .with_power_toughness(1, 1),
    )
}

/// The names of the creatures left after a sweep of everything not of the type picked, when the player picks `pick`.
fn sweep(pick: &str) -> Vec<String> {
    let spell = translated(
        "Name:Sweep\nManaCost:R\nTypes:Sorcery\nA:SP$ ChooseType | Defined$ You | Type$ Creature | SubAbility$ Go\nSVar:Go:DB$ DestroyAll | ValidCards$ Creature.IsNotChosenType | Zone$ Battlefield\n",
        "Sweep",
        "Sorcery",
        "{R}",
    );
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(elf("Elf A")),
                ready(goblin("Goblin A")),
            ],
        )
        .battlefield(1, [ready(elf("Elf B")), ready(goblin("Goblin B"))])
        .library(0, [forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Sweep").unwrap();
    for _ in 0..50 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|o| format!("{o:?}").contains(&format!("\"{pick}\"")))
                    .unwrap();
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    let state = driver.state();
    let mut left: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| state.object(*id).unwrap().card().name.clone())
        .filter(|name| name.contains(' ') && !name.starts_with("Mountain"))
        .collect();
    left.sort();
    left
}

#[test]
fn a_sweep_of_what_is_not_the_chosen_type_keeps_that_type_of_both_players() {
    assert_eq!(sweep("Elf"), ["Elf A", "Elf B"]);
    assert_eq!(sweep("Goblin"), ["Goblin A", "Goblin B"]);
}
