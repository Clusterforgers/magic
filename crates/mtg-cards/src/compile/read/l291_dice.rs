//! "Whenever you roll a 1" and "Whenever you roll a 1 or 2": a die that rolls one of the numbers (`DiceRolled` with
//! `results`).

use super::tokens::Reader;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// "a 1", "a 1 or 2", "a 2, 4, or 6" after "roll": the results, at least one, each from 1 and none twice.
pub(super) fn results(r: &mut Reader<'_>, who: Ref) -> Option<TriggerPattern> {
    r.word("a").then_some(())?;
    let mut results = vec![r.number().filter(|n| *n > 0)?];
    // "a 1 or 2", "a 2, 4, or 6": a list of more than two has commas, and its last number follows "or".
    let mut last = false;
    while let Some((or, n)) = r.attempt(|r| {
        r.punct(',').then_some(())?;
        let or = r.word("or");
        Some((or, r.number().filter(|n| *n > 0)?))
    }) {
        results.push(n);
        if or {
            last = true;
            break;
        }
    }
    if results.len() == 1 {
        if let Some(n) = r.attempt(|r| {
            r.word("or").then_some(())?;
            r.number().filter(|n| *n > 0)
        }) {
            results.push(n);
        }
    } else if results.len() == 2 || !last {
        // "a 1, or 2" and "a 1, 2, 3" are no list a card prints.
        return None;
    }
    let mut sorted = results.clone();
    sorted.sort_unstable();
    sorted.dedup();
    (sorted == results).then_some(TriggerPattern::DiceRolled {
        who,
        highest: false,
        results,
    })
}

/// The words of [`results`]: "a 1", "a 1 or 2", "a 2, 4, or 6".
pub(super) fn results_text(results: &[u32]) -> Option<String> {
    let words: Vec<String> = results.iter().map(u32::to_string).collect();
    Some(match words.as_slice() {
        [] => return None,
        [one] => format!("a {one}"),
        [one, two] => format!("a {one} or {two}"),
        [rest @ .., last] => format!("a {}, or {last}", rest.join(", ")),
    })
}
