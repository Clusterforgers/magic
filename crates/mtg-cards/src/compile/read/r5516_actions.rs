//! r55-16 replacement of a copy of a spell: "If you would copy a spell one or more times, instead copy it that many times
//! plus an additional time. You may choose new targets for the additional copy." (`ReplaceWhen::CopiesSpell` with
//! `ReplaceBy::AddAmount(1)`) and "As long as this card is in your graveyard, if you would learn, you may instead return
//! this card to the battlefield." (`ReplaceWhen::Learns` with `ReplaceBy::MayInstead`), read and written whole.

use super::tokens::same_text;
use crate::ability::Ability;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// The clause of a copy of a spell made once more.
const R5516_COPY_MORE: &str = "If you would copy a spell one or more times, instead copy it that many times plus an \
                               additional time. You may choose new targets for the additional copy.";

/// The clause of a learn replaced by returning the card from the graveyard.
const R5516_LEARN_RETURN: &str = "As long as this card is in your graveyard, if you would learn, you may instead return \
                                  this card to the battlefield.";

/// "As long as this card is in your graveyard, if you would learn, you may instead return this card to the battlefield."
fn r5516_learn_return() -> Ability {
    Ability::Replacement {
        when: ReplaceWhen::Learns { who: Ref::You },
        by: ReplaceBy::MayInstead(crate::r5516_learn::r5516_return_this()),
        condition: None,
        from: ZoneKind::Graveyard,
    }
}

/// "If you would copy a spell one or more times, instead copy it that many times plus an additional time. ..."
fn r5516_copy_more() -> Ability {
    Ability::Replacement {
        when: ReplaceWhen::CopiesSpell { who: Ref::You },
        by: ReplaceBy::AddAmount(Amount::Fixed(1)),
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// The replacement of a copy of a spell the clause words.
pub(super) fn r5516_read(text: &str) -> Option<Ability> {
    if same_text(text, R5516_LEARN_RETURN) {
        return Some(r5516_learn_return());
    }
    same_text(text, R5516_COPY_MORE).then(r5516_copy_more)
}

/// The clause of a replacement of a copy of a spell.
pub(super) fn r5516_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in [
        (r5516_copy_more(), R5516_COPY_MORE),
        (r5516_learn_return(), R5516_LEARN_RETURN),
    ] {
        if *ability == shape && !texts.iter().any(|known| known == text) {
            texts.push(text.to_owned());
        }
    }
}
