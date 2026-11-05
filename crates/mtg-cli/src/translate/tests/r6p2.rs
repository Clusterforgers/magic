//! Goal: the snow symbol {S} in a cost reads as a cost, in an ability and in a spell, so a card whose only gap was the symbol has none.
use rstest::rstest;

use super::gaps;

#[rstest]
#[case::ability(
    "Name:T\nTypes:Snow Creature Shade\nPT:1/1\nA:AB$ Pump | Cost$ S | Defined$ Self | NumAtt$ +1 | NumDef$ +1\n"
)]
#[case::two_symbols(
    "Name:T\nTypes:Snow Creature Shade\nPT:1/1\nA:AB$ Pump | Cost$ S S | Defined$ Self | NumAtt$ +2 | NumDef$ +2\n"
)]
#[case::with_generic(
    "Name:T\nTypes:Snow Creature Shade\nPT:1/1\nA:AB$ Pump | Cost$ 1 S | Defined$ Self | NumAtt$ +1 | NumDef$ +1\n"
)]
#[case::spell_cost("Name:T\nManaCost:S S\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 1\n")]
fn a_snow_cost_is_read(#[case] card: &str) {
    assert_eq!(gaps(card), Vec::<String>::new());
}
