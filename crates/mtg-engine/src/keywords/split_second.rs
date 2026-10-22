//! Split second (CR 702.61): while a spell with it is on the stack, players cannot cast spells or activate abilities that
//! are not mana abilities. Mana abilities, special actions and triggered abilities go on as usual. The priority options
//! and the casts that resolutions grant read `keywords::split_second_on_stack`.

use mtg_cards::keyword::Keyword;

use super::KeywordRules;

/// The rules of split second.
#[derive(Debug)]
pub(super) struct SplitSecond;

impl KeywordRules for SplitSecond {
    fn silences_players(&self, keyword: &Keyword) -> bool {
        matches!(keyword, Keyword::SplitSecond)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: split second is the one keyword that silences the players, and no other keyword does.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::split_second(Keyword::SplitSecond, true)]
    #[case::flash(Keyword::Flash, false)]
    #[case::undaunted(Keyword::Undaunted, false)]
    fn only_split_second_silences_the_players(#[case] keyword: Keyword, #[case] silences: bool) {
        assert_eq!(SplitSecond.silences_players(&keyword), silences);
    }
}
