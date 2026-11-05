//! Goal: a Role token attached to a creature translates to a create and an attach; the one-Role-per-controller rule is the engine's state-based action.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::json;

use super::r3p5::token_card;

#[rstest]
#[case::target("Targeted", "Creature.YouCtrl+Other", json!({"Target": 0}))]
fn a_role_token_is_created_and_attached(
    #[case] to: &str,
    #[case] valid: &str,
    #[case] host: serde_json::Value,
) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Token | TokenScript$ role_test | TokenOwner$ You | AttachedTo${to} | ValidTgts$ {valid}\n"
    );
    let card = token_card(&text);
    assert_eq!(card.gaps().count(), 0);
    let found = card.faces.into_iter().next().unwrap().abilities;
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[1]["Attach"]["to"], host);
    assert_eq!(
        effects[1]["Attach"]["what"],
        json!({"Bound": effects[0]["CreateToken"]["as"].clone()})
    );
}
