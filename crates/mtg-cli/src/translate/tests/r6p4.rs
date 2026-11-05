//! Goal: round 6 piece 4 translates a defender-conditioned attack restriction into `Cant`/`Attack` with the condition read for the defending player, and names a gap for a condition it cannot say.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn card(line: &str) -> String {
    format!("Name:T\nTypes:Creature\nPT:2/2\nS:{line}\n")
}

fn attack(unless: &Value) -> Value {
    normal(
        json!({"Rule": {"rule": {"Cant": {"who": "This", "action": {"Attack": {"unless": unless}}}}}}),
    )
}

fn controls(selector: &Value) -> Value {
    json!({"Compare": {"left": {"Count": selector}, "op": "Ge", "right": 1}})
}

#[rstest]
#[case::island("controlsIsland", controls(&json!([{"Subtype": "Island"}, {"ControlledBy": "You"}])))]
#[case::monarch("isMonarch", json!({"Monarch": "You"}))]
fn the_condition_is_read_for_the_defender(#[case] word: &str, #[case] unless: Value) {
    let line = format!("Mode$ CantAttack | ValidCard$ Card.Self | UnlessDefender$ {word}");
    assert_eq!(gaps(&card(&line)), Vec::<String>::new());
    assert_eq!(abilities(&card(&line)), vec![attack(&unless)]);
}

#[rstest]
#[case::negated("!controlsLand.untapped")]
#[case::properties("controlsCreature.untapped+powerGE3")]
fn a_negation_and_properties_translate(#[case] word: &str) {
    let line = format!("Mode$ CantAttack | ValidCard$ Card.Self | UnlessDefender$ {word}");
    assert_eq!(gaps(&card(&line)), Vec::<String>::new());
}

#[rstest]
#[case::poison("IsPoisoned")]
#[case::fewer("hasFewerCreaturesInPlayThanYou")]
#[case::alternatives("controlsEnchantment,Permanent.enchanted")]
#[case::graveyard("HasCardsInGraveyard_Card_GE7")]
fn other_conditions_stay_gaps(#[case] word: &str) {
    let line = format!("Mode$ CantAttack | ValidCard$ Card.Self | UnlessDefender$ {word}");
    assert_eq!(
        gaps(&card(&line)),
        vec![format!("key:Static.CantAttack.UnlessDefender={word}")]
    );
}
