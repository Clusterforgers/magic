//! Card multisets and their comparison: names the first difference and excuses the few cards Arena and MTGO rename.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use mtg_import::decklist::Section;

use super::aliases::MARKERS;

/// Most cards of one export that may carry a digital name; the corpus has two per deck at most.
const MAX_DIGITAL_NAMES: usize = 4;

/// One distinct card of a deck: where it sits and which printing the text names (if it names any).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    /// Section of the deck.
    pub section: Section,
    /// Name with `A / B` written as `A // B`.
    pub name: String,
    /// Lower-case set code.
    pub set: Option<String>,
    /// Collector number.
    pub number: Option<String>,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.section, self.name)?;
        if let Some(set) = &self.set {
            write!(f, " ({set})")?;
        }
        match &self.number {
            Some(number) => write!(f, " {number}"),
            None => Ok(()),
        }
    }
}

/// Copies per distinct card.
pub type Cards = BTreeMap<Key, u64>;

/// Whether an export may name cards differently from paper (Universes Beyond cards have other names on Arena and MTGO).
#[derive(Clone, Copy, Debug)]
pub enum Digital<'a> {
    /// Every name is an oracle name.
    No,
    /// A few names may be digital ones, and no printing says which: the leftovers of both sides must balance.
    ByElimination,
    /// Only these digital names may appear, as learned from the deck's lossless export.
    Known(&'a BTreeSet<String>),
}

/// The copies of `a` that `b` does not account for.
pub fn leftovers(a: &Cards, b: &Cards) -> Cards {
    a.iter()
        .filter_map(|(key, &n)| {
            let rest = n.saturating_sub(b.get(key).copied().unwrap_or(0));
            (rest > 0).then(|| (key.clone(), rest))
        })
        .collect()
}

/// Copy counts of the cards of `section`, largest first.
fn counts(cards: &Cards, section: Section) -> Vec<u64> {
    let mut counts: Vec<u64> = cards
        .iter()
        .filter(|(key, _)| key.section == section)
        .map(|(_, &n)| n)
        .collect();
    counts.sort_unstable_by(|a, b| b.cmp(a));
    counts
}

/// Whether a parsed leftover is a plausible digital name: clean, and not an extension or truncation of a site name.
fn is_clean(parsed: &Key, under: &Cards) -> bool {
    !parsed.name.contains(MARKERS)
        && !under.keys().any(|site| {
            site.section == parsed.section
                && (parsed.name.contains(&site.name) || site.name.contains(&parsed.name))
        })
}

/// The cards excused as renamed: both sides' leftovers, if they are few, clean and (by elimination) balanced.
fn renamed(expected: &Cards, observed: &Cards, digital: Digital<'_>) -> BTreeSet<Key> {
    let (over, under) = (leftovers(observed, expected), leftovers(expected, observed));
    let fits = match digital {
        Digital::No => false,
        Digital::Known(names) => over.keys().all(|key| names.contains(&key.name)),
        Digital::ByElimination => {
            let sections: BTreeSet<Section> =
                over.keys().chain(under.keys()).map(|k| k.section).collect();
            over.len() <= MAX_DIGITAL_NAMES
                && sections.into_iter().all(|section| {
                    let mut pool = counts(&under, section);
                    counts(&over, section).iter().all(|n| {
                        pool.iter()
                            .position(|m| m == n)
                            .map(|at| pool.remove(at))
                            .is_some()
                    })
                })
        }
    };
    if fits && over.keys().all(|key| is_clean(key, &under)) {
        over.into_keys().chain(under.into_keys()).collect()
    } else {
        BTreeSet::new()
    }
}

/// The names of the cards the text has and the site does not.
pub fn unknown_names(expected: &Cards, observed: &Cards) -> BTreeSet<String> {
    leftovers(observed, expected)
        .into_keys()
        .map(|key| key.name)
        .collect()
}

/// The first card whose count differs (or, when `lossy`, that the text has too many of), with how many more differ.
pub fn first_difference(
    expected: &Cards,
    observed: &Cards,
    lossy: bool,
    digital: Digital<'_>,
) -> Option<String> {
    let excused = renamed(expected, observed, digital);
    let keys: BTreeSet<&Key> = expected.keys().chain(observed.keys()).collect();
    let mut wrong = keys.into_iter().filter_map(|key| {
        let want = expected.get(key).copied().unwrap_or(0);
        let got = observed.get(key).copied().unwrap_or(0);
        let differs = if lossy { got > want } else { got != want };
        (differs && !excused.contains(key)).then(|| format!("{key}: site has {want}, parsed {got}"))
    });
    let first = wrong.next()?;
    match wrong.count() {
        0 => Some(first),
        more => Some(format!("{first} (+{more} more)")),
    }
}
