//! Goal: the audit fixes of round 8 q1 read the wording of a ban on activation as written: a plain ban includes mana abilities, a qualified one keeps its qualifier or stays a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

fn aura(statics: &str) -> String {
    format!("Name:Subject\nTypes:Enchantment Aura\nK:Enchant:Creature\n{statics}\n")
}

#[rstest]
#[case::plain_ban(
    "Mode$ CantAttack,CantBlock,CantBeActivated | ValidCard$ Creature.EnchantedBy",
    "Every"
)]
fn a_plain_ban_on_activation_names_every_ability(#[case] line: &str, #[case] kind: &str) {
    let text = aura(&format!("S:{line}"));
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(json.contains(&format!(r#""kind":"{kind}""#)), "{json}");
}

#[test]
fn a_ban_that_spares_mana_abilities_keeps_its_words() {
    let text = aura(
        "S:Mode$ CantAttack,CantBlock,CantBeActivated | ValidCard$ Creature.EnchantedBy | ValidSA$ Activated.!ManaAbility",
    );
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(!json.contains("ActivateKind"), "{json}");
    assert!(json.contains(r#""Activate""#), "{json}");
}

#[test]
fn a_ban_on_abilities_with_a_tap_cost_is_not_widened_to_all() {
    let text = aura(
        "S:Mode$ CantAttack,CantBlock,CantBeActivated | ValidCard$ Creature.EnchantedBy | ValidSA$ Activated.hasTapCost",
    );
    assert_eq!(gaps(&text), ["key:Static.CantBeActivated.ValidSA"]);
}

#[test]
fn a_target_of_a_delayed_trigger_body_is_chosen_when_it_triggers() {
    let text = "Name:Subject\nTypes:Instant\nA:SP$ DelayedTrigger | ValidTgts$ Creature | Mode$ ChangesZone | RememberObjects$ Targeted | ValidCard$ Card.IsTriggerRemembered | Origin$ Battlefield | Destination$ Graveyard | ThisTurn$ True | Execute$ Land | SpellDescription$ x\nSVar:Land:DB$ Earthbend | Num$ 4\n";
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    let json = serde_json::to_string(&abilities(text)).unwrap();
    // The spell announces the creature alone; the land is a target of the reflexive ability.
    assert_eq!(json.matches(r#""targets":["#).count(), 2, "{json}");
    assert!(json.contains(r#""Reflexive""#), "{json}");
}
