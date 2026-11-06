//! Goal: equip with a once-a-turn limit, a second cost or a reduction reads into the activated ability of equip; shapes outside them stay named gaps.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn equipment(line: &str) -> String {
    format!("Name:Test Blade\nTypes:Artifact Equipment\nManaCost:2\n{line}\n")
}

fn attach(cost: Value, limit: bool) -> Value {
    let mut found = json!({"Activated": {"cost": cost, "timing": "Sorcery",
        "targets": [{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}],
        "effects": [{"Attach": {"what": "This", "to": {"Target": 0}}}]}});
    if limit {
        found["Activated"]["limit"] = json!("OncePerTurn");
    }
    found
}

#[rstest]
#[case::limit_with_mana("K:Equip:0:::ActivationLimit$ 1:Activate only once each turn", json!("{0}"), true)]
#[case::alternative_mana("K:Equip:2:::AlternateCost$ B", json!("{2}"), false)]
fn equip_options(#[case] line: &str, #[case] cost: Value, #[case] limit: bool) {
    let text = equipment(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0], attach(cost, limit));
}

#[test]
fn equip_with_a_second_cost_is_two_abilities() {
    let found = abilities(&equipment("K:Equip:2:::AlternateCost$ B"));
    assert_eq!(
        found,
        vec![attach(json!("{2}"), false), attach(json!("{B}"), false)]
    );
}

#[rstest]
#[case::fixed_reduction("K:Equip:3:::ReduceCost$ 1:less", "keyword:Equip-fixed-reduction")]
#[case::unknown_option("K:Equip:3:::Foo$ 1:less", "key:Equip.Foo")]
#[case::restriction_and_option(
    "K:Equip:3:Creature.Pirate:::ActivationLimit$ 1",
    "keyword:Equip-restriction-with-options"
)]
fn equip_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&equipment(line)), vec![gap.to_owned()]);
}
