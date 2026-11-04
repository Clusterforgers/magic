//! Goal: round 4 audit faults of the translator: a per-player "up to one" keeps its bounds, a total reads every object
//! the slot holds, and a value the table does not know stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

const DREADNOUGHT: &str = "Name:Phyrexian Dreadnought\nManaCost:1\nTypes:Artifact Creature Phyrexian Dreadnought\nPT:12/12\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ ChooseCreatures | TriggerDescription$ x\nSVar:ChooseCreatures:DB$ ChooseCard | Defined$ You | Amount$ X | MinAmount$ 0 | Choices$ Creature.YouCtrl | ChoiceZone$ Battlefield | Reveal$ True | RememberChosen$ True | SubAbility$ SacrificeSelf\nSVar:SacrificeSelf:DB$ Sacrifice | ConditionCheckSVar$ TotalPower | ConditionSVarCompare$ LT12 | SubAbility$ SacrificeCreatures\nSVar:SacrificeCreatures:DB$ SacrificeAll | Defined$ Remembered | ConditionCheckSVar$ TotalPower | ConditionSVarCompare$ GE12 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:TotalPower:Remembered$CardPower\nSVar:X:Count$Valid Creature.YouCtrl\n";

#[test]
fn the_total_power_of_a_chosen_group_is_a_total_not_the_first_object() {
    assert_eq!(gaps(DREADNOUGHT), Vec::<String>::new());
    let text = abilities(DREADNOUGHT)[0].to_string();
    assert!(text.contains(r#""Total":{"of":[{"Is":{"Bound":"#), "{text}");
    assert!(!text.contains(r#""Stat""#), "{text}");
}

#[rstest]
#[case::up_to_one_each("TargetMin$ 0 | TargetMax$ OneEach")]
fn a_per_player_target_is_up_to_one_each(#[case] bounds: &str) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Artifact | {bounds} | TargetsForEachPlayer$ True\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    assert_eq!(
        found[0]["Spell"]["targets"][0]["bounds"],
        json!({"min": 0, "max": 1})
    );
}

#[rstest]
#[case::attacked_target(
    "TokenAttacking$ TriggeredAttackedTarget",
    "key:TokenAttacking=TriggeredAttackedTarget"
)]
#[case::defender(
    "TokenAttacking$ TriggeredDefender",
    "key:TokenAttacking=TriggeredDefender"
)]
#[case::remembered("TokenAttacking$ Remembered", "key:TokenAttacking=Remembered")]
#[case::tapped_word("TokenTapped$ Yes", "key:TokenTapped=Yes")]
fn a_token_flag_the_table_does_not_know_stays_a_gap(#[case] keys: &str, #[case] gap: &str) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Token | TokenAmount$ 1 | TokenScript$ w_1_1_soldier | {keys}\n"
    );
    assert!(gaps(&text).iter().any(|g| g == gap), "{:?}", gaps(&text));
}
