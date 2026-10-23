//! Gift (CR 702.174): "As an additional cost to cast this spell, you may promise an opponent a gift." The chosen
//! opponent gets the gift as the spell resolves, before its other effects, or when the permanent enters.
//!
//! The promise is an optional additional cost without mana that names the opponent: the cost is offered once for each
//! opponent as `SEAT_BASE + seat`, and that number is what the spell, the permanent and their abilities keep among the
//! costs paid, so `Ref::Promised` and `Condition::AdditionalCostPaid` read it wherever the costs are read. A spell
//! runs the gift first (`before`); a permanent has an enters trigger that gives it.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::card::TokenSpec;
use mtg_cards::effect::{Effect, TokenRef};
use mtg_cards::keyword::{GiftKind, Keyword};
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Condition};
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;

use super::{AdditionalCost, KeywordRules, OptionalKind};
use crate::ability_reader;
use crate::object::GameObject;

/// The first number among the optional costs paid that names a seat: `SEAT_BASE + seat` is a gift promised to it.
pub const SEAT_BASE: usize = 1 << 16;

/// The number that stands for a gift promised to `player` among the costs paid.
pub(crate) fn promise(player: PlayerId) -> usize {
    SEAT_BASE + player.index()
}

/// The player the costs paid promise a gift to, if any.
pub(crate) fn promised(paid: &[usize]) -> Option<PlayerId> {
    paid.iter()
        .find(|cost| **cost >= SEAT_BASE)
        .map(|cost| PlayerId::from_index(*cost - SEAT_BASE))
}

/// The rules of gift.
#[derive(Debug)]
pub(super) struct Gift;

impl KeywordRules for Gift {
    fn additional_cost(&self, keyword: &Keyword) -> Option<AdditionalCost> {
        let Keyword::Gift(kind) = keyword else {
            return None;
        };
        Some(AdditionalCost {
            name: label(*kind),
            cost: mtg_cards::cost::Cost::default(),
            repeatable: false,
            kind: OptionalKind::Gift,
        })
    }

    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Gift(kind) = keyword else {
            return Vec::new();
        };
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: Some(Condition::AdditionalCostPaid),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: given(*kind),
        }]
    }
}

/// The gift as the card names it, which labels the cost.
const fn label(kind: GiftKind) -> &'static str {
    match kind {
        GiftKind::Card => "Gift a card",
        GiftKind::Treasure => "Gift a Treasure",
        GiftKind::Food => "Gift a Food",
        GiftKind::TappedFish => "Gift a tapped Fish",
        GiftKind::Octopus => "Gift an Octopus",
        GiftKind::ExtraTurn => "Gift an extra turn",
    }
}

/// The effects that give the gift to the player it was promised to.
pub(crate) fn given(kind: GiftKind) -> Vec<Effect> {
    let who = Ref::Promised;
    let token = |token: TokenRef, tapped: bool| Effect::CreateToken {
        who: Ref::Promised,
        token,
        count: Amount::one(),
        tapped,
        attacking: false,
        bind: None,
    };
    vec![match kind {
        GiftKind::Card => Effect::Draw {
            who,
            count: Amount::one(),
        },
        GiftKind::Treasure => token(TokenRef::Named("treasure".to_owned()), false),
        GiftKind::Food => token(TokenRef::Named("food".to_owned()), false),
        GiftKind::TappedFish => token(creature("Fish", 1), true),
        GiftKind::Octopus => token(creature("Octopus", 8), false),
        GiftKind::ExtraTurn => Effect::ExtraTurn { who },
    }]
}

/// A blue creature token of the subtype whose power and toughness are both `size`.
fn creature(subtype: &str, size: i32) -> TokenRef {
    TokenRef::Custom(Box::new(TokenSpec {
        name: subtype.to_owned(),
        colors: vec![Color::Blue],
        supertypes: Vec::new(),
        types: vec![CardType::Creature],
        subtypes: vec![Subtype(subtype.to_owned())],
        power: Some(size),
        toughness: Some(size),
        abilities: Vec::new(),
    }))
}

/// The rules of the spell `object` runs, with the gift given first when its card has gift (CR 702.174): the effects of
/// its `Spell` ability are preceded by "if the gift was promised", the gift. Other rules come back as they are.
pub(crate) fn before(object: &GameObject, rules: Arc<Ability>) -> Arc<Ability> {
    let Some(kind) = ability_reader::printed_rules(object.card()).find_map(|rule| match &**rule {
        Ability::Keyword(Keyword::Gift(kind)) => Some(*kind),
        _ => None,
    }) else {
        return rules;
    };
    let Ability::Spell { targets, effects } = &*rules else {
        return rules;
    };
    let mut first = vec![Effect::If {
        condition: Condition::AdditionalCostPaid,
        then: given(kind),
        otherwise: Vec::new(),
    }];
    first.extend(effects.iter().cloned());
    Arc::new(Ability::Spell {
        targets: targets.clone(),
        effects: first,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: Gift offers one optional cost without mana and adds one enters trigger that gives the gift to the promised player if the cost was paid, a promise is a number that names a seat and nothing below it, and a spell with gift runs the gift before its other effects.
    use mtg_cards::keyword::GiftKind;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::card(GiftKind::Card, "Gift a card")]
    #[case::treasure(GiftKind::Treasure, "Gift a Treasure")]
    #[case::fish(GiftKind::TappedFish, "Gift a tapped Fish")]
    #[case::turn(GiftKind::ExtraTurn, "Gift an extra turn")]
    fn gift_offers_one_cost_without_mana(#[case] kind: GiftKind, #[case] name: &str) {
        let extra = Gift.additional_cost(&Keyword::Gift(kind)).unwrap();
        assert_eq!(extra.name, name);
        assert!(!extra.repeatable);
        assert_eq!(extra.kind, OptionalKind::Gift);
        assert_eq!(extra.cost.0, []);
    }

    #[rstest]
    #[case::card(GiftKind::Card)]
    #[case::treasure(GiftKind::Treasure)]
    #[case::food(GiftKind::Food)]
    #[case::fish(GiftKind::TappedFish)]
    #[case::octopus(GiftKind::Octopus)]
    #[case::turn(GiftKind::ExtraTurn)]
    fn gift_adds_an_enters_trigger_that_gives_it_to_the_promised_player(#[case] kind: GiftKind) {
        let added = Gift.adds_abilities(&Keyword::Gift(kind));
        let [
            Ability::Triggered {
                intervening_if,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one trigger, got {added:?}");
        };
        assert_eq!(*intervening_if, Some(Condition::AdditionalCostPaid));
        assert_eq!(*effects, given(kind));
        assert!(format!("{effects:?}").contains("Promised"));
    }

    #[rstest]
    #[case::no_other_keyword(Keyword::Flying)]
    #[case::casualty(Keyword::Casualty(2))]
    fn no_other_keyword_adds_anything(#[case] keyword: Keyword) {
        assert_eq!(Gift.additional_cost(&keyword), None);
        assert_eq!(Gift.adds_abilities(&keyword), []);
    }

    #[rstest]
    #[case::first_seat(0)]
    #[case::third_seat(2)]
    #[case::fourth_seat(3)]
    fn a_promise_names_its_seat(#[case] seat: usize) {
        let player = PlayerId::from_index(seat);
        assert_eq!(promised(&[0, promise(player)]), Some(player));
        assert_eq!(promised(&[0, 1, 2]), None);
        assert_eq!(promised(&[]), None);
    }
}
