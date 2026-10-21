//! Madness (CR 702.35): "If a player would discard this card, that player discards it, but exiles it instead of putting
//! it into their graveyard" and "When this card is exiled this way, its owner may cast it by paying [cost] rather than
//! its mana cost. If that player doesn't cast it, they put it into their graveyard."
//!
//! The keyword adds a replacement effect that works from a hand and a trigger that works from exile. The trigger casts
//! the card during its resolution (`Effect::CastFromZone` with the madness way); the card the player does not cast, or
//! cannot, goes to the graveyard.

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use mtg_cards::value::CastWay;

use super::KeywordRules;

/// The rules of madness.
#[derive(Debug)]
pub(super) struct Madness;

impl KeywordRules for Madness {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Madness(_)) {
            return Vec::new();
        }
        vec![
            Ability::Replacement {
                when: ReplaceWhen::Discarded {
                    what: Selector::this(),
                },
                by: ReplaceBy::MoveTo(ZoneKind::Exile),
                condition: None,
                from: ZoneKind::Hand,
            },
            Ability::Triggered {
                when: TriggerPattern::Discarded {
                    what: Selector::this(),
                    by: Ref::You,
                },
                intervening_if: None,
                limit: None,
                from: ZoneKind::Exile,
                targets: Vec::new(),
                effects: vec![Effect::CastFromZone {
                    who: Ref::You,
                    what: Ref::This,
                    free: false,
                    way: Some(CastWay::Madness),
                    otherwise: vec![Effect::Move {
                        what: Ref::This,
                        to: Destination::Graveyard,
                    }],
                }],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Madness adds a replacement of the discard that works from a hand and a trigger that works from exile and casts the card for its madness way, and no other keyword adds them.
    use mtg_cards::cost::Cost;

    use super::*;

    #[test]
    fn madness_replaces_the_discard_and_triggers_from_exile() {
        let added = Madness.adds_abilities(&Keyword::Madness(Cost::mana("{R}".parse().unwrap())));
        assert!(matches!(
            added.as_slice(),
            [
                Ability::Replacement {
                    from: ZoneKind::Hand,
                    ..
                },
                Ability::Triggered {
                    from: ZoneKind::Exile,
                    ..
                }
            ]
        ));
    }

    #[test]
    fn no_other_keyword_adds_it() {
        assert_eq!(Madness.adds_abilities(&Keyword::Flying), []);
    }
}
