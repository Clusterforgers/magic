//! The clause of an ability's words that a place in its effects stands for: the asked clause of a decision.
//!
//! The words of an ability are written from its effects as a whole, so the place is found by what it changes: the
//! ability is written again with a stand-in effect in its place, and the words that differ are its clause. Where the
//! writer cannot word the stand-in, the place's own words (alone, then with the neighbours a sentence joins it to) are
//! looked for in the whole; failing both, the effect that holds the place is tried. A mode is its bullet line, counted
//! on the card.

use std::ops::Range;

use super::alternatives;
use crate::ability::Ability;
use crate::effect::{Effect, branch_mut, branches};
use crate::refs::TargetSpec;

/// What a decision asks about in the effects of an ability, as a path in the card's own terms
/// (`crate::effect::branches`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskedPlace<'a> {
    /// The modes of the modal effect at the path.
    Modes(&'a [usize]),
    /// The effect at the path.
    Effect(&'a [usize]),
}

/// The most effects in a row replaced or worded together, for a place a sentence joins to its neighbours.
const WIDEST: usize = 4;

/// The wordings of one run of effects tried, the first ones the writer gives.
const WORDINGS: usize = 4;

/// Effects the writer words in any place, put where the asked effects stand; the clause is what differs from each.
const STAND_INS: [&str; 2] = [r#"{"GainLife":{"amount":1}}"#, r#"{"Draw":{}}"#];

/// One list of effects on the way down a path.
struct Level<'a> {
    list: &'a [Effect],
    index: usize,
    targets: &'a [TargetSpec],
    /// The mode this list is, by its index on the card, when it is a mode of the modal effect one level up.
    mode: Option<usize>,
}

/// The whole words of an ability as the clause is looked for in them.
struct Whole<'a> {
    ability: &'a Ability,
    name: &'a str,
    /// The words in lower case.
    lower: String,
    /// Whether the words are the writer's words of `ability`, so that a rewriting may be compared with them.
    written: bool,
}

/// The span of `text` (the writer's words of `ability`, its card's name `name` for `~`) that words `place`, in bytes;
/// `None` when the place is not in the ability or its words are not found.
pub fn asked_clause(
    ability: &Ability,
    place: AskedPlace<'_>,
    name: &str,
    text: &str,
) -> Option<Range<usize>> {
    let (AskedPlace::Modes(path) | AskedPlace::Effect(path)) = place;
    let levels = levels(ability, path)?;
    let lower = text.to_ascii_lowercase();
    let whole = Whole {
        ability,
        name,
        written: written(ability, name).is_some_and(|words| words == lower),
        lower,
    };
    // The range each level is found in: inside the bullet of each mode on the way down.
    let mut ranges = Vec::with_capacity(levels.len());
    let mut range = 0..text.len();
    for level in &levels {
        if let Some(mode) = level.mode {
            range = bullet(text, range.clone(), mode).unwrap_or(range);
        }
        ranges.push(range.clone());
    }
    for (at, level) in levels.iter().enumerate().rev() {
        let within = ranges.get(at)?.clone();
        let holder = path.get(..2 * at)?;
        let found = (1..=WIDEST.min(level.list.len())).find_map(|width| {
            stood_in(&whole, holder, level, width, within.clone())
                .or_else(|| worded(&whole, level, width, within.clone()))
        });
        if found.is_some() {
            return found;
        }
        if level.mode.is_some() && within != (0..text.len()) {
            return Some(within);
        }
    }
    None
}

/// The words of an ability as the table shows them: the writer's, else the first of the wordings written for its
/// effects as a whole (`written_variants`); `None` when neither words it.
pub fn rules_words(ability: &Ability) -> Option<String> {
    super::ability(ability).or_else(|| super::written_variants(ability).into_iter().next())
}

/// The words of `ability` in lower case (`rules_words`), its card's name for `~`.
fn written(ability: &Ability, name: &str) -> Option<String> {
    rules_words(ability).map(|words| words.replace('~', name).to_ascii_lowercase())
}

/// The lists of effects down `path`, each with the targets its effects read.
fn levels<'a>(ability: &'a Ability, path: &[usize]) -> Option<Vec<Level<'a>>> {
    let mut levels = Vec::new();
    let mut list = ability.effects();
    let mut targets = ability.targets();
    let mut mode = None;
    let mut rest = path;
    loop {
        let (&index, deeper) = rest.split_first()?;
        let effect = list.get(index)?;
        levels.push(Level {
            list,
            index,
            targets,
            mode,
        });
        let Some((&branch, deeper)) = deeper.split_first() else {
            return Some(levels);
        };
        if let Effect::Modal { modes, .. } = effect {
            let chosen = modes.get(branch)?;
            targets = &chosen.targets;
            list = &chosen.effects;
            mode = Some(branch);
        } else {
            list = branches(effect).get(branch).copied()?;
            mode = None;
        }
        rest = deeper;
    }
}

/// The runs of `width` effects of the level that hold its effect, the one that starts with it first.
fn runs(level: &Level<'_>, width: usize) -> impl Iterator<Item = Range<usize>> {
    let first = (level.index + 1).saturating_sub(width);
    let len = level.list.len();
    (first..=level.index)
        .rev()
        .map(move |start| start..start + width)
        .filter(move |run| run.end <= len)
}

