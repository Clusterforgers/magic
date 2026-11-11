//! A die of a given result as a trigger event: "whenever you roll a 1 or 2", "whenever you roll a 6" and "when you roll
//! a natural 20" (CR 706.2).

use super::tokens::Reader;
use super::triggers::player_verb;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// After the verb "roll": "a 1", "a 1 or 2", "a natural 20".
pub(super) fn r289_rolled_result(r: &mut Reader<'_>, by: Ref) -> Option<TriggerPattern> {
    r.need("a")?;
    let natural = r.word("natural");
    let mut results = vec![r.number()?];
    if r.word("or") {
        results.push(r.number()?);
    }
    Some(TriggerPattern::DiceRolled {
        who: by,
        highest: false,
        results,
        natural,
    })
}

/// The head `r289_rolled_result` reads, written back.
pub(super) fn r289_rolled_result_text(by: &Ref, results: &[u32], natural: bool) -> Option<String> {
    let numbers = match results {
        [one] => one.to_string(),
        [one, two] => format!("{one} or {two}"),
        _ => return None,
    };
    let natural = if natural { "natural " } else { "" };
    Some(format!(
        "Whenever {} a {natural}{numbers}",
        player_verb(by, "roll")?
    ))
}
