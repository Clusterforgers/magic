//! Ninjutsu (CR 702.49): "[Cost], Return an unblocked attacker you control to hand: Put this card onto the battlefield
//! from your hand tapped and attacking."
//!
//! An activated ability that works from a hand. The creature returned is a part of the cost, picked with the activation
//! (`parts/bounce.rs`); only an attacker whose defending player has declared blockers and that nothing blocks is offered, so
//! the ability can be activated from the declare blockers step on and only while such an attacker exists. The card is bound
//! as it leaves the hand, so a card that left in response does nothing (CR 400.7), and it joins the attack of the creature
//! the cost returned without being declared as an attacker (`Effect::JoinAttack`).

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::{Amount, Condition};

use super::KeywordRules;

/// The name the card put onto the battlefield is bound to.
const NINJA: &str = "ninja";

/// The rules of ninjutsu.
#[derive(Debug)]
pub(super) struct Ninjutsu;

fn ninja() -> Ref {
    Ref::Bound(NINJA.to_owned())
}

impl KeywordRules for Ninjutsu {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Ninjutsu(cost) = keyword else {
            return Vec::new();
        };
        vec![activation(cost)]
    }
}

/// The ability the keyword adds for `cost`.
fn activation(cost: &Cost) -> Ability {
    let mut items = cost.0.clone();
    items.push(CostItem::ReturnToHand {
        filter: Selector(vec![
            Pred::Attacking,
            Pred::Unblocked,
            Pred::ControlledBy(Ref::You),
        ]),
        count: Amount::one(),
    });
    Ability::Activated {
        cost: Cost(items),
        timing: Timing::Instant,
        condition: None,
        limit: None,
        from: ZoneKind::Hand,
        targets: Vec::new(),
        effects: vec![
            Effect::Choose {
                who: Ref::You,
                from: Ref::This,
                count: Bounds::default(),
                bind: NINJA.to_owned(),
                rest_as: None,
            },
            Effect::Move {
                what: ninja(),
                to: Destination::Battlefield {
                    tapped: true,
                    under: None,
                },
            },
            // Only the permanent that arrived is in play: a card that was moved away in response stays where it is.
            Effect::If {
                condition: Condition::Matches {
                    subject: ninja(),
                    selector: Selector(vec![Pred::In(ZoneKind::Battlefield)]),
                },
                then: vec![Effect::JoinAttack { what: ninja() }],
                otherwise: Vec::new(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Ninjutsu adds one activated ability that works from a hand at instant speed, costs the ninjutsu cost and the return of an unblocked attacker the player controls, binds the card, puts it onto the battlefield tapped and joins the attack, and no other keyword adds it.
    use rstest::rstest;

    use super::*;

    fn cost_of(mana: &str) -> Cost {
        Cost::mana(mana.parse().unwrap())
    }

    #[rstest]
    #[case::one_blue("{U}")]
    #[case::generic_and_colored("{1}{B}")]
    #[case::two_colors("{2}{U}{B}")]
    fn ninjutsu_activates_from_a_hand_for_its_cost_and_an_unblocked_attacker(#[case] mana: &str) {
        let added = Ninjutsu.adds_abilities(&Keyword::Ninjutsu(cost_of(mana)));
        let [
            Ability::Activated {
                cost,
                timing,
                from,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one activated ability, got {added:?}");
        };
        assert_eq!(*timing, Timing::Instant);
        assert_eq!(*from, ZoneKind::Hand);
        assert_eq!(cost.0[0], cost_of(mana).0[0], "the mana comes first");
        assert!(matches!(
            cost.0.last(),
            Some(CostItem::ReturnToHand { filter, .. })
                if filter.0.contains(&Pred::Unblocked) && filter.0.contains(&Pred::Attacking)
        ));
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::Choose { .. },
                Effect::Move {
                    to: Destination::Battlefield { tapped: true, .. },
                    ..
                },
                Effect::If { .. }
            ]
        ));
    }

    #[rstest]
    #[case::dash(Keyword::Dash(cost_of("{1}")))]
    #[case::unearth(Keyword::Unearth(cost_of("{1}")))]
    #[case::haste(Keyword::Haste)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Ninjutsu.adds_abilities(&keyword), []);
    }
}
