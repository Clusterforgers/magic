//! Two imperative sentences the card joins with ", then": "draw a card, then put a +1/+1 counter on this creature" for
//! the writer's "Draw a card. Put a +1/+1 counter on ~.", read as the two sentences in order.

use super::effects::{sentence, templates};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;

/// The verbs that open the second sentence of a ", then" join: steps that choose no target and name nothing of the
/// sentence before.
const W8B_VERBS: [&str; 5] = ["Put", "Create", "Mill", "Scry", "Surveil"];

/// "Draw a card, then put a +1/+1 counter on ~.": a sentence, ", then" and an imperative sentence (not "then you",
/// which `joined::then_you` reads), read as the two sentences in order.
pub(super) fn w8b_then(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let first = templates().find_map(|template| {
        r.attempt(|r| {
            let found = template(r)?;
            (w8b_then_follows(r) || w8b_and_put_follows(r, &found)).then_some(found)
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

/// Reads "and" before "put" after the counters you get ("you get three rad counters and put three +1/+1 counters on
/// ~"): the second verb phrase is yours as well.
fn w8b_and_put_follows(r: &mut Reader<'_>, first: &[Effect]) -> bool {
    let yours = matches!(
        first,
        [Effect::PutCounters { on: Ref::You, counter, .. }] if super::w7b_player_counters::w7b_player_kind(counter)
    );
    let joins = yours && r.word_at(0) == Some("and") && r.word_at(1) == Some("put");
    if joins {
        r.skip();
    }
    joins
}

/// The wordings of `texts` in which one sentence that opens with a verb of `W8B_VERBS` follows the one before with
/// ", then": one wording for each such sentence; and "you get ... counters and put ..." for "you get ... counters. Put
/// ...".
pub(super) fn w8b_then_wordings(texts: &[String]) -> Vec<String> {
    let mut more = Vec::new();
    for text in texts {
        for (at, _) in text.match_indices(" counters. Put ") {
            let before = &text[..at];
            if before.rfind("you get ") > before.rfind(". ") {
                let after = &text[at + " counters. Put ".len()..];
                more.push(format!("{before} counters and put {after}"));
            }
        }
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
