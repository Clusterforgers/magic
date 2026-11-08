//! Goal: "if you revealed a Dragon card or controlled a Dragon as you cast this spell" reads back as the reveal paid or a
//! Dragon held as the spell was cast; another subtype, another first part or the reveal alone never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "When this creature enters, if `condition`, draw a card."
fn enters_if(condition: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"if":{condition},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

/// "you revealed a `kind` card or controlled a `kind` as you cast this spell", with `paid` as the first part.
fn presence(paid: &str, kind: &str) -> String {
    format!(
        r#"{{"Any":[{paid},{{"AsCast":{{"Compare":{{"left":{{"Count":[{{"Subtype":"{kind}"}},{{"ControlledBy":"You"}}]}},"op":"Ge","right":1}}}}}}]}}"#
    )
}

const DRAGON_CLAUSE: &str = "When this creature enters, if you revealed a Dragon card or controlled a Dragon as you cast this spell, draw a card.";

#[test]
fn a_revealed_or_controlled_dragon_reads_back() {
    let json = enters_if(&presence(r#""AdditionalCostPaid""#, "Dragon"));
    assert!(written(&json, DRAGON_CLAUSE));
}

#[rstest]
#[case::another_subtype(presence(r#""AdditionalCostPaid""#, "Goblin"))]
#[case::another_first_part(presence(r#""Kicked""#, "Dragon"))]
#[case::the_reveal_alone(r#""AdditionalCostPaid""#.to_owned())]
fn another_presence_differs(#[case] condition: String) {
    assert!(!written(&enters_if(&condition), DRAGON_CLAUSE));
}
