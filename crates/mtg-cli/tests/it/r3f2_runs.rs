//! Goal: a translated step that moves objects counts what it moved even when they were tokens that ceased to exist, so "if you can't" and "for each sacrificed this way" read the right number in a played game.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, swamp};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use std::sync::Arc;

use crate::r3p3_runs::{settle, translated, translated_in};
use mtg_cli::forge::script::Script;
use mtg_cli::translate::ctx::World;

const RUST: &str = "Name:Rust Elemental\nManaCost:4\nTypes:Artifact Creature Elemental\nPT:4/4\nK:Flying\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigSac | TriggerDescription$ At the beginning of your upkeep, sacrifice an artifact other than CARDNAME. If you can't, tap CARDNAME and you lose 4 life.\nSVar:TrigSac:DB$ Sacrifice | Defined$ You | SacValid$ Artifact.Other | SubAbility$ DBTap | RememberSacrificed$ True\nSVar:DBTap:DB$ Tap | Defined$ Self | ConditionCheckSVar$ X | ConditionSVarCompare$ LT1 | SubAbility$ DBLoseLife\nSVar:DBLoseLife:DB$ LoseLife | Defined$ You | LifeAmount$ 4 | ConditionCheckSVar$ X | ConditionSVarCompare$ LT1 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Amount\n";
const PIT: &str = "Name:Lord of the Pit\nManaCost:4 B B B\nTypes:Creature Demon\nPT:7/7\nK:Flying\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigSac | TriggerDescription$ At the beginning of your upkeep, sacrifice a creature other than CARDNAME. If you can't, CARDNAME deals 7 damage to you.\nSVar:TrigSac:DB$ Sacrifice | Defined$ You | SacValid$ Creature.Other | SubAbility$ DBDamage | RememberSacrificed$ True\nSVar:DBDamage:DB$ DealDamage | Defined$ You | NumDmg$ 7 | ConditionCheckSVar$ X | ConditionSVarCompare$ LT1 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Amount\n";
const BEAST: &str = "Name:Voracious Fell Beast\nManaCost:4 B B\nTypes:Creature Drake Beast\nPT:4/4\nK:Flying\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigSac | TriggerDescription$ When CARDNAME enters, each opponent sacrifices a creature. Create a Food token for each creature sacrificed this way.\nSVar:TrigSac:DB$ Sacrifice | Defined$ Opponent | SacValid$ Creature | RememberSacrificed$ True | SubAbility$ DBFood\nSVar:DBFood:DB$ Token | TokenScript$ c_a_food_sac | TokenOwner$ You | TokenAmount$ X | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Amount\n";

fn maker(name: &str, ty: CardType, token: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Spell":{{"effects":[{{"CreateToken":{{"token":{{"Custom":{token}}}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new(name)
            .with_type(ty)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(serde_json::from_str::<Ability>(&json).unwrap()),
    )
}

const CLUE: &str = r#"{"name":"Clue","types":["Artifact"],"subtypes":["Clue"]}"#;
const GOBLIN: &str =
    r#"{"name":"Goblin","types":["Creature"],"subtypes":["Goblin"],"power":1,"toughness":1}"#;

fn library(n: usize) -> Vec<Arc<CardDefinition>> {
    (0..n).map(|_| forest()).collect()
}

