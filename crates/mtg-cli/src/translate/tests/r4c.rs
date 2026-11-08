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

#[rstest]
#[case::enchanted("Creature.AttachedBy", "Cost$ 3", r#""mana":"{3}""#)]
#[case::itself_per_counter(
    "Card.Self",
    "Cost$ Y\nSVar:Y:Count$CardCounters.P1P1",
    r#""attackers":[{"Is":"This"}]"#
)]
fn a_creature_that_pays_to_attack_is_a_tax_on_itself(
    #[case] valid: &str,
    #[case] cost: &str,
    #[case] part: &str,
) {
    let script = card(&format!(
        "S:Mode$ CantAttackUnless | ValidCard$ {valid} | {cost}"
    ));
    let found = abilities(&script);
    assert!(!found.is_empty(), "{:?}", gaps(&script));
    let text = found[0].to_string();
    assert!(text.contains(r#""AttackTax""#), "{text}");
    assert!(text.contains(part), "{text}");
}

#[rstest]
#[case::sacrifice("Cost$ Sac<1/Land>", "cost:Static.CantAttackUnless.Sac<1/Land>")]
#[case::an_unknown_word("Cost$ Z", "cost:Static.CantAttackUnless.Z")]
fn a_cost_it_cannot_charge_is_a_gap(#[case] cost: &str, #[case] gap: &str) {
    let script = card(&format!(
        "S:Mode$ CantAttackUnless | ValidCard$ Card.Self | {cost}"
    ));
    assert!(
        gaps(&script).contains(&gap.to_owned()),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::next_end_step("UntilYourNextEndStep", Some(r#""duration":"UntilYourNextEndStep""#))]
#[case::next_turn("UntilYourNextTurn", Some(r#""duration":"UntilYourNextTurn""#))]
#[case::unknown("UntilSomeday", None)]
fn an_exiled_card_is_played_for_the_duration_the_line_names(
    #[case] duration: &str,
    #[case] found: Option<&str>,
) {
    let script = card(&format!(
        "A:SP$ Dig | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | StaticAbilities$ STPlay | RememberObjects$ Remembered | Duration$ {duration} | SubAbility$ DBCleanup | ForgetOnMoved$ Exile\nSVar:STPlay:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True"
    ));
    let text = abilities(&script)
        .first()
        .map(ToString::to_string)
        .unwrap_or_default();
    match found {
        Some(word) => assert!(text.contains(word), "{text} {:?}", gaps(&script)),
        None => assert!(gaps(&script).contains(&format!("duration:{duration}"))),
    }
}

#[rstest]
#[case::more("You", "2", r#"{"HandSizeChange":{"by":2,"who":"You"}}"#)]
#[case::fewer("Opponent", "-3", r#"{"HandSizeChange":{"by":-3,"who":"Opponents"}}"#)]
fn a_changed_maximum_hand_size_is_a_rule(
    #[case] affected: &str,
    #[case] by: &str,
    #[case] rule: &str,
) {
    let script = card(&format!(
        "S:Mode$ Continuous | Affected$ {affected} | RaiseMaxHandSize$ {by}"
    ));
    let found = abilities(&script);
    assert!(!found.is_empty(), "{:?}", gaps(&script));
    assert_eq!(found[0]["Rule"]["rule"].to_string(), rule);
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
