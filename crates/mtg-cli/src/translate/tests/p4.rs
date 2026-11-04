//! Goal: piece P4 (ability header): an unread trigger mode stays a gap named after the mode.
use super::gaps;

#[test]
fn unknown_trigger_mode_is_a_gap() {
    let found = gaps(
        "Name:T\nTypes:Creature Elf\nT:Mode$ Nonsense | ValidCard$ Card.Self | Execute$ Go\nSVar:Go:DB$ Draw\n",
    );
    assert_eq!(found, ["trigger:Nonsense"]);
}
