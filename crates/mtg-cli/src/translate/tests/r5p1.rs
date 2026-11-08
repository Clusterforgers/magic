//! Goal: the keys that remember what a step affected (`RememberLKI`, `RememberDamaged`) bind it for the lines after it, and stay gaps where more than one object is named.
use rstest::rstest;

use super::gaps;

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Instant\nA:SP$ {line}\nSVar:D:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Creature.YouCtrl\n"
    )
}

#[rstest]
#[case::destroy("Destroy | ValidTgts$ Creature | RememberLKI$ True | SubAbility$ D")]
#[case::damage(
    "DealDamage | ValidTgts$ Creature | NumDmg$ 2 | RememberDamaged$ True | SubAbility$ D"
)]
#[case::damage_defined(
    "DealDamage | Defined$ Self | NumDmg$ 2 | RememberDamaged$ True | SubAbility$ D"
)]
fn a_remembering_step_is_read(#[case] line: &str) {
    assert_eq!(gaps(&spell(line)), Vec::<String>::new());
}

#[rstest]
#[case::divided(
    "DealDamage | ValidTgts$ Any | NumDmg$ 3 | DividedAsYouChoose$ 3 | RememberDamaged$ True | SubAbility$ D",
    "key:DealDamage.RememberDamaged-with-several-objects"
)]
#[case::up_to(
    "DealDamage | ValidTgts$ Creature | TargetMax$ 2 | NumDmg$ 1 | RememberDamaged$ True | SubAbility$ D",
    "key:DealDamage.RememberDamaged-with-several-objects"
)]
fn several_objects_stay_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}
