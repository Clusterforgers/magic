//! Keywords a player has: "You have hexproof." and "You have shroud." are static abilities that give the keyword to their
//! controller in layer 6, under the condition when one leads ("As long as you control your commander, you have hexproof.").

use super::effects::granted;
use super::statics::{leading_condition, statics};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::refs::Ref;

/// "You have hexproof.", "You have hexproof and shroud.": the keywords a player can have (hexproof, shroud, protection).
pub(super) fn ability(r: &mut Reader<'_>) -> Option<Ability> {
    let condition = leading_condition(r);
    r.words("you have").then_some(())?;
    let mods = granted(r)?;
    mods.iter()
        .all(crate::modify::Modification::is_player_keyword)
        .then(|| statics(Ref::You, mods, condition))
}
