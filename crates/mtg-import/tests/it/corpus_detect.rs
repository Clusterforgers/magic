//! Goal: `detect` gives an honest, explicit answer for every kind of export in the corpus: the format itself where the text carries one of its markers, a documented fallback where it cannot know, and the decisive answers read exactly like the format the user would have named.
//!
//! What each kind of text can prove (the rules below look at the raw text, not at `detect`):
//! - Moxfield `full*`: a foil or etched flag, or an all-caps header such as `SIDEBOARD:` or `STICKERS:`, next to printings. A list of printings with neither reads as plain text.
//! - Moxfield `mtgo`: only the all-caps header; without one it is `N Name` lines, which is plain text.
//! - Moxfield `arena*`: the `About` block, always.
//! - Archidekt with brackets: a `[Category]` after the printing, always. `sets-only` has none, so it is plain text, or Moxfield when a foil flag shares Moxfield's syntax. `names-only` is plain text; `headers-generic-plain` opens with a header word like an Arena export.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::corpus;

use corpus::{Deck, Site, shape};
use mtg_import::decklist::{
    DeckFormat, Section, detect, parse, parse_as, promote_implied_commander,
};

use DeckFormat::{Archidekt, Arena, Moxfield, MtgoText, PlainText};

/// What a text can tell about its own format.
#[derive(Debug, PartialEq, Eq)]
enum Answer {
    /// The text carries a marker of its format, so `detect` must name it.
    Decisive(DeckFormat),
    /// The text cannot say; `detect` must fall back to this format, which is not the export's own.
    CannotKnow(DeckFormat),
}

fn has_foil_flag(text: &str) -> bool {
    text.contains(" *F*") || text.contains(" *E*")
}

/// An all-caps `NAME:` line, the way Moxfield and MTGO text open a section.
fn has_shouting_header(text: &str) -> bool {
    text.lines().map(str::trim).any(|line| {
        line.len() > 1 && line.ends_with(':') && line.chars().all(|c| !c.is_lowercase())
    })
}

/// The answer the markers in `text` allow for the export `variant` of `site`.
fn answer(site: Site, variant: &str, text: &str) -> Answer {
    let (flag, header) = (has_foil_flag(text), has_shouting_header(text));
    match (site, variant) {
        (Site::Moxfield, "mtgo") if header => Answer::Decisive(MtgoText),
        (Site::Moxfield, "full-nofinish") if header => Answer::Decisive(Moxfield),
        (Site::Moxfield, "full" | "full-noflavor" | "full-tags") if flag || header => {
            Answer::Decisive(Moxfield)
        }
        (Site::Moxfield, "arena" | "arena-flags") => Answer::Decisive(Arena),
        (Site::Archidekt, "sets-only") if flag => Answer::CannotKnow(Moxfield),
        (Site::Archidekt, "headers-generic-plain") => Answer::CannotKnow(Arena),
        (Site::Archidekt, "names-only" | "sets-only") | (Site::Moxfield, _) => {
            Answer::CannotKnow(PlainText)
        }
        (Site::Archidekt, _) => Answer::Decisive(Archidekt),
    }
}

#[test]
fn detect_names_the_format_where_the_text_proves_it_and_falls_back_where_it_cannot() {
    let (mut decisive, mut unknowable) = (0, 0);
    for deck in Deck::all() {
        for name in deck.truth.variants.keys() {
            let text = deck.text(name);
            let explicit = shape(deck.site, name).format;
            let label = format!("{}/{name}", deck.label());
            let detected = detect(&text);
            match answer(deck.site, name, &text) {
                Answer::Decisive(format) => {
                    assert_eq!(detected, format, "{label}");
                    assert_eq!(
                        format, explicit,
                        "{label}: a decisive answer is the export's own format"
                    );
                    decisive += 1;
                }
                Answer::CannotKnow(fallback) => {
                    assert_eq!(detected, fallback, "{label}");
                    assert_ne!(
                        fallback, explicit,
                        "{label}: if it can be told, say so in `answer`"
                    );
                    unknowable += 1;
                }
            }
        }
    }
    assert!(
        decisive >= 120 && unknowable >= 50,
        "{decisive} decisive, {unknowable} unknowable"
    );
}

/// Where `detect` is decisive, `parse` and the commander import read the export exactly as the named format does.
#[test]
fn detect_then_parse_agrees_with_the_explicit_format_wherever_detect_is_decisive() {
    for deck in Deck::all() {
        for name in deck.truth.variants.keys() {
            let text = deck.text(name);
            if !matches!(answer(deck.site, name, &text), Answer::Decisive(_)) {
                continue;
            }
            let explicit = shape(deck.site, name).format;
            let label = format!("{}/{name}", deck.label());
            assert_eq!(parse(&text), parse_as(explicit, &text), "{label}");
            assert_eq!(
                promote_implied_commander(detect(&text), parse(&text)),
                promote_implied_commander(explicit, parse_as(explicit, &text)),
                "{label}"
            );
        }
    }
}

/// Where it cannot know, the plain-text fallback still reads the same cards in the same sections, with one exception:
/// an MTGO text without header ends in a commander block, which plain text takes for the sideboard.
#[test]
fn the_fallback_reads_the_same_deck_except_for_an_mtgo_commander_block() {
    let mut commander_blocks = 0;
    for deck in Deck::all() {
        for name in deck.truth.variants.keys() {
            let text = deck.text(name);
            if !matches!(answer(deck.site, name, &text), Answer::CannotKnow(_)) {
                continue;
            }
            let explicit = shape(deck.site, name).format;
            let (read, named) = (parse(&text), parse_as(explicit, &text));
            let label = format!("{}/{name}", deck.label());
            if explicit == MtgoText && !deck.truth.boards.commanders.is_empty() {
                assert_eq!(read.card_count(Section::Commander), 0, "{label}");
                assert_eq!(
                    read.card_count(Section::Sideboard),
                    named.card_count(Section::Commander),
                    "{label}"
                );
                commander_blocks += 1;
            } else {
                assert_eq!(read, named, "{label}");
            }
        }
    }
    assert!(
        commander_blocks >= 5,
        "only {commander_blocks} headerless MTGO commander blocks were found"
    );
}
