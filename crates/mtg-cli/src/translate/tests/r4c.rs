//! Goal: a self-only `CantBeCast` line is a casting restriction that holds exactly when the line's check fails, and a line it cannot state stays a named gap.
use rstest::rstest;

use super::{abilities, gaps};

fn card(lines: &str) -> String {
    format!("Name:T\nTypes:Creature\n{lines}\n")
}

#[rstest]
#[case::another_spell(
    "S:Mode$ CantBeCast | ValidCard$ Card.Self | EffectZone$ All | CheckSVar$ X | SVarCompare$ LE0\nSVar:X:Count$ThisTurnCast_Card.YouCtrl",
    r#"{"CastOnly":{"condition":{"Not":{"Compare""#
)]
#[case::snow_land(
    "S:Mode$ CantBeCast | ValidCard$ Card.Self | EffectZone$ All | IsPresent$ Snow.Land+YouCtrl | PresentCompare$ EQ0",
    r#"{"CastOnly":{"condition":{"Not":"#
)]
fn cast_only_if_is_the_negated_check(#[case] lines: &str, #[case] start: &str) {
    let script = card(lines);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let text = found[0].to_string();
    assert!(text.starts_with(start), "{text}");
}

#[rstest]
#[case::no_check(
    "S:Mode$ CantBeCast | ValidCard$ Card.Self | EffectZone$ All",
    "key:Static.CantBeCast.EffectZone"
)]
#[case::origin(
    "S:Mode$ CantBeCast | ValidCard$ Card.Self | EffectZone$ All | Origin$ Hand | CheckSVar$ X\nSVar:X:Count$Morbid.1.0",
    "key:Static.CantBeCast.Origin"
)]
fn a_line_it_cannot_state_is_a_gap(#[case] lines: &str, #[case] gap: &str) {
    assert!(
        gaps(&card(lines)).iter().any(|g| g == gap),
        "{:?}",
        gaps(&card(lines))
    );
}

const DIG: &str = "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigExile\nSVar:TrigExile:DB$ Dig | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | RememberObjects$ Remembered | StaticAbilities$ STPlay | SubAbility$ DBCleanup | ForgetOnMoved$ Exile | Duration$ Permanent\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";

#[rstest]
#[case::attacked_with_a_token("Creature.token", r#""filter":[{"Type":"Creature"},"Token"]"#)]
#[case::attacked_with_itself("Creature.EffectSource", r#""filter":[{"Type":"Creature"},"This"]"#)]
fn a_card_played_during_a_turn_you_attacked_is_a_conditional_permission(
    #[case] valid: &str,
    #[case] filter: &str,
) {
    let script = card(&format!(
        "{DIG}SVar:STPlay:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile | CheckSVar$ Z\nSVar:Z:Count$CreaturesAttackedThisTurn {valid}"
    ));
    let found = abilities(&script);
    assert!(!found.is_empty(), "{:?}", gaps(&script));
    let text = found[0].to_string();
    assert!(text.contains(r#""when":{"Compare""#), "{text}");
    assert!(text.contains(r#""event":"Attacked""#), "{text}");
    assert!(text.contains(filter), "{text}");
}

const ALT: &str = "S:Mode$ AlternativeCost | ValidSA$ Spell | ValidCard$ Card.Self | ValidPlayer$ You | Cost$ 0 | EffectZone$ All";

#[rstest]
#[case::both_checks(
    "| CheckSVar$ X | CheckSecondSVar$ Y\nSVar:X:Count$Valid Creature.OppCtrl\nSVar:Y:Count$Valid Creature.YouCtrl",
    2
)]
#[case::one_check("| CheckSVar$ X\nSVar:X:Count$Valid Creature.OppCtrl", 1)]
fn a_second_check_joins_the_first(#[case] tail: &str, #[case] checks: usize) {
    let script = card(&format!("{ALT} {tail}"));
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let text = found[0].to_string();
    assert_eq!(text.matches("\"Compare\"").count(), checks, "{text}");
    assert_eq!(text.contains("\"All\""), checks == 2, "{text}");
}

#[test]
fn a_second_compare_without_its_check_is_a_gap() {
    let script = card(&format!(
        "{ALT} | CheckSVar$ X | SecondSVarCompare$ GE2\nSVar:X:Count$Valid Creature.OppCtrl"
    ));
    assert!(gaps(&script).contains(&"key:Condition.SVarCompareWithoutCheckSVar".to_owned()));
}
