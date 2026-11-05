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

#[rstest]
#[case::with_objects("You,Planeswalker.YouCtrl,Creature.YouCtrl+Other")]
#[case::with_a_subtype("You,Human.YouCtrl")]
fn a_player_among_the_objects_a_static_line_affects_is_a_gap(#[case] affected: &str) {
    let script = format!(
        "Name:T\nTypes:Creature\nS:Mode$ Continuous | Affected$ {affected} | AddKeyword$ Hexproof\n"
    );
    assert!(abilities(&script).is_empty(), "{:?}", abilities(&script));
}

#[test]
fn a_player_among_valid_targets_is_not_a_subtype() {
    let script = "Name:T\nTypes:Creature\nT:Mode$ BecomesTarget | ValidTarget$ You | ValidSource$ Spell | Execute$ Go\nSVar:Go:DB$ Untap | Defined$ Self\n";
    let found = abilities(script);
    assert!(
        !found
            .iter()
            .any(|a| a.to_string().contains(r#""Subtype":"You""#)),
        "{found:?} {:?}",
        gaps(script)
    );
}

#[test]
fn a_word_of_a_type_line_that_is_no_subtype_stays_out_of_the_world() {
    use crate::forge::script::Script;
    use crate::translate::ctx::World;

    let scripts = [Script::parse(
        "Name:Minion\nTypes:Legendary Planeswalker You\n",
    )];
    let world = World::new(&scripts, std::collections::BTreeMap::new());
    assert!(!world.subtypes.contains("You"), "{:?}", world.subtypes);
    let scripts = [Script::parse("Name:Bear\nTypes:Creature Bear\n")];
    let world = World::new(&scripts, std::collections::BTreeMap::new());
    assert!(world.subtypes.contains("Bear"));
}

#[rstest]
#[case::on_the_stack("AffectedZone$ Stack", true)]
#[case::on_the_battlefield("EffectZone$ Battlefield", false)]
fn the_spells_a_first_spell_line_counts_exclude_the_spell_it_affects(
    #[case] zone: &str,
    #[case] excluded: bool,
) {
    let script = format!(
        "Name:T\nTypes:Creature Wizard\nS:Mode$ Continuous | Affected$ Creature.YouCtrl | {zone} | CheckSVar$ X | SVarCompare$ EQ0 | AddKeyword$ Lifelink\nSVar:X:Count$ThisTurnCast_Creature.YouCtrl\n"
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let text = found[0].to_string();
    assert_eq!(
        text.contains(r#"{"Not":{"Is":"Affected"}}"#),
        excluded,
        "{text}"
    );
}