/// The words that differ when a run of `width` effects holding the level's effect gives way to a stand-in, joined over
/// the stand-ins; `holder` is the path of the effect whose branch the level's list is.
fn stood_in(
    whole: &Whole<'_>,
    holder: &[usize],
    level: &Level<'_>,
    width: usize,
    within: Range<usize>,
) -> Option<Range<usize>> {
    if !whole.written {
        return None;
    }
    let stand_ins: Vec<Effect> = STAND_INS
        .iter()
        .filter_map(|json| serde_json::from_str(json).ok())
        .collect();
    runs(level, width).find_map(|run| {
        stand_ins
            .iter()
            .filter_map(|stand_in| {
                let mut changed = whole.ability.clone();
                let list = list_mut(&mut changed, holder)?;
                list.splice(run.clone(), [stand_in.clone()]);
                let other = written(&changed, whole.name)?;
                differing(&whole.lower, &other)
            })
            .reduce(|one, other| one.start.min(other.start)..one.end.max(other.end))
            .map(|span| clause_edges(&whole.lower, span))
            .filter(|span| !span.is_empty() && within.start <= span.start && span.end <= within.end)
    })
}

/// The list of effects at `holder` (the path of the effect it is a branch of, and that branch) to change.
fn list_mut<'a>(ability: &'a mut Ability, holder: &[usize]) -> Option<&'a mut Vec<Effect>> {
    let mut list = ability.effects_mut()?;
    let mut rest = holder;
    while let [index, branch, deeper @ ..] = rest {
        list = branch_mut(list.get_mut(*index)?, *branch)?;
        rest = deeper;
    }
    rest.is_empty().then_some(list)
}

/// The span of `whole` that differs from `other`, past their common start and before their common end.
fn differing(whole: &str, other: &str) -> Option<Range<usize>> {
    let (a, b) = (whole.as_bytes(), other.as_bytes());
    let start = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let most = a.len().min(b.len()) - start;
    let same_end = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(most)
        .take_while(|(x, y)| x == y)
        .count();
    let end = a.len() - same_end;
    (start < end).then_some(start..end)
}

/// `span` grown to whole words, without the joining words and marks at its edges, with the period that ends it.
fn clause_edges(lower: &str, span: Range<usize>) -> Range<usize> {
    let bytes = lower.as_bytes();
    let inside = |at: usize| {
        bytes
            .get(at)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !b",.".contains(byte))
    };
    let (mut start, mut end) = (span.start, span.end);
    while start > 0 && inside(start - 1) && inside(start) {
        start -= 1;
    }
    while end < bytes.len() && inside(end - 1) && inside(end) {
        end += 1;
    }
    loop {
        let rest = lower.get(start..end).unwrap_or_default();
        let trimmed =
            rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',' || c == '.');
        let trimmed = ["then ", "and "]
            .iter()
            .find_map(|lead| trimmed.strip_prefix(lead))
            .unwrap_or(trimmed);
        if trimmed.len() == rest.len() {
            break;
        }
        start = end - trimmed.len();
    }
    loop {
        let rest = lower.get(start..end).unwrap_or_default();
        let trimmed =
            rest.trim_end_matches(|c: char| c.is_ascii_whitespace() || c == ',' || c == '.');
        let trimmed = [", then", " then", " and"]
            .iter()
            .find_map(|tail| trimmed.strip_suffix(tail))
            .unwrap_or(trimmed);
        if trimmed.len() == rest.len() {
            break;
        }
        end = start + trimmed.len();
    }
    if lower.get(end..=end) == Some(".") {
        end += 1;
    }
    start..end.max(start)
}

/// The first run of `width` effects holding the level's effect whose own words lie in `within`.
fn worded(
    whole: &Whole<'_>,
    level: &Level<'_>,
    width: usize,
    within: Range<usize>,
) -> Option<Range<usize>> {
    runs(level, width).find_map(|run| {
        alternatives(level.targets, level.list.get(run)?)
            .into_iter()
            .take(WORDINGS)
            .find_map(|sentences| found(&sentences.join(". "), whole, within.clone()))
    })
}

/// Where `words` first lie in the whole within `within`, with the period that closes them.
fn found(words: &str, whole: &Whole<'_>, within: Range<usize>) -> Option<Range<usize>> {
    let words = words.replace('~', whole.name).to_ascii_lowercase();
    let words = words.trim().trim_end_matches('.');
    if words.is_empty() {
        return None;
    }
    let start = within.start + whole.lower.get(within.clone())?.find(words)?;
    let mut end = start + words.len();
    if whole.lower.get(end..=end) == Some(".") {
        end += 1;
    }
    Some(start..end)
}

/// The span of the bullet line of `mode` (counted from 0) within `range`, without its bullet and the space after it;
/// `None` when the range has no such line.
fn bullet(text: &str, range: Range<usize>, mode: usize) -> Option<Range<usize>> {
    let part = text.get(range.clone())?;
    let starts: Vec<usize> = part
        .match_indices(super::super::modal::BULLET)
        .map(|(at, _)| range.start + at)
        .collect();
    let start = *starts.get(mode)?;
    let end = starts.get(mode + 1).copied().unwrap_or(range.end);
    let line = text.get(start..end)?;
    let words = line
        .trim_start_matches(super::super::modal::BULLET)
        .trim_start();
    let begin = end - words.len();
    Some(begin..begin + words.trim_end().len())
}

#[cfg(test)]
#[path = "asked_tests.rs"]
mod tests;
