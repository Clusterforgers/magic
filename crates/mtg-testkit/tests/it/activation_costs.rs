//! Goal: a cost modifier of one kind of activated ability changes the equip cost only for that kind, honours the permanents it applies to and the least mana it leaves, and a prohibition of one kind forbids that kind and no other, with the log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, creature, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// Whether the Blade (Equip {2}) of seat 0 can be equipped with `lands` Forests and the Warden's rule in play.
fn equip_offered(warden: &str, lands: usize) -> bool {
    let blade = Arc::new(
        CardDefinition::new("Blade")
            .with_type(CardType::Artifact)
            .with_rules(Ability::Keyword(Keyword::Equip(Cost::mana(
                "{2}".parse().unwrap(),
            )))),
    );
    let warden = Arc::new(
        CardDefinition::new("Warden")
            .with_type(CardType::Artifact)
            .with_rules(ability(warden)),
    );
    let mut mine = vec![ready(blade), ready(warden), ready(bear())];
    mine.extend((0..lands).map(|_| ready(forest())));
    let scenario = Scenario::new(2)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine)
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    let driver = Driver::from_scenario(scenario).unwrap();
    let offered = !driver.offered_abilities("Blade").unwrap().is_empty();
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    offered
}

fn modifier(applies: &str, kind: &str, change: &str, mana: &str, least: u32) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":{applies},"change":"{change}","mana":"{mana}","activating":"{kind}","least":{least}}}}}"#
    )
}

#[rstest]
#[case::equip_reduction_makes_it_affordable(&modifier("[]", "Equip", "Reduce", "{1}", 0), 1, true)]
#[case::without_it_one_land_is_short(r#"{"Keyword":"Defender"}"#, 1, false)]
#[case::another_kind_changes_nothing(&modifier("[]", "Cycling", "Reduce", "{1}", 0), 1, false)]
#[case::any_kind_covers_equip(&modifier("[]", "Any", "Reduce", "{1}", 0), 1, true)]
#[case::the_floor_keeps_the_cost(&modifier("[]", "Any", "Reduce", "{1}", 2), 1, false)]
#[case::a_floor_below_the_result_changes_nothing(&modifier("[]", "Any", "Reduce", "{1}", 1), 1, true)]
#[case::only_creatures_are_covered(&modifier(r#"[{"Type":"Creature"}]"#, "Equip", "Reduce", "{1}", 0), 1, false)]
#[case::only_artifacts_are_covered(&modifier(r#"[{"Type":"Artifact"}]"#, "Equip", "Reduce", "{1}", 0), 1, true)]
#[case::an_increase_costs_more(&modifier("[]", "Equip", "Increase", "{1}", 0), 2, false)]
#[case::an_increase_of_another_kind(&modifier("[]", "Cycling", "Increase", "{1}", 0), 2, true)]
fn a_modifier_of_one_kind_changes_that_kind_only(
    #[case] warden: &str,
    #[case] lands: usize,
    #[case] expected: bool,
) {
    assert_eq!(equip_offered(warden, lands), expected);
}

#[rstest]
#[case::equip_is_forbidden("Equip", false)]
#[case::cycling_is_not_equip("Cycling", true)]
#[case::loyalty_is_not_equip("Loyalty", true)]
#[case::any_forbids_it("Any", false)]
fn a_prohibition_of_a_kind_forbids_that_kind(#[case] kind: &str, #[case] expected: bool) {
    let rule = format!(
        r#"{{"Rule":{{"rule":{{"Cant":{{"who":"EachPlayer","action":{{"ActivateKind":{{"kind":"{kind}"}}}}}}}}}}}}"#
    );
    assert_eq!(equip_offered(&rule, 2), expected);
}

/// Whether each cast of `order` succeeds when the Warden makes the first creature spell each turn cost {1} less and the
/// player has no mana: Cubs cost {1}, the Charm costs {1}.
fn first_spell_casts(order: &[&str]) -> Vec<bool> {
    let tally = r#"{"Compare":{"left":{"Tally":{"event":"SpellCast","who":"You","filter":[{"Type":"Creature"}]}},"op":"Eq","right":0}}"#;
    let rule = format!(
        r#"{{"CostModifier":{{"applies_to":["Spell",{{"Type":"Creature"}},{{"ControlledBy":"You"}}],"change":"Reduce","mana":"{{1}}","condition":{tally}}}}}"#
    );
    let warden = Arc::new(
        CardDefinition::new("Warden")
            .with_type(CardType::Artifact)
            .with_rules(ability(&rule)),
    );
    let scenario = Scenario::new(2)
        .at(2, 0, Step::Main1)
        .hand(
            0,
            vec![
                creature("Cub", 1, &[], 1, 1),
                creature("Whelp", 1, &[], 1, 1),
                mtg_testkit::cards::instant("Charm", 1, &[]),
            ],
        )
        .battlefield(0, vec![ready(warden)])
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let cast = order
        .iter()
        .map(|name| {
            let cast = driver.cast(name).is_ok();
            for _ in 0..40 {
                if driver.state().stack().is_empty() {
                    break;
                }
                driver.pass().unwrap();
            }
            cast
        })
        .collect();
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    cast
}

#[rstest]
#[case::the_first_creature_is_free(&["Cub"], &[true])]
#[case::the_second_pays_full(&["Cub", "Whelp"], &[true, false])]
#[case::a_noncreature_first_changes_nothing(&["Charm", "Cub"], &[false, true])]
fn only_the_first_spell_of_a_kind_each_turn_is_cheaper(
    #[case] order: &[&str],
    #[case] expected: &[bool],
) {
    assert_eq!(first_spell_casts(order), expected);
}
