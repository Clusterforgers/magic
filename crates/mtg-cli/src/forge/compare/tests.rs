//! Goal: an item of a script is read when one of the verbs the map names for it is in our rules and missing when none is, an item without a verb or a map entry says so, bookkeeping gives nothing, a key that asks for a verb of its own is checked on a read item, and the verbs of a card are the primitives of the abilities it models.
use rstest::rstest;
use serde_json::json;

use super::*;
use crate::forge::script::Script;

const SMALL: &str = "\
effect Zap => Effect.Alpha
effect Zap Mode=Hand => Effect.Beta
effect Mend => -
effect Bookkeep => ~
trigger Bell => Trigger.Ring
keyword Odd => -
also effect Zap Extra => Mod.X Mod.Y
also effect Mend Extra => Mod.Z
variant effect Zap Mode
";

fn map() -> Map {
    Map::parse(SMALL).unwrap_or_else(|error| panic!("{error}"))
}

fn ours(verbs: &[&str]) -> BTreeSet<String> {
    verbs.iter().map(|verb| norm(verb)).collect()
}

fn findings(lines: &str, have: &[&str]) -> Vec<Finding> {
    let items = Script::parse(&format!("Name:Test\n{lines}")).items();
    compare(&items, &ours(have), &map())
}

fn statuses(found: &[Finding]) -> Vec<(&str, Option<&str>, Status)> {
    found
        .iter()
        .map(|f| (f.name.as_str(), f.key.as_deref(), f.status))
        .collect()
}

#[rstest]
#[case::read("A:SP$ Zap", &["Effect.Alpha"], Status::Read)]
#[case::read_in_any_case("A:SP$ Zap", &["effect.alpha"], Status::Read)]
#[case::missing("A:SP$ Zap", &["Effect.Other"], Status::Missing)]
#[case::missing_from_nothing("A:SP$ Zap", &[], Status::Missing)]
#[case::the_verb_of_the_specific_rule("A:SP$ Zap | Mode$ Hand", &["Effect.Beta"], Status::Read)]
#[case::the_general_verb_does_not_do_for_the_specific_rule("A:SP$ Zap | Mode$ Hand", &["Effect.Alpha"], Status::Missing)]
#[case::no_verb("A:SP$ Mend", &["Effect.Alpha"], Status::NoVerb)]
#[case::not_in_the_map("A:SP$ Unheard", &["Effect.Alpha"], Status::Unmapped)]
#[case::a_trigger("T:Mode$ Bell", &["Trigger.Ring"], Status::Read)]
#[case::a_keyword_of_its_name("K:Flying", &["Keyword.Flying"], Status::Read)]
#[case::a_keyword_we_lack("K:Flying", &["Keyword.Reach"], Status::Missing)]
#[case::a_keyword_with_no_verb("K:Odd", &[], Status::NoVerb)]
fn an_item_stands_against_the_verbs_of_our_rules(
    #[case] line: &str,
    #[case] have: &[&str],
    #[case] expected: Status,
) {
    let found = findings(line, have);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].status, expected);
    assert_eq!(found[0].key, None);
}

#[test]
fn bookkeeping_gives_no_finding_and_the_rest_keep_their_order() {
    let found = findings(
        "A:SP$ Zap | SubAbility$ DBClean\nSVar:DBClean:DB$ Bookkeep | SubAbility$ DBEnd\nSVar:DBEnd:DB$ Mend",
        &["Effect.Alpha"],
    );
    assert_eq!(
        statuses(&found),
        [("Zap", None, Status::Read), ("Mend", None, Status::NoVerb)]
    );
}

#[test]
fn a_key_that_asks_for_a_verb_is_checked_on_a_read_item() {
    let line = "A:SP$ Zap | Extra$ 1";
    let missing = findings(line, &["Effect.Alpha"]);
    assert_eq!(
        statuses(&missing),
        [
            ("Zap", Some("Extra"), Status::Missing),
            ("Zap", None, Status::Read)
        ]
    );
    assert_eq!(missing[0].expect, ["Mod.X", "Mod.Y"]);

    let given = findings(line, &["Effect.Alpha", "Mod.Y"]);
    assert_eq!(statuses(&given), [("Zap", None, Status::Read)]);
}

#[test]
fn a_key_is_checked_on_a_missing_item_too_but_not_on_an_item_without_a_verb() {
    let missing = findings("A:SP$ Zap | Extra$ 1", &[]);
    assert_eq!(missing.len(), 2);
    assert_eq!(missing[1].status, Status::Missing);
    let without = findings("A:SP$ Mend | Extra$ 1", &[]);
    assert_eq!(statuses(&without), [("Mend", None, Status::NoVerb)]);
}

#[test]
fn the_keys_of_a_finding_are_the_meaningful_ones_and_the_variants_with_their_value() {
    let found = findings(
        "A:SP$ Zap | Cost$ T | Mode$ Hand | Optional$ True | SpellDescription$ text | ConditionPresent$ Card",
        &[],
    );
    assert_eq!(
        found[0].keys,
        ["Mode", "Optional", "Condition", "Mode=Hand"]
    );
}

fn card(abilities: &serde_json::Value) -> Card {
    serde_json::from_value(json!({
        "oracle_id": "00000000-0000-4000-8000-000000000001",
        "name": "Test",
        "impl_status": "Partial",
        "faces": [{ "name": "Test", "abilities": abilities }],
    }))
    .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn the_verbs_of_a_card_are_the_primitives_of_the_abilities_it_models() {
    let verbs = verbs_of(&card(&json!([
        { "Spell": { "effects": [{ "Draw": {} }] } },
        { "Keyword": "Flying" },
        { "Manual": { "reason": "left", "span": { "start": 0, "end": 1 } } },
    ])));
    assert!(verbs.contains("ability.spell") && verbs.contains("effect.draw"));
    assert!(verbs.contains("keyword.flying"));
    assert!(!verbs.iter().any(|verb| verb.contains("manual")));
}

#[test]
fn a_plain_mana_ability_counts_as_a_mana_effect_with_a_tap_cost() {
    let verbs = verbs_of(&card(&json!([
        { "Activated": { "cost": ["Tap"], "effects": [{ "AddMana": { "mana": { "Fixed": ["Green"] } } }] } },
    ])));
    for wanted in ["effect.addmana", "cost.tap", "ability.activated"] {
        assert!(verbs.contains(wanted), "{wanted} in {verbs:?}");
    }
}

#[test]
fn a_card_with_nothing_modelled_has_no_verbs() {
    let verbs = verbs_of(&card(&json!([
        { "Manual": { "reason": "left", "span": { "start": 0, "end": 1 } } },
    ])));
    assert!(verbs.is_empty());
}
