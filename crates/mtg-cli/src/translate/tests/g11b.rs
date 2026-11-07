//! Goal: round 11 pack g11b reads the damage words of Forge scripts: damage that can't be prevented (`NoPrevention$`)
//! keeps the flag on its damage, "prevent the next X damage" counts X as the shield begins (`PreventCounted`), a shield of
//! sources of a chosen colour reads the choice and one for a target player or planeswalker the target, and an unknown
//! value, a redirection limit, a divided prevention and a chosen card with a property stay gaps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// Asserts that the card reads `expected` (and none of `absent`) with no gap, or stays the gap `gap` when one is given.
fn reads(types: &str, lines: &str, expected: &[&str], absent: &[&str], gap: &str) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    let found = gaps(&text);
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    if gap.is_empty() {
        assert!(found.is_empty(), "{found:?}");
        for part in expected {
            assert!(json.contains(part), "{part} in {json}");
        }
        for part in absent {
            assert!(!json.contains(part), "{part} not in {json}");
        }
    } else {
        assert!(found.iter().any(|g| g.contains(gap)), "{found:?} {json}");
    }
}

#[rstest]
#[case::cant_be_prevented(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | NoPrevention$ True",
    &[r#""unpreventable":true"#],
    &[],
    ""
)]
#[case::plain_damage(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4",
    &[r#""amount":4"#],
    &["unpreventable"],
    ""
)]
#[case::false_is_plain(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | NoPrevention$ False",
    &[r#""amount":4"#],
    &["unpreventable"],
    ""
)]
#[case::an_unknown_value(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | NoPrevention$ Combat",
    &[],
    &[],
    "key:NoPrevention=Combat"
)]
#[case::no_redirection_stays_a_gap(
    "Instant",
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | NoPrevention$ True | NoRedirection$ True",
    &[],
    &[],
    "NoRedirection"
)]
#[case::prevent_the_next_x(
    "Instant",
    "A:SP$ PreventDamage | ValidTgts$ Any | Amount$ X\nSVar:X:Count$xPaid",
    &[r#"{"PreventCounted":"X"}"#],
    &["PreventUpTo"],
    ""
)]
#[case::prevent_a_count(
    "Creature",
    "A:AB$ PreventDamage | Cost$ T | ValidTgts$ Creature | Amount$ X\nSVar:X:Count$Valid Creature.YouCtrl",
    &[r#"{"PreventCounted":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#],
    &[],
    ""
)]
#[case::prevent_a_number(
    "Instant",
    "A:SP$ PreventDamage | ValidTgts$ Any | Amount$ 3",
    &[r#"{"PreventUpTo":3}"#],
    &["PreventCounted"],
    ""
)]
#[case::prevent_divided_stays_a_gap(
    "Instant",
    "A:SP$ PreventDamage | ValidTgts$ Any | Amount$ 3 | TargetMin$ 0 | TargetMax$ 3 | DividedAsYouChoose$ 3",
    &[],
    &[],
    "DividedAsYouChoose"
)]
#[case::sources_of_a_chosen_color(
    "Instant",
    "A:SP$ ChooseColor | Defined$ You | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | ReplacementEffects$ RPrevent\nSVar:RPrevent:Event$ DamageDone | Prevent$ True | ActiveZones$ Command | ValidSource$ Card.ChosenColor | Description$ Prevent all damage that sources of the chosen color would deal this turn.",
    &[r#""source":[{"ChosenAs":"#],
    &[],
    ""
)]
#[case::a_chosen_card_source_stays_a_gap(
    "Instant",
    "A:SP$ ChooseCard | Choices$ Creature.withShadow | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | ReplacementEffects$ RPrevent\nSVar:RPrevent:Event$ DamageDone | ValidSource$ Card.ChosenCardStrict+withShadow | ValidTarget$ You | Prevent$ True | ActiveZones$ Command",
    &[],
    &[],
    "shield-source"
)]
#[case::a_target_player_or_planeswalker(
    "Creature",
    "A:AB$ ChooseColor | Cost$ 1 | Defined$ You | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | ValidTgts$ Player,Planeswalker | ReplacementEffects$ RPrevent | RememberObjects$ Targeted\nSVar:RPrevent:Event$ DamageDone | Prevent$ True | ActiveZones$ Command | ValidTarget$ Player.IsRemembered,Planeswalker.IsRemembered | ValidSource$ Card.ChosenColor",
    &[r#""to":{"Target":0}"#, r#""ChosenAs""#],
    &[],
    ""
)]
fn damage_words(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] expected: &[&str],
    #[case] absent: &[&str],
    #[case] gap: &str,
) {
    reads(types, lines, expected, absent, gap);
}
