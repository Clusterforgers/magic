//! Devour (CR 702.82): "As this object enters, you may sacrifice any number of [permanents]. This permanent enters
//! with N +1/+1 counters on it for each permanent sacrificed this way."
//!
//! The keyword adds an `Enters` replacement (`ReplaceBy::Devour`); the replacement step asks the controller which
//! permanents to sacrifice, sacrifices them as the permanent enters and puts the counters on it.

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};

use super::KeywordRules;

/// The rules of devour.
#[derive(Debug)]
pub(super) struct Devour;

impl KeywordRules for Devour {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Devour { counters, of } = keyword else {
            return Vec::new();
        };
        vec![Ability::Replacement {
            when: ReplaceWhen::Enters {
                what: Selector::this(),
            },
            by: ReplaceBy::Devour {
                counters: *counters,
                of: of.clone(),
            },
            condition: None,
            from: ZoneKind::Battlefield,
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Devour adds one enters replacement that devours for its counters and what it may sacrifice, and no other keyword adds it.
    use mtg_cards::cost::Cost;
    use mtg_cards::refs::Pred;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;

    fn creatures() -> Selector {
        Selector(vec![Pred::Type(CardType::Creature)])
    }

    #[rstest]
    #[case::one(1)]
    #[case::two(2)]
    #[case::three(3)]
    fn devour_adds_an_enters_replacement_with_its_counters(#[case] counters: u32) {
        let keyword = Keyword::Devour {
            counters,
            of: creatures(),
        };
        let added = Devour.adds_abilities(&keyword);
        let [
            Ability::Replacement {
                when: ReplaceWhen::Enters { .. },
                by: ReplaceBy::Devour { counters: each, of },
                condition: None,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one enters replacement, got {added:?}");
        };
        assert_eq!(*each, counters);
        assert_eq!(*of, creatures());
    }

    #[rstest]
    #[case::squad(Keyword::Squad(Cost::default()))]
    #[case::casualty(Keyword::Casualty(1))]
    #[case::ravenous(Keyword::Ravenous)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Devour.adds_abilities(&keyword), []);
    }
}
