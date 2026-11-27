//! Beam me up: "You may cast this card from your graveyard for <cost> if you also return a creature you control to its
//! owner's hand. Then exile this spell."
//!
//! The cast from the graveyard is an alternative cost chosen with the cast (`ways.rs`): the beam me up cost and, as part
//! of the total cost, the return of a creature its caster controls to its owner's hand (CR 601.2f, 601.2h), picked with
//! the cast like any part of a spell's cost. The spell is exiled when it leaves the stack (`CastWay::exiles_the_spell`).
//! Only instant and sorcery cards are cast this way. A beam me up that a static ability gives may name the return in its
//! own cost: `Granter` there is the permanent with that static ability ("Scotty can't be beamed up to cast a spell this
//! way"), fixed as the keyword is given (`r116_fix_granter`).

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use super::KeywordRules;

/// The rules of beam me up.
#[derive(Debug)]
pub(super) struct BeamMeUp;

impl KeywordRules for BeamMeUp {}

/// The whole cost of casting a card with beam me up `cost`: that cost and the return of a creature its caster controls,
/// unless the cost names that return itself.
pub(crate) fn r116_beam_cost(cost: &Cost) -> Cost {
    let mut whole = cost.clone();
    if whole
        .0
        .iter()
        .any(|item| matches!(item, CostItem::ReturnToHand { .. }))
    {
        return whole;
    }
    whole.0.push(CostItem::ReturnToHand {
        filter: Selector(vec![
            Pred::Type(CardType::Creature),
            Pred::R119CanBeBeamedUp,
        ]),
        count: mtg_cards::value::Amount::one(),
    });
    whole
}

/// The beam me up `keyword` that the static ability of `source` gives, with `Granter` in its return fixed as `source`.
pub(crate) fn r116_fix_granter(keyword: Keyword, source: ObjectId) -> Keyword {
    let Keyword::R116BeamMeUp(mut cost) = keyword else {
        return keyword;
    };
    for item in &mut cost.0 {
        if let CostItem::ReturnToHand { filter, .. } = item {
            for pred in &mut filter.0 {
                r116_fix_pred(pred, source);
            }
        }
    }
    Keyword::R116BeamMeUp(cost)
}

fn r116_fix_pred(pred: &mut Pred, source: ObjectId) {
    match pred {
        Pred::Is(reference) if *reference == Ref::Granter => *reference = Ref::R111Object(source),
        Pred::Not(inner) => r116_fix_pred(inner, source),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a given beam me up fixes `Granter` in its return as the giving permanent, and a cost that names the return
    //! keeps it in place of the default one.
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;

    fn r116_cost(json: &str) -> Cost {
        serde_json::from_str(json).unwrap()
    }

    #[rstest::rstest]
    #[case::plain(r#"[{"Mana":"{1}{U}"}]"#)]
    #[case::named_return(
        r#"[{"Mana":"{1}{U}"},{"ReturnToHand":{"filter":[{"Type":"Creature"},{"Not":{"Is":"Granter"}}]}}]"#
    )]
    fn r116_the_return_is_added_once(#[case] json: &str) {
        let whole = r116_beam_cost(&r116_cost(json));
        assert_eq!(whole.0.len(), 2);
        let returns = whole
            .0
            .iter()
            .filter(|item| matches!(item, CostItem::ReturnToHand { .. }))
            .count();
        assert_eq!(returns, 1);
    }

    #[rstest::rstest]
    #[case::granter(r#"{"Not":{"Is":"Granter"}}"#, true)]
    #[case::this(r#"{"Not":{"Is":"This"}}"#, false)]
    fn r116_the_granter_is_fixed(#[case] pred: &str, #[case] fixed: bool) {
        let json = format!(r#"[{{"Mana":"{{1}}"}},{{"ReturnToHand":{{"filter":[{pred}]}}}}]"#);
        let keyword = r116_fix_granter(Keyword::R116BeamMeUp(r116_cost(&json)), ObjectId(7));
        let Keyword::R116BeamMeUp(cost) = keyword else {
            panic!("still beam me up")
        };
        let written = serde_json::to_string(&cost).unwrap();
        assert_eq!(written.contains("R111Object"), fixed, "{written}");
    }
}
