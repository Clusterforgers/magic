//! Riot (CR 702.136): "You may have this permanent enter with an additional +1/+1 counter on it. If you don't, it gains
//! haste for as long as you control it."
//!
//! The keyword adds one `Enters` replacement (`ReplaceBy::Riot`); the replacement step asks the controller which of the
//! two the permanent gets and puts the counter on it or starts a haste effect that lasts while it stays. A creature that
//! is given riot by a static ability as it enters makes the choice too (`replace::gather` reads those statics), and each
//! instance of the keyword makes its own choice.

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};

use super::KeywordRules;

/// The rules of riot.
#[derive(Debug)]
pub(super) struct Riot;

impl KeywordRules for Riot {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Riot) {
            return Vec::new();
        }
        vec![Ability::Replacement {
            when: ReplaceWhen::Enters {
                what: Selector::this(),
            },
            by: ReplaceBy::Riot,
            condition: None,
            from: ZoneKind::Battlefield,
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Riot adds one enters replacement that makes the riot choice, and no other keyword adds it.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    #[test]
    fn riot_adds_an_enters_replacement_of_the_permanent_itself() {
        let added = Riot.adds_abilities(&Keyword::Riot);
        let [
            Ability::Replacement {
                when: ReplaceWhen::Enters { what },
                by: ReplaceBy::Riot,
                condition: None,
                from: ZoneKind::Battlefield,
            },
        ] = added.as_slice()
        else {
            panic!("one enters replacement, got {added:?}");
        };
        assert_eq!(*what, Selector::this());
    }

    #[rstest]
    #[case::ravenous(Keyword::Ravenous)]
    #[case::squad(Keyword::Squad(Cost::default()))]
    #[case::haste(Keyword::Haste)]
    #[case::casualty(Keyword::Casualty(1))]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Riot.adds_abilities(&keyword), []);
    }
}
