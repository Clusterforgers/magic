//! Goal: an amount that Forge names `AffectedX` is read from the object each change applies to, and a bare `You` among the
//! objects a static line affects stays a gap rather than a creature subtype.
use rstest::rstest;

use super::{abilities, gaps};

#[rstest]
#[case::shares_a_type(
    "Creature",
    "AddPower$ AffectedX | AddToughness$ AffectedX",
    "Count$Valid Creature.sharesCreatureTypeWith+Other",
    r#"{"Count":[{"Type":"Creature"},{"SharesSubtype":"Affected"},{"Not":{"Is":"Affected"}}]}"#
)]
#[case::counters_on_it(
    "Creature.YouDontCtrl",
    "AddPower$ AffectedX | AddToughness$ AffectedX",
    "Count$CardCounters.SPORE",
    r#""Counters":{"counter":{"Other":"spore"},"on":"Affected"}"#
)]
#[case::mana_value(
    "Artifact.nonCreature",
    "SetPower$ AffectedX | SetToughness$ AffectedX | AddType$ Creature",
    "Count$CardManaCost",
    r#"{"Stat":{"of":"Affected","stat":"ManaValue"}}"#
)]
fn an_affected_amount_names_the_object_it_applies_to(
    #[case] affected: &str,
    #[case] change: &str,
    #[case] count: &str,
    #[case] expected: &str,
) {
    let script = format!(
        "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ {affected} | {change} | EffectZone$ Battlefield\nSVar:AffectedX:{count}\n"
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert!(found[0].to_string().contains(expected), "{}", found[0]);
}

#[test]
fn a_source_amount_keeps_this_as_the_source() {
    let script = "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Creature.YouCtrl | AddPower$ X | EffectZone$ Battlefield\nSVar:X:Count$CardCounters.UNITY\n";
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert!(
        found[0].to_string().contains(r#""on":"This""#),
        "{}",
        found[0]
    );
    assert!(!found[0].to_string().contains("Affected"), "{}", found[0]);
}
