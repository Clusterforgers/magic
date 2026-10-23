//! Goal: mana counted by the game ("X mana of any one color, where X is ...", "an amount of {G} equal to ...") and mana a condition replaces ("Add {G}. If you control four or more creatures, add {G}{G} instead.") read whole in an activated mana ability and write back as the same text; near-misses stay with the table.
use super::*;

#[rstest]
#[case::allies(
    "{T}: Add X mana of any one color, where X is the number of Allies you control.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":{"Count":[{"Subtype":"Ally"},{"ControlledBy":"You"}]}}}}]}}"#
)]
#[case::its_power(
    "{T}: Add X mana of any one color, where X is this creature's power.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":{"Stat":{"of":"This","stat":"Power"}}}}}]}}"#
)]
#[case::elves_on_the_battlefield(
    "{T}: Add X mana of any one color, where X is the number of Elves on the battlefield.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":{"Count":[{"Subtype":"Elf"}]}}}}]}}"#
)]
#[case::enchantments(
    "{T}: Add X mana of any one color, where X is the number of enchantments you control.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":{"Count":[{"Type":"Enchantment"},{"ControlledBy":"You"}]}}}}]}}"#
)]
#[case::an_amount_of_one_type_by_the_greatest_power(
    "{T}: Add an amount of {G} equal to the greatest power among creatures you control.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}}}]}}"#
)]
#[case::an_amount_of_one_type_by_its_power(
    "{T}: Add an amount of {G} equal to ~'s power.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#
)]
#[case::more_with_four_creatures(
    "{T}: Add {G}. If you control four or more creatures, add {G}{G} instead.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"Fixed":["Green"]},"when":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":4}},"instead":{"Fixed":["Green","Green"]}}}}}]}}"#
)]
#[case::two_of_one_color_with_a_big_creature(
    "{T}: Add one mana of any color. If you control a creature with power 4 or greater, add two mana of any one color instead.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"AnyColor":1},"when":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":4}},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"instead":{"AnyColor":2}}}}}]}}"#
)]
fn counted_mana_and_mana_a_condition_replaces_read_and_write_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::a_fixed_count_of_one_type("{T}: Add an amount of {G} equal to two.")]
#[case::no_instead("{T}: Add {G}. If you control four or more creatures, add {G}{G}.")]
#[case::no_condition_word("{T}: Add {G}. You control four or more creatures, add {G}{G} instead.")]
#[case::x_without_a_count("{T}: Add X mana of any one color.")]
fn near_misses_of_counted_mana_stay_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Activated, false).is_none(),
        "{text}"
    );
}
