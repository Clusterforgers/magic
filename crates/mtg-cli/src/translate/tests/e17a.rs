//! Goal: round 17 pack e17a reads "until this card is cast from exile" with its permission to cast the exiled card, and
//! a count the Oracle words "as you cast this spell" as the count the spell kept as it was cast; another shape or wording
//! stays a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

const OUTLAW_ORACLE: &str = "{2}, Exile Subject from your hand: Target land gains \"{T}: Add {U}, {B}, or {R}\" until Subject is cast from exile. You may cast Subject for as long as it remains exiled.";

/// A creature with the hand ability of the New Capenna hideaways, with `from` replaced by `to` in its script.
fn outlaw(from: &str, to: &str, oracle: &str) -> String {
    let script = "Name:Subject\nManaCost:3 U B R\nTypes:Creature Vampire Rogue\nPT:4/5\nA:AB$ Effect | Cost$ 2 ExileFromHand<1/CARDNAME> | ActivationZone$ Hand | ValidTgts$ Land | RememberObjects$ Targeted & Self | StaticAbilities$ Land,MayPlay | Triggers$ Cast | ImprintCards$ Self | Duration$ Permanent | ForgetOnMoved$ Exile | SpellDescription$ Target land gains it.\nSVar:Land:Mode$ Continuous | AffectedZone$ Battlefield | Affected$ Card.IsRemembered+!IsImprinted | AddAbility$ Mana | Description$ Target land gains it.\nSVar:Mana:AB$ Mana | Cost$ T | Produced$ Combo U B R | Amount$ 1 | SpellDescription$ Add {U}, {B}, or {R}.\nSVar:MayPlay:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsImprinted+IsRemembered | AffectedZone$ Exile | Secondary$ True | Description$ You may cast it.\nSVar:Cast:Mode$ SpellCast | ValidCard$ Card.IsImprinted+IsRemembered+wasCastFromExile | Execute$ ExileSelf | Static$ True\nSVar:ExileSelf:DB$ ChangeZone | Origin$ Command | Destination$ Exile | Defined$ Self\n";
    format!("{}Oracle:{oracle}\n", script.replace(from, to))
}

#[test]
fn a_land_gains_the_ability_until_the_card_is_cast_from_exile() {
    let text = outlaw("", "", OUTLAW_ORACLE);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = serde_json::to_string(&abilities(&text)).unwrap();
    for part in [
        r#""duration":"UntilCastFromExile""#,
        r#""affected":{"Target":0}"#,
        r#"{"MayCast":{"from":"Exile","only":{"Arrived":{"of":"This","zone":"Exile"}}}}"#,
        r#"[{"Mana":"{2}"},"ExileSelf"]"#,
    ] {
        assert!(found.contains(part), "{part}: {found}");
    }
}

#[rstest]
#[case::cast_from_anywhere("+wasCastFromExile", "")]
#[case::played_from_another_zone("AffectedZone$ Exile", "AffectedZone$ Graveyard")]
#[case::lasting_a_turn("Duration$ Permanent", "Duration$ UntilEndOfTurn")]
#[case::from_the_battlefield("ActivationZone$ Hand | ", "")]
fn another_shape_is_a_gap(#[case] from: &str, #[case] to: &str) {
    let text = outlaw(from, to, OUTLAW_ORACLE);
    assert!(!gaps(&text).is_empty(), "{from}");
}

#[test]
fn another_wording_is_a_gap() {
    let oracle = OUTLAW_ORACLE.replace(" for as long as it remains exiled", " this turn");
    assert!(!gaps(&outlaw("", "", &oracle)).is_empty());
}

/// An instant that deals 4 damage when a creature was yours as it was cast, else 2, with the Oracle text `oracle`.
fn steer(oracle: &str) -> String {
    format!(
        "Name:Subject\nManaCost:W\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ X | SpellDescription$ Deal.\nSVar:X:Count$Compare Y GE1.4.2\nSVar:Y:Count$LastStateBattlefieldWithFallback Creature.YouCtrl\nOracle:{oracle}\n"
    )
}

#[test]
fn a_count_as_you_cast_this_spell_is_the_count_the_spell_kept() {
    let text = steer(
        "Subject deals 2 damage to target creature. Subject deals 4 damage to that creature instead if you controlled a creature as you cast this spell.",
    );
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(
        found.contains(
            r#"{"CountedAsCast":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#
        ),
        "{found}"
    );
}

#[test]
fn a_count_the_oracle_does_not_word_as_cast_is_a_gap() {
    let text = steer("Subject deals 2 damage to target creature, or 4 if you control a creature.");
    assert!(
        gaps(&text)
            .iter()
            .any(|gap| gap.contains("LastStateBattlefieldWithFallback")),
        "{:?}",
        gaps(&text)
    );
}
