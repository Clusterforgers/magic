//! Goal: round 6 builder B: a search for a "permanent card" keeps the permanent types, and a protection that spares its
//! own Aura stays a named gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps, normal};

const PERMANENTS: &str = r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}"#;

#[rstest]
#[case::permanent_of_a_type("Permanent.Goblin", true)]
#[case::any_permanent("Permanent", true)]
#[case::a_creature("Creature.Goblin", false)]
#[case::any_card("Card.Goblin", false)]
fn r6b_a_permanent_card_searched_keeps_the_permanent_types(
    #[case] wanted: &str,
    #[case] permanent: bool,
) {
    let script = format!(
        "Name:T\nTypes:Creature Goblin\nPT:1/1\nA:AB$ ChangeZone | Cost$ 3 T | Origin$ Library | Destination$ Battlefield | ChangeType$ {wanted} | ChangeNum$ 1\n"
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let filter = normal(found[0].clone())["Activated"]["effects"][0]["Search"]["filter"].clone();
    let any: serde_json::Value = serde_json::from_str(PERMANENTS).unwrap_or_default();
    let has = filter.as_array().is_some_and(|parts| parts.contains(&any));
    assert_eq!(has, permanent, "{filter}");
}

#[rstest]
#[case::spares_the_aura("Protection:Card.Black:black:Card.CardUID_HostCardUID", true)]
#[case::plain("Protection:Card.Black:black", false)]
fn r6b_a_protection_that_spares_a_source_is_a_gap(#[case] keyword: &str, #[case] gap: bool) {
    let script = format!(
        "Name:T\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ Continuous | Affected$ Creature.EnchantedBy | AddKeyword$ {keyword}\n"
    );
    let named = gaps(&script)
        .iter()
        .any(|found| found.contains("Protection-with-exception"));
    assert_eq!(named, gap, "{:?}", gaps(&script));
    if !gap {
        assert!(abilities(&script).iter().any(|found| {
            found
                .to_string()
                .contains(&json!({"Protection": [{"Color": "Black"}]}).to_string())
        }));
    }
}
