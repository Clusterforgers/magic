//! Goal: the pieces of ep27 translate as the card prints them. A card cast as though it had flash under a condition (`CastWithFlash | ValidCard$ Card.Self | EffectZone$ All | Caster$ You`) is a `MayCast` rule of the card from its hand with the line's condition; a check of the spell (X, its targets, teamwork), another caster or zone stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn spinners(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Faerie\nPT:1/1\nS:Mode$ CastWithFlash | ValidCard$ Card.Self | {line}\n"
    )
}

#[test]
fn ep27_self_flash_under_a_condition() {
    let script = spinners(
        "ValidSA$ Spell | IsPresent$ Creature.YouCtrl | EffectZone$ All | Caster$ You | Description$ You may cast this spell as though it had flash if you control a creature.",
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script),
        vec![json!({"Rule": {
            "rule": {"MayCast": {"what": ["This"], "flash": true}},
            "from": "Hand",
            "condition": {"Compare": {"left": {"Count": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "op": "Ge", "right": 1}},
        }})]
    );
}

#[rstest]
#[case::x_checked("ValidSA$ Spell.XCostLE3 | EffectZone$ All | Caster$ You")]
#[case::targets_checked(
    "ValidSA$ Spell.IsTargeting Valid Card.IsCommander | EffectZone$ All | Caster$ You"
)]
#[case::teamwork("ValidSA$ Spell.Teamwork | EffectZone$ All | Caster$ You")]
#[case::any_caster(
    "ValidSA$ Spell | IsPresent$ Creature.YouCtrl | EffectZone$ All | Caster$ Player"
)]
#[case::from_the_battlefield("ValidSA$ Spell | IsPresent$ Creature.YouCtrl | Caster$ You")]
fn ep27_other_self_flash_stays_a_gap(#[case] line: &str) {
    let found = gaps(&spinners(line));
    assert!(
        found.iter().any(|gap| gap.contains("CastWithFlash")),
        "{line}: {found:?}"
    );
}
