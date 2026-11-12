//! r40-5 trigger events: "Whenever you activate an eternalize or embalm ability".

use super::tokens::Reader;
use crate::did::Action;
use crate::refs::{Ref, Selector};
use crate::trigger::TriggerPattern;

/// The event after "When" or "Whenever": "you activate an eternalize or embalm ability".
pub(super) fn r405_event(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    r.need("you activate an eternalize or embalm ability")?;
    Some(TriggerPattern::Did {
        action: Action::ActivatedEternalizeOrEmbalm,
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
        _ => None,
    }
}
