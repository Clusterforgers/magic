//! Goal: round 1 pack D reads the leftover parameters of the zone verbs into the rules the engine runs, and keeps what it cannot say a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn sorcery(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line} | SpellDescription$ x\n")
}

/// The effects of the first ability of a sorcery made of the line.
fn effects(line: &str) -> Value {
    let text = sorcery(line);
    assert_eq!(gaps(&text), Vec::<String>::new(), "{line}");
    abilities(&text)[0]["Spell"]["effects"].clone()
}

#[rstest]
#[case::mana_value_limit(
    "Play | Valid$ Card.nonLand+YouOwn | ValidSA$ Spell.cmcLE4 | ValidZone$ Hand | WithoutManaCost$ True | Optional$ True",
    json!([{"In": "Hand"}])
)]
#[case::two_types(
    "Play | Valid$ Card.YouOwn | ValidSA$ Instant.cmcLE3,Sorcery.cmcLE3 | ValidZone$ Hand | WithoutManaCost$ True | Optional$ True",
    json!([{"In": "Hand"}])
)]
fn a_play_limited_by_the_spell_is_a_cast_of_cards_that_match(
    #[case] line: &str,
    #[case] has: Value,
) {
    let found = effects(line).to_string();
    for pred in has.as_array().unwrap() {
        assert!(found.contains(&pred.to_string()), "{found}");
    }
    assert!(
        found.contains("CastFromZone") && !found.contains("\"play\""),
        "{found}"
    );
}

#[rstest]
#[case::activated(
    "Play | Valid$ Card.YouOwn | ValidSA$ Activated | ValidZone$ Hand | Optional$ True"
)]
fn a_play_limited_by_another_kind_stays_a_gap(#[case] line: &str) {
    let text = sorcery(line);
    assert!(gaps(&text).contains(&"key:Play.ValidSA".to_owned()));
}
