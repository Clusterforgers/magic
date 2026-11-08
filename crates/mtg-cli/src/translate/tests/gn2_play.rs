//! Goal: the damage replacements the translator writes for Forge scripts, played through the engine, land on the right
//! object: the prevented damage is not dealt, the counter or the life goes to the object the script names, and the
//! redirected damage is dealt by the same source to the enchanted creature.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ready};

use super::abilities;

/// A card with the abilities a Forge script translates to.
fn translated(script: &str, ty: CardType, stats: Option<(i32, i32)>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject").with_type(ty);
    if script.contains("Aura") {
        card = card.with_subtype("Aura");
    }
    if let Some((power, toughness)) = stats {
        card = card.with_power_toughness(power, toughness);
    }
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

/// Casts the bolt at `target` from the first seat and passes until the stack is empty.
fn bolt(driver: &mut Driver, target: &Target) {
    driver.pass_until(Step::Main1).unwrap();
    driver.begin_cast("Bolt").unwrap();
    driver.choose_targets(std::slice::from_ref(target)).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    for _ in 0..50 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if matches!(
            driver.engine().decision().map(|d| &d.kind),
            Some(DecisionKind::Priority(_))
        ) && idle
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

fn object_target(driver: &Driver, name: &str) -> Target {
    let id = driver.id_of(name).unwrap();
    Target::Object(driver.state().object(id).unwrap().reference())
}

const PURITY: &str = "Name:Subject\nTypes:Enchantment\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You | ReplaceWith$ Gain | PreventionEffect$ True | IsCombat$ False\nSVar:Gain:DB$ GainLife | Defined$ ReplacedTarget | LifeAmount$ X\nSVar:X:ReplaceCount$DamageAmount\n";
const HYDRA: &str = "Name:Subject\nTypes:Creature Hydra\nPT:5/5\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ Card.Self | ReplaceWith$ C | PreventionEffect$ True\nSVar:C:DB$ PutCounter | Defined$ ReplacedTarget | CounterType$ M1M1 | CounterNum$ X\nSVar:X:ReplaceCount$DamageAmount\n";
const PARIAH: &str = "Name:Subject\nTypes:Enchantment Aura\nK:Enchant:Creature\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You | ReplaceWith$ D | DamageTarget$ Enchanted\nSVar:D:DB$ ReplaceEffect | VarName$ Affected | VarValue$ Enchanted | VarType$ Card\n";

#[test]
fn damage_to_you_is_prevented_and_you_gain_that_much() {
    let game = Scenario::new(2).hand(0, [burn("Bolt", 3)]).battlefield(
        0,
        [
            ready(mountain()),
            ready(translated(PURITY, CardType::Enchantment, None)),
        ],
    );
    let mut driver = Driver::from_scenario(game).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    bolt(&mut driver, &Target::Player(PlayerId::from_index(0)));
    assert_eq!(
        life(&driver, 0),
        23,
        "the 3 damage is prevented and gained back"
    );
    assert_eq!(life(&driver, 1), 20);
}

#[test]
fn damage_to_the_creature_becomes_minus_counters_on_it() {
    let game = Scenario::new(2)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(0, [ready(mountain())])
        .battlefield(
            1,
            [ready(translated(HYDRA, CardType::Creature, Some((5, 5))))],
        );
    let mut driver = Driver::from_scenario(game).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    let hydra = object_target(&driver, "Subject");
    bolt(&mut driver, &hydra);
    let id = driver.id_of("Subject").unwrap();
    let object = driver.state().object(id).unwrap();
    assert_eq!(
        object.permanent().unwrap().damage(),
        0,
        "the damage is prevented"
    );
    assert_eq!(
        object
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::MinusOneMinusOne),
        3
    );
}

#[test]
fn damage_to_you_is_dealt_to_the_enchanted_creature_instead() {
    let pariah = attached_to(ready(translated(PARIAH, CardType::Enchantment, None)), 1, 0);
    let game = Scenario::new(2)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(card("Grizzly Bears").unwrap()), pariah]);
    let mut driver = Driver::from_scenario(game).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    bolt(&mut driver, &Target::Player(PlayerId::from_index(1)));
    assert_eq!(life(&driver, 1), 20, "the player is dealt nothing");
    assert!(
        driver.id_of("Grizzly Bears").is_err(),
        "the enchanted creature takes the 3 damage and dies"
    );
}
