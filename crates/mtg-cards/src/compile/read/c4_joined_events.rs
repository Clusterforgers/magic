//! An event and a batch in one trigger: "When ~ enters and whenever one or more creature tokens you control
//! deal combat damage to a player, ...". The reader joins the two "When" clauses with "and"; this is the writer.

use super::triggers::pattern_text;
use crate::trigger::TriggerPattern;

/// The text of two events of an `AnyOf` whose texts both begin with "When" or "Whenever": the first, "and" and the second
/// without its capital.
pub(super) fn joined_text(patterns: &[TriggerPattern]) -> Option<String> {
    let [first, second] = patterns else {
        return None;
    };
    // Only a batch is joined this way; the lists of events of one subject, and a card and another creature, are worded
    // by `any_of_listed`.
    patterns
        .iter()
        .any(|pattern| matches!(pattern, TriggerPattern::Batch(_)))
        .then_some(())?;
    let (first, second) = (pattern_text(first)?, pattern_text(second)?);
    let opens = |text: &str| text.starts_with("When ") || text.starts_with("Whenever ");
    (opens(&first) && opens(&second)).then_some(())?;
    let mut rest = second.chars();
    let lowered: String = rest.next()?.to_lowercase().chain(rest).collect();
    Some(format!("{first} and {lowered}"))
}
