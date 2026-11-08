//! Goal: round 14 pack e14a reads the mana restrictions that name a fact of the spell (colorless, cast from exile or
//! from anywhere but the hand, devoid, flashback from a graveyard, outlaw) into spend facts, and leaves the ones the
//! engine has no fact for a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The uses of the mana of a land that taps for one mana of any colour with the restriction.
fn uses(restriction: &str) -> (String, Vec<String>) {
    let text = format!(
        "Name:Subject\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ Any | RestrictValid$ {restriction}\n"
    );
    let json = serde_json::to_value(abilities(&text)).unwrap();
    let only = json[0]["Activated"]["effects"][0]["AddMana"]["only"].clone();
    (serde_json::to_string(&only).unwrap(), gaps(&text))
}

#[rstest]
#[case::colorless("Spell.Colorless", r#"[{"Spell":{"facts":["Colorless"]}}]"#)]
#[case::colorless_or_an_ability(
    "Spell.Colorless,Activated",
    r#"[{"Spell":{"facts":["Colorless"]}},{"Ability":{}}]"#
)]
#[case::from_exile("Spell.wasCastFromExile", r#"[{"Spell":{"facts":["FromExile"]}}]"#)]
#[case::not_from_the_hand(
    "Spell.!wasCastFromYourHand",
    r#"[{"Spell":{"facts":["NotFromHand"]}}]"#
)]
#[case::devoid("Spell.withDevoid", r#"[{"Spell":{"facts":["Devoid"]}}]"#)]
#[case::flashback_from_a_graveyard(
    "Spell.wasCastFromGraveyard+withFlashback",
    r#"[{"Spell":{"facts":["FromGraveyard","Flashback"]}}]"#
)]
#[case::outlaw(
    "Spell.Outlaw,Activated.Outlaw",
    r#"[{"Spell":{"subtypes":["Assassin","Mercenary","Pirate","Rogue","Warlock"]}},{"Ability":{"subtypes":["Assassin","Mercenary","Pirate","Rogue","Warlock"]}}]"#
)]
#[case::cant_cast_from_the_hand(
    "CantCastSpellFromHand",
    r#"[{"Spell":{"facts":["NotFromHand"]}},{"Ability":{}},"OtherCosts"]"#
)]
fn a_restriction_on_a_fact_of_the_spell_is_a_spend_fact(
    #[case] restriction: &str,
    #[case] expected: &str,
) {
    let (only, found) = uses(restriction);
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(only, expected);
}

#[rstest]
#[case::kicked("Spell.Kicked")]
#[case::exactly_three_colors("Spell.numColorsEQ3")]
#[case::an_outlaw_creature("Spell.Outlaw+Creature")]
#[case::exile_for_an_ability("Activated.wasCastFromExile")]
#[case::equip_abilities("Spell.Equipment,Activated.Equip")]
fn a_restriction_without_an_engine_fact_stays_a_gap(#[case] restriction: &str) {
    let (_, found) = uses(restriction);
    assert_eq!(
        found,
        [format!("key:Mana.RestrictValid={restriction}")],
        "{restriction}"
    );
}

#[rstest]
#[case::two("| Amount$ 2", true)]
#[case::two_in_the_description("| SpellDescription$ Add two mana of different colors.", true)]
#[case::no_amount_said("", false)]
#[case::three("| Amount$ 3", false)]
fn two_mana_of_different_colors_is_the_ten_pairs(#[case] tail: &str, #[case] read: bool) {
    let text = format!(
        "Name:Subject\nTypes:Artifact\nA:AB$ Mana | Cost$ T | Produced$ Combo AnyDifferent {tail}\n"
    );
    let found = gaps(&text);
    if read {
        assert!(found.is_empty(), "{found:?}");
        let json = serde_json::to_value(abilities(&text)).unwrap();
        let mana = json[0]["Activated"]["effects"][0]["AddMana"]["mana"].clone();
        let expected =
            serde_json::to_value(mtg_cards::effect::Produce::two_different_colors()).unwrap();
        assert_eq!(mana, expected);
    } else {
        assert_eq!(found, ["key:Mana.Produced=AnyDifferent"]);
    }
}
