//! Goal: the filter grammar splits a valid-string into alternatives, a base and properties without losing a word, names every unknown word of a string, and keeps the readers' output unchanged.
use rstest::rstest;
use serde_json::{Value, json};

use super::super::filter::parse;
use super::{abilities, gaps};

#[rstest]
#[case::one_base("Creature")]
#[case::all_of("Creature.YouCtrl+Other")]
#[case::negated("Creature.!token+!Other")]
#[case::any_of("Instant.YouCtrl,Sorcery.YouCtrl")]
#[case::argument_with_a_plus("Aura.AttachedTo Creature.attacking+OppCtrl")]
#[case::word_with_a_space("Creature.sharesCreatureTypeWith Valid Creature.YouCtrl")]
fn a_filter_renders_as_it_was_written(#[case] text: &str) {
    assert_eq!(
        parse(text).map(|filter| filter.render()),
        Ok(text.to_owned())
    );
}

#[rstest]
#[case::empty_alternative("Creature,", "valid:empty-alternative")]
#[case::no_base(".YouCtrl", "valid:empty-alternative")]
#[case::bare_negation("Creature.!", "valid:empty-property")]
fn an_unreadable_string_is_refused(#[case] text: &str, #[case] gap: &str) {
    assert_eq!(
        parse(text).map(|filter| filter.render()),
        Err(gap.to_owned())
    );
}

fn counting(valid: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ X\nSVar:X:Count$Valid {valid}\n"
    )
}

#[rstest]
#[case::two_unknown_properties("Creature.Bogus+Worse+YouCtrl", &["valid:Bogus", "valid:Worse"])]
#[case::unknown_base_and_property("Bogus.Worse", &["valid-base:Bogus", "valid:Worse"])]
#[case::across_alternatives("Creature.Bogus,Land.Worse", &["valid:Bogus", "valid:Worse"])]
#[case::negated_unknown("Creature.!Bogus+Worse", &["valid:Bogus", "valid:Worse"])]
#[case::once("Creature.Bogus+Bogus", &["valid:Bogus"])]
fn every_unknown_word_is_named(#[case] valid: &str, #[case] expected: &[&str]) {
    assert_eq!(gaps(&counting(valid)), vec![expected.join("; ")]);
}

#[rstest]
#[case::control("Creature.YouCtrl+Other", json!([{"Type": "Creature"}, {"ControlledBy": "You"}, "Other"]))]
#[case::negation("Creature.!token", json!([{"Type": "Creature"}, {"Not": "Token"}]))]
fn known_words_read_as_before(#[case] valid: &str, #[case] expected: Value) {
    let found = abilities(&counting(valid));
    let amount = found[0]["Spell"]["effects"][0]["DealDamage"]["amount"].clone();
    assert_eq!(amount, json!({ "Count": expected }));
}
