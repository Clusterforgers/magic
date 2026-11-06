//! Sneak: a creature spell cast for its sneak cost enters tapped and attacking what the unblocked attacker returned for the
//! cost was attacking. The spell remembers the creature the cost returned (`Choices::cost_picks`), and combat remembers what
//! it was attacking as it left.

use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use crate::combat::AttackTarget;
use crate::state::GameState;
use crate::zones::Zone;

/// What the creature spell `id` attacks as it enters, when it was cast for its sneak cost: the player or planeswalker the
/// creature returned for the cost was attacking, if the attack is still under way and what it attacked is still there.
pub(crate) fn target_of(state: &GameState, id: ObjectId) -> Option<AttackTarget> {
    let spell = state.object(id)?;
    if spell.choices().way != Some(CastWay::Sneak)
        || !spell.base_types().contains(CardType::Creature)
        || !state.combat().attackers_declared()
    {
        return None;
    }
    spell
        .choices()
        .cost_picks()
        .iter()
        .flatten()
        .find_map(|picked| state.combat().left_attacking(*picked))
        .filter(|target| match target {
            AttackTarget::Player(player) => state.player(*player).is_some(),
            AttackTarget::Planeswalker(walker) => state
                .object(*walker)
                .is_some_and(|object| object.zone() == Zone::Battlefield),
        })
}
