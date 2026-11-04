//! Goal: piece P3 (memory): an unread `Defined$` word stays a gap named after the word.
use super::gaps;

#[test]
fn unknown_defined_word_is_a_gap() {
    let found = gaps("Name:T\nTypes:Sorcery\nA:SP$ Pump | Defined$ Nonsense | NumAtt$ +1\n");
    assert_eq!(found, ["defined:Nonsense"]);
}
