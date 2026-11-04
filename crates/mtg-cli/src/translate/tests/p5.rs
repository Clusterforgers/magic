//! Goal: piece P5 (verbs): an unread static mode stays a gap named after the mode.
use super::gaps;

#[test]
fn unknown_static_mode_is_a_gap() {
    let found = gaps("Name:T\nTypes:Enchantment\nS:Mode$ Nonsense | Affected$ Creature\n");
    assert_eq!(found, ["static:Nonsense"]);
}
