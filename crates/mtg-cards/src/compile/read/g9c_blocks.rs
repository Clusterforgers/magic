//! g9c: blocks of a kind, read and written: "~ blocks one or more black creatures", "~ becomes blocked by one or more
//! colorless creatures", "~ blocks or becomes blocked by one or more Orcs", once; "becomes blocked by an artifact creature"
//! is read here, once for each pair, and written by `g9a_blocks`.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::refs::Selector;
use crate::trigger::TriggerPattern;

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

/// The subject and verb of a block or of becoming blocked, of any creature or of a kind.
pub(super) fn g9c_happening(pattern: &TriggerPattern) -> Option<(Selector, String)> {
    let (what, verb) = match pattern {
        TriggerPattern::Blocks { what, blocking } if blocking.0.is_empty() => {
            (what, "blocks".to_owned())
        }
        TriggerPattern::Blocks { what, blocking } => (what, g9c_blocks_verb(blocking)?),
        TriggerPattern::BecomesBlocked { what, by, .. } if by.0.is_empty() => {
            (what, "becomes blocked".to_owned())
        }
        // A blocker of a kind once for each pair is `g9a_blocks`.
        TriggerPattern::BecomesBlocked {
            what,
            by,
            once: true,
        } => (
            what,
            format!(
                "becomes blocked by one or more {}",
                phrase_text(by, Form::All)?
            ),
        ),
        _ => return None,
    };
    Some((what.clone(), verb))
}

/// The verb of a block of a kind ("blocks one or more black creatures"); none without a kind.
fn g9c_blocks_verb(blocking: &Selector) -> Option<String> {
    Some(format!(
        "blocks one or more {}",
        phrase_text(blocking, Form::All)?
    ))
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
