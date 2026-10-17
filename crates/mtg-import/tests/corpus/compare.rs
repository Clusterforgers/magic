//! Builds both sides of the comparison, the site's truth and the parsed export, and checks one export against the other.

use std::collections::BTreeSet;

use mtg_import::decklist::{
    DeckFormat, IgnoredSection, ParsedDeck, Section, parse_as, promote_implied_commander,
};

use super::aliases::{Aliases, canonical};
use super::diff::{Cards, Digital, Key, first_difference, unknown_names};
use super::truth::{Deck, Marking, Row, Site, Variant};

/// What a variant's text looks like, as far as the comparison needs to know.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// The format a user would name for this export.
    pub format: DeckFormat,
    /// Whether lines carry a set and a collector number.
    pub printings: bool,
    /// Whether multi-face cards may be written by their front face alone.
    pub front_names: bool,
    /// Whether a few cards may carry their Arena or MTGO name, which no printing in the text can map back.
    pub digital_names: bool,
}

/// The variant classes of the README, by site and file stem; an unknown stem fails loudly.
pub fn shape(site: Site, variant: &str) -> Shape {
    // (format, printings, front names, digital names)
    let (format, printings, front_names, digital_names) = match (site, variant) {
        (Site::Moxfield, "mtgo") => (DeckFormat::MtgoText, false, true, true),
        (Site::Moxfield, v) if v.starts_with("arena") => (DeckFormat::Arena, false, true, true),
        (Site::Moxfield, v) if v.starts_with("full") => (DeckFormat::Moxfield, true, false, false),
        (Site::Archidekt, "names-only" | "headers-generic-plain") => {
            (DeckFormat::Archidekt, false, false, false)
        }
        (Site::Archidekt, "front-name") => (DeckFormat::Archidekt, true, true, false),
        (Site::Archidekt, _) => (DeckFormat::Archidekt, true, false, false),
        (_, other) => panic!("variant {other:?} has no class in the corpus table"),
    };
    Shape {
        format,
        printings,
        front_names,
        digital_names,
    }
}

/// Whether the caller knows the deck has one single-copy commander and so runs `promote_implied_commander`.
///
/// Neither Moxfield's first lines nor a headerless MTGO block tell a bare list how many commanders
/// there are; that count is the caller's knowledge, taken from the truth here.
pub fn caller_promotes(deck: &Deck, variant: &Variant) -> bool {
    matches!(variant.commanders, Marking::FirstLines | Marking::LastBlock)
        && matches!(deck.truth.boards.commanders.as_slice(), [only] if only.qty == 1)
}

fn commander_section(deck: &Deck, variant: &Variant) -> Section {
    match variant.commanders {
        Marking::Marked | Marking::Section | Marking::LastBlock => Section::Commander,
        Marking::FirstLines if caller_promotes(deck, variant) => Section::Commander,
        Marking::FirstLines | Marking::None => Section::Main,
    }
}

/// The cards the text must hold according to the site; stickers are not deck cards and never appear.
pub fn expected(deck: &Deck, variant: &Variant, shape: Shape) -> Cards {
    let boards = &deck.truth.boards;
    let flat = variant.commanders == Marking::None;
    let plan: [(&str, &[Row], Section); 5] = [
        (
            "commanders",
            &boards.commanders,
            commander_section(deck, variant),
        ),
        ("signatureSpells", &boards.signature_spells, Section::Main),
        ("mainboard", &boards.mainboard, Section::Main),
        ("sideboard", &boards.sideboard, Section::Sideboard),
        ("maybeboard", &boards.maybeboard, Section::Maybeboard),
    ];
    let mut cards = Cards::new();
    for (board, rows, section) in plan {
        if !flat && !variant.boards.iter().any(|b| b == board) {
            continue;
        }
        for row in rows {
            let key = Key {
                section: if flat { Section::Main } else { section },
                name: canonical(&row.name),
                set: shape.printings.then(|| row.set.to_lowercase()),
                number: shape.printings.then(|| row.cn.clone()),
            };
            *cards.entry(key).or_insert(0) += u64::from(row.qty);
        }
    }
    cards
}

/// The parsed deck as card multiset, with names the text may abbreviate or rename mapped back to oracle names.
pub fn observed(parsed: &ParsedDeck, shape: Shape, aliases: &Aliases) -> Cards {
    let mut cards = Cards::new();
    for entry in &parsed.entries {
        let key = Key {
            section: entry.section,
            name: aliases.oracle_name(&entry.name, shape.front_names),
            set: entry.set.as_ref().map(|set| set.to_lowercase()),
            number: entry.collector_number.clone(),
        };
        *cards.entry(key).or_insert(0) += u64::from(entry.count);
    }
    cards
}

/// Parses `text` as the export's own format, then promotes the commander when the caller knows there is just one.
pub fn import(deck: &Deck, variant: &Variant, shape: Shape, text: &str) -> ParsedDeck {
    let parsed = parse_as(shape.format, text);
    if caller_promotes(deck, variant) {
        promote_implied_commander(shape.format, parsed)
    } else {
        parsed
    }
}

/// The sections a variant must report as left out: Moxfield stickers when the text carries them.
pub fn expected_ignored(deck: &Deck, variant: &Variant) -> Vec<IgnoredSection> {
    let carried = variant.boards.iter().any(|b| b == "stickers");
    let count: u64 = deck
        .truth
        .boards
        .stickers
        .iter()
        .map(|row| u64::from(row.qty))
        .sum();
    if carried && count > 0 {
        vec![IgnoredSection {
            name: "STICKERS".to_owned(),
            count,
        }]
    } else {
        Vec::new()
    }
}

/// The digital names the deck's lossless MTGO export uses; Arena exports hold a subset of those cards.
fn digital_names(deck: &Deck, aliases: &Aliases) -> BTreeSet<String> {
    let Some(variant) = deck.truth.variants.get("mtgo") else {
        return BTreeSet::new();
    };
    let shape = shape(deck.site, "mtgo");
    let parsed = import(deck, variant, shape, &deck.text("mtgo"));
    unknown_names(
        &expected(deck, variant, shape),
        &observed(&parsed, shape, aliases),
    )
}

/// Compares one export with the truth: no line errors, the right cards and the right ignored sections.
pub fn check(deck: &Deck, name: &str, aliases: &Aliases) -> Result<(), String> {
    let variant = &deck.truth.variants[name];
    let shape = shape(deck.site, name);
    let text = deck.text(name);
    let label = format!("{}/{name}", deck.label());
    let parsed = import(deck, variant, shape, &text);
    if let Some(error) = parsed.errors.first() {
        let line = text.lines().nth(error.line - 1).unwrap_or_default();
        let total = parsed.errors.len();
        return Err(format!("{label}: {error}: {line:?} ({total} line errors)"));
    }
    let known;
    let digital = match (shape.digital_names, variant.lossy) {
        (false, _) => Digital::No,
        (true, false) => Digital::ByElimination,
        (true, true) => {
            known = digital_names(deck, aliases);
            Digital::Known(&known)
        }
    };
    let want = expected(deck, variant, shape);
    let got = observed(&parsed, shape, aliases);
    if let Some(diff) = first_difference(&want, &got, variant.lossy, digital) {
        return Err(format!("{label}: {diff}"));
    }
    let ignored = expected_ignored(deck, variant);
    if parsed.ignored != ignored {
        return Err(format!(
            "{label}: ignored sections {:?}, wanted {ignored:?}",
            parsed.ignored
        ));
    }
    Ok(())
}
