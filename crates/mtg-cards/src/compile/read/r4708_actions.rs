//! r47-08 replacements of a scry or a surveil: "If you would scry a number of cards, draw that many cards instead."
//! (`ReplaceWhen::Scries`) and "You may look at an additional two cards each time you surveil."
//! (`ReplaceWhen::Surveils` with `ReplaceBy::MayAddAmount`), read and written as whole sentences.

use super::amounts::{count_word, number_word};
use super::tokens::same_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// The sentence of a scry replaced by draws.
const SCRY_DRAWS: &str = "If you would scry a number of cards, draw that many cards instead.";

/// A replacement of the battlefield, with no condition.
const fn replacement(when: ReplaceWhen, by: ReplaceBy) -> Ability {
    Ability::Replacement {
        when,
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// "If you would scry a number of cards, draw that many cards instead."
fn scry_draws() -> Ability {
    replacement(
        ReplaceWhen::Scries { who: Ref::You },
        ReplaceBy::Instead(vec![Effect::Draw {
            who: Ref::You,
            count: Amount::Triggering,
            bind: None,
        }]),
    )
}

/// The sentence of a surveil of `more` more cards.
fn surveil_more_text(more: u32) -> Option<String> {
    Some(format!(
        "You may look at an additional {} cards each time you surveil.",
        count_word(more)?
    ))
}

/// The replacement of a scry or a surveil the sentence words.
pub(super) fn r4708_read(text: &str) -> Option<Ability> {
    if same_text(text, SCRY_DRAWS) {
        return Some(scry_draws());
    }
    let word = text
        .strip_prefix("You may look at an additional ")?
        .strip_suffix(" cards each time you surveil.")?;
    let more = number_word(word).filter(|more| *more > 1)?;
    Some(replacement(
        ReplaceWhen::Surveils { who: Ref::You },
        ReplaceBy::MayAddAmount(Amount::Fixed(i32::try_from(more).ok()?)),
    ))
}

/// The sentence of a replacement of a scry or a surveil.
pub(super) fn r4708_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let text = if *ability == scry_draws() {
        Some(SCRY_DRAWS.to_owned())
    } else {
        match ability {
            Ability::Replacement {
                when: ReplaceWhen::Surveils { who: Ref::You },
                by: ReplaceBy::MayAddAmount(Amount::Fixed(more)),
                condition: None,
                from: ZoneKind::Battlefield,
            } => u32::try_from(*more)
                .ok()
                .filter(|more| *more > 1)
                .and_then(surveil_more_text),
            _ => None,
        }
    };
    if let Some(text) = text
        && !texts.contains(&text)
    {
        texts.push(text);
    }
}
