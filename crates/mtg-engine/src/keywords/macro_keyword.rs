//! Keyword macros: the abilities a macro keyword stands for come from the table in `mtg_cards::macros`.
//!
//! One rules value serves every macro keyword (Bushido, Modular, Mobilize); a new one is a table entry.

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::macros;

use super::KeywordRules;

/// The rules of the keywords the macro table expands.
#[derive(Debug)]
pub(super) struct MacroKeyword;

impl KeywordRules for MacroKeyword {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Macro { name, amount } = keyword else {
            return Vec::new();
        };
        macros::keyword_abilities(name, *amount).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a macro keyword adds exactly the abilities its table entry expands to, and no other keyword adds any.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::bushido("bushido", 2, 1)]
    #[case::modular("modular", 1, 2)]
    #[case::mobilize("mobilize", 3, 1)]
    #[case::unknown("flanking", 1, 0)]
    fn a_macro_adds_the_abilities_of_its_entry(
        #[case] name: &str,
        #[case] amount: u32,
        #[case] count: usize,
    ) {
        let keyword = Keyword::Macro {
            name: name.to_owned(),
            amount,
        };
        assert_eq!(MacroKeyword.adds_abilities(&keyword).len(), count);
    }

    #[test]
    fn another_keyword_adds_nothing() {
        assert_eq!(MacroKeyword.adds_abilities(&Keyword::Flying).len(), 0);
    }
}
