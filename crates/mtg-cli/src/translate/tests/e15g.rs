//! Goal: round 15 pack e15g reads the Licid line (the permanent loses the ability, becomes an Aura with enchant creature
//! until its controller pays the revert cost) and keyword grants to the spell a cast trigger names, and leaves every
//! other shape of these keys a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const LICID: &str = "A:AB$ Animate | Cost$ W T | Defined$ Self | RemoveThisAbility$ True | Duration$ Permanent | RevertCost$ REVERT | Keywords$ Enchant:Creature | Types$ Enchantment,Aura | RemoveCardTypes$ True | RemoveEnchantmentTypes$ True | SubAbility$ DBAttach\nSVar:DBAttach:DB$ Attach | ValidTgts$ Creature";

#[rstest]
#[case::white("W", "{W}")]
#[case::generic("1", "{1}")]
fn a_licid_lasts_until_its_controller_pays(#[case] revert: &str, #[case] mana: &str) {
    let (json, found) = card("Creature Licid", &LICID.replace("REVERT", revert));
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(&format!(r#""duration":{{"UntilPaid":"{mana}"}}"#)),
        "{json}"
    );
    assert!(json.contains(r#""LoseThisAbility""#), "{json}");
    assert!(json.contains(r#"{"SetTypes":["Enchantment"]}"#), "{json}");
    assert!(
        json.contains(r#"{"Attach":{"to":{"Target":0},"what":"This"}}"#),
        "{json}"
    );
}

#[rstest]
#[case::an_enchantment_already("Enchantment Creature Licid", LICID.replace("REVERT", "W"))]
#[case::for_a_turn("Creature Licid", LICID.replace("REVERT", "W").replace("Permanent", "UntilEndOfTurn"))]
#[case::no_revert_cost("Creature Licid", LICID.replace(" | RevertCost$ REVERT", ""))]
#[case::keeps_its_types("Creature Licid", LICID.replace("REVERT", "W").replace(" | RemoveCardTypes$ True", ""))]
#[case::another_object("Creature Licid", LICID.replace("REVERT", "W").replace("Defined$ Self", "Defined$ Targeted"))]
fn another_licid_shape_is_a_gap(#[case] types: &str, #[case] lines: String) {
    let (_, found) = card(types, &lines);
    assert!(!found.is_empty(), "{lines}");
}

const GRANT: &str = "T:Mode$ SpellCast | ValidCard$ Instant | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigPump\nSVar:TrigPump:DB$ Pump | Defined$ TriggeredCard | PumpZone$ Stack | ";

#[rstest]
#[case::rebound("KW$ Rebound", r#"[{"AddKeyword":"Rebound"}]"#)]
#[case::two(
    "KW$ Deathtouch & Lifelink",
    r#"[{"AddKeyword":"Deathtouch"},{"AddKeyword":"Lifelink"}]"#
)]
#[case::for_good("KW$ Haste | Duration$ Permanent", r#"[{"AddKeyword":"Haste"}]"#)]
fn a_spell_gains_keywords_for_good(#[case] tail: &str, #[case] mods: &str) {
    let (json, found) = card("Creature Monk", &format!("{GRANT}{tail}"));
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(&format!(
            r#"{{"Continuous":{{"affected":"Triggering","duration":"Forever","mods":{mods}}}}}"#
        )),
        "{json}"
    );
}

#[rstest]
#[case::stats("NumAtt$ +1 | KW$ Haste")]
#[case::a_turn("KW$ Haste | Duration$ UntilYourNextTurn")]
fn another_spell_grant_is_a_gap(#[case] tail: &str) {
    let (_, found) = card("Creature Monk", &format!("{GRANT}{tail}"));
    assert!(!found.is_empty(), "{tail}");
}
