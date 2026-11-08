//! A keyword in the form two readings of the same rule share: a selector is a conjunction and an "or" list a set, so
//! their order is no part of the rule; Equip targets only a creature its controller controls (CR 702.6a), so a filter
//! that says so says nothing more; a card exiled from the hand to pay a keyword cost of a spell is another card, since
//! the spell is on the stack (CR 601.2a).

use serde_json::Value;

use crate::keyword::Keyword;
use crate::refs::Pred;

/// The keyword with its selectors in one order and the predicates it implies left out; the keyword itself when it
/// does not serialize.
pub fn canonical(keyword: &Keyword) -> Keyword {
    let Ok(mut value) = serde_json::to_value(keyword) else {
        return keyword.clone();
    };
    if let Some(to) = value
        .pointer_mut("/EquipTo/to")
        .and_then(Value::as_array_mut)
    {
        to.retain(|pred| {
            *pred != serde_json::json!({"Type": "Creature"})
                && *pred != serde_json::json!({"ControlledBy": "You"})
        });
    }
    drop_other_from_hand(&mut value);
    sort_selectors(&mut value);
    serde_json::from_value(value).unwrap_or_else(|_| keyword.clone())
}

/// Removes `Other` from the filter of each card exiled from the hand.
fn drop_other_from_hand(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(filter) = map
                .get_mut("ExileFromHand")
                .and_then(|exiled| exiled.get_mut("filter"))
                .and_then(Value::as_array_mut)
            {
                filter.retain(|pred| pred != "Other");
            }
            map.values_mut().for_each(drop_other_from_hand);
        }
        Value::Array(items) => items.iter_mut().for_each(drop_other_from_hand),
        _ => {}
    }
}

/// Sorts each list whose items are all predicates, at any depth.
fn sort_selectors(value: &mut Value) {
    match value {
        Value::Object(map) => map.values_mut().for_each(sort_selectors),
        Value::Array(items) => {
            items.iter_mut().for_each(sort_selectors);
            let preds = !items.is_empty()
                && items
                    .iter()
                    .all(|item| serde_json::from_value::<Pred>(item.clone()).is_ok());
            if preds {
                items.sort_by_key(Value::to_string);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    //! Goal: two keywords read the same only when they differ in the order of a selector, an "or" list or the parts
    //! Equip and a cost exiled from the hand imply; another type, controller, color or zone keeps them apart.
    use rstest::rstest;

    use super::*;

    fn keyword(json: &str) -> Keyword {
        serde_json::from_str(json).expect("a keyword")
    }

    #[rstest]
    #[case::or_order(
        r#"{"Enchant":[{"AnyOf":[{"Type":"Creature"},{"Type":"Artifact"}]}]}"#,
        r#"{"Enchant":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}]}"#,
        true
    )]
    #[case::selector_order(
        r#"{"Enchant":[{"Type":"Land"},{"ControlledBy":"You"},{"Supertype":"Basic"}]}"#,
        r#"{"Enchant":[{"Type":"Land"},{"Supertype":"Basic"},{"ControlledBy":"You"}]}"#,
        true
    )]
    #[case::another_type(
        r#"{"Enchant":[{"AnyOf":[{"Type":"Creature"},{"Type":"Artifact"}]}]}"#,
        r#"{"Enchant":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Land"}]}]}"#,
        false
    )]
    #[case::equip_implied(
        r#"{"EquipTo":{"cost":"{1}","to":[{"Type":"Creature"},{"ControlledBy":"You"},{"Subtype":"Halfling"}]}}"#,
        r#"{"EquipTo":{"cost":"{1}","to":[{"Subtype":"Halfling"}]}}"#,
        true
    )]
    #[case::equip_another_controller(
        r#"{"EquipTo":{"cost":"{1}","to":[{"ControlledBy":"Opponents"},{"Subtype":"Halfling"}]}}"#,
        r#"{"EquipTo":{"cost":"{1}","to":[{"Subtype":"Halfling"}]}}"#,
        false
    )]
    #[case::evoke_other(
        r#"{"Evoke":[{"ExileFromHand":{"filter":[{"Color":"White"},"Other"]}}]}"#,
        r#"{"Evoke":[{"ExileFromHand":{"filter":[{"Color":"White"}]}}]}"#,
        true
    )]
    #[case::evoke_another_color(
        r#"{"Evoke":[{"ExileFromHand":{"filter":[{"Color":"White"},"Other"]}}]}"#,
        r#"{"Evoke":[{"ExileFromHand":{"filter":[{"Color":"Blue"}]}}]}"#,
        false
    )]
    #[case::graveyard_keeps_other(
        r#"{"Escape":[{"Mana":"{3}{B}"},{"ExileFromGraveyard":{"count":4,"filter":["Other"]}}]}"#,
        r#"{"Escape":[{"Mana":"{3}{B}"},{"ExileFromGraveyard":{"count":4,"filter":[]}}]}"#,
        false
    )]
    fn keywords_of_one_meaning_compare_equal(
        #[case] left: &str,
        #[case] right: &str,
        #[case] same: bool,
    ) {
        assert_eq!(
            canonical(&keyword(left)) == canonical(&keyword(right)),
            same
        );
    }
}
