//! Goal: round 6 builder f translates the shield that deals the next N damage to another player or permanent instead to a fixed redirect of that amount (to the target, the card itself or you), and leaves a split of a shared or divided shield, an imprinted recipient, a recipient that is also the covered one and an amount that is no number gaps; protection that does not remove its own Aura is a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// A creature with an `Effect` shield ability: the activated line's keys, then the replacement and its part.
fn shield_card(line: &str, replacement: &str, part: &str) -> String {
    format!(
        "Name:K\nTypes:Creature Kor\nPT:1/1\nA:AB$ Effect | Cost$ 0 | {line} | ReplacementEffects$ R | SpellDescription$ x\nSVar:R:Event$ DamageDone | {replacement} | ReplaceWith$ P | Description$ x\nSVar:P:DB$ ReplaceSplitDamage | {part}\nSVar:Three:Number$3\n"
    )
}

/// The `by` of the shield the card's first ability makes.
fn by(text: &str) -> Value {
    abilities(text)[0]["Activated"]["effects"][0]["Shield"]["by"].clone()
}

const TARGET_MINE: &str = "ValidTgts$ Creature.YouCtrl | Duration$ UntilHostLeavesPlayOrEOT | RememberObjects$ Targeted | ExileOnMoved$ Battlefield";
const TARGET_WHITE: &str = "ValidTgts$ Creature.White | Duration$ UntilHostLeavesPlayOrEOT | RememberObjects$ Targeted | ExileOnMoved$ Battlefield";

#[rstest]
#[case::to_the_target(
    TARGET_MINE,
    "ValidTarget$ Creature.EffectSource | DamageTarget$ Remembered",
    "DamageTarget$ Remembered",
    json!({"RedirectUpTo": {"most": 1, "to": {"Target": 0}}}),
)]
#[case::to_the_card(
    TARGET_WHITE,
    "ValidTarget$ Creature.IsRemembered | DamageTarget$ EffectSource",
    "DamageTarget$ EffectSource",
    json!({"RedirectUpTo": {"most": 1, "to": "This"}}),
)]
#[case::to_you(
    TARGET_WHITE,
    "ValidTarget$ Creature.IsRemembered | DamageTarget$ You",
    "DamageTarget$ You",
    json!({"RedirectUpTo": {"most": 1, "to": "You"}}),
)]
#[case::a_named_amount(
    TARGET_MINE,
    "ValidTarget$ Creature.EffectSource | DamageTarget$ Remembered",
    "DamageTarget$ Remembered | VarName$ Three",
    json!({"RedirectUpTo": {"most": 3, "to": {"Target": 0}}}),
)]
fn the_next_damage_is_redirected_to_a_fixed_recipient(
    #[case] line: &str,
    #[case] replacement: &str,
    #[case] part: &str,
    #[case] expected: Value,
) {
    let text = shield_card(line, replacement, part);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(by(&text), expected);
}

#[rstest]
#[case::a_divided_shield(
    TARGET_MINE,
    "ValidTarget$ Creature.EffectSource | DamageTarget$ Remembered",
    "DamageTarget$ Remembered | VarName$ Three | DivideShield$ True",
    "key:Replacement.DamageDone.ReplaceSplitDamage.DivideShield"
)]
#[case::an_imprinted_recipient(
    TARGET_MINE,
    "ValidTarget$ Creature.EffectSource | DamageTarget$ Imprinted",
    "DamageTarget$ Imprinted",
    "replacement:DamageDone-redirect-target"
)]
#[case::the_remembered_is_the_covered_one(
    TARGET_WHITE,
    "ValidTarget$ Creature.IsRemembered | DamageTarget$ Remembered",
    "DamageTarget$ Remembered",
    "replacement:DamageDone-redirect-target"
)]
#[case::an_amount_that_is_no_number(
    TARGET_MINE,
    "ValidTarget$ Creature.EffectSource | DamageTarget$ Remembered",
    "DamageTarget$ Remembered | VarName$ X",
    "replacement:DamageDone-redirect-amount"
)]
fn a_split_the_shield_cannot_fix_is_a_gap(
    #[case] line: &str,
    #[case] replacement: &str,
    #[case] part: &str,
    #[case] gap: &str,
) {
    assert!(
        gaps(&shield_card(line, replacement, part)).contains(&gap.to_owned()),
        "{:?}",
        gaps(&shield_card(line, replacement, part))
    );
}

/// An Aura that gives the creature it enchants protection, with Forge's exception for the Aura itself or without.
fn ward(protection: &str) -> String {
    format!(
        "Name:W\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | AddKeyword$ {protection} | Description$ x\n"
    )
}

#[rstest]
#[case::the_aura_excepted("Protection:Card.White:white:Card.CardUID_HostCardUID", true)]
#[case::auras_excepted("Protection:Card.nonColorless:each color:Aura", true)]
#[case::no_exception("Protection:Card.White:white", false)]
fn protection_that_does_not_remove_its_aura_is_a_gap(#[case] protection: &str, #[case] gap: bool) {
    let found = gaps(&ward(protection));
    assert_eq!(
        found.contains(&"keyword:Protection-with-an-exception".to_owned()),
        gap,
        "{found:?}"
    );
}
