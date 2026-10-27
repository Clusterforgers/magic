//! A loss of life for each opponent with enough poison counters: "Whenever this artifact becomes tapped, each opponent who
//! has three or more poison counters loses 1 life."

use super::twin_variants;
use crate::ability::Ability;

/// The numbers of poison counters a card asks for, as the JSON of the opponent's check and as words.
const COUNTS: [(u32, &str); 4] = [(2, "two"), (3, "three"), (4, "four"), (5, "five")];

/// Adds the wordings of an ability that makes each opponent with some poison counters lose life.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for (number, word) in COUNTS {
        let each = format!(
            r#"{{"RepeatEach":{{"over":"Opponents","as":"opp","effects":[{{"If":{{"condition":{{"Compare":{{"left":{{"Counters":{{"on":{{"Bound":"opp"}},"counter":"poison"}}}},"op":"Ge","right":{number}}}}},"then":[{{"LoseLife":{{"who":{{"Bound":"opp"}},"amount":1}}}}]}}}}]}}}}"#
        );
        let plain = r#"{"LoseLife":{"who":"Opponents","amount":1}}"#;
        let said: Vec<String> = twin_variants(ability, &each, plain)
            .iter()
            .filter(|text| text.contains("each opponent loses 1 life"))
            .map(|text| {
                text.replacen(
                    "each opponent loses 1 life",
                    &format!("each opponent who has {word} or more poison counters loses 1 life"),
                    1,
                )
            })
            .collect();
        texts.extend(said);
    }
}
