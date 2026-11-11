//! Cleave read back (CR 702.148): the keyword line is "Cleave" and its cost, and its cleaved spell writes the face's
//! spell text without the words in square brackets; the spell writes the text with them.

use crate::ability::Ability;
use crate::compile::render_cost;
use crate::keyword::Keyword;

/// The keyword line and the cleaved spell of a cleave keyword: "Cleave {1}{U}" and its `Spell` ability.
pub(super) fn cleave(ability: &Ability) -> Option<(String, Ability)> {
    let Ability::Keyword(Keyword::Cleave { cost, spell }) = ability else {
        return None;
    };
    let words = render_cost(cost)?;
    Some((format!("Cleave {words}"), (**spell).clone()))
}
