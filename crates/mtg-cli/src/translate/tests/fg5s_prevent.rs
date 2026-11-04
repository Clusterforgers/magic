//! Goal: the prevention keywords in words ("Prevent all damage that would be dealt to CARDNAME.") translate to
//! replacements of the creature itself, a line of another wording stays a gap, and the damage the line names is
//! prevented when it is played through the engine while other damage is not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

fn creature_with(line: &str) -> Arc<CardDefinition> {
    let text = format!("Name:Subject\nTypes:Creature Elf\nPT:2/2\n{line}\n");
    let mut card = CardDefinition::new("Subject")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    for value in abilities(&text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn settle(driver: &mut Driver) {
    for _ in 0..50 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle
            && matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::Priority(_))
            )
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

/// Whether the subject of seat 1 survives a bolt of 3 damage from seat 0.
fn survives_a_bolt(line: &str) -> bool {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(creature_with(line))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let id = driver.id_of("Subject").unwrap();
    let target = Target::Object(driver.state().object(id).unwrap().reference());
    driver.begin_cast("Bolt").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    settle(&mut driver);
    driver.id_of("Subject").is_ok()
}

/// The life of seat 1 after the subject of seat 0 attacks it unblocked and the turn reaches the second main phase.
fn life_after_attack(line: &str) -> i32 {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(creature_with(line))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Subject", 1)]).unwrap();
    for _ in 0..30 {
        if driver.state().turn().step() == Step::Main2 {
            break;
        }
        driver.answer_passively().unwrap();
    }
    driver
        .state()
        .player(PlayerId::from_index(1))
        .unwrap()
        .life()
}

#[rstest]
#[case::to("K:Prevent all damage that would be dealt to CARDNAME.", true)]
#[case::to_and_by(
    "K:Prevent all damage that would be dealt to and dealt by CARDNAME.",
    true
)]
#[case::combat_to("K:Prevent all combat damage that would be dealt to CARDNAME.", false)]
#[case::combat_to_and_by(
    "K:Prevent all combat damage that would be dealt to and dealt by CARDNAME.",
    false
)]
#[case::by_only("K:Prevent all damage that would be dealt by CARDNAME.", false)]
fn a_prevention_line_protects_the_creature_from_the_damage_it_names(
    #[case] line: &str,
    #[case] survives: bool,
) {
    assert_eq!(survives_a_bolt(line), survives, "{line}");
}

#[rstest]
#[case::to("K:Prevent all damage that would be dealt to CARDNAME.", 1)]
#[case::to_and_by(
    "K:Prevent all damage that would be dealt to and dealt by CARDNAME.",
    2
)]
#[case::combat_by("K:Prevent all combat damage that would be dealt by CARDNAME.", 1)]
#[case::combat_to_and_by(
    "K:Prevent all combat damage that would be dealt to and dealt by CARDNAME.",
    2
)]
#[case::by("K:Prevent all damage that would be dealt by CARDNAME.", 1)]
fn a_prevention_line_is_one_replacement_for_each_direction(
    #[case] line: &str,
    #[case] count: usize,
) {
    let text = format!("Name:Subject\nTypes:Creature Elf\nPT:2/2\n{line}\n");
    let found = abilities(&text);
    assert_eq!(found.len(), count);
    assert!(found.iter().all(|one| one.get("Replacement").is_some()));
}

#[rstest]
#[case::to("K:Prevent all damage that would be dealt to CARDNAME.", 18)]
#[case::by("K:Prevent all damage that would be dealt by CARDNAME.", 20)]
#[case::combat_by("K:Prevent all combat damage that would be dealt by CARDNAME.", 20)]
#[case::to_and_by(
    "K:Prevent all damage that would be dealt to and dealt by CARDNAME.",
    20
)]
fn a_prevention_line_stops_the_damage_the_creature_deals(#[case] line: &str, #[case] life: i32) {
    assert_eq!(life_after_attack(line), life, "{line}");
}

#[rstest]
#[case::another_wording("K:Prevent all damage that would be dealt to target creature.")]
#[case::redirect("K:Prevent all damage that would be dealt to CARDNAME if able.")]
fn a_prevention_line_of_another_wording_stays_a_gap(#[case] line: &str) {
    let text = format!("Name:Subject\nTypes:Creature Elf\nPT:2/2\n{line}\n");
    assert_eq!(gaps(&text).len(), 1);
}
