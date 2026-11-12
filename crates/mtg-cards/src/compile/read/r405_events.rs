//! r40-5 trigger events: "Whenever you activate an eternalize or embalm ability", "whenever you solve a Case".

use super::tokens::Reader;
use crate::did::Action;
use crate::refs::{Ref, Selector};
use crate::trigger::TriggerPattern;

/// The event after "When" or "Whenever": "you activate an eternalize or embalm ability", "you solve a Case".
pub(super) fn r405_event(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let action = if r.words("you activate an eternalize or embalm ability") {
        Action::ActivatedEternalizeOrEmbalm
    } else {
        r.need("you solve a case")?;
        Action::SolvedCase
    };
    Some(TriggerPattern::Did {
        action,
        who: Ref::You,
        what: Selector::default(),
    })
}

/// The words of [`r405_event`] with "Whenever".
pub(super) fn r405_did_text(pattern: &TriggerPattern) -> Option<String> {
    match pattern {
        TriggerPattern::Did {
            action: Action::ActivatedEternalizeOrEmbalm,
            who: Ref::You,
            what,
        } if what.is_empty() => {
            Some("Whenever you activate an eternalize or embalm ability".to_owned())
        }
        TriggerPattern::Did {
            action: Action::SolvedCase,
            who: Ref::You,
            what,
        } if what.is_empty() => Some("Whenever you solve a Case".to_owned()),
        _ => None,
    }
}
