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
