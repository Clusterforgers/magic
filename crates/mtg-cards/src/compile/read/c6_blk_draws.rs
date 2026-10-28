//! A draw that a card doubles: "If you would draw a card, draw two cards instead." and "If you would draw a card
//! except the first one you draw in each of your draw steps, draw two cards instead." (a `Replacement` of `Draw`,
//! written back by `replacements.rs`).

use super::amounts::number_word;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// "If you would draw a card[ except the first N you draw in each of your draw steps], draw two cards instead."
pub(super) fn c6_blk_draw_replacement(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if you would draw a card")?;
    let except_draw_step = if r.words("except the first") {
        let count = if r.word("one") {
            1
        } else {
            number_word(r.take_word()?).filter(|count| *count > 1)?
        };
        r.need("you draw in each of your draw steps")?;
        count
    } else {
        0
    };
    r.punct(',').then_some(())?;
    r.need("draw two cards instead")?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Draw {
            who: Ref::You,
            except_draw_step,
        },
        by: ReplaceBy::Multiply(2),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}
