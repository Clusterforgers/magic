//! Goal: round 6 audit fixes translate a name bound inside a condition or a may to the steps that count it or test that it holds something (an unbound name reads as empty) and leave a step that needs the objects themselves a gap.
use rstest::rstest;

use super::{abilities, gaps};

fn spell(body: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {body}\n")
}

#[rstest]
#[case::inside_a_condition(
    "Draw | ConditionPresent$ Creature.YouCtrl | RememberDrawn$ True | SubAbility$ L\nSVar:L:DB$ GainLife | LifeAmount$ 3 | ConditionDefined$ Remembered | ConditionPresent$ Card | ConditionCompare$ EQ0"
)]
#[case::inside_a_may(
    "Draw | Optional$ True | RememberDrawn$ True | SubAbility$ L\nSVar:L:DB$ Discard | Defined$ You | NumCards$ 2 | ConditionDefined$ Remembered | ConditionPresent$ Card | ConditionCompare$ GE1"
)]
#[case::a_count_after_a_may(
    "Discard | Mode$ TgtChoose | NumCards$ 1 | OptionalDecider$ You | RememberDiscarded$ True | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X\nSVar:X:Remembered$Amount"
)]
fn a_name_bound_in_a_branch_is_counted_or_tested_after_it(#[case] line: &str) {
    assert_eq!(gaps(&spell(line)), Vec::<String>::new());
    assert!(!abilities(&spell(line)).is_empty());
}

#[rstest]
#[case::the_objects_themselves_after_a_may(
    "Draw | Optional$ True | RememberDrawn$ True | SubAbility$ L\nSVar:L:DB$ DealDamage | Defined$ Remembered | NumDmg$ 2",
    "defined:Remembered-without-a-change"
)]
#[case::their_total_after_a_condition(
    "Draw | ConditionPresent$ Creature.YouCtrl | RememberDrawn$ True | SubAbility$ L\nSVar:L:DB$ GainLife | LifeAmount$ X\nSVar:X:Remembered$CardManaCost",
    "amount:Remembered-without-a-change"
)]
fn a_step_that_needs_the_objects_of_a_branch_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

const DEAD: &str = "Name:Dead\nTypes:Creature Zombie\nPT:2/2\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ TrigChange | TriggerDescription$ x\nSVar:TrigChange:AB$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.Zombie+Other | Cost$ X\nSVar:X:Targeted$CardManaCost\n";

#[rstest]
#[case::tied_to_a_target(DEAD, &["cost:X-tied-to-a-value"])]
#[case::a_free_x(&DEAD.replace("Targeted$CardManaCost", "Count$xPaid"), &[])]
fn an_x_cost_tied_to_another_value_is_a_gap(#[case] text: &str, #[case] expected: &[&str]) {
    assert_eq!(gaps(text), expected);
}

#[rstest]
#[case::or_generic("Color$ U | Amount$ Y", "(or {1})", &["key:Static.CostModifier.Color-or-generic"])]
#[case::colour_only("Color$ U | Amount$ 1", "", &[])]
fn a_reduction_that_may_take_generic_instead_is_a_gap(
    #[case] keys: &str,
    #[case] words: &str,
    #[case] expected: &[&str],
) {
    let text = format!(
        "Name:E\nTypes:Creature Fish\nPT:1/1\nS:Mode$ ReduceCost | ValidCard$ Instant | Activator$ You | Type$ Spell | {keys}\nSVar:Y:Count$Valid Land.YouCtrl\nOracle:The first spell costs {{U}} {words} less.\n"
    );
    assert_eq!(gaps(&text), expected);
}
