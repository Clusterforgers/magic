//! Goal: the hidden keywords a pump grants (`KW$ HIDDEN ...`) translate to the rule they state with the duration the
//! card means, an unread shape stays a gap, and the translated spell changes the object it targets when it is cast.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::{Driver, Scenario, ready, tapped};
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(kw: &str, duration: &str) -> String {
    format!(
        "Name:Spell\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | KW$ {kw}{duration} | SpellDescription$ x\n"
    )
}

#[rstest]
#[case::switch("HIDDEN CARDNAME's power and toughness are switched", "", json!("SwitchPT"), "UntilEndOfTurn")]
#[case::no_untap("HIDDEN This card doesn't untap during your next untap step.", " | Duration$ Permanent", json!("DoesntUntap"), "NextUntapStep")]
fn a_hidden_keyword_is_its_modification_for_its_duration(
    #[case] kw: &str,
    #[case] duration: &str,
    #[case] modification: Value,
    #[case] lasts: &str,
) {
    let found = abilities(&spell(kw, duration));
    let effect = &found[0]["Spell"]["effects"][0]["Continuous"];
    assert_eq!(effect["mods"], json!([modification]));
    assert_eq!(effect["duration"], json!(lasts));
}

#[rstest]
#[case::attack_or_block("HIDDEN CARDNAME can't attack or block.", json!(["CantAttack", "CantBlock"]))]
fn a_hidden_keyword_can_be_several_modifications(#[case] kw: &str, #[case] mods: Value) {
    let found = abilities(&spell(kw, ""));
    assert_eq!(found[0]["Spell"]["effects"][0]["Continuous"]["mods"], mods);
}

#[rstest]
#[case::untap_for_a_turn("HIDDEN This card doesn't untap during your next untap step.", "")]
#[case::untap_until_end_of_combat(
    "HIDDEN This card doesn't untap during your next untap step.",
    " | Duration$ UntilEndOfCombat"
)]
#[case::must_be_blocked("HIDDEN CARDNAME must be blocked if able.", "")]
#[case::untap_with_another(
    "HIDDEN This card doesn't untap during your next untap step. & Flying",
    " | Duration$ Permanent"
)]
fn a_doesnt_untap_pump_with_another_shape_stays_a_gap(#[case] kw: &str, #[case] duration: &str) {
    let text = spell(kw, duration);
    assert_eq!(gaps(&text).len(), 1, "{text}");
}

/// The spell as a card that costs nothing.
fn card_of(kw: &str, duration: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Spell")
        .with_type(CardType::Instant)
        .with_mana_cost(cost(0, &[]));
    for value in abilities(&spell(kw, duration)) {
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

fn cast_on_host(driver: &mut Driver) {
    let id = driver.id_of("Host").unwrap();
    let target = Target::Object(driver.state().object(id).unwrap().reference());
    driver.cast_at("Spell", &[target]).unwrap();
    settle(driver);
}

#[test]
fn a_switch_pump_swaps_the_power_and_toughness_of_the_target() {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [card_of(
                "HIDDEN CARDNAME's power and toughness are switched",
                "",
            )],
        )
        .battlefield(0, [ready(free_creature("Host", 1, 4))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_on_host(&mut driver);
    let id = driver.id_of("Host").unwrap();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(4), Some(1)));
}

#[test]
fn a_creature_that_doesnt_untap_stays_tapped_through_exactly_one_untap_step() {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [card_of(
                "HIDDEN This card doesn't untap during your next untap step.",
                " | Duration$ Permanent",
            )],
        )
        .battlefield(0, [tapped(free_creature("Host", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_on_host(&mut driver);
    let is_tapped = |driver: &Driver| {
        let id = driver.id_of("Host").unwrap();
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    };
    // The opponent's turn, then the controller's: the untap step is skipped for the permanent.
    driver.pass_until(Step::Main2).unwrap();
    for _ in 0..2 {
        driver.pass_until(Step::Upkeep).unwrap();
        driver.pass_until(Step::Main1).unwrap();
    }
    assert!(
        is_tapped(&driver),
        "held down through its controller's untap step"
    );
    for _ in 0..2 {
        driver.pass_until(Step::Upkeep).unwrap();
        driver.pass_until(Step::Main1).unwrap();
    }
    assert!(!is_tapped(&driver), "untaps the turn after");
}
