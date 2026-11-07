//! g9c: blocks of a kind, read and written: "~ blocks one or more black creatures", "~ becomes blocked by an artifact
//! creature", "~ becomes blocked by one or more colorless creatures", "~ blocks or becomes blocked by one or more Orcs".

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::refs::{Pred, Selector};
use crate::trigger::TriggerPattern;
use mtg_core::card::CardType;

/// After "blocks": the block, of one or more attackers of a kind when the words name one.
pub(super) fn g9c_blocks(r: &mut Reader<'_>, what: &Selector) -> TriggerPattern {
    let blocking = r
        .attempt(|r| {
            r.words("one or more").then_some(())?;
            phrase(r, Form::All)
        })
        .unwrap_or_default();
    TriggerPattern::Blocks {
        what: what.clone(),
        blocking,
    }
}

/// After "becomes blocked": by any creature, by "a" creature of a kind (once for each) or by "one or more" of a kind
/// (once).
pub(super) fn g9c_blocked(r: &mut Reader<'_>, what: &Selector) -> TriggerPattern {
    let by = r.attempt(|r| {
        r.word("by").then_some(())?;
        if r.words("one or more") {
            return Some((phrase(r, Form::All)?, true));
        }
        (r.word("a") || r.word("an")).then_some(())?;
        Some((phrase(r, Form::Subject)?, false))
    });
    let (by, once) = by.unwrap_or_default();
    TriggerPattern::BecomesBlocked {
        what: what.clone(),
        by,
        once,
    }
}

/// "blocks or becomes blocked by one or more Orcs" names the kind once for both events: the block takes it too.
pub(super) fn g9c_shared_kind(found: &mut [TriggerPattern]) {
    if let [
        TriggerPattern::Blocks { what, blocking },
        TriggerPattern::BecomesBlocked {
            what: other,
            by,
            once: true,
        },
    ] = found
        && what == other
        && blocking.is_empty()
    {
        *blocking = by.clone();
    }
}

/// The verb of a block of a kind ("blocks one or more black creatures"); none without a kind.
pub(super) fn g9c_blocks_verb(blocking: &Selector) -> Option<String> {
    Some(format!(
        "blocks one or more {}",
        phrase_text(blocking, Form::All)?
    ))
}

/// The verb of a block by a kind: "becomes blocked by an artifact creature", "becomes blocked by one or more Orcs".
pub(super) fn g9c_blocked_verb(by: &Selector, once: bool) -> Option<String> {
    if once {
        return Some(format!(
            "becomes blocked by one or more {}",
            phrase_text(by, Form::All)?
        ));
    }
    let kind = phrase_text(by, Form::Subject)
        .or_else(|| phrase_text(&creature_last(by), Form::Subject))?;
    Some(format!("becomes blocked by {} {kind}", article(&kind)))
}

/// "Whenever ~ blocks or becomes blocked" and "Whenever ~ blocks or becomes blocked by one or more Orcs": one kind,
/// or none, for both events, in either order.
pub(super) fn g9c_shared_text(patterns: &[TriggerPattern]) -> Option<String> {
    let (blocks, blocked) = match patterns {
        [first @ TriggerPattern::Blocks { .. }, second] => (first, second),
        [first, second @ TriggerPattern::Blocks { .. }] => (second, first),
        _ => return None,
    };
    let (
        TriggerPattern::Blocks { what, blocking },
        TriggerPattern::BecomesBlocked {
            what: other,
            by,
            once,
        },
    ) = (blocks, blocked)
    else {
        return None;
    };
    (what == other && blocking == by && (*once || by.is_empty())).then_some(())?;
    let kind = if by.is_empty() {
        String::new()
    } else {
        format!(" by one or more {}", phrase_text(by, Form::All)?)
    };
    Some(format!(
        "Whenever {} blocks or becomes blocked{kind}",
        super::triggers::subject_text(what)?
    ))
}

/// The selector with its creature type after the other card types: Forge's "Creature.Artifact" is "artifact creature".
fn creature_last(what: &Selector) -> Selector {
    let creature = Pred::Type(CardType::Creature);
    let mut preds: Vec<Pred> = what
        .0
        .iter()
        .filter(|pred| **pred != creature)
        .cloned()
        .collect();
    if preds.len() < what.0.len() {
        let after = preds
            .iter()
            .rposition(|pred| matches!(pred, Pred::Type(_)))
            .map_or(0, |at| at + 1);
        preds.insert(after, creature);
    }
    Selector(preds)
}
