//! The card names chosen as a permanent enters that only some engine paths run: a noncreature nonland name, a
//! nonbasic land name and the name of a card revealed from the opponents' hands.

use crate::trigger::EntersChoice;

/// The capability name of a choice of a name this module adds (`ReplaceBy.Choose.<shape>`); `None` for another choice.
pub fn r4702_choice_shape(kind: EntersChoice) -> Option<&'static str> {
    Some(match kind {
        EntersChoice::NoncreatureNonlandCardName => "Choose.NoncreatureNonlandCardName",
        EntersChoice::NonbasicLandCardName => "Choose.NonbasicLandCardName",
        EntersChoice::RevealedNonlandName => "Choose.RevealedNonlandName",
        _ => return None,
    })
}
