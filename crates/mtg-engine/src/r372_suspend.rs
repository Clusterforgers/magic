//! The haste of a creature cast through suspend (CR 702.62a): "If you cast a creature spell this way, it gains haste
//! until you lose control of the spell or the permanent it becomes." It comes from the casting, not from the card
//! having suspend, so a card that gained suspend in exile (`Effect::GainSuspend`) has it too.

use mtg_cards::value::CastWay;
use mtg_core::card::CardType;

use crate::object::GameObject;
use crate::zones::Zone;

/// Whether `object` is a creature permanent cast for suspend that its owner, who cast it, still controls.
pub(crate) fn cast_hasty(object: &GameObject) -> bool {
    object.zone() == Zone::Battlefield
        && object.cast_way() == Some(CastWay::Suspend)
        && object.controller() == object.owner()
        && object.card().types.contains(&CardType::Creature)
}
