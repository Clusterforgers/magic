//! Goal: a number a step remembers (counters removed, the mana value of a countered spell) is stored on the source before the removal or after the counter and read back by `Count$RememberedNumber` and `Count$RememberedSize`; shapes with an amount the engine cannot know stay named gaps.
use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

/// The JSON of the first ability, as text to search.
fn text(script: &str) -> String {
    let found = abilities(script);
    assert!(!found.is_empty(), "{:?}", gaps(script));
    Value::Array(found).to_string()
}

#[rstest]
#[case::countered_value(
    "Name:T\nTypes:Instant\nA:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | RememberCounteredCMC$ True | SubAbility$ M\nSVar:M:DB$ Mill | NumCards$ X | Defined$ TargetedController | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Count$RememberedNumber\n",
    &[r#"{"Counter":{"what":{"Target":0}}}"#, r#""stat":"ManaValue""#, r#"{"Remembered":{"on":"This","slot":"remembered_number"}}"#]
)]
#[case::removed_all(
    "Name:T\nTypes:Creature Bear\nT:Mode$ Phase | Phase$ Upkeep | Execute$ R | TriggerZones$ Battlefield\nSVar:R:DB$ RemoveCounter | CounterType$ STUDY | CounterNum$ All | RememberRemoved$ True | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Count$RememberedSize\n",
    &[r#""RememberValue""#, r#""value":{"Counters":{"counter""#,r#"{"Remembered":{"on":"This","slot":"remembered_number"}}"#]
)]
#[case::removed_one_if_there_is_one(
    "Name:T\nTypes:Creature Bear\nT:Mode$ Phase | Phase$ Upkeep | Execute$ R | TriggerZones$ Battlefield\nSVar:R:DB$ RemoveCounter | CounterType$ P1P1 | CounterNum$ 1 | RememberRemoved$ True | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Count$RememberedSize\n",
    &[r#""When""#, r#""op":"Ge""#, r#""RememberValue""#]
)]
fn a_remembered_number_is_stored_and_read(#[case] script: &str, #[case] wants: &[&str]) {
    let found = text(script);
    for want in wants {
        assert!(found.contains(want), "{want} missing in {found}");
    }
}

#[test]
fn a_player_one_line_remembers_is_read_by_another_line() {
    let script = "Name:T\nTypes:Creature Bear\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Rem\nSVar:Rem:DB$ Remember | Defined$ You\nA:AB$ Draw | Cost$ T | Defined$ Remembered | NumCards$ 1\n";
    let found = text(script);
    assert!(
        found.contains(r#""who":{"Remembered":{"on":"This","slot":"remembered"}}"#),
        "{found}"
    );
    assert_eq!(gaps(script), Vec::<String>::new());
}

#[rstest]
#[case::up_to(
    "Name:T\nTypes:Creature Bear\nT:Mode$ Phase | Phase$ Upkeep | Execute$ R | TriggerZones$ Battlefield\nSVar:R:DB$ RemoveCounter | CounterType$ P1P1 | CounterNum$ 3 | UpTo$ True | RememberRemoved$ True\n"
)]
fn an_amount_the_engine_cannot_know_stays_a_gap(#[case] script: &str) {
    assert!(
        gaps(script)
            .iter()
            .any(|gap| gap.contains("RememberRemoved") || gap.contains("RemoveCounter")),
        "{:?}",
        gaps(script)
    );
}
