//! The wordings of a card name chosen as a permanent enters: "a card name", "a nonland card name", "a noncreature,
//! nonland card name", "a nonbasic land card name" and "each opponent reveals their hand. You choose the name of a
//! nonland card revealed this way".

use super::tokens::Reader;
use crate::ability::{ActivationKind, CostChange};
use crate::mana::ManaCostX;
use crate::refs::{Pred, Selector};
use crate::trigger::EntersChoice;

/// What follows "As ~ enters, " for each choice of a name, in the card's words.
const R4702_NAMES: [(EntersChoice, &str); 5] = [
    (EntersChoice::CardName, "choose a card name"),
    (EntersChoice::NonlandCardName, "choose a nonland card name"),
    (
        EntersChoice::NoncreatureNonlandCardName,
        "choose a noncreature, nonland card name",
    ),
    (
        EntersChoice::NonbasicLandCardName,
        "choose a nonbasic land card name",
    ),
    (
        EntersChoice::RevealedNonlandName,
        "each opponent reveals their hand. You choose the name of a nonland card revealed this way",
    ),
];

/// "As ~ enters, choose a nonland card name." for a choice of a name; empty for another choice.
pub(super) fn r4702_name_texts(kind: EntersChoice) -> Vec<String> {
    R4702_NAMES
        .iter()
        .filter(|(named, _)| *named == kind)
        .map(|(_, words)| format!("As ~ enters, {words}"))
        .collect()
}

/// The choice of a name after "As ~ enters,"; `None` for other words, which leave the reader where it was.
pub(super) fn r4702_read_name(r: &mut Reader<'_>) -> Option<EntersChoice> {
    R4702_NAMES
        .iter()
        .find_map(|(kind, words)| r.r376_text(words).then_some(*kind))
}

/// "Activated abilities of sources with the chosen name cost {2} more to activate unless they're mana abilities.":
/// a change of every ability but mana abilities of the sources with the chosen name; `None` for another change.
pub(super) fn r4702_named_activation_text(
    kind: ActivationKind,
    applies_to: &Selector,
    change: CostChange,
    mana: &ManaCostX,
) -> Option<String> {
    (kind == ActivationKind::Any && applies_to.0 == [Pred::ChosenName]).then_some(())?;
    let more = match change {
        CostChange::Increase => "more",
        CostChange::Reduce => "less",
    };
    Some(format!(
        "Activated abilities of sources with the chosen name cost {mana} {more} to activate unless they're mana abilities"
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: each choice of a name reads back only as its own words, and the reader takes each wording to its choice
    //! and no other.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::any(EntersChoice::CardName, "As ~ enters, choose a card name")]
    #[case::nonland(
        EntersChoice::NonlandCardName,
        "As ~ enters, choose a nonland card name"
    )]
    #[case::noncreature(
        EntersChoice::NoncreatureNonlandCardName,
        "As ~ enters, choose a noncreature, nonland card name"
    )]
    #[case::nonbasic_land(
        EntersChoice::NonbasicLandCardName,
        "As ~ enters, choose a nonbasic land card name"
    )]
    #[case::revealed(
        EntersChoice::RevealedNonlandName,
        "As ~ enters, each opponent reveals their hand. You choose the name of a nonland card revealed this way"
    )]
    fn r4702_a_name_reads_back(#[case] kind: EntersChoice, #[case] text: &str) {
        assert_eq!(r4702_name_texts(kind), vec![text.to_owned()]);
    }

    #[rstest]
    #[case::not_noncreature(
        EntersChoice::NonlandCardName,
        "As ~ enters, choose a noncreature, nonland card name"
    )]
    #[case::not_any(EntersChoice::CardName, "As ~ enters, choose a nonland card name")]
    #[case::not_a_land(
        EntersChoice::NonbasicLandCardName,
        "As ~ enters, choose a nonland card name"
    )]
    #[case::no_look(
        EntersChoice::CardName,
        "As ~ enters, look at an opponent's hand, then choose any card name"
    )]
    #[case::not_any_revealed(
        EntersChoice::RevealedNonlandName,
        "As ~ enters, each opponent reveals their hand. You choose the name of a card revealed this way"
    )]
    #[case::a_type(EntersChoice::CreatureType, "As ~ enters, choose a card name")]
    fn r4702_a_near_miss_does_not_read_back(#[case] kind: EntersChoice, #[case] text: &str) {
        assert!(r4702_name_texts(kind).iter().all(|said| said != text));
    }

    #[rstest]
    #[case::more(ActivationKind::Any, &[Pred::ChosenName], CostChange::Increase, true)]
    #[case::less_reads_less(ActivationKind::Any, &[Pred::ChosenName], CostChange::Reduce, true)]
    #[case::mana_too(ActivationKind::Every, &[Pred::ChosenName], CostChange::Increase, false)]
    #[case::another_source(ActivationKind::Any, &[Pred::ChosenColor], CostChange::Increase, false)]
    fn r4702_named_abilities_cost_more(
        #[case] kind: ActivationKind,
        #[case] sources: &[Pred],
        #[case] change: CostChange,
        #[case] written: bool,
    ) {
        let mana: ManaCostX = "{2}".parse().unwrap();
        let text = r4702_named_activation_text(kind, &Selector(sources.to_vec()), change, &mana);
        assert_eq!(text.is_some(), written);
        let more = if change == CostChange::Increase {
            "more"
        } else {
            "less"
        };
        if let Some(text) = text {
            assert_eq!(
                text,
                format!(
                    "Activated abilities of sources with the chosen name cost {{2}} {more} to activate unless they're mana abilities"
                )
            );
        }
    }

    #[rstest]
    #[case::noncreature(
        "choose a noncreature, nonland card name",
        Some(EntersChoice::NoncreatureNonlandCardName)
    )]
    #[case::revealed(
        "each opponent reveals their hand. You choose the name of a nonland card revealed this way",
        Some(EntersChoice::RevealedNonlandName)
    )]
    #[case::a_type("choose a creature type", None)]
    fn r4702_the_reader_takes_each_wording(#[case] text: &str, #[case] kind: Option<EntersChoice>) {
        let tokens = super::super::tokens::tokenize(text);
        let mut r = Reader::new(&tokens, false);
        assert_eq!(r4702_read_name(&mut r), kind);
        assert_eq!(r.end(), kind.is_some());
    }
}
