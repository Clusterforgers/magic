//! Goal: round 12 pack g12e reads target slots of verbs: a colour change of a target spell or permanent, a copy of up to
//! one target, a cast of a target card and a choice made by the targeted player, and leaves the other shapes a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given type line, script lines and Oracle text, as compact JSON, and its gaps.
fn card(types: &str, lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\nPT:2/2\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::spell_or_permanent_becomes_red(
    "Instant",
    "A:SP$ Animate | Colors$ Red | OverwriteColors$ True | ValidTgts$ Card | TgtZone$ Stack,Battlefield | Duration$ Permanent | SpellDescription$ Target spell or permanent becomes red.",
    "Target spell or permanent becomes red.",
    r#"SetColors":["Red"]"#
)]
#[case::spell_becomes_colorless(
    "Artifact Creature",
    "A:AB$ Animate | Cost$ T | Colors$ Colorless | OverwriteColors$ True | ValidTgts$ Card | TargetType$ Spell | TgtZone$ Stack | Duration$ Permanent | SpellDescription$ Target spell becomes colorless.",
    "{T}: Target spell becomes colorless.",
    r#""Object":["Spell"]"#
)]
#[case::copy_of_up_to_one(
    "Creature",
    "T:Mode$ Phase | Phase$ Main1 | ValidPlayer$ You | Execute$ C | TriggerZones$ Battlefield | TriggerDescription$ At the beginning of your first main phase, CARDNAME becomes a copy of up to one target land until end of turn.\nSVar:C:DB$ Clone | ValidTgts$ Land | TargetMin$ 0 | TargetMax$ 1 | Duration$ UntilEndOfTurn",
    "At the beginning of your first main phase, Subject becomes a copy of up to one target land until end of turn.",
    r#""min":0"#
)]
#[case::target_opponent_chooses(
    "Creature",
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ G | TriggerDescription$ When CARDNAME enters, target opponent may have you draw three cards. If the player doesn't, you gain 3 life.\nSVar:G:DB$ GenericChoice | ValidTgts$ Opponent | Choices$ D,L\nSVar:D:DB$ Draw | Defined$ You | NumCards$ 3 | SpellDescription$ You draw three cards.\nSVar:L:DB$ GainLife | Defined$ You | LifeAmount$ 3 | SpellDescription$ You gain 3 life.",
    "When Subject enters, target opponent may have you draw three cards. If the player doesn't, you gain 3 life.",
    r#""who":{"Target":0}"#
)]
#[case::cast_target_card(
    "Creature",
    "T:Mode$ SpellCast | ValidCard$ Instant | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ P | TriggerDescription$ Whenever you cast an instant spell, you may cast up to one target sorcery card from your graveyard.\nSVar:P:DB$ Play | TgtZone$ Graveyard | ValidTgts$ Sorcery.YouOwn | ValidSA$ Spell | TargetMin$ 0 | TargetMax$ 1 | Optional$ True",
    "Whenever you cast an instant spell, you may cast up to one target sorcery card from your graveyard.",
    r#""CastFromZone":{"what":{"Target":0}}"#
)]
fn target_slots_are_read(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] part: &str,
) {
    let (json, gaps) = card(types, lines, oracle);
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::spell_gains_a_keyword(
    "Instant",
    "A:SP$ Animate | Keywords$ Flying | ValidTgts$ Card | TgtZone$ Stack | SpellDescription$ Target spell gains flying.",
    "Target spell gains flying.",
    "key:Animate.TgtZone=Stack"
)]
#[case::copy_of_up_to_two(
    "Creature",
    "A:AB$ Clone | Cost$ T | ValidTgts$ Creature | TargetMin$ 0 | TargetMax$ 2 | SpellDescription$ CARDNAME becomes a copy of up to two target creatures.",
    "{T}: Subject becomes a copy of up to two target creatures.",
    "key:Clone.TargetMin"
)]
#[case::target_creature_chooses(
    "Sorcery",
    "A:SP$ GenericChoice | ValidTgts$ Creature | Choices$ D,L | SpellDescription$ Target creature's controller chooses.\nSVar:D:DB$ Draw | Defined$ You | NumCards$ 3 | SpellDescription$ You draw three cards.\nSVar:L:DB$ GainLife | Defined$ You | LifeAmount$ 3 | SpellDescription$ You gain 3 life.",
    "Target creature's controller chooses.",
    "key:GenericChoice.ValidTgts-chooser"
)]
fn other_target_slots_stay_gaps(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] oracle: &str,
    #[case] gap: &str,
) {
    let (_, gaps) = card(types, lines, oracle);
    assert!(gaps.iter().any(|found| found == gap), "{gaps:?}");
}
