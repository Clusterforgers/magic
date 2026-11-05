//! Goal: the filter grammar splits a valid-string into alternatives, a base and properties without losing a word, names every unknown word of a string, and keeps the readers' output unchanged.
use rstest::rstest;
use serde_json::{Value, json};

use super::super::filter::{ATOMS, parse};
use super::{abilities, gaps};

#[test]
fn every_row_has_its_own_word_and_names_a_probe_card_that_uses_it() {
    let words: std::collections::BTreeSet<&str> = ATOMS.iter().map(|row| row.word).collect();
    assert_eq!(words.len(), ATOMS.len(), "a word has two rows");
    let checkout = std::path::Path::new(&std::env::var("HOME").unwrap_or_default())
        .join(".cache/mtg-cli/forge");
    let Ok(scripts) = crate::forge::script::read_checkout(&checkout) else {
        return;
    };
    for row in ATOMS {
        let used = scripts.iter().any(|script| {
            script.names().any(|name| name == row.probe) && format!("{script:?}").contains(row.word)
        });
        assert!(used, "{}: probe {} does not use it", row.word, row.probe);
    }
}

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

fn count_of(valid: &str) -> Value {
    let found = abilities(&counting(valid));
    found[0]["Spell"]["effects"][0]["DealDamage"]["amount"].clone()
}

#[rstest]
#[case::you_dont_own("Card.YouDontOwn", json!({"Not": {"OwnedBy": "You"}}))]
#[case::ring_bearer("Creature.IsRingbearer", json!("RingBearer"))]
#[case::suspended("Card.suspended", json!("Suspended"))]
#[case::escaped("Permanent.escaped", json!({"CastWith": "Escape"}))]
#[case::damaged_by("Creature.DamagedBy", json!({"DamagedBy": "This"}))]
#[case::keyword_of_the_object("Spell.hasKeywordFlash", json!({"HasKeyword": "Flash"}))]
#[case::supertype_base("Legendary.YouCtrl", json!({"Supertype": "Legendary"}))]
#[case::snow_base("Snow.YouCtrl", json!({"Supertype": "Snow"}))]
fn a_table_word_reads_as_its_predicate(#[case] valid: &str, #[case] expected: Value) {
    let found = count_of(valid);
    let preds = found["Count"].as_array().cloned().unwrap_or_default();
    assert!(preds.contains(&expected), "{found}");
}

#[rstest]
#[case::red("RedSource", "Red")]
#[case::white("WhiteSource", "White")]
fn a_source_word_is_legal_for_a_source(#[case] word: &str, #[case] color: &str) {
    let text = format!(
        "Name:T\nTypes:Enchantment\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidSource$ Card.{word} | ValidTarget$ You | ReplaceWith$ DBReplace | PreventionEffect$ True | Description$ If a source would deal damage to you, prevent 2 of that damage.\nSVar:DBReplace:DB$ ReplaceDamage | Amount$ 2\n"
    );
    let found = gaps(&text);
    assert!(found.iter().all(|gap| !gap.contains(word)), "{found:?}");
    assert!(
        json!(abilities(&text)).to_string().contains(color),
        "{:?}",
        abilities(&text)
    );
}

#[test]
fn a_source_word_is_refused_for_an_object() {
    assert_eq!(
        gaps(&counting("Creature.RedSource")),
        vec!["valid:RedSource-in-Object".to_owned()]
    );
}

#[test]
fn a_word_is_matched_whole() {
    // `YouDontOwnIt` is not `YouDontOwn`, `suspendedX` is not `suspended`.
    assert_eq!(
        gaps(&counting("Card.YouDontOwnIt+suspendedX")),
        vec!["valid:YouDontOwnIt; valid:suspendedX".to_owned()]
    );
}
