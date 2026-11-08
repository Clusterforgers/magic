//! Plot (CR 702.170): "You may pay [cost] and exile this card from your hand. Cast it as a sorcery on a later turn
//! without paying its mana cost. Plot only as a sorcery."
//!
//! The keyword adds no ability. The special action is `PriorityAction::Plot` (`priority.rs` offers it, `announce/plot.rs`
//! pays for it), the card lies face up in exile marked with the turn it was plotted in (`GameObject::plotted`), and
//! `ways.rs` offers the free cast at sorcery timing once that turn is over.

use super::KeywordRules;

/// The rules of plot.
#[derive(Debug)]
pub(super) struct Plot;

impl KeywordRules for Plot {}

#[cfg(test)]
mod tests {
    //! Goal: the keyword of plot adds no ability of its own: the special action and the free cast live in the priority and ways rules.
    use mtg_cards::ability::Ability;
    use mtg_cards::cost::Cost;
    use mtg_cards::keyword::Keyword;

    use super::*;

    #[test]
    fn the_keyword_adds_no_ability() {
        let keyword = Keyword::Plot(Cost::mana("{1}{R}".parse().unwrap()));
        let added: Vec<Ability> = Plot.adds_abilities(&keyword);
        assert_eq!(added, Vec::<Ability>::new());
    }
}
