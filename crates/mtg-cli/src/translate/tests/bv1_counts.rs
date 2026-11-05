//! Goal: the count-expression grammar reads a valid-string over a zone list, over the objects a reference names and with steps into the DSL's `Amount`, names every unknown word of a string and the real head of a reference, and keeps what it cannot count a gap.
use mtg_cards::value::Amount;
use rstest::rstest;
use serde_json::{Value, json};

use super::super::count::{Body, parse};
use super::{abilities, gaps};

fn burning(svars: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ X\n{svars}\n")
}

fn damage(text: &str) -> Value {
    let found = abilities(text);
    found[0]["Spell"]["effects"][0]["DealDamage"]["amount"].clone()
}

#[rstest]
#[case::exile_too(
    "SVar:X:Count$ValidGraveyard,Exile Instant.YouOwn,Sorcery.YouOwn/Plus.1",
    json!({"Sum": [{"Count": [{"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}, {"OwnedBy": "You"},
        {"AnyOf": [{"In": "Graveyard"}, {"In": "Exile"}]}]}, 1]})
)]
#[case::counters_of_a_kind_over_objects(
    "SVar:X:Count$Valid Creature.YouCtrl$CardCounters.P1P1",
    json!({"Counters": {"on": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "counter": "+1/+1"}})
)]
#[case::every_counter_over_objects(
    "SVar:X:Count$Valid Creature.YouCtrl$CardCounters.ALL",
    json!({"AnyCounters": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}})
)]
#[case::most_common_type(
    "SVar:X:Count$MostProminentCreatureType Creature.YouCtrl",
    json!({"MostCommon": {"of": [{"Type": "Creature"}, {"ControlledBy": "You"}], "stat": "CreatureType"}})
)]
#[case::most_common_name(
    "SVar:X:Count$MostCardName Land.YouCtrl",
    json!({"MostCommon": {"of": [{"Type": "Land"}, {"ControlledBy": "You"}], "stat": "Name"}})
)]
#[case::counters_on_this_by_a_mixed_case_kind(
    "SVar:X:Count$CardCounters.Time",
    json!({"Counters": {"on": "This", "counter": {"Other": "time"}}})
)]
#[case::every_counter_on_this("SVar:X:Count$CardCounters.ALL", json!({"AnyCounters": "This"}))]
#[case::a_factor_that_is_another_part(
    "SVar:X:SVar$Y/Times.Z\nSVar:Y:Count$Party\nSVar:Z:Count$Devotion.Red",
    json!({"Product": [{"Party": "You"}, {"Devotion": ["Red"]}]})
)]
fn counts_the_grammar_reads(#[case] svars: &str, #[case] expected: Value) {
    let found = damage(&burning(svars));
    assert_eq!(found, expected);
    assert!(serde_json::from_value::<Amount>(found).is_ok());
}

#[rstest]
#[case::the_target_matches(
    "SVar:X:Targeted$Valid Creature.Elf/Plus.1",
    json!({"Sum": [{"CountOf": {"of": {"Target": 0}, "filter": [{"Subtype": "Elf"}]}}, 1]})
)]
#[case::the_stat_with_a_step(
    "SVar:X:Targeted$CardPower/Times.2",
    json!({"Times": {"by": 2, "of": {"Stat": {"of": {"Target": 0}, "stat": "Power"}}}})
)]
#[case::every_counter_on_the_target(
    "SVar:X:Targeted$CardCounters.ALL",
    json!({"AnyCounters": {"Target": 0}})
)]
fn what_the_target_is_counted_by(#[case] svars: &str, #[case] expected: Value) {
    let found = damage(&burning(svars));
    assert_eq!(found, expected);
    assert!(serde_json::from_value::<Amount>(found).is_ok());
}

#[rstest]
#[case::the_real_head(
    "SVar:X:Targeted$AllTypes",
    &["amount:Targeted$AllTypes"]
)]
#[case::an_unknown_word_after_a_known_one(
    "SVar:X:Count$ValidGraveyard,Exile Instant.YouOwn/Bogus.1/Plus.1",
    &["amount:Count/Bogus"]
)]
#[case::every_unknown_word_of_the_string(
    "SVar:X:Count$ValidGraveyard,Stack Instant.YouOwn/Bogus.1/Worse",
    &["zone:Stack", "amount:Count/Bogus", "amount:Count/Worse"]
)]
#[case::a_product_names_the_factor_that_stops_it(
    "SVar:X:SVar$Y/Times.Z\nSVar:Y:Count$Party",
    &["amount:Z"]
)]
#[case::a_cap_by_a_part_without_a_default(
    "SVar:X:Count$Party/LimitMax.W",
    &["amount:W"]
)]
fn unknown_words_are_named_each(#[case] svars: &str, #[case] expected: &[&str]) {
    assert_eq!(gaps(&burning(svars)), vec![expected.join("; ")]);
}

#[test]
fn a_filter_over_a_reference_is_never_a_count_of_the_object_itself() {
    let found = damage(&burning("SVar:X:Targeted$Valid Creature.Elf"));
    assert!(found.get("CountOf").is_some());
    assert!(!found.to_string().contains("\"Count\":"));
}

#[test]
fn a_last_state_count_with_a_fallback_stays_a_gap() {
    let text = burning("SVar:X:Count$LastStateBattlefieldWithFallback Creature.YouCtrl");
    assert_eq!(
        gaps(&text),
        vec!["amount:Count$LastStateBattlefieldWithFallback"]
    );
}

#[test]
fn a_zone_list_with_the_battlefield_is_one_alternative_per_zone() {
    let found = damage(&burning(
        "SVar:X:Count$ValidGraveyard,Battlefield Goblin.YouCtrl",
    ));
    let alternatives = found["Count"][0]["AnyOf"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(alternatives.len(), 2);
    // Control counts on the battlefield, ownership in the graveyard.
    assert!(alternatives[1].to_string().contains("ControlledBy"));
    assert!(alternatives[0].to_string().contains("OwnedBy"));
    assert!(!alternatives[0].to_string().contains("ControlledBy"));
}

#[rstest]
#[case("Count$ValidGraveyard,Battlefield Creature.YouCtrl$GreatestCardPower")]
#[case("Count$Valid Creature.YouCtrl$CardCounters.P1P1/Plus.1")]
#[case("Sacrificed$Valid Human/Twice")]
#[case("Count$MostCardName Land.YouCtrl")]
#[case("Count$ValidBattlefield,Command Card.IsCommander+YouOwn$GreatestCardManaCost")]
#[case("SVar$X/Times.Y")]
#[case("Targeted$CardPower/Times.2/Minus.1")]
#[case("Count$Valid$CardCounters.ALL")]
fn a_string_renders_as_it_was_written(#[case] text: &str) {
    let expr = parse(text);
    assert_eq!(expr.map(|expr| expr.render()), Some(text.to_owned()));
}

#[test]
fn the_zones_stick_to_the_valid_word() {
    let Some(expr) = parse("Count$ValidGraveyard,Exile Instant.YouOwn") else {
        panic!("it parses");
    };
    assert_eq!(
        expr.body,
        Body::Valid {
            zones: vec!["Graveyard", "Exile"],
            filter: Some("Instant.YouOwn"),
            reduce: None
        }
    );
}
