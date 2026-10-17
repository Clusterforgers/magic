//! Commander imports: promotes the commander a source dialect implies but does not mark.

use super::types::{DeckFormat, ParsedDeck, Section};

/// Commander imports only: when no commander is marked, moves the one `format` implies (Moxfield text: the unmarked first line) into the command zone; partner pairs are left to the deck-builder UI.
pub fn promote_implied_commander(format: DeckFormat, mut deck: ParsedDeck) -> ParsedDeck {
    let marked = deck.entries.iter().any(|e| e.section == Section::Commander);
    if format == DeckFormat::Moxfield
        && !marked
        && let Some(first) = deck.entries.first_mut()
        && first.section == Section::Main
        && first.count == 1
    {
        first.section = Section::Commander;
    }
    deck
}

#[cfg(test)]
mod tests {
    //! Goal: only a Moxfield list without a marked commander has its first single card promoted, every other deck passes through untouched, and nothing else about the deck changes.
    use super::*;
    use crate::decklist::{DeckEntry, parse_as};
    use proptest::prelude::*;
    use rstest::rstest;

    use DeckFormat::{Archidekt, Arena, Moxfield, MtgoDek, MtgoText, PlainText};

    #[rstest]
    #[case::first_line_of_a_full_export(
        "1 Najeela (BBD) 62 *F*\n1 Ad Nauseam (ALA) 63\n\nSIDEBOARD:\n1 Necropotence (DKM) 7\n",
        "Najeela"
    )]
    #[case::no_sideboard("1 Najeela (BBD) 62 *F*\n1 Ad Nauseam (ALA) 63 *F*\n", "Najeela")]
    #[case::partner_pair_is_left_to_the_ui(
        "1 Thrasios, Triton Hero (CMD) 1 *F*\n1 Tymna the Weaver (CMD) 2\n1 Sol Ring (CMD) 3\n",
        "Thrasios, Triton Hero"
    )]
    #[case::crlf("1 Najeela (BBD) 62 *F*\r\n1 Ad Nauseam (ALA) 63\r\n", "Najeela")]
    fn moxfield_first_line_becomes_the_commander(#[case] input: &str, #[case] name: &str) {
        let parsed = parse_as(Moxfield, input);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert_eq!(parsed.entries[0].section, Section::Main);
        let promoted = promote_implied_commander(Moxfield, parsed.clone());
        assert_eq!(promoted.entries[0].name, name);
        assert_eq!(promoted.entries[0].section, Section::Commander);
        assert_eq!(promoted.entries[1..], parsed.entries[1..]);
        assert_eq!(promoted.errors, parsed.errors);
    }

    #[rstest]
    #[case::marked_by_a_header(
        Moxfield,
        "COMMANDER:\n1 Atraxa (C16) 28\n\n1 Sol Ring (CMM) 400 *F*\n"
    )]
    #[case::several_copies_are_no_commander(
        Moxfield,
        "4 Lightning Bolt (M10) 146 *F*\n1 Shock (M19) 156\n"
    )]
    #[case::first_card_is_a_sideboard_card(Moxfield, "SIDEBOARD:\n1 Fury (MH2) 126 *F*\n")]
    #[case::empty_list(Moxfield, "")]
    #[case::only_errors(Moxfield, "Lightning Bolt\n")]
    #[case::plain_text_implies_nothing(PlainText, "1 Najeela (BBD) 62\n1 Sol Ring (CMM) 400\n")]
    #[case::arena_without_its_commander(Arena, "Deck\n1 Sol Ring (CMM) 400\n")]
    #[case::arena_marks_it(Arena, "Commander\n1 Atraxa (C16) 28\n\nDeck\n1 Sol Ring (CMM) 400\n")]
    #[case::archidekt_marks_it(
        Archidekt,
        "1x Atraxa (c16) 28 [Commander{top}]\n1x Sol Ring (cmm) 400 [Ramp]\n"
    )]
    #[case::archidekt_without_one(Archidekt, "1x Sol Ring (cmm) 400 [Ramp]\n")]
    #[case::mtgo_text_marks_it_last(MtgoText, "1 Sol Ring\n\nSIDEBOARD:\n1 Fury\n\n1 Atraxa\n")]
    #[case::mtgo_text_without_one(MtgoText, "1 Sol Ring\n\nSIDEBOARD:\n1 Fury\n")]
    #[case::mtgo_dek(
        MtgoDek,
        "<Deck><Cards CatID=\"1\" Quantity=\"1\" Sideboard=\"false\" Name=\"Sol Ring\"/></Deck>"
    )]
    fn every_other_deck_passes_through(#[case] format: DeckFormat, #[case] input: &str) {
        let parsed = parse_as(format, input);
        assert_eq!(promote_implied_commander(format, parsed.clone()), parsed);
    }

    // Weighted so that the promoting case (Moxfield, a single Main card first, no commander) is common.
    fn deck() -> impl Strategy<Value = ParsedDeck> {
        let section = prop_oneof![
            6 => Just(Section::Main),
            2 => Just(Section::Sideboard),
            1 => Just(Section::Commander),
            1 => Just(Section::Companion),
            1 => Just(Section::Maybeboard),
        ];
        let count = prop_oneof![4 => Just(1u32), 1 => 2u32..=4];
        let entry = (count, "[A-Z][a-z]{2,8}", section)
            .prop_map(|(count, name, section)| DeckEntry::new(count, name, section));
        prop::collection::vec(entry, 0..12).prop_map(|entries| ParsedDeck {
            entries,
            errors: Vec::new(),
        })
    }

    fn format() -> impl Strategy<Value = DeckFormat> {
        prop_oneof![
            1 => Just(PlainText),
            1 => Just(Arena),
            1 => Just(MtgoDek),
            1 => Just(MtgoText),
            3 => Just(Moxfield),
            1 => Just(Archidekt),
        ]
    }

    proptest! {
        #[test]
        fn promotes_exactly_when_the_rule_applies_and_changes_nothing_else(
            format in format(),
            deck in deck(),
        ) {
            let promoted = promote_implied_commander(format, deck.clone());
            let applies = format == Moxfield
                && deck.entries.iter().all(|e| e.section != Section::Commander)
                && deck.entries.first().is_some_and(|e| e.section == Section::Main && e.count == 1);
            let mut expected = deck.clone();
            if applies {
                expected.entries[0].section = Section::Commander;
            }
            prop_assert_eq!(&promoted, &expected);
            let before = deck.entries.iter().filter(|e| e.section == Section::Commander).count();
            let after = promoted.entries.iter().filter(|e| e.section == Section::Commander).count();
            prop_assert_eq!(after, before + usize::from(applies));
        }

        #[test]
        fn promoting_twice_is_promoting_once(format in format(), deck in deck()) {
            let once = promote_implied_commander(format, deck);
            prop_assert_eq!(promote_implied_commander(format, once.clone()), once);
        }
    }
}
