//! Two imperative sentences the card joins with ", then": "draw a card, then put a +1/+1 counter on this creature" for
//! the writer's "Draw a card. Put a +1/+1 counter on ~.", read as the two sentences in order.

use super::effects::{sentence, templates};
use super::tokens::Reader;
use crate::effect::Effect;

/// The verbs that open the second sentence of a ", then" join.
const W8B_VERBS: [&str; 12] = [
    "Put",
    "Draw",
    "Create",
    "Return",
    "Exile",
    "Destroy",
    "Sacrifice",
    "Discard",
    "Mill",
    "Scry",
    "Surveil",
    "Tap",
];

/// "Draw a card, then put a +1/+1 counter on ~.": a sentence, ", then" and an imperative sentence (not "then you",
/// which `joined::then_you` reads), read as the two sentences in order.
pub(super) fn w8b_then(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let first = templates().find_map(|template| {
        r.attempt(|r| {
            let found = template(r)?;
            w8b_then_follows(r).then_some(found)
        })
    })?;
    let mut rest = Vec::new();
    if !sentence(r, &mut rest) {
        return None;
    }
    effects.extend(first);
    effects.extend(rest);
    Some(())
}

/// Reads ", then" before a verb of `W8B_VERBS`.
fn w8b_then_follows(r: &mut Reader<'_>) -> bool {
    let verb = r.word_at(2).map(str::to_owned);
    let joins = r.at_punct(',')
        && r.word_at(1) == Some("then")
        && verb.is_some_and(|verb| W8B_VERBS.iter().any(|v| v.eq_ignore_ascii_case(&verb)));
    if joins {
        r.skip();
        r.skip();
    }
    joins
}

/// The wordings of `texts` in which one sentence that opens with a verb of `W8B_VERBS` follows the one before with
/// ", then": one wording for each such sentence.
pub(super) fn w8b_then_wordings(texts: &[String]) -> Vec<String> {
    let mut more = Vec::new();
    for text in texts {
        for verb in W8B_VERBS {
            let joint = format!(". {verb} ");
            for (at, _) in text.match_indices(&joint) {
                let lower = verb.to_lowercase();
                more.push(format!(
                    "{}, then {lower} {}",
                    &text[..at],
                    &text[at + joint.len()..]
                ));
            }
        }
    }
    more
}
