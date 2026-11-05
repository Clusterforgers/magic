//! Goal: the value words that ask a fact about an object are table rows: each reads to the predicate or amount that means what Forge means, in every filter it appears in, or stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// The selector `spec` translates to as the affected objects of a static ability.
fn selected(spec: &str) -> Value {
    let script = format!(
        "Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ {spec} | AddPower$ 1\n"
    );
    assert_eq!(gaps(&script), Vec::<String>::new(), "{spec}");
    let each = abilities(&script)[0]["Static"]["affected"]["Each"].clone();
    assert!(serde_json::from_value::<mtg_cards::refs::Selector>(each.clone()).is_ok());
    each
}

#[rstest]
#[case::adventure("Creature.AdventureCard+YouOwn", json!([{"Type": "Creature"}, "HasAdventure", {"OwnedBy": "You"}]))]
#[case::type_cycling("Card.YouOwn+withTypeCycling", json!([{"OwnedBy": "You"}, {"HasKeywordNamed": "Typecycling"}]))]
#[case::damaged_by_equipped("Creature.DamagedBy Equipped", json!([{"Type": "Creature"}, {"DamagedBy": "Attached"}]))]
#[case::spell_or_ability("SpellAbility.OppCtrl", json!([
    {"AnyOf": ["Spell", {"Ability": "Activated"}, {"Ability": "Triggered"}]},
    {"ControlledBy": "Opponents"}]))]
fn a_fact_word_reads_to_its_predicate(#[case] spec: &str, #[case] expected: Value) {
    assert_eq!(selected(spec), expected, "{spec}");
}

#[rstest]
#[case::power_even("powerEven", "Eq")]
#[case::power_odd("powerOdd", "Ne")]
#[case::mana_value_even("cmcEven", "Eq")]
#[case::mana_value_odd("cmcOdd", "Ne")]
fn parity_compares_a_stat_with_twice_its_half(#[case] word: &str, #[case] op: &str) {
    let stat = if word.starts_with("power") {
        "Power"
    } else {
        "ManaValue"
    };
    let value = json!({"Stat": {"of": "Candidate", "stat": stat}});
    let doubled = json!({"Times": {"by": 2, "of": {"Half": {"of": value, "up": false}}}});
    let expected =
        json!([{"Type": "Creature"}, {"Test": {"left": value, "op": op, "right": doubled}}]);
    assert_eq!(selected(&format!("Creature.{word}")), expected);
}

/// The first ability of a spell that draws `X` cards where `SVar:X:` is `Count$` and `count`.
fn drawn(count: &str) -> Result<String, Vec<String>> {
    let script = format!("Name:T\nTypes:Instant\nA:SP$ Draw | NumCards$ X\nSVar:X:Count${count}\n");
    let found = abilities(&script);
    if found.is_empty() {
        return Err(gaps(&script));
    }
    Ok(found[0].to_string())
}

#[rstest]
#[case::top_of_library("TopOfLibraryCMC", "\"stat\":\"ManaValue\"")]
#[case::own_main_phase("IfCastInOwnMainPhase.4.2", "\"Main2\"")]
#[case::madness("Madness.3.1", "{\"CastWith\":\"Madness\"}")]
#[case::foretold("Foretold.1.0", "{\"CastWith\":\"Foretold\"}")]
#[case::urza_lands("UrzaLands.3.1", "Urza's Power Plant")]
fn a_count_word_reads_to_its_amount(#[case] count: &str, #[case] part: &str) {
    let found = drawn(count).unwrap_or_else(|gaps| panic!("{count}: {gaps:?}"));
    assert!(found.contains(part), "{count}: {found}");
}

#[rstest]
#[case::madness("Madness.X.2", "amount:Count$Madness-with-X")]
#[case::foretold("Foretold.X.1", "amount:Count$Foretold-with-X")]
fn an_x_paid_in_an_alternative_cost_stays_a_gap(#[case] count: &str, #[case] gap: &str) {
    assert_eq!(drawn(count), Err(vec![gap.to_owned()]));
}