/// Life and tapped state of the upkeep sacrificer after the token (or nothing) was there to sacrifice.
fn upkeep(card: Arc<CardDefinition>, name: &str, token: Option<&str>) -> (i32, bool) {
    let mut scenario = Scenario::new(2)
        .battlefield(0, [ready(card)])
        .library(0, library(9))
        .library(1, library(9));
    if token.is_some() {
        scenario = scenario.hand(
            0,
            [maker("Maker", CardType::Sorcery, token.unwrap_or(CLUE))],
        );
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    if token.is_some() {
        driver.cast("Maker").unwrap();
        settle(&mut driver);
    }
    driver.idle_until(3, Step::Upkeep).unwrap();
    settle(&mut driver);
    let id = driver.id_of(name).unwrap();
    let life = driver
        .state()
        .player(mtg_core::ids::PlayerId::from_index(0))
        .unwrap()
        .life();
    (
        life,
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped(),
    )
}

#[rstest]
#[case::token_artifact(Some(CLUE), 20, false)]
#[case::nothing_to_sacrifice(None, 16, true)]
fn rust_elemental_counts_a_sacrificed_token(
    #[case] token: Option<&str>,
    #[case] life: i32,
    #[case] tapped: bool,
) {
    let card = translated(
        RUST,
        "Rust Elemental",
        "Artifact Creature — Elemental",
        "{4}",
    );
    assert_eq!(upkeep(card, "Rust Elemental", token), (life, tapped));
}

#[test]
fn lord_of_the_pit_does_not_hurt_its_controller_after_a_token_is_sacrificed() {
    let card = translated(PIT, "Lord of the Pit", "Creature — Demon", "{4}{B}{B}{B}");
    assert_eq!(upkeep(card, "Lord of the Pit", Some(GOBLIN)), (20, false));
}

#[test]
fn voracious_fell_beast_makes_a_food_for_a_sacrificed_token() {
    let beast = translated_in(
        &food_world(),
        BEAST,
        "Voracious Fell Beast",
        "Creature — Drake Beast",
        "{4}{B}{B}",
    );
    let scenario = Scenario::new(2)
        .battlefield(0, (0..6).map(|_| ready(swamp())).collect::<Vec<_>>())
        .hand(0, [beast])
        .hand(1, [maker("Flash Maker", CardType::Instant, GOBLIN)])
        .library(0, library(9))
        .library(1, library(9));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Voracious Fell Beast").unwrap();
    for _ in 0..8 {
        if driver.decision().unwrap().player == mtg_core::ids::PlayerId::from_index(1) {
            break;
        }
        driver.answer_passively().unwrap();
    }
    driver.cast("Flash Maker").unwrap();
    settle(&mut driver);
    let names: Vec<String> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    assert!(names.iter().any(|name| name == "Food"), "{names:?}");
}

fn food_world() -> World {
    let food = Script::parse(
        "Name:Food Token\nManaCost:no cost\nTypes:Artifact Food\nA:AB$ GainLife | Cost$ 2 T Sac<1/CARDNAME/this token> | LifeAmount$ 3 | SpellDescription$ You gain 3 life.\n",
    );
    let mut world = World::default();
    world.tokens.insert(
        "c_a_food_sac".to_owned(),
        food.faces.into_iter().next().unwrap(),
    );
    world
}

const URZA: &str = "Name:Urza's Saga\nManaCost:no cost\nTypes:Enchantment Land Urza's Saga\nK:Chapter:3:Animate1,Animate2,Tutor\nSVar:Animate1:DB$ Animate | Defined$ Self | Abilities$ ABMana | Duration$ Permanent | SpellDescription$ CARDNAME gains \"{T}: Add {C}.\"\nSVar:ABMana:AB$ Mana | Cost$ T | Produced$ C | SpellDescription$ Add {C}.\nSVar:Animate2:DB$ Animate | Defined$ Self | Abilities$ ABToken | Duration$ Permanent | SpellDescription$ CARDNAME gains a token ability.\nSVar:ABToken:AB$ Token | Cost$ 2 T | TokenScript$ c_0_0_a_construct_total_artifacts | SpellDescription$ Create a Construct.\nSVar:Tutor:DB$ ChangeZone | Origin$ Library | Destination$ Battlefield | ChangeType$ Artifact.ManaCost0,Artifact.ManaCost1 | ChangeNum$ 1 | SpellDescription$ Search your library for an artifact card with mana cost {0} or {1}, put it onto the battlefield, then shuffle.\n";

fn artifact(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

#[test]
fn urzas_saga_offers_only_artifacts_whose_cost_is_zero_or_one_generic() {
    let mut cards = library(20);
    cards.extend([
        artifact("Cost W", "{W}"),
        artifact("Cost 1", "{1}"),
        artifact("Cost 0", "{0}"),
        artifact("Cost 2", "{2}"),
        artifact("Cost UB", "{U/B}"),
        artifact("Cost P", "{B/P}"),
    ]);
    let mut world = World::default();
    let construct = Script::parse(
        "Name:Construct Token\nManaCost:no cost\nTypes:Artifact Creature Construct\nPT:0/0\n",
    );
    world.tokens.insert(
        "c_0_0_a_construct_total_artifacts".to_owned(),
        construct.faces.into_iter().next().unwrap(),
    );
    let saga = translated_in(
        &world,
        URZA,
        "Urza's Saga",
        "Enchantment Land — Urza's Saga",
        "",
    );
    let scenario = Scenario::new(2)
        .hand(0, [saga])
        .battlefield(0, (0..2).map(|_| ready(forest())).collect::<Vec<_>>())
        .library(0, cards)
        .library(1, library(20));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.play_land("Urza's Saga").unwrap();
    driver.idle_until(7, Step::Main1).unwrap();
    let mut offered: Vec<String> = Vec::new();
    for _ in 0..40 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            mtg_engine::decision::DecisionKind::ChooseCards(o) => {
                offered = o
                    .from
                    .iter()
                    .map(|i| driver.state().object(*i).unwrap().card().name.clone())
                    .collect();
                let pick = o.from.iter().copied().take(1).collect();
                driver
                    .answer(mtg_engine::decision::Answer::ChooseCards(
                        mtg_engine::choose::CardsChoice(pick),
                    ))
                    .unwrap();
            }
            _ if driver.state().stack().is_empty() && !offered.is_empty() => break,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    offered.sort();
    assert_eq!(offered, ["Cost 0", "Cost 1"]);
}
