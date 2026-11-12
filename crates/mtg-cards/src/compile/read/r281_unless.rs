//! "Target player loses 3 life unless they sacrifice a nonland permanent of their choice or discard a card.": a sentence
//! that happens to another player unless they pay one of two costs, the first a sacrifice of their choice.

use super::costs;
use super::payments::phrase;
use crate::cost::{Cost, CostItem};
use crate::refs::Ref;
use crate::value::Amount;

/// "unless they sacrifice a creature of their choice or pay 3 life": a choice of a sacrifice of one permanent and another
/// payment, by a player other than you; `None` for any other cost.
pub(super) fn r281_unless_sacrifice_or_text(who: &Ref, cost: &Cost) -> Option<String> {
    let [CostItem::OneOf(costs)] = cost.0.as_slice() else {
        return None;
    };
    let [sacrifice, other] = costs.as_slice() else {
        return None;
    };
    let [
        CostItem::Sacrifice {
            count: Amount::Fixed(1),
            ..
        },
    ] = sacrifice.0.as_slice()
    else {
        return None;
    };
    let sacrificed = costs::text(sacrifice)?;
    let mut chars = sacrificed.chars();
    let first = chars.next()?.to_lowercase();
    let other = phrase(other)?;
    (*who != Ref::You && !other.starts_with("sacrifice")).then(|| {
        format!(
            "unless they {first}{} of their choice or {other}",
            chars.as_str()
        )
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a choice of a sacrifice and another payment by another player is written "unless they sacrifice ... of their choice or ...", and nothing else is.
    use rstest::rstest;

    use super::*;

    fn cost(json: &str) -> Cost {
        serde_json::from_str(json).unwrap()
    }

    #[rstest]
    #[case::sacrifice_or_discard(
        r#"[{"OneOf":[[{"Sacrifice":{"filter":[{"Not":{"Type":"Land"}}]}}],[{"Discard":{}}]]}]"#,
        "Target",
        Some("unless they sacrifice a nonland permanent of their choice or discard a card")
    )]
    #[case::sacrifice_or_life(
        r#"[{"OneOf":[[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],[{"PayLife":3}]]}]"#,
        "Target",
        Some("unless they sacrifice a creature of their choice or pay 3 life")
    )]
    #[case::you_pay(
        r#"[{"OneOf":[[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],[{"PayLife":3}]]}]"#,
        "You",
        None
    )]
    #[case::two_sacrificed(r#"[{"OneOf":[[{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":2}}],[{"PayLife":3}]]}]"#, "Target", None)]
    #[case::the_sacrifice_second(
        r#"[{"OneOf":[[{"PayLife":3}],[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]]}]"#,
        "Target",
        None
    )]
    #[case::a_single_cost(r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#, "Target", None)]
    fn writes_a_sacrifice_or_another_payment(
        #[case] json: &str,
        #[case] who: &str,
        #[case] expected: Option<&str>,
    ) {
        let who = if who == "You" {
            Ref::You
        } else {
            Ref::Target(0)
        };
        assert_eq!(
            r281_unless_sacrifice_or_text(&who, &cost(json)).as_deref(),
            expected
        );
    }

    const LANTERN: &str = "{T}: Target player loses 3 life unless they sacrifice a nonland permanent of their choice or discard a card.";

    #[rstest]
    #[case::the_lantern(
        r#"[{"OneOf":[[{"Sacrifice":{"filter":[{"Not":{"Type":"Land"}}]}}],[{"Discard":{}}]]}]"#,
        3,
        true
    )]
    #[case::another_amount(
        r#"[{"OneOf":[[{"Sacrifice":{"filter":[{"Not":{"Type":"Land"}}]}}],[{"Discard":{}}]]}]"#,
        2,
        false
    )]
    #[case::a_creature(
        r#"[{"OneOf":[[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],[{"Discard":{}}]]}]"#,
        3,
        false
    )]
    #[case::the_discard_alone(r#"[{"Discard":{}}]"#, 3, false)]
    fn an_activated_payment_reads_back_only_as_written(
        #[case] cost: &str,
        #[case] life: u32,
        #[case] reads: bool,
    ) {
        let json = format!(
            r#"{{"Activated":{{"cost":["Tap"],"targets":[{{"what":"Player"}}],"effects":[{{"Pay":{{"who":{{"Target":0}},"cost":{cost},"unpaid":[{{"LoseLife":{{"who":{{"Target":0}},"amount":{life}}}}}]}}}}]}}}}"#
        );
        let ability: crate::ability::Ability = serde_json::from_str(&json).unwrap();
        let written = super::super::render::written_variants(&ability);
        assert_eq!(
            written.iter().any(|text| text == LANTERN),
            reads,
            "{written:?}"
        );
    }
}
