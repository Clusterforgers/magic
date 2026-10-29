//! Two instructions joined with "and" that the writer prints as two sentences: "it can't be blocked this turn and you
//! scry 1" (Clockwork Droid) and "untap all other creatures you control and after this phase, there is an additional
//! combat phase" (Combat Celebrant).

use super::effects::templates;
use super::tokens::Reader;
use crate::effect::Effect;

/// The second sentences the writer prints that a card joins with "and": `(written, joined)`.
const W11A_JOINS: [(&str, &str); 3] = [
    (". Scry ", " and you scry "),
    (". Surveil ", " and you surveil "),
    (
        ". After this phase, there is ",
        " and after this phase, there is ",
    ),
];

/// The wordings of `texts` with each second sentence of [`W11A_JOINS`] joined to the one before.
pub(super) fn w11a_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .flat_map(|text| {
            W11A_JOINS
                .iter()
                .filter(|(written, _)| text.matches(written).count() == 1)
                .map(|(written, joined)| text.replacen(written, joined, 1))
        })
        .collect()
}

/// "Untap all other creatures you control and after this phase, there is an additional combat phase.": a sentence,
/// "and" and a sentence that opens with "after", read as the two sentences in order. The first is an untap alone: a
/// payment before it ("If you do, untap all attacking creatures and after this phase, ...") holds both in its branch,
/// which this join would split.
pub(super) fn w11a_and_after(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let found = templates().find_map(|template| {
        r.attempt(|r| {
            let mut first = template(r)?;
            matches!(first.as_slice(), [Effect::Untap { .. }]).then_some(())?;
            r.need("and")?;
            (r.peek_word() == Some("after")).then_some(())?;
            let second = templates().find_map(|template| {
                r.attempt(|r| {
                    let found = template(r)?;
                    r.sentence_end().then_some(found)
                })
            })?;
            first.extend(second);
            Some(first)
        })
    })?;
    effects.extend(found);
    Some(())
}
