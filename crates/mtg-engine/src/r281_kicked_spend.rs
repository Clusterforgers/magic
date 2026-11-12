//! Mana restricted to kicked spells ("Spend this mana only to cast kicked spells"): a spell is kicked for its payment
//! once its kicker is announced, which comes before the payment (CR 601.2b, 601.2h, 702.33d).

use crate::announce::What;
use crate::choose::Choices;
use crate::keywords::OptionalKind;
use crate::spend::Spend;
use crate::state::GameState;

/// `spend` for the spell being announced with `choices`, kicked when they pay a kicker of it.
pub(crate) fn r281_kicked(
    state: &GameState,
    what: &What,
    choices: &Choices,
    mut spend: Spend,
) -> Spend {
    if let What::Spell(spell) = what
        && let Some(object) = state.object(*spell)
    {
        spend.traits.kicked =
            crate::announce::payments(object.card(), &choices.optional_costs, OptionalKind::Kicker)
                > 0;
    }
    spend
}
