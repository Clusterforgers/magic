//! Dredge (CR 702.52): "If you would draw a card, you may instead mill N cards and return this card from your graveyard
//! to your hand."
//!
//! The keyword adds a replacement effect of a draw that works from a graveyard; the replacement step asks the player
//! and does the milling and the return (`replace/dredge.rs`).

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};

use super::KeywordRules;

/// The rules of dredge.
#[derive(Debug)]
pub(super) struct Dredge;

impl KeywordRules for Dredge {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Dredge(cards) = keyword else {
            return Vec::new();
        };
        vec![Ability::Replacement {
            when: ReplaceWhen::Draw { who: Ref::You },
            by: ReplaceBy::Dredge(*cards),
            condition: None,
            from: ZoneKind::Graveyard,
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: dredge adds one replacement of its owner's draw that works from a graveyard and names the number of cards, and nothing for another keyword.
    use super::*;

    #[test]
    fn dredge_adds_a_replacement_of_a_draw_from_the_graveyard() {
        let added = Dredge.adds_abilities(&Keyword::Dredge(3));
        assert_eq!(
            added,
            [Ability::Replacement {
                when: ReplaceWhen::Draw { who: Ref::You },
                by: ReplaceBy::Dredge(3),
                condition: None,
                from: ZoneKind::Graveyard,
            }]
        );
        assert_eq!(Dredge.adds_abilities(&Keyword::Flying), []);
    }
}
