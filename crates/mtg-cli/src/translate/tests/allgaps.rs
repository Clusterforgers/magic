//! Goal: the gaps behind the first gap of a line are found by dropping parameters, while the translation of the card and its first gaps stay as they were.
use rstest::rstest;

use super::{gaps, translate, world};
use crate::forge::script::Script;
use crate::translate::allgaps::others;

fn behind(text: &str) -> Vec<String> {
    others(&world(), &Script::parse(text)).into_iter().collect()
}

#[rstest]
#[case::two_unread_counter_keys(
    "Name:T\nTypes:Sorcery\nA:SP$ PutCounter | Choices$ Creature.YouCtrl | CounterType$ P1P1 | CounterNum$ 1 | ChoiceAmount$ 1 | MinChoiceAmount$ 1\n",
    "key:PutCounter.MinChoiceAmount"
)]
#[case::one_in_the_line_one_in_its_sub_ability(
    "Name:T\nTypes:Sorcery\nA:SP$ PutCounter | Choices$ Creature.YouCtrl | CounterType$ P1P1 | CounterNum$ 1 | ChoiceAmount$ 1 | SubAbility$ B\nSVar:B:DB$ Pump | Defined$ You | KW$ Flying | Duration$ Perpetual\n",
    "duration:Perpetual"
)]
fn a_second_gap_is_listed(#[case] script: &str, #[case] second: &str) {
    let first = gaps(script);
    assert_eq!(first.len(), 1, "{first:?}");
    assert!(
        behind(script).iter().any(|name| name == second),
        "{first:?} {:?}",
        behind(script)
    );
    assert_eq!(translate(script).translated(), 0);
}

#[rstest]
#[case::a_card_that_translates("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 2\n")]
#[case::a_plain_card("Name:T\nTypes:Creature Elf\nPT:1/1\n")]
fn a_card_without_gaps_has_none_behind(#[case] script: &str) {
    assert_eq!(behind(script).len(), 0);
}
