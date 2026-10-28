//! A spell copied as the event of a trigger: "Whenever you copy a spell", "Whenever you cast or copy an instant or
//! sorcery spell" (magecraft). The reader is `triggers::player_event`; these are the writers.

use super::triggers::subject_text;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;

/// The player and the verb: "you copy", "an opponent copies"; with `cast`, "you cast or copy".
fn w2e_who(by: &Ref, cast: bool) -> Option<String> {
    let third = match by {
        Ref::You => false,
        Ref::Opponents | Ref::EachPlayer | Ref::AttachedPlayer => true,
        _ => return None,
    };
    let who = match by {
        Ref::You => "you",
        Ref::Opponents => "an opponent",
        Ref::EachPlayer => "a player",
        _ => "enchanted player",
    };
    let verb = match (cast, third) {
        (false, false) => "copy",
        (false, true) => "copies",
        (true, false) => "cast or copy",
        (true, true) => "casts or copies",
    };
    Some(format!("{who} {verb}"))
}

/// "a spell", "an instant or sorcery spell": the spell of the selector.
fn w2e_spell_text(what: &Selector) -> Option<String> {
    let mut preds = vec![Pred::Spell];
    preds.extend(what.0.iter().cloned());
    subject_text(&Selector(preds))
}

/// "Whenever you copy an instant spell".
pub(super) fn w2e_copy_text(what: &Selector, by: &Ref) -> Option<String> {
    Some(format!(
        "Whenever {} {}",
        w2e_who(by, false)?,
        w2e_spell_text(what)?
    ))
}

/// "Whenever you cast or copy an instant or sorcery spell": a cast and a copy of the same spell by the same player.
pub(super) fn w2e_cast_or_copy_text(patterns: &[TriggerPattern]) -> Option<String> {
    let [
        TriggerPattern::SpellCast { what, by },
        TriggerPattern::SpellCopied {
            what: copied,
            by: copier,
        },
    ] = patterns
    else {
        return None;
    };
    (what == copied && by == copier).then_some(())?;
    Some(format!(
        "Whenever {} {}",
        w2e_who(by, true)?,
        w2e_spell_text(what)?
    ))
}
